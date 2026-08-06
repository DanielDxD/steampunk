mod runtime;
mod serde_rt;

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{
    types, AbiParam, Block, Function, InstBuilder, MemFlags, Signature, UserFuncName, Value,
};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context as ClifContext;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use stk_ast::{BinOp, UnOp};
use stk_types::{
    CheckedAssignTarget, CheckedExpr, CheckedFunction, CheckedMethod, CheckedPattern,
    CheckedProgram, CheckedStmt, Ty,
};

pub use runtime::{
    stk_alloc, stk_cancel_token_cancel, stk_cancel_token_is_cancelled, stk_cancel_token_new,
    stk_channel_buffered, stk_channel_close, stk_channel_new, stk_channel_recv,
    stk_channel_recv_future, stk_channel_recv_ok, stk_channel_send, stk_env_args, stk_env_get,
    stk_env_set, stk_fs_read_to_string, stk_fs_write_string, stk_future_await,
    stk_future_complete, stk_future_join, stk_future_new, stk_future_race, stk_future_ready,
    stk_http_get, stk_list_get, stk_list_len, stk_list_new, stk_list_push, stk_list_set,
    stk_mutex_get, stk_mutex_lock, stk_mutex_new, stk_mutex_set, stk_mutex_unlock, stk_panic,
    stk_parallel_map_int, stk_process_exit, stk_rwlock_get, stk_rwlock_new, stk_rwlock_read_lock,
    stk_rwlock_read_unlock, stk_rwlock_set, stk_rwlock_write_lock, stk_rwlock_write_unlock,
    stk_set_argv, stk_sleep_ms, stk_spawn, stk_spawn_drain, stk_std_log, stk_string_concat,
    stk_string_contains, stk_string_from_int, stk_string_len, stk_string_parse_int,
    stk_string_slice, stk_task_yield, stk_time_now_ms, stk_waitgroup_add, stk_waitgroup_done,
    stk_waitgroup_new, stk_waitgroup_wait, stk_waitgroup_wait_future,
};
pub use serde_rt::{stk_serde_decode, stk_serde_encode};

fn async_wrap_name(fn_name: &str) -> String {
    format!("__async_wrap_{fn_name}")
}

fn cpu_submit_wrap_name(fn_name: &str) -> String {
    format!("__cpu_submit_{fn_name}")
}

fn collect_cpu_submit_fns(program: &CheckedProgram) -> Vec<String> {
    use std::collections::BTreeSet;
    let mut names = BTreeSet::new();
    let mut collect = |e: &CheckedExpr| {
        if let CheckedExpr::CpuSubmitNamed { fn_name } = e {
            names.insert(fn_name.clone());
        }
    };
    for f in &program.functions {
        for stmt in &f.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    for m in &program.methods {
        for stmt in &m.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    names.into_iter().collect()
}

fn program_has_cpu_submit_closure(program: &CheckedProgram) -> bool {
    let mut found = false;
    let mut collect = |e: &CheckedExpr| {
        if matches!(e, CheckedExpr::CpuSubmitClosure { .. }) {
            found = true;
        }
    };
    for f in &program.functions {
        for stmt in &f.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    for m in &program.methods {
        for stmt in &m.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    found
}

fn make_flags(is_pic: bool) -> settings::Flags {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder
        .set("is_pic", if is_pic { "true" } else { "false" })
        .unwrap();
    settings::Flags::new(flag_builder)
}

fn clif_ty(ty: &Ty) -> types::Type {
    match ty {
        Ty::Int
        | Ty::Float
        | Ty::String
        | Ty::Bool
        | Ty::Class(_)
        | Ty::Interface(_)
        | Ty::Array { .. }
        | Ty::Future(_)
        | Ty::Channel { .. }
        | Ty::WaitGroup
        | Ty::Mutex { .. }
        | Ty::RwLock { .. }
        | Ty::CancelToken
        | Ty::Result { .. }
        | Ty::Option { .. }
        | Ty::List { .. }
        | Ty::Fn { .. } => types::I64,
        Ty::Void => types::I64,
    }
}

fn std_log_signature() -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    for _ in 0..6 {
        sig.params.push(AbiParam::new(types::I64));
    }
    sig
}

fn alloc_signature() -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

fn unary_i64_signature(ret: bool) -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.push(AbiParam::new(types::I64));
    if ret {
        sig.returns.push(AbiParam::new(types::I64));
    }
    sig
}

fn binary_i64_signature(ret: bool) -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    if ret {
        sig.returns.push(AbiParam::new(types::I64));
    }
    sig
}

fn void_signature() -> Signature {
    Signature::new(CallConv::SystemV)
}

fn spawn_thunk_signature() -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.params.push(AbiParam::new(types::I64)); // env
    sig
}

type StrMap = HashMap<Vec<u8>, (DataId, usize)>;

#[derive(Clone, Copy)]
struct LoopTargets {
    continue_block: Block,
    break_block: Block,
}

struct EmitCtx<'a, M: Module> {
    module: &'a mut M,
    func_ids: &'a HashMap<String, FuncId>,
    log_id: FuncId,
    sleep_id: FuncId,
    alloc_id: FuncId,
    future_new_id: FuncId,
    future_complete_id: FuncId,
    future_ready_id: FuncId,
    future_await_id: FuncId,
    future_join_id: FuncId,
    future_race_id: FuncId,
    spawn_id: FuncId,
    channel_new_id: FuncId,
    channel_buffered_id: FuncId,
    channel_send_id: FuncId,
    channel_recv_id: FuncId,
    channel_recv_future_id: FuncId,
    channel_recv_ok_id: FuncId,
    channel_close_id: FuncId,
    waitgroup_new_id: FuncId,
    waitgroup_add_id: FuncId,
    waitgroup_done_id: FuncId,
    waitgroup_wait_id: FuncId,
    waitgroup_wait_future_id: FuncId,
    mutex_new_id: FuncId,
    mutex_lock_id: FuncId,
    mutex_unlock_id: FuncId,
    mutex_get_id: FuncId,
    mutex_set_id: FuncId,
    rwlock_new_id: FuncId,
    rwlock_read_lock_id: FuncId,
    rwlock_read_unlock_id: FuncId,
    rwlock_write_lock_id: FuncId,
    rwlock_write_unlock_id: FuncId,
    rwlock_get_id: FuncId,
    rwlock_set_id: FuncId,
    parallel_map_int_id: FuncId,
    http_get_id: FuncId,
    task_yield_id: FuncId,
    cancel_token_new_id: FuncId,
    cancel_token_cancel_id: FuncId,
    cancel_token_is_cancelled_id: FuncId,
    list_new_id: FuncId,
    list_push_id: FuncId,
    list_get_id: FuncId,
    list_set_id: FuncId,
    list_len_id: FuncId,
    panic_id: FuncId,
    process_exit_id: FuncId,
    time_now_ms_id: FuncId,
    env_args_id: FuncId,
    env_get_id: FuncId,
    env_set_id: FuncId,
    fs_read_to_string_id: FuncId,
    fs_write_string_id: FuncId,
    string_len_id: FuncId,
    string_concat_id: FuncId,
    string_slice_id: FuncId,
    string_contains_id: FuncId,
    string_from_int_id: FuncId,
    string_parse_int_id: FuncId,
    serde_encode_id: FuncId,
    serde_decode_id: FuncId,
    strings: &'a StrMap,
    vars: HashMap<String, Variable>,
    next_var: u32,
    str_lens: HashMap<String, i64>,
    loops: Vec<LoopTargets>,
    /// When set, `return v` completes this future handle instead of returning a value.
    async_complete_handle: Option<Variable>,
}

fn collect_string_lits(program: &CheckedProgram) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut seen = HashMap::<Vec<u8>, ()>::new();
    let mut collect = |e: &CheckedExpr| {
        match e {
            CheckedExpr::StringLit(s) => {
                let b = s.as_bytes().to_vec();
                if seen.insert(b.clone(), ()).is_none() {
                    out.push(b);
                }
            }
            CheckedExpr::SerdeEncode { schema, .. } | CheckedExpr::SerdeDecode { schema, .. } => {
                let b = schema.as_bytes().to_vec();
                if seen.insert(b.clone(), ()).is_none() {
                    out.push(b);
                }
            }
            _ => {}
        }
    };
    for f in &program.functions {
        for stmt in &f.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    for m in &program.methods {
        for stmt in &m.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    out
}

fn walk_stmt(stmt: &CheckedStmt, f: &mut impl FnMut(&CheckedExpr)) {
    match stmt {
        CheckedStmt::VarDecl { init, .. } => walk_expr(init, f),
        CheckedStmt::Assign { target, value } => {
            match target {
                CheckedAssignTarget::Local { .. } => {}
                CheckedAssignTarget::Field { object, .. }
                | CheckedAssignTarget::Setter { object, .. } => walk_expr(object, f),
                CheckedAssignTarget::Index { array, index, .. } => {
                    walk_expr(array, f);
                    walk_expr(index, f);
                }
            }
            walk_expr(value, f);
        }
        CheckedStmt::Drop { object, .. } => walk_expr(object, f),
        CheckedStmt::Spawn { body, .. } => {
            for s in body {
                walk_stmt(s, f);
            }
        }
        CheckedStmt::Return { value } => {
            if let Some(v) = value {
                walk_expr(v, f);
            }
        }
        CheckedStmt::Expr { expr } => walk_expr(expr, f),
        CheckedStmt::If { arms, else_block } => {
            for (c, body) in arms {
                walk_expr(c, f);
                for s in body {
                    walk_stmt(s, f);
                }
            }
            if let Some(eb) = else_block {
                for s in eb {
                    walk_stmt(s, f);
                }
            }
        }
        CheckedStmt::While { cond, body } => {
            walk_expr(cond, f);
            for s in body {
                walk_stmt(s, f);
            }
        }
        CheckedStmt::ForRange {
            start, end, body, ..
        } => {
            walk_expr(start, f);
            walk_expr(end, f);
            for s in body {
                walk_stmt(s, f);
            }
        }
        CheckedStmt::ForIn { iter, body, .. } => {
            walk_expr(iter, f);
            for s in body {
                walk_stmt(s, f);
            }
        }
        CheckedStmt::Match { scrutinee, arms } => {
            walk_expr(scrutinee, f);
            for (_, body) in arms {
                for s in body {
                    walk_stmt(s, f);
                }
            }
        }
        CheckedStmt::Break | CheckedStmt::Continue => {}
    }
}

/// Whether an expression yields a `float` (carried as f64 bits in an i64 slot).
fn expr_is_float(expr: &CheckedExpr) -> bool {
    match expr {
        CheckedExpr::FloatLit(_) => true,
        CheckedExpr::Local { ty, .. }
        | CheckedExpr::Binary { ty, .. }
        | CheckedExpr::Unary { ty, .. }
        | CheckedExpr::FieldGet { ty, .. } => *ty == Ty::Float,
        CheckedExpr::Call { ret: ty, .. }
        | CheckedExpr::MethodCall { ret: ty, .. }
        | CheckedExpr::CallClosure { ret: ty, .. }
        | CheckedExpr::Await { inner: ty, .. }
        | CheckedExpr::Index { elem: ty, .. }
        | CheckedExpr::ListGet { elem: ty, .. } => *ty == Ty::Float,
        _ => false,
    }
}

fn walk_expr(expr: &CheckedExpr, f: &mut impl FnMut(&CheckedExpr)) {
    f(expr);
    match expr {
        CheckedExpr::Binary { left, right, .. } => {
            walk_expr(left, f);
            walk_expr(right, f);
        }
        CheckedExpr::Unary { expr, .. } => walk_expr(expr, f),
        CheckedExpr::Call { args, .. }
        | CheckedExpr::StdLog { args }
        | CheckedExpr::New { args, .. }
        | CheckedExpr::ArrayLit { elems: args, .. } => {
            for a in args {
                walk_expr(a, f);
            }
        }
        CheckedExpr::StdSleep { ms } => walk_expr(ms, f),
        CheckedExpr::FieldGet { object, .. } | CheckedExpr::Await { expr: object, .. } => {
            walk_expr(object, f)
        }
        CheckedExpr::Index { array, index, .. } => {
            walk_expr(array, f);
            walk_expr(index, f);
        }
        CheckedExpr::MethodCall { object, args, .. } => {
            walk_expr(object, f);
            for a in args {
                walk_expr(a, f);
            }
        }
        CheckedExpr::ChannelSend { channel, value } => {
            walk_expr(channel, f);
            walk_expr(value, f);
        }
        CheckedExpr::ChannelRecv { channel }
        | CheckedExpr::ChannelRecvFuture { channel }
        | CheckedExpr::ChannelClose { channel } => walk_expr(channel, f),
        CheckedExpr::CpuSubmitNamed { .. } => {}
        CheckedExpr::CpuSubmitClosure { closure } => walk_expr(closure, f),
        CheckedExpr::ResultOk { value }
        | CheckedExpr::ResultErr { value }
        | CheckedExpr::OptionSome { value } => walk_expr(value, f),
        CheckedExpr::OptionNone => {}
        CheckedExpr::WaitGroupAdd { wg, delta } => {
            walk_expr(wg, f);
            walk_expr(delta, f);
        }
        CheckedExpr::WaitGroupDone { wg }
        | CheckedExpr::WaitGroupWait { wg }
        | CheckedExpr::WaitGroupWaitFuture { wg } => walk_expr(wg, f),
        CheckedExpr::FutureJoin { left, right }
        | CheckedExpr::FutureRace { left, right } => {
            walk_expr(left, f);
            walk_expr(right, f);
        }
        CheckedExpr::FutureReady { value } => walk_expr(value, f),
        CheckedExpr::ChannelBuffered { capacity } => walk_expr(capacity, f),
        CheckedExpr::MutexNew { initial } => walk_expr(initial, f),
        CheckedExpr::MutexLock { mutex }
        | CheckedExpr::MutexUnlock { mutex }
        | CheckedExpr::MutexGet { mutex } => walk_expr(mutex, f),
        CheckedExpr::MutexSet { mutex, value } => {
            walk_expr(mutex, f);
            walk_expr(value, f);
        }
        CheckedExpr::ListNew
        | CheckedExpr::StdEnvArgs
        | CheckedExpr::StdTimeNowMs
        | CheckedExpr::TaskYield
        | CheckedExpr::CancelTokenNew => {}
        CheckedExpr::RwLockNew { initial } => walk_expr(initial, f),
        CheckedExpr::RwLockReadLock { lock }
        | CheckedExpr::RwLockReadUnlock { lock }
        | CheckedExpr::RwLockWriteLock { lock }
        | CheckedExpr::RwLockWriteUnlock { lock }
        | CheckedExpr::RwLockGet { lock } => walk_expr(lock, f),
        CheckedExpr::RwLockSet { lock, value } => {
            walk_expr(lock, f);
            walk_expr(value, f);
        }
        CheckedExpr::ParallelMap { list, .. } => walk_expr(list, f),
        CheckedExpr::HttpGet { url } => walk_expr(url, f),
        CheckedExpr::SerdeEncode { value, .. } => walk_expr(value, f),
        CheckedExpr::SerdeDecode { text, .. } => walk_expr(text, f),
        CheckedExpr::CancelTokenCancel { token }
        | CheckedExpr::CancelTokenIsCancelled { token } => walk_expr(token, f),
        CheckedExpr::ListPush { list, value } => {
            walk_expr(list, f);
            walk_expr(value, f);
        }
        CheckedExpr::ListGet { list, index, .. } => {
            walk_expr(list, f);
            walk_expr(index, f);
        }
        CheckedExpr::ListSet { list, index, value } => {
            walk_expr(list, f);
            walk_expr(index, f);
            walk_expr(value, f);
        }
        CheckedExpr::ListLen { list } => walk_expr(list, f),
        CheckedExpr::StdPanic { msg: inner }
        | CheckedExpr::StdEnvGet { name: inner }
        | CheckedExpr::StdProcessExit { code: inner }
        | CheckedExpr::StdFsReadToString { path: inner }
        | CheckedExpr::StdStringLen { s: inner }
        | CheckedExpr::StdStringFromInt { n: inner }
        | CheckedExpr::StdStringParseInt { s: inner } => walk_expr(inner, f),
        CheckedExpr::StdEnvSet { name: a, value: b }
        | CheckedExpr::StdFsWriteString {
            path: a,
            contents: b,
        }
        | CheckedExpr::StdStringConcat { a, b }
        | CheckedExpr::StdStringContains {
            hay: a,
            needle: b,
        } => {
            walk_expr(a, f);
            walk_expr(b, f);
        }
        CheckedExpr::StdStringSlice { s, start, end } => {
            walk_expr(s, f);
            walk_expr(start, f);
            walk_expr(end, f);
        }
        CheckedExpr::AsyncBlock { body, .. } | CheckedExpr::Closure { body, .. } => {
            for s in body {
                walk_stmt(s, f);
            }
        }
        CheckedExpr::CallClosure { callee, args, .. } => {
            walk_expr(callee, f);
            for a in args {
                walk_expr(a, f);
            }
        }
        _ => {}
    }
}

struct SpawnThunk {
    body: Vec<CheckedStmt>,
    captures: Vec<String>,
}

struct AsyncBlockThunk {
    body: Vec<CheckedStmt>,
    captures: Vec<String>,
}

struct ClosureThunk {
    params: Vec<(String, Ty)>,
    ret: Ty,
    body: Vec<CheckedStmt>,
    captures: Vec<String>,
}

fn closure_thunk_signature(params: &[(String, Ty)], ret: &Ty) -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    // env pointer
    sig.params.push(AbiParam::new(types::I64));
    for (_, ty) in params {
        sig.params.push(AbiParam::new(clif_ty(ty)));
    }
    if *ret != Ty::Void {
        sig.returns.push(AbiParam::new(clif_ty(ret)));
    }
    sig
}

fn collect_closures(program: &CheckedProgram) -> Vec<ClosureThunk> {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    let mut collect = |e: &CheckedExpr| {
        if let CheckedExpr::Closure {
            index,
            params,
            ret,
            body,
            captures,
        } = e
        {
            map.entry(*index).or_insert_with(|| ClosureThunk {
                params: params.clone(),
                ret: ret.clone(),
                body: body.clone(),
                captures: captures.clone(),
            });
        }
    };
    for f in &program.functions {
        for stmt in &f.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    for m in &program.methods {
        for stmt in &m.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    map.into_values().collect()
}

fn collect_async_blocks(program: &CheckedProgram) -> Vec<AsyncBlockThunk> {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    let mut collect = |e: &CheckedExpr| {
        if let CheckedExpr::AsyncBlock {
            index,
            body,
            captures,
        } = e
        {
            map.entry(*index).or_insert_with(|| AsyncBlockThunk {
                body: body.clone(),
                captures: captures.clone(),
            });
        }
    };
    for f in &program.functions {
        for stmt in &f.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    for m in &program.methods {
        for stmt in &m.body {
            walk_stmt(stmt, &mut collect);
        }
    }
    map.into_values().collect()
}

fn collect_spawn_bodies(program: &CheckedProgram) -> Vec<SpawnThunk> {
    use std::collections::BTreeMap;
    let mut map = BTreeMap::new();
    fn walk(stmts: &[CheckedStmt], map: &mut BTreeMap<usize, SpawnThunk>) {
        for s in stmts {
            match s {
                CheckedStmt::Spawn {
                    index,
                    body,
                    captures,
                } => {
                    map.insert(
                        *index,
                        SpawnThunk {
                            body: body.clone(),
                            captures: captures.clone(),
                        },
                    );
                    walk(body, map);
                }
                CheckedStmt::If { arms, else_block } => {
                    for (c, b) in arms {
                        walk_expr_spawns(c, map);
                        walk(b, map);
                    }
                    if let Some(eb) = else_block {
                        walk(eb, map);
                    }
                }
                CheckedStmt::While { cond, body } => {
                    walk_expr_spawns(cond, map);
                    walk(body, map);
                }
                CheckedStmt::ForRange {
                    start,
                    end,
                    body,
                    ..
                } => {
                    walk_expr_spawns(start, map);
                    walk_expr_spawns(end, map);
                    walk(body, map);
                }
                CheckedStmt::ForIn { iter, body, .. } => {
                    walk_expr_spawns(iter, map);
                    walk(body, map);
                }
                CheckedStmt::Match { scrutinee, arms } => {
                    walk_expr_spawns(scrutinee, map);
                    for (_, b) in arms {
                        walk(b, map);
                    }
                }
                CheckedStmt::VarDecl { init, .. } => walk_expr_spawns(init, map),
                CheckedStmt::Assign { value, .. } => walk_expr_spawns(value, map),
                CheckedStmt::Return { value } => {
                    if let Some(v) = value {
                        walk_expr_spawns(v, map);
                    }
                }
                CheckedStmt::Expr { expr } | CheckedStmt::Drop { object: expr, .. } => {
                    walk_expr_spawns(expr, map)
                }
                CheckedStmt::Break | CheckedStmt::Continue => {}
            }
        }
    }
    fn walk_expr_spawns(e: &CheckedExpr, map: &mut BTreeMap<usize, SpawnThunk>) {
        walk_expr(e, &mut |inner| {
            if let CheckedExpr::AsyncBlock { body, .. } = inner {
                walk(body, map);
            }
        });
    }
    for f in &program.functions {
        walk(&f.body, &mut map);
    }
    for m in &program.methods {
        walk(&m.body, &mut map);
    }
    map.into_values().collect()
}

fn declare_functions<M: Module>(
    module: &mut M,
    program: &CheckedProgram,
) -> Result<HashMap<String, FuncId>> {
    let mut ids = HashMap::new();
    for f in &program.functions {
        let mut sig = Signature::new(CallConv::SystemV);
        for (_, ty) in &f.params {
            sig.params.push(AbiParam::new(clif_ty(ty)));
        }
        if f.return_ty != Ty::Void {
            sig.returns.push(AbiParam::new(clif_ty(&f.return_ty)));
        }
        let linkage = if f.name == "main" {
            Linkage::Export
        } else {
            Linkage::Local
        };
        let id = module
            .declare_function(&f.name, linkage, &sig)
            .map_err(|e| anyhow!("{e}"))?;
        ids.insert(f.name.clone(), id);
        if f.is_async && f.name != "main" {
            let wrap = async_wrap_name(&f.name);
            let wsig = spawn_thunk_signature();
            let wid = module
                .declare_function(&wrap, Linkage::Local, &wsig)
                .map_err(|e| anyhow!("{e}"))?;
            ids.insert(wrap, wid);
        }
    }
    for name in collect_cpu_submit_fns(program) {
        let wrap = cpu_submit_wrap_name(&name);
        let wsig = spawn_thunk_signature();
        let wid = module
            .declare_function(&wrap, Linkage::Local, &wsig)
            .map_err(|e| anyhow!("{e}"))?;
        ids.insert(wrap, wid);
    }
    if program_has_cpu_submit_closure(program) {
        let wsig = spawn_thunk_signature();
        let wid = module
            .declare_function("__cpu_submit_closure", Linkage::Local, &wsig)
            .map_err(|e| anyhow!("{e}"))?;
        ids.insert("__cpu_submit_closure".into(), wid);
    }
    for m in &program.methods {
        let mut sig = Signature::new(CallConv::SystemV);
        // Implicit self receiver (pointer).
        sig.params.push(AbiParam::new(types::I64));
        for (_, ty) in &m.params {
            sig.params.push(AbiParam::new(clif_ty(ty)));
        }
        if m.return_ty != Ty::Void {
            sig.returns.push(AbiParam::new(clif_ty(&m.return_ty)));
        }
        let id = module
            .declare_function(&m.symbol, Linkage::Local, &sig)
            .map_err(|e| anyhow!("{e}"))?;
        ids.insert(m.symbol.clone(), id);
    }
    let spawns = collect_spawn_bodies(program);
    for i in 0..spawns.len() {
        let name = format!("__spawn_{i}");
        let sig = spawn_thunk_signature();
        let id = module
            .declare_function(&name, Linkage::Local, &sig)
            .map_err(|e| anyhow!("{e}"))?;
        ids.insert(name, id);
    }
    let async_blocks = collect_async_blocks(program);
    for i in 0..async_blocks.len() {
        let name = format!("__async_block_{i}");
        let sig = spawn_thunk_signature();
        let id = module
            .declare_function(&name, Linkage::Local, &sig)
            .map_err(|e| anyhow!("{e}"))?;
        ids.insert(name, id);
    }
    let closures = collect_closures(program);
    for (i, thunk) in closures.iter().enumerate() {
        let name = format!("__closure_{i}");
        let sig = closure_thunk_signature(&thunk.params, &thunk.ret);
        let id = module
            .declare_function(&name, Linkage::Local, &sig)
            .map_err(|e| anyhow!("{e}"))?;
        ids.insert(name, id);
    }
    Ok(ids)
}

fn define_strings<M: Module>(module: &mut M, lits: &[Vec<u8>]) -> Result<StrMap> {
    let mut map = StrMap::new();
    for (i, bytes) in lits.iter().enumerate() {
        let name = format!(".Lstr{i}");
        let id = module
            .declare_data(&name, Linkage::Local, false, false)
            .map_err(|e| anyhow!("{e}"))?;
        let mut desc = DataDescription::new();
        let mut with_nul = bytes.clone();
        with_nul.push(0);
        desc.define(with_nul.into_boxed_slice());
        module.define_data(id, &desc).map_err(|e| anyhow!("{e}"))?;
        map.insert(bytes.clone(), (id, bytes.len()));
    }
    Ok(map)
}

pub fn jit_run(program: &CheckedProgram) -> Result<()> {
    let isa = cranelift_native::builder()
        .map_err(|e| anyhow!("{e}"))?
        .finish(make_flags(false))
        .map_err(|e| anyhow!("{e}"))?;

    let mut jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    jit_builder.symbol("stk_std_log", stk_std_log as *const u8);
    jit_builder.symbol("stk_sleep_ms", stk_sleep_ms as *const u8);
    jit_builder.symbol("stk_alloc", stk_alloc as *const u8);
    jit_builder.symbol("stk_future_new", stk_future_new as *const u8);
    jit_builder.symbol("stk_future_complete", stk_future_complete as *const u8);
    jit_builder.symbol("stk_future_ready", stk_future_ready as *const u8);
    jit_builder.symbol("stk_future_await", stk_future_await as *const u8);
    jit_builder.symbol("stk_future_join", stk_future_join as *const u8);
    jit_builder.symbol("stk_future_race", stk_future_race as *const u8);
    jit_builder.symbol("stk_spawn", stk_spawn as *const u8);
    jit_builder.symbol("stk_spawn_drain", stk_spawn_drain as *const u8);
    jit_builder.symbol("stk_channel_new", stk_channel_new as *const u8);
    jit_builder.symbol("stk_channel_buffered", stk_channel_buffered as *const u8);
    jit_builder.symbol("stk_channel_send", stk_channel_send as *const u8);
    jit_builder.symbol("stk_channel_recv", stk_channel_recv as *const u8);
    jit_builder.symbol("stk_channel_recv_future", stk_channel_recv_future as *const u8);
    jit_builder.symbol("stk_channel_recv_ok", stk_channel_recv_ok as *const u8);
    jit_builder.symbol("stk_channel_close", stk_channel_close as *const u8);
    jit_builder.symbol("stk_waitgroup_new", stk_waitgroup_new as *const u8);
    jit_builder.symbol("stk_waitgroup_add", stk_waitgroup_add as *const u8);
    jit_builder.symbol("stk_waitgroup_done", stk_waitgroup_done as *const u8);
    jit_builder.symbol("stk_waitgroup_wait", stk_waitgroup_wait as *const u8);
    jit_builder.symbol("stk_waitgroup_wait_future", stk_waitgroup_wait_future as *const u8);
    jit_builder.symbol("stk_mutex_new", stk_mutex_new as *const u8);
    jit_builder.symbol("stk_mutex_lock", stk_mutex_lock as *const u8);
    jit_builder.symbol("stk_mutex_unlock", stk_mutex_unlock as *const u8);
    jit_builder.symbol("stk_mutex_get", stk_mutex_get as *const u8);
    jit_builder.symbol("stk_mutex_set", stk_mutex_set as *const u8);
    jit_builder.symbol("stk_rwlock_new", stk_rwlock_new as *const u8);
    jit_builder.symbol("stk_rwlock_read_lock", stk_rwlock_read_lock as *const u8);
    jit_builder.symbol("stk_rwlock_read_unlock", stk_rwlock_read_unlock as *const u8);
    jit_builder.symbol("stk_rwlock_write_lock", stk_rwlock_write_lock as *const u8);
    jit_builder.symbol("stk_rwlock_write_unlock", stk_rwlock_write_unlock as *const u8);
    jit_builder.symbol("stk_rwlock_get", stk_rwlock_get as *const u8);
    jit_builder.symbol("stk_rwlock_set", stk_rwlock_set as *const u8);
    jit_builder.symbol("stk_parallel_map_int", stk_parallel_map_int as *const u8);
    jit_builder.symbol("stk_http_get", stk_http_get as *const u8);
    jit_builder.symbol("stk_task_yield", stk_task_yield as *const u8);
    jit_builder.symbol("stk_cancel_token_new", stk_cancel_token_new as *const u8);
    jit_builder.symbol("stk_cancel_token_cancel", stk_cancel_token_cancel as *const u8);
    jit_builder.symbol("stk_cancel_token_is_cancelled", stk_cancel_token_is_cancelled as *const u8);
    jit_builder.symbol("stk_list_new", stk_list_new as *const u8);
    jit_builder.symbol("stk_list_push", stk_list_push as *const u8);
    jit_builder.symbol("stk_list_get", stk_list_get as *const u8);
    jit_builder.symbol("stk_list_set", stk_list_set as *const u8);
    jit_builder.symbol("stk_list_len", stk_list_len as *const u8);
    jit_builder.symbol("stk_panic", stk_panic as *const u8);
    jit_builder.symbol("stk_process_exit", stk_process_exit as *const u8);
    jit_builder.symbol("stk_time_now_ms", stk_time_now_ms as *const u8);
    jit_builder.symbol("stk_env_args", stk_env_args as *const u8);
    jit_builder.symbol("stk_env_get", stk_env_get as *const u8);
    jit_builder.symbol("stk_env_set", stk_env_set as *const u8);
    jit_builder.symbol("stk_fs_read_to_string", stk_fs_read_to_string as *const u8);
    jit_builder.symbol("stk_fs_write_string", stk_fs_write_string as *const u8);
    jit_builder.symbol("stk_string_len", stk_string_len as *const u8);
    jit_builder.symbol("stk_string_concat", stk_string_concat as *const u8);
    jit_builder.symbol("stk_string_slice", stk_string_slice as *const u8);
    jit_builder.symbol("stk_string_contains", stk_string_contains as *const u8);
    jit_builder.symbol("stk_string_from_int", stk_string_from_int as *const u8);
    jit_builder.symbol("stk_string_parse_int", stk_string_parse_int as *const u8);
    jit_builder.symbol("stk_serde_encode", stk_serde_encode as *const u8);
    jit_builder.symbol("stk_serde_decode", stk_serde_decode as *const u8);
    let mut module = JITModule::new(jit_builder);

    let lits = collect_string_lits(program);
    let strings = define_strings(&mut module, &lits)?;
    let spawn_bodies = collect_spawn_bodies(program);
    let async_bodies = collect_async_blocks(program);
    let closure_bodies = collect_closures(program);
    let func_ids = declare_functions(&mut module, program)?;
    let runtime = declare_runtime(&mut module)?;

    let mut ctx = module.make_context();
    let mut fb_ctx = FunctionBuilderContext::new();
    for (i, thunk) in spawn_bodies.iter().enumerate() {
        define_spawn_thunk(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            i,
            thunk,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }
    for (i, thunk) in async_bodies.iter().enumerate() {
        define_async_block_thunk(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            i,
            thunk,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }
    for (i, thunk) in closure_bodies.iter().enumerate() {
        define_closure_thunk(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            i,
            thunk,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }
    for f in &program.functions {
        define_func(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            f,
            &func_ids,
            &runtime,
            &strings,
        )?;
        if f.is_async && f.name != "main" {
            define_async_wrap(
                &mut module,
                &mut ctx,
                &mut fb_ctx,
                f,
                &func_ids,
                &runtime,
                &strings,
            )?;
        }
    }
    for name in collect_cpu_submit_fns(program) {
        define_cpu_submit_wrap(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            &name,
            &func_ids,
            &runtime,
        )?;
    }
    if program_has_cpu_submit_closure(program) {
        define_cpu_submit_closure_wrap(&mut module, &mut ctx, &mut fb_ctx, &func_ids, &runtime)?;
    }
    for m in &program.methods {
        define_method(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            m,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }

    module.finalize_definitions().map_err(|e| anyhow!("{e}"))?;
    install_host_argv();
    let main_id = *func_ids.get("main").ok_or_else(|| anyhow!("missing main"))?;
    let ptr = module.get_finalized_function(main_id);
    let main_fn: extern "C" fn() = unsafe { std::mem::transmute(ptr) };
    main_fn();
    unsafe { stk_spawn_drain() };
    Ok(())
}

/// Publish the host process arguments so `std.env.args()` works under the JIT.
fn install_host_argv() {
    let owned: Vec<std::ffi::CString> = std::env::args()
        .map(|a| std::ffi::CString::new(a).unwrap_or_default())
        .collect();
    let ptrs: Vec<i64> = owned.iter().map(|c| c.as_ptr() as i64).collect();
    // `stk_set_argv` copies the strings, so `owned` may be dropped after the call.
    unsafe { stk_set_argv(ptrs.len() as i64, ptrs.as_ptr()) };
}

struct RuntimeIds {
    log_id: FuncId,
    sleep_id: FuncId,
    alloc_id: FuncId,
    future_new_id: FuncId,
    future_complete_id: FuncId,
    future_ready_id: FuncId,
    future_await_id: FuncId,
    future_join_id: FuncId,
    future_race_id: FuncId,
    spawn_id: FuncId,
    drain_id: FuncId,
    channel_new_id: FuncId,
    channel_buffered_id: FuncId,
    channel_send_id: FuncId,
    channel_recv_id: FuncId,
    channel_recv_future_id: FuncId,
    channel_recv_ok_id: FuncId,
    channel_close_id: FuncId,
    waitgroup_new_id: FuncId,
    waitgroup_add_id: FuncId,
    waitgroup_done_id: FuncId,
    waitgroup_wait_id: FuncId,
    waitgroup_wait_future_id: FuncId,
    mutex_new_id: FuncId,
    mutex_lock_id: FuncId,
    mutex_unlock_id: FuncId,
    mutex_get_id: FuncId,
    mutex_set_id: FuncId,
    rwlock_new_id: FuncId,
    rwlock_read_lock_id: FuncId,
    rwlock_read_unlock_id: FuncId,
    rwlock_write_lock_id: FuncId,
    rwlock_write_unlock_id: FuncId,
    rwlock_get_id: FuncId,
    rwlock_set_id: FuncId,
    parallel_map_int_id: FuncId,
    http_get_id: FuncId,
    task_yield_id: FuncId,
    cancel_token_new_id: FuncId,
    cancel_token_cancel_id: FuncId,
    cancel_token_is_cancelled_id: FuncId,
    list_new_id: FuncId,
    list_push_id: FuncId,
    list_get_id: FuncId,
    list_set_id: FuncId,
    list_len_id: FuncId,
    panic_id: FuncId,
    process_exit_id: FuncId,
    time_now_ms_id: FuncId,
    env_args_id: FuncId,
    env_get_id: FuncId,
    env_set_id: FuncId,
    fs_read_to_string_id: FuncId,
    fs_write_string_id: FuncId,
    string_len_id: FuncId,
    string_concat_id: FuncId,
    string_slice_id: FuncId,
    string_contains_id: FuncId,
    string_from_int_id: FuncId,
    string_parse_int_id: FuncId,
    serde_encode_id: FuncId,
    serde_decode_id: FuncId,
}

fn ternary_i64_signature(ret: bool) -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    for _ in 0..3 {
        sig.params.push(AbiParam::new(types::I64));
    }
    if ret {
        sig.returns.push(AbiParam::new(types::I64));
    }
    sig
}

fn declare_runtime<M: Module>(module: &mut M) -> Result<RuntimeIds> {
    Ok(RuntimeIds {
        log_id: module
            .declare_function("stk_std_log", Linkage::Import, &std_log_signature())
            .map_err(|e| anyhow!("{e}"))?,
        sleep_id: module
            .declare_function("stk_sleep_ms", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        alloc_id: module
            .declare_function("stk_alloc", Linkage::Import, &alloc_signature())
            .map_err(|e| anyhow!("{e}"))?,
        future_new_id: module
            .declare_function("stk_future_new", Linkage::Import, &void_ret_i64_signature())
            .map_err(|e| anyhow!("{e}"))?,
        future_complete_id: module
            .declare_function(
                "stk_future_complete",
                Linkage::Import,
                &binary_i64_signature(false),
            )
            .map_err(|e| anyhow!("{e}"))?,
        future_ready_id: module
            .declare_function("stk_future_ready", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        future_await_id: module
            .declare_function("stk_future_await", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        future_join_id: module
            .declare_function("stk_future_join", Linkage::Import, &binary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        future_race_id: module
            .declare_function("stk_future_race", Linkage::Import, &binary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        spawn_id: module
            .declare_function("stk_spawn", Linkage::Import, &binary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        drain_id: module
            .declare_function("stk_spawn_drain", Linkage::Import, &void_signature())
            .map_err(|e| anyhow!("{e}"))?,
        channel_new_id: module
            .declare_function("stk_channel_new", Linkage::Import, &void_ret_i64_signature())
            .map_err(|e| anyhow!("{e}"))?,
        channel_buffered_id: module
            .declare_function("stk_channel_buffered", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        channel_send_id: module
            .declare_function("stk_channel_send", Linkage::Import, &binary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        channel_recv_id: module
            .declare_function("stk_channel_recv", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        channel_recv_future_id: module
            .declare_function(
                "stk_channel_recv_future",
                Linkage::Import,
                &unary_i64_signature(true),
            )
            .map_err(|e| anyhow!("{e}"))?,
        channel_recv_ok_id: module
            .declare_function("stk_channel_recv_ok", Linkage::Import, &binary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        channel_close_id: module
            .declare_function("stk_channel_close", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        waitgroup_new_id: module
            .declare_function("stk_waitgroup_new", Linkage::Import, &void_ret_i64_signature())
            .map_err(|e| anyhow!("{e}"))?,
        waitgroup_add_id: module
            .declare_function("stk_waitgroup_add", Linkage::Import, &binary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        waitgroup_done_id: module
            .declare_function("stk_waitgroup_done", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        waitgroup_wait_id: module
            .declare_function("stk_waitgroup_wait", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        waitgroup_wait_future_id: module
            .declare_function(
                "stk_waitgroup_wait_future",
                Linkage::Import,
                &unary_i64_signature(true),
            )
            .map_err(|e| anyhow!("{e}"))?,
        mutex_new_id: module
            .declare_function("stk_mutex_new", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        mutex_lock_id: module
            .declare_function("stk_mutex_lock", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        mutex_unlock_id: module
            .declare_function("stk_mutex_unlock", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        mutex_get_id: module
            .declare_function("stk_mutex_get", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        mutex_set_id: module
            .declare_function("stk_mutex_set", Linkage::Import, &binary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        rwlock_new_id: module
            .declare_function("stk_rwlock_new", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        rwlock_read_lock_id: module
            .declare_function(
                "stk_rwlock_read_lock",
                Linkage::Import,
                &unary_i64_signature(false),
            )
            .map_err(|e| anyhow!("{e}"))?,
        rwlock_read_unlock_id: module
            .declare_function(
                "stk_rwlock_read_unlock",
                Linkage::Import,
                &unary_i64_signature(false),
            )
            .map_err(|e| anyhow!("{e}"))?,
        rwlock_write_lock_id: module
            .declare_function(
                "stk_rwlock_write_lock",
                Linkage::Import,
                &unary_i64_signature(false),
            )
            .map_err(|e| anyhow!("{e}"))?,
        rwlock_write_unlock_id: module
            .declare_function(
                "stk_rwlock_write_unlock",
                Linkage::Import,
                &unary_i64_signature(false),
            )
            .map_err(|e| anyhow!("{e}"))?,
        rwlock_get_id: module
            .declare_function("stk_rwlock_get", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        rwlock_set_id: module
            .declare_function("stk_rwlock_set", Linkage::Import, &binary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        parallel_map_int_id: module
            .declare_function(
                "stk_parallel_map_int",
                Linkage::Import,
                &binary_i64_signature(true),
            )
            .map_err(|e| anyhow!("{e}"))?,
        http_get_id: module
            .declare_function("stk_http_get", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        task_yield_id: module
            .declare_function("stk_task_yield", Linkage::Import, &Signature::new(CallConv::SystemV))
            .map_err(|e| anyhow!("{e}"))?,
        cancel_token_new_id: module
            .declare_function(
                "stk_cancel_token_new",
                Linkage::Import,
                &void_ret_i64_signature(),
            )
            .map_err(|e| anyhow!("{e}"))?,
        cancel_token_cancel_id: module
            .declare_function(
                "stk_cancel_token_cancel",
                Linkage::Import,
                &unary_i64_signature(false),
            )
            .map_err(|e| anyhow!("{e}"))?,
        cancel_token_is_cancelled_id: module
            .declare_function(
                "stk_cancel_token_is_cancelled",
                Linkage::Import,
                &unary_i64_signature(true),
            )
            .map_err(|e| anyhow!("{e}"))?,
        list_new_id: module
            .declare_function("stk_list_new", Linkage::Import, &void_ret_i64_signature())
            .map_err(|e| anyhow!("{e}"))?,
        list_push_id: module
            .declare_function("stk_list_push", Linkage::Import, &binary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        list_get_id: module
            .declare_function("stk_list_get", Linkage::Import, &binary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        list_set_id: module
            .declare_function("stk_list_set", Linkage::Import, &ternary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        list_len_id: module
            .declare_function("stk_list_len", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        panic_id: module
            .declare_function("stk_panic", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        process_exit_id: module
            .declare_function("stk_process_exit", Linkage::Import, &unary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        time_now_ms_id: module
            .declare_function("stk_time_now_ms", Linkage::Import, &void_ret_i64_signature())
            .map_err(|e| anyhow!("{e}"))?,
        env_args_id: module
            .declare_function("stk_env_args", Linkage::Import, &void_ret_i64_signature())
            .map_err(|e| anyhow!("{e}"))?,
        env_get_id: module
            .declare_function("stk_env_get", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        env_set_id: module
            .declare_function("stk_env_set", Linkage::Import, &binary_i64_signature(false))
            .map_err(|e| anyhow!("{e}"))?,
        fs_read_to_string_id: module
            .declare_function(
                "stk_fs_read_to_string",
                Linkage::Import,
                &unary_i64_signature(true),
            )
            .map_err(|e| anyhow!("{e}"))?,
        fs_write_string_id: module
            .declare_function(
                "stk_fs_write_string",
                Linkage::Import,
                &binary_i64_signature(true),
            )
            .map_err(|e| anyhow!("{e}"))?,
        string_len_id: module
            .declare_function("stk_string_len", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        string_concat_id: module
            .declare_function("stk_string_concat", Linkage::Import, &binary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        string_slice_id: module
            .declare_function("stk_string_slice", Linkage::Import, &ternary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        string_contains_id: module
            .declare_function(
                "stk_string_contains",
                Linkage::Import,
                &binary_i64_signature(true),
            )
            .map_err(|e| anyhow!("{e}"))?,
        string_from_int_id: module
            .declare_function("stk_string_from_int", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        string_parse_int_id: module
            .declare_function("stk_string_parse_int", Linkage::Import, &unary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        serde_encode_id: module
            .declare_function("stk_serde_encode", Linkage::Import, &ternary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
        serde_decode_id: module
            .declare_function("stk_serde_decode", Linkage::Import, &ternary_i64_signature(true))
            .map_err(|e| anyhow!("{e}"))?,
    })
}

fn void_ret_i64_signature() -> Signature {
    let mut sig = Signature::new(CallConv::SystemV);
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

pub fn build_executable(program: &CheckedProgram, out: &Path) -> Result<()> {
    let isa = cranelift_native::builder()
        .map_err(|e| anyhow!("{e}"))?
        .finish(make_flags(true))
        .map_err(|e| anyhow!("{e}"))?;

    let builder = ObjectBuilder::new(isa, "steampunk", cranelift_module::default_libcall_names())
        .map_err(|e| anyhow!("{e}"))?;
    let mut module = ObjectModule::new(builder);

    let lits = collect_string_lits(program);
    let strings = define_strings(&mut module, &lits)?;
    let spawn_bodies = collect_spawn_bodies(program);
    let async_bodies = collect_async_blocks(program);
    let closure_bodies = collect_closures(program);
    let func_ids = declare_functions(&mut module, program)?;
    let runtime = declare_runtime(&mut module)?;

    let mut ctx = module.make_context();
    let mut fb_ctx = FunctionBuilderContext::new();
    for (i, thunk) in spawn_bodies.iter().enumerate() {
        define_spawn_thunk(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            i,
            thunk,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }
    for (i, thunk) in async_bodies.iter().enumerate() {
        define_async_block_thunk(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            i,
            thunk,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }
    for (i, thunk) in closure_bodies.iter().enumerate() {
        define_closure_thunk(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            i,
            thunk,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }
    for f in &program.functions {
        define_func(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            f,
            &func_ids,
            &runtime,
            &strings,
        )?;
        if f.is_async && f.name != "main" {
            define_async_wrap(
                &mut module,
                &mut ctx,
                &mut fb_ctx,
                f,
                &func_ids,
                &runtime,
                &strings,
            )?;
        }
    }
    for name in collect_cpu_submit_fns(program) {
        define_cpu_submit_wrap(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            &name,
            &func_ids,
            &runtime,
        )?;
    }
    if program_has_cpu_submit_closure(program) {
        define_cpu_submit_closure_wrap(&mut module, &mut ctx, &mut fb_ctx, &func_ids, &runtime)?;
    }
    for m in &program.methods {
        define_method(
            &mut module,
            &mut ctx,
            &mut fb_ctx,
            m,
            &func_ids,
            &runtime,
            &strings,
        )?;
    }

    let product = module.finish();
    let obj_bytes = product.emit().map_err(|e| anyhow!("{e}"))?;

    let tmp = std::env::temp_dir().join(format!("steampunk-build-{}", std::process::id()));
    fs::create_dir_all(&tmp)?;
    let obj_path = tmp.join("program.o");
    let rt_c = tmp.join("runtime.c");
    let rt_o = tmp.join("runtime.o");
    fs::write(&obj_path, &obj_bytes).context("write object")?;
    fs::write(&rt_c, include_str!("runtime.c"))?;

    let cc = find_cc()?;
    run_cmd(&cc, &["-c", "-o", rt_o.to_str().unwrap(), rt_c.to_str().unwrap()])?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).ok();
    }
    run_cmd(
        &cc,
        &[
            "-o",
            out.to_str().unwrap(),
            obj_path.to_str().unwrap(),
            rt_o.to_str().unwrap(),
            "-lpthread",
        ],
    )?;
    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}

fn find_cc() -> Result<String> {
    for cand in ["cc", "clang", "gcc"] {
        if Command::new(cand)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(cand.into());
        }
    }
    bail!("no C compiler/linker found (need cc, clang, or gcc to link the native object)")
}

fn run_cmd(bin: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(bin).args(args).status()?;
    if !status.success() {
        bail!("{bin} {:?} failed", args);
    }
    Ok(())
}

fn make_emit<'a, M: Module>(
    module: &'a mut M,
    func_ids: &'a HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &'a StrMap,
) -> EmitCtx<'a, M> {
    EmitCtx {
        module,
        func_ids,
        log_id: runtime.log_id,
        sleep_id: runtime.sleep_id,
        alloc_id: runtime.alloc_id,
        future_new_id: runtime.future_new_id,
        future_complete_id: runtime.future_complete_id,
        future_ready_id: runtime.future_ready_id,
        future_await_id: runtime.future_await_id,
        future_join_id: runtime.future_join_id,
        future_race_id: runtime.future_race_id,
        spawn_id: runtime.spawn_id,
        channel_new_id: runtime.channel_new_id,
        channel_buffered_id: runtime.channel_buffered_id,
        channel_send_id: runtime.channel_send_id,
        channel_recv_id: runtime.channel_recv_id,
        channel_recv_future_id: runtime.channel_recv_future_id,
        channel_recv_ok_id: runtime.channel_recv_ok_id,
        channel_close_id: runtime.channel_close_id,
        waitgroup_new_id: runtime.waitgroup_new_id,
        waitgroup_add_id: runtime.waitgroup_add_id,
        waitgroup_done_id: runtime.waitgroup_done_id,
        waitgroup_wait_id: runtime.waitgroup_wait_id,
        waitgroup_wait_future_id: runtime.waitgroup_wait_future_id,
        mutex_new_id: runtime.mutex_new_id,
        mutex_lock_id: runtime.mutex_lock_id,
        mutex_unlock_id: runtime.mutex_unlock_id,
        mutex_get_id: runtime.mutex_get_id,
        mutex_set_id: runtime.mutex_set_id,
        rwlock_new_id: runtime.rwlock_new_id,
        rwlock_read_lock_id: runtime.rwlock_read_lock_id,
        rwlock_read_unlock_id: runtime.rwlock_read_unlock_id,
        rwlock_write_lock_id: runtime.rwlock_write_lock_id,
        rwlock_write_unlock_id: runtime.rwlock_write_unlock_id,
        rwlock_get_id: runtime.rwlock_get_id,
        rwlock_set_id: runtime.rwlock_set_id,
        parallel_map_int_id: runtime.parallel_map_int_id,
        http_get_id: runtime.http_get_id,
        task_yield_id: runtime.task_yield_id,
        cancel_token_new_id: runtime.cancel_token_new_id,
        cancel_token_cancel_id: runtime.cancel_token_cancel_id,
        cancel_token_is_cancelled_id: runtime.cancel_token_is_cancelled_id,
        list_new_id: runtime.list_new_id,
        list_push_id: runtime.list_push_id,
        list_get_id: runtime.list_get_id,
        list_set_id: runtime.list_set_id,
        list_len_id: runtime.list_len_id,
        panic_id: runtime.panic_id,
        process_exit_id: runtime.process_exit_id,
        time_now_ms_id: runtime.time_now_ms_id,
        env_args_id: runtime.env_args_id,
        env_get_id: runtime.env_get_id,
        env_set_id: runtime.env_set_id,
        fs_read_to_string_id: runtime.fs_read_to_string_id,
        fs_write_string_id: runtime.fs_write_string_id,
        string_len_id: runtime.string_len_id,
        string_concat_id: runtime.string_concat_id,
        string_slice_id: runtime.string_slice_id,
        string_contains_id: runtime.string_contains_id,
        string_from_int_id: runtime.string_from_int_id,
        string_parse_int_id: runtime.string_parse_int_id,
        serde_encode_id: runtime.serde_encode_id,
        serde_decode_id: runtime.serde_decode_id,
        strings,
        vars: HashMap::new(),
        next_var: 0,
        str_lens: HashMap::new(),
        loops: Vec::new(),
        async_complete_handle: None,
    }
}

fn define_async_wrap<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    f: &CheckedFunction,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &StrMap,
) -> Result<()> {
    let wrap_name = async_wrap_name(&f.name);
    let func_id = func_ids[&wrap_name];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let env = builder.block_params(entry)[0];
    let flags = MemFlags::trusted();
    let handle = builder.ins().load(types::I64, flags, env, 0);

    let mut arg_vals = Vec::new();
    for i in 0..f.params.len() {
        let off = builder.ins().iadd_imm(env, ((i as i64) + 1) * 8);
        arg_vals.push(builder.ins().load(types::I64, flags, off, 0));
    }

    let callee = *func_ids
        .get(&f.name)
        .ok_or_else(|| anyhow!("missing async fn {}", f.name))?;
    let fref = module.declare_func_in_func(callee, builder.func);
    let call = builder.ins().call(fref, &arg_vals);
    let result = if f.return_ty == Ty::Void {
        builder.ins().iconst(types::I64, 0)
    } else {
        builder.inst_results(call)[0]
    };

    let cref = module.declare_func_in_func(runtime.future_complete_id, builder.func);
    builder.ins().call(cref, &[handle, result]);
    builder.ins().return_(&[]);

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    let _ = strings;
    Ok(())
}

fn define_cpu_submit_wrap<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    fn_name: &str,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
) -> Result<()> {
    let wrap_name = cpu_submit_wrap_name(fn_name);
    let func_id = func_ids[&wrap_name];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let env = builder.block_params(entry)[0];
    let flags = MemFlags::trusted();
    let handle = builder.ins().load(types::I64, flags, env, 0);

    let callee = *func_ids
        .get(fn_name)
        .ok_or_else(|| anyhow!("missing cpu submit fn {fn_name}"))?;
    let fref = module.declare_func_in_func(callee, builder.func);
    let call = builder.ins().call(fref, &[]);
    let result = builder.inst_results(call)[0];

    let cref = module.declare_func_in_func(runtime.future_complete_id, builder.func);
    builder.ins().call(cref, &[handle, result]);
    builder.ins().return_(&[]);

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    Ok(())
}

fn define_cpu_submit_closure_wrap<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
) -> Result<()> {
    let func_id = func_ids["__cpu_submit_closure"];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let submit_env = builder.block_params(entry)[0];
    let flags = MemFlags::trusted();
    let handle = builder.ins().load(types::I64, flags, submit_env, 0);
    let fat = builder.ins().load(types::I64, flags, submit_env, 8);
    let code = builder.ins().load(types::I64, flags, fat, 0);
    let env = builder.ins().load(types::I64, flags, fat, 8);

    let mut call_sig = Signature::new(CallConv::SystemV);
    call_sig.params.push(AbiParam::new(types::I64));
    call_sig.returns.push(AbiParam::new(types::I64));
    let sig_ref = builder.import_signature(call_sig);
    let call = builder.ins().call_indirect(sig_ref, code, &[env]);
    let result = builder.inst_results(call)[0];

    let cref = module.declare_func_in_func(runtime.future_complete_id, builder.func);
    builder.ins().call(cref, &[handle, result]);
    builder.ins().return_(&[]);

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    Ok(())
}

fn define_closure_thunk<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    index: usize,
    thunk: &ClosureThunk,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &StrMap,
) -> Result<()> {
    let name = format!("__closure_{index}");
    let func_id = func_ids[&name];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut emit = make_emit(module, func_ids, runtime, strings);
    let env = builder.block_params(entry)[0];
    let flags = MemFlags::trusted();
    for (i, cap) in thunk.captures.iter().enumerate() {
        let v = emit.declare_var(&mut builder, cap, types::I64);
        let off = builder.ins().iadd_imm(env, (i as i64) * 8);
        let val = builder.ins().load(types::I64, flags, off, 0);
        builder.def_var(v, val);
    }
    for (i, (pname, _)) in thunk.params.iter().enumerate() {
        let v = emit.declare_var(&mut builder, pname, types::I64);
        let val = builder.block_params(entry)[1 + i];
        builder.def_var(v, val);
    }

    let terminated = emit.emit_stmts(&mut builder, &thunk.body)?;
    if !terminated {
        if thunk.ret == Ty::Void {
            builder.ins().return_(&[]);
        } else {
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);
        }
    }

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    Ok(())
}

fn define_spawn_thunk<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    index: usize,
    thunk: &SpawnThunk,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &StrMap,
) -> Result<()> {
    let name = format!("__spawn_{index}");
    let func_id = func_ids[&name];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut emit = make_emit(module, func_ids, runtime, strings);
    let env = builder.block_params(entry)[0];
    let flags = MemFlags::trusted();
    for (i, cap) in thunk.captures.iter().enumerate() {
        let v = emit.declare_var(&mut builder, cap, types::I64);
        let off = builder.ins().iadd_imm(env, (i as i64) * 8);
        let val = builder.ins().load(types::I64, flags, off, 0);
        builder.def_var(v, val);
    }

    let terminated = emit.emit_stmts(&mut builder, &thunk.body)?;
    if !terminated {
        builder.ins().return_(&[]);
    }

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    Ok(())
}

fn define_async_block_thunk<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    index: usize,
    thunk: &AsyncBlockThunk,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &StrMap,
) -> Result<()> {
    let name = format!("__async_block_{index}");
    let func_id = func_ids[&name];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut emit = make_emit(module, func_ids, runtime, strings);
    let env = builder.block_params(entry)[0];
    let flags = MemFlags::trusted();

    let handle_var = emit.declare_var(&mut builder, "__async_handle", types::I64);
    let handle = builder.ins().load(types::I64, flags, env, 0);
    builder.def_var(handle_var, handle);
    emit.async_complete_handle = Some(handle_var);

    for (i, cap) in thunk.captures.iter().enumerate() {
        let v = emit.declare_var(&mut builder, cap, types::I64);
        let off = builder.ins().iadd_imm(env, ((i as i64) + 1) * 8);
        let val = builder.ins().load(types::I64, flags, off, 0);
        builder.def_var(v, val);
    }

    let terminated = emit.emit_stmts(&mut builder, &thunk.body)?;
    if !terminated {
        let h = builder.use_var(handle_var);
        let zero = builder.ins().iconst(types::I64, 0);
        let cref = emit
            .module
            .declare_func_in_func(emit.future_complete_id, builder.func);
        builder.ins().call(cref, &[h, zero]);
        builder.ins().return_(&[]);
    }

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    Ok(())
}

fn define_func<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    f: &CheckedFunction,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &StrMap,
) -> Result<()> {
    let func_id = func_ids[&f.name];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut emit = make_emit(module, func_ids, runtime, strings);

    for (i, (name, ty)) in f.params.iter().enumerate() {
        let v = emit.declare_var(&mut builder, name, clif_ty(ty));
        let val = builder.block_params(entry)[i];
        builder.def_var(v, val);
    }

    let terminated = emit.emit_stmts(&mut builder, &f.body)?;
    if !terminated {
        if f.name == "main" {
            let dref = emit
                .module
                .declare_func_in_func(runtime.drain_id, builder.func);
            builder.ins().call(dref, &[]);
        }
        if f.return_ty == Ty::Void {
            builder.ins().return_(&[]);
        } else {
            bail!("function '{}' may not return", f.name);
        }
    }

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    Ok(())
}

fn define_method<M: Module>(
    module: &mut M,
    ctx: &mut ClifContext,
    fb_ctx: &mut FunctionBuilderContext,
    m: &CheckedMethod,
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &StrMap,
) -> Result<()> {
    let func_id = func_ids[&m.symbol];
    ctx.clear();
    let sig = module
        .declarations()
        .get_function_decl(func_id)
        .signature
        .clone();
    ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

    let mut builder = FunctionBuilder::new(&mut ctx.func, fb_ctx);
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    builder.seal_block(entry);

    let mut emit = make_emit(module, func_ids, runtime, strings);

    let self_v = emit.declare_var(&mut builder, "self", types::I64);
    builder.def_var(self_v, builder.block_params(entry)[0]);

    for (i, (name, ty)) in m.params.iter().enumerate() {
        let v = emit.declare_var(&mut builder, name, clif_ty(ty));
        let val = builder.block_params(entry)[i + 1];
        builder.def_var(v, val);
    }

    let terminated = emit.emit_stmts(&mut builder, &m.body)?;
    if !terminated {
        if m.return_ty == Ty::Void {
            builder.ins().return_(&[]);
        } else {
            bail!("method '{}' may not return", m.symbol);
        }
    }

    builder.finalize();
    module
        .define_function(func_id, ctx)
        .map_err(|e| anyhow!("{e}"))?;
    module.clear_context(ctx);
    Ok(())
}

impl<'a, M: Module> EmitCtx<'a, M> {
    fn declare_var(
        &mut self,
        builder: &mut FunctionBuilder,
        name: &str,
        ty: types::Type,
    ) -> Variable {
        let v = Variable::from_u32(self.next_var);
        self.next_var += 1;
        builder.declare_var(v, ty);
        self.vars.insert(name.to_string(), v);
        v
    }

    /// Returns true if control never falls through (all paths terminated).
    fn emit_stmts(
        &mut self,
        builder: &mut FunctionBuilder,
        stmts: &[CheckedStmt],
    ) -> Result<bool> {
        for stmt in stmts {
            if self.emit_stmt(builder, stmt)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn emit_stmt(
        &mut self,
        builder: &mut FunctionBuilder,
        stmt: &CheckedStmt,
    ) -> Result<bool> {
        match stmt {
            CheckedStmt::VarDecl { name, ty, init } => {
                let v = self.declare_var(builder, name, clif_ty(ty));
                let (val, slen) = self.emit_expr(builder, init)?;
                builder.def_var(v, val.ok_or_else(|| anyhow!("void used as value"))?);
                if let Some(len) = slen {
                    self.str_lens.insert(name.clone(), len);
                }
                Ok(false)
            }
            CheckedStmt::Assign { target, value } => {
                let (val, slen) = self.emit_expr(builder, value)?;
                let val = val.ok_or_else(|| anyhow!("void assign"))?;
                match target {
                    CheckedAssignTarget::Local { name } => {
                        let v = *self
                            .vars
                            .get(name)
                            .ok_or_else(|| anyhow!("unknown var {name}"))?;
                        builder.def_var(v, val);
                        if let Some(len) = slen {
                            self.str_lens.insert(name.clone(), len);
                        }
                    }
                    CheckedAssignTarget::Field { object, offset, .. } => {
                        let (obj, _) = self.emit_expr(builder, object)?;
                        let obj = obj.ok_or_else(|| anyhow!("void object"))?;
                        let flags = MemFlags::trusted();
                        builder
                            .ins()
                            .store(flags, val, obj, i32::try_from(*offset).unwrap_or(0));
                    }
                    CheckedAssignTarget::Setter { object, symbol } => {
                        let (obj, _) = self.emit_expr(builder, object)?;
                        let obj = obj.ok_or_else(|| anyhow!("void object"))?;
                        let callee = *self
                            .func_ids
                            .get(symbol)
                            .ok_or_else(|| anyhow!("unknown setter {symbol}"))?;
                        let fref = self.module.declare_func_in_func(callee, builder.func);
                        builder.ins().call(fref, &[obj, val]);
                    }
                    CheckedAssignTarget::Index {
                        array,
                        index,
                        len,
                        ..
                    } => {
                        let (arr, _) = self.emit_expr(builder, array)?;
                        let (idx, _) = self.emit_expr(builder, index)?;
                        let arr = arr.unwrap();
                        let idx = idx.unwrap();
                        // bounds: if idx >= len abort via store anyway (MVP unchecked)
                        let _ = len;
                        let off = builder.ins().imul_imm(idx, 8);
                        let ptr = builder.ins().iadd(arr, off);
                        let flags = MemFlags::trusted();
                        builder.ins().store(flags, val, ptr, 0);
                    }
                }
                Ok(false)
            }
            CheckedStmt::Spawn {
                index,
                captures,
                ..
            } => {
                let name = format!("__spawn_{index}");
                let callee = *self
                    .func_ids
                    .get(&name)
                    .ok_or_else(|| anyhow!("missing spawn thunk {name}"))?;
                let fref = self.module.declare_func_in_func(callee, builder.func);
                let ptr = builder.ins().func_addr(types::I64, fref);
                let env = if captures.is_empty() {
                    builder.ins().iconst(types::I64, 0)
                } else {
                    let nbytes = builder
                        .ins()
                        .iconst(types::I64, (captures.len() as i64) * 8);
                    let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                    let call = builder.ins().call(aref, &[nbytes]);
                    let base = builder.inst_results(call)[0];
                    let flags = MemFlags::trusted();
                    for (i, cap) in captures.iter().enumerate() {
                        let v = *self
                            .vars
                            .get(cap)
                            .ok_or_else(|| anyhow!("spawn capture missing local {cap}"))?;
                        let val = builder.use_var(v);
                        let off = builder.ins().iadd_imm(base, (i as i64) * 8);
                        builder.ins().store(flags, val, off, 0);
                    }
                    base
                };
                let sref = self.module.declare_func_in_func(self.spawn_id, builder.func);
                builder.ins().call(sref, &[ptr, env]);
                Ok(false)
            }
            CheckedStmt::Drop { object, symbol } => {
                let (obj, _) = self.emit_expr(builder, object)?;
                let obj = obj.ok_or_else(|| anyhow!("void drop object"))?;
                let callee = *self
                    .func_ids
                    .get(symbol)
                    .ok_or_else(|| anyhow!("unknown drop {symbol}"))?;
                let fref = self.module.declare_func_in_func(callee, builder.func);
                builder.ins().call(fref, &[obj]);
                Ok(false)
            }
            CheckedStmt::Return { value } => {
                if let Some(hvar) = self.async_complete_handle {
                    let result = if let Some(v) = value {
                        let (val, _) = self.emit_expr(builder, v)?;
                        val.unwrap()
                    } else {
                        builder.ins().iconst(types::I64, 0)
                    };
                    let h = builder.use_var(hvar);
                    let cref = self
                        .module
                        .declare_func_in_func(self.future_complete_id, builder.func);
                    builder.ins().call(cref, &[h, result]);
                    builder.ins().return_(&[]);
                } else if let Some(v) = value {
                    let (val, _) = self.emit_expr(builder, v)?;
                    builder.ins().return_(&[val.unwrap()]);
                } else {
                    builder.ins().return_(&[]);
                }
                Ok(true)
            }
            CheckedStmt::Expr { expr } => {
                let _ = self.emit_expr(builder, expr)?;
                Ok(false)
            }
            CheckedStmt::Break => {
                let target = self
                    .loops
                    .last()
                    .ok_or_else(|| anyhow!("break without loop"))?
                    .break_block;
                builder.ins().jump(target, &[]);
                Ok(true)
            }
            CheckedStmt::Continue => {
                let target = self
                    .loops
                    .last()
                    .ok_or_else(|| anyhow!("continue without loop"))?
                    .continue_block;
                builder.ins().jump(target, &[]);
                Ok(true)
            }
            CheckedStmt::If { arms, else_block } => self.emit_if(builder, arms, else_block),
            CheckedStmt::While { cond, body } => self.emit_while(builder, cond, body),
            CheckedStmt::ForRange {
                name,
                start,
                end,
                body,
            } => self.emit_for(builder, name, start, end, body),
            CheckedStmt::ForIn {
                name,
                iter,
                elem,
                body,
            } => self.emit_for_in(builder, name, iter, elem, body),
            CheckedStmt::Match { scrutinee, arms } => self.emit_match(builder, scrutinee, arms),
        }
    }

    fn emit_for_in(
        &mut self,
        builder: &mut FunctionBuilder,
        name: &str,
        iter: &CheckedExpr,
        elem: &Ty,
        body: &[CheckedStmt],
    ) -> Result<bool> {
        let (ch, _) = self.emit_expr(builder, iter)?;
        let ch = ch.unwrap();
        let header = builder.create_block();
        let body_b = builder.create_block();
        let exit = builder.create_block();

        builder.ins().jump(header, &[]);
        builder.switch_to_block(header);

        let out_slot = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            8,
            0,
        ));
        let out_ptr = builder.ins().stack_addr(types::I64, out_slot, 0);
        let rok = self
            .module
            .declare_func_in_func(self.channel_recv_ok_id, builder.func);
        let call = builder.ins().call(rok, &[ch, out_ptr]);
        let ok = builder.inst_results(call)[0];
        builder.ins().brif(ok, body_b, &[], exit, &[]);

        builder.switch_to_block(body_b);
        builder.seal_block(body_b);
        let v = self.declare_var(builder, name, types::I64);
        let val = builder.ins().stack_load(types::I64, out_slot, 0);
        builder.def_var(v, val);
        if *elem == Ty::String {
            self.str_lens.insert(name.to_string(), -1);
        }

        self.loops.push(LoopTargets {
            continue_block: header,
            break_block: exit,
        });
        let term = self.emit_stmts(builder, body)?;
        self.loops.pop();
        if !term {
            builder.ins().jump(header, &[]);
        }
        builder.seal_block(header);

        builder.switch_to_block(exit);
        builder.seal_block(exit);
        Ok(false)
    }

    fn emit_if(
        &mut self,
        builder: &mut FunctionBuilder,
        arms: &[(CheckedExpr, Vec<CheckedStmt>)],
        else_block: &Option<Vec<CheckedStmt>>,
    ) -> Result<bool> {
        let join = builder.create_block();
        let mut all_terminated = true;

        for (i, (cond, body)) in arms.iter().enumerate() {
            let then_b = builder.create_block();
            let next_b = if i + 1 < arms.len() || else_block.is_some() {
                builder.create_block()
            } else {
                join
            };

            let (cval, _) = self.emit_expr(builder, cond)?;
            builder.ins().brif(cval.unwrap(), then_b, &[], next_b, &[]);

            builder.switch_to_block(then_b);
            builder.seal_block(then_b);
            let term = self.emit_stmts(builder, body)?;
            if !term {
                builder.ins().jump(join, &[]);
                all_terminated = false;
            }

            if next_b != join {
                builder.switch_to_block(next_b);
                builder.seal_block(next_b);
            } else {
                // no else and last arm false falls to join — already branched
            }
        }

        if let Some(eb) = else_block {
            // After last arm's false branch we should be on a next block that isn't join,
            // OR if only one arm, next was join meaning no else — but we have else so next was created.
            // Actually after the loop, if else exists, the last next_b is where we are... 
            // Wait: after processing last arm, we switched to next_b only if next_b != join.
            // And if else exists, next_b != join. So we're on next_b already after last arm.
            // But we also seal next_b inside the loop. Good - current block is next_b.
            let term = self.emit_stmts(builder, eb)?;
            if !term {
                builder.ins().jump(join, &[]);
                all_terminated = false;
            }
        } else {
            // Last arm's false goes to join; fallthrough means not all paths terminate
            all_terminated = false;
        }

        builder.switch_to_block(join);
        builder.seal_block(join);
        Ok(all_terminated && else_block.is_some())
    }

    fn emit_while(
        &mut self,
        builder: &mut FunctionBuilder,
        cond: &CheckedExpr,
        body: &[CheckedStmt],
    ) -> Result<bool> {
        let header = builder.create_block();
        let body_b = builder.create_block();
        let exit = builder.create_block();

        builder.ins().jump(header, &[]);
        builder.switch_to_block(header);
        // header sealed later after all preds

        let (cval, _) = self.emit_expr(builder, cond)?;
        builder.ins().brif(cval.unwrap(), body_b, &[], exit, &[]);

        builder.switch_to_block(body_b);
        builder.seal_block(body_b);
        self.loops.push(LoopTargets {
            continue_block: header,
            break_block: exit,
        });
        let term = self.emit_stmts(builder, body)?;
        self.loops.pop();
        if !term {
            builder.ins().jump(header, &[]);
        }
        builder.seal_block(header);

        builder.switch_to_block(exit);
        builder.seal_block(exit);
        Ok(false)
    }

    fn emit_for(
        &mut self,
        builder: &mut FunctionBuilder,
        name: &str,
        start: &CheckedExpr,
        end: &CheckedExpr,
        body: &[CheckedStmt],
    ) -> Result<bool> {
        let (sval, _) = self.emit_expr(builder, start)?;
        let (eval, _) = self.emit_expr(builder, end)?;
        let sval = sval.unwrap();
        let eval = eval.unwrap();

        let ivar = self.declare_var(builder, name, types::I64);
        builder.def_var(ivar, sval);

        let header = builder.create_block();
        let body_b = builder.create_block();
        let step = builder.create_block();
        let exit = builder.create_block();

        builder.ins().jump(header, &[]);

        builder.switch_to_block(header);
        let cur = builder.use_var(ivar);
        let cmp = self.icmp_i64(builder, IntCC::SignedLessThan, cur, eval);
        builder.ins().brif(cmp, body_b, &[], exit, &[]);

        builder.switch_to_block(body_b);
        builder.seal_block(body_b);
        self.loops.push(LoopTargets {
            continue_block: step,
            break_block: exit,
        });
        let term = self.emit_stmts(builder, body)?;
        self.loops.pop();
        if !term {
            builder.ins().jump(step, &[]);
        }

        builder.switch_to_block(step);
        builder.seal_block(step);
        let cur = builder.use_var(ivar);
        let one = builder.ins().iconst(types::I64, 1);
        let next = builder.ins().iadd(cur, one);
        builder.def_var(ivar, next);
        builder.ins().jump(header, &[]);
        builder.seal_block(header);

        builder.switch_to_block(exit);
        builder.seal_block(exit);
        Ok(false)
    }

    fn emit_match(
        &mut self,
        builder: &mut FunctionBuilder,
        scrutinee: &CheckedExpr,
        arms: &[(CheckedPattern, Vec<CheckedStmt>)],
    ) -> Result<bool> {
        let (sval, _) = self.emit_expr(builder, scrutinee)?;
        let sval = sval.unwrap();
        let join = builder.create_block();
        let mut all_term = true;

        let mut next = builder.create_block();
        builder.ins().jump(next, &[]);

        for (i, (pat, body)) in arms.iter().enumerate() {
            builder.switch_to_block(next);
            builder.seal_block(next);

            match pat {
                CheckedPattern::Wildcard => {
                    let term = self.emit_stmts(builder, body)?;
                    if !term {
                        builder.ins().jump(join, &[]);
                        all_term = false;
                    }
                }
                CheckedPattern::IntLit(n) => {
                    let lit = builder.ins().iconst(types::I64, *n);
                    let eq = self.icmp_i64(builder, IntCC::Equal, sval, lit);
                    let then_b = builder.create_block();
                    let else_b = if i + 1 < arms.len() {
                        builder.create_block()
                    } else {
                        join
                    };
                    builder.ins().brif(eq, then_b, &[], else_b, &[]);

                    builder.switch_to_block(then_b);
                    builder.seal_block(then_b);
                    let term = self.emit_stmts(builder, body)?;
                    if !term {
                        builder.ins().jump(join, &[]);
                        all_term = false;
                    }
                    next = else_b;
                }
                CheckedPattern::Ok { name }
                | CheckedPattern::Err { name }
                | CheckedPattern::Some { name } => {
                    let want_tag = match pat {
                        CheckedPattern::Ok { .. } | CheckedPattern::Some { .. } => 0i64,
                        _ => 1i64,
                    };
                    let flags = MemFlags::trusted();
                    let tag = builder.ins().load(types::I64, flags, sval, 0);
                    let want = builder.ins().iconst(types::I64, want_tag);
                    let eq = self.icmp_i64(builder, IntCC::Equal, tag, want);
                    let then_b = builder.create_block();
                    let else_b = if i + 1 < arms.len() {
                        builder.create_block()
                    } else {
                        join
                    };
                    builder.ins().brif(eq, then_b, &[], else_b, &[]);

                    builder.switch_to_block(then_b);
                    builder.seal_block(then_b);
                    let payload = builder.ins().load(types::I64, flags, sval, 8);
                    let v = self.declare_var(builder, name, types::I64);
                    builder.def_var(v, payload);
                    if matches!(pat, CheckedPattern::Err { .. }) {
                        self.str_lens.insert(name.clone(), -1);
                    }
                    let term = self.emit_stmts(builder, body)?;
                    if !term {
                        builder.ins().jump(join, &[]);
                        all_term = false;
                    }
                    next = else_b;
                }
                CheckedPattern::None => {
                    let flags = MemFlags::trusted();
                    let tag = builder.ins().load(types::I64, flags, sval, 0);
                    let want = builder.ins().iconst(types::I64, 1);
                    let eq = self.icmp_i64(builder, IntCC::Equal, tag, want);
                    let then_b = builder.create_block();
                    let else_b = if i + 1 < arms.len() {
                        builder.create_block()
                    } else {
                        join
                    };
                    builder.ins().brif(eq, then_b, &[], else_b, &[]);

                    builder.switch_to_block(then_b);
                    builder.seal_block(then_b);
                    let term = self.emit_stmts(builder, body)?;
                    if !term {
                        builder.ins().jump(join, &[]);
                        all_term = false;
                    }
                    next = else_b;
                }
            }
        }

        builder.switch_to_block(join);
        builder.seal_block(join);
        Ok(all_term)
    }

    fn emit_expr(
        &mut self,
        builder: &mut FunctionBuilder,
        expr: &CheckedExpr,
    ) -> Result<(Option<Value>, Option<i64>)> {
        match expr {
            CheckedExpr::IntLit(n) => Ok((Some(builder.ins().iconst(types::I64, *n)), None)),
            // Floats travel through the i64 ABI as raw IEEE-754 bits.
            CheckedExpr::FloatLit(f) => Ok((
                Some(builder.ins().iconst(types::I64, f.to_bits() as i64)),
                None,
            )),
            CheckedExpr::BoolLit(b) => {
                Ok((Some(builder.ins().iconst(types::I64, if *b { 1 } else { 0 })), None))
            }
            CheckedExpr::StringLit(s) => {
                let bytes = s.as_bytes();
                let (id, len) = self
                    .strings
                    .get(bytes)
                    .copied()
                    .ok_or_else(|| anyhow!("missing string data"))?;
                let gv = self.module.declare_data_in_func(id, builder.func);
                let ptr = builder.ins().global_value(types::I64, gv);
                Ok((Some(ptr), Some(len as i64)))
            }
            CheckedExpr::Local { name, ty } => {
                let v = *self
                    .vars
                    .get(name)
                    .ok_or_else(|| anyhow!("unknown local {name}"))?;
                let val = builder.use_var(v);
                let len = match ty {
                    Ty::String => Some(self.str_lens.get(name).copied().unwrap_or(-1)),
                    _ => None,
                };
                Ok((Some(val), len))
            }
            CheckedExpr::SelfExpr { .. } => {
                let v = *self
                    .vars
                    .get("self")
                    .ok_or_else(|| anyhow!("missing self"))?;
                Ok((Some(builder.use_var(v)), None))
            }
            CheckedExpr::New {
                size,
                ctor_symbol,
                args,
                ..
            } => {
                let size_v = builder.ins().iconst(types::I64, *size);
                let alloc_ref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                let alloc_call = builder.ins().call(alloc_ref, &[size_v]);
                let ptr = builder.inst_results(alloc_call)[0];

                let ctor = *self
                    .func_ids
                    .get(ctor_symbol)
                    .ok_or_else(|| anyhow!("unknown ctor {ctor_symbol}"))?;
                let mut arg_vals = vec![ptr];
                for a in args {
                    let (v, _) = self.emit_expr(builder, a)?;
                    arg_vals.push(v.unwrap());
                }
                let fref = self.module.declare_func_in_func(ctor, builder.func);
                let call = builder.ins().call(fref, &arg_vals);
                let results = builder.inst_results(call);
                if results.is_empty() {
                    Ok((Some(ptr), None))
                } else {
                    Ok((Some(results[0]), None))
                }
            }
            CheckedExpr::FieldGet { object, offset, ty } => {
                let (obj, _) = self.emit_expr(builder, object)?;
                let obj = obj.ok_or_else(|| anyhow!("void object"))?;
                let flags = MemFlags::trusted();
                let val = builder.ins().load(
                    clif_ty(ty),
                    flags,
                    obj,
                    i32::try_from(*offset).unwrap_or(0),
                );
                let len = if *ty == Ty::String { Some(-1) } else { None };
                Ok((Some(val), len))
            }
            CheckedExpr::MethodCall {
                object,
                symbol,
                args,
                ret,
            } => {
                let (obj, _) = self.emit_expr(builder, object)?;
                let obj = obj.ok_or_else(|| anyhow!("void object"))?;
                let callee = *self
                    .func_ids
                    .get(symbol)
                    .ok_or_else(|| anyhow!("unknown method {symbol}"))?;
                let mut arg_vals = vec![obj];
                for a in args {
                    let (v, _) = self.emit_expr(builder, a)?;
                    arg_vals.push(v.unwrap());
                }
                let fref = self.module.declare_func_in_func(callee, builder.func);
                let call = builder.ins().call(fref, &arg_vals);
                let results = builder.inst_results(call);
                if results.is_empty() {
                    Ok((None, None))
                } else {
                    let len = if *ret == Ty::String { Some(-1) } else { None };
                    Ok((Some(results[0]), len))
                }
            }
            CheckedExpr::Unary { op, expr, ty } => {
                let (v, _) = self.emit_expr(builder, expr)?;
                let v = v.unwrap();
                let val = match op {
                    UnOp::Neg if *ty == Ty::Float => {
                        let f = Self::bits_to_f64(builder, v);
                        let neg = builder.ins().fneg(f);
                        Self::f64_to_bits(builder, neg)
                    }
                    UnOp::Neg => {
                        let z = builder.ins().iconst(types::I64, 0);
                        builder.ins().isub(z, v)
                    }
                    UnOp::Not => {
                        let z = builder.ins().iconst(types::I64, 0);
                        self.icmp_i64(builder, IntCC::Equal, v, z)
                    }
                };
                Ok((Some(val), None))
            }
            CheckedExpr::Binary {
                op,
                left,
                right,
                operand_ty,
                ..
            } => match op {
                BinOp::And => self.emit_and(builder, left, right),
                BinOp::Or => self.emit_or(builder, left, right),
                _ => {
                    let (l, _) = self.emit_expr(builder, left)?;
                    let (r, _) = self.emit_expr(builder, right)?;
                    let l = l.unwrap();
                    let r = r.unwrap();
                    if *operand_ty == Ty::Float {
                        // The typechecker rejects mixed int/float operands, so both
                        // sides are already f64 bit patterns here.
                        let lf = Self::bits_to_f64(builder, l);
                        let rf = Self::bits_to_f64(builder, r);
                        let val = match op {
                            BinOp::Add => {
                                let s = builder.ins().fadd(lf, rf);
                                Self::f64_to_bits(builder, s)
                            }
                            BinOp::Sub => {
                                let s = builder.ins().fsub(lf, rf);
                                Self::f64_to_bits(builder, s)
                            }
                            BinOp::Mul => {
                                let s = builder.ins().fmul(lf, rf);
                                Self::f64_to_bits(builder, s)
                            }
                            BinOp::Div => {
                                let s = builder.ins().fdiv(lf, rf);
                                Self::f64_to_bits(builder, s)
                            }
                            BinOp::Eq => Self::fcmp_i64(builder, FloatCC::Equal, lf, rf),
                            BinOp::Ne => Self::fcmp_i64(builder, FloatCC::NotEqual, lf, rf),
                            BinOp::Lt => Self::fcmp_i64(builder, FloatCC::LessThan, lf, rf),
                            BinOp::Le => Self::fcmp_i64(builder, FloatCC::LessThanOrEqual, lf, rf),
                            BinOp::Gt => Self::fcmp_i64(builder, FloatCC::GreaterThan, lf, rf),
                            BinOp::Ge => {
                                Self::fcmp_i64(builder, FloatCC::GreaterThanOrEqual, lf, rf)
                            }
                            BinOp::Rem => bail!("'%' is not supported for float operands"),
                            BinOp::And | BinOp::Or => unreachable!(),
                        };
                        return Ok((Some(val), None));
                    }
                    let val = match op {
                        BinOp::Add => builder.ins().iadd(l, r),
                        BinOp::Sub => builder.ins().isub(l, r),
                        BinOp::Mul => builder.ins().imul(l, r),
                        BinOp::Div => builder.ins().sdiv(l, r),
                        BinOp::Rem => builder.ins().srem(l, r),
                        BinOp::Eq => self.icmp_i64(builder, IntCC::Equal, l, r),
                        BinOp::Ne => self.icmp_i64(builder, IntCC::NotEqual, l, r),
                        BinOp::Lt => self.icmp_i64(builder, IntCC::SignedLessThan, l, r),
                        BinOp::Le => self.icmp_i64(builder, IntCC::SignedLessThanOrEqual, l, r),
                        BinOp::Gt => self.icmp_i64(builder, IntCC::SignedGreaterThan, l, r),
                        BinOp::Ge => self.icmp_i64(builder, IntCC::SignedGreaterThanOrEqual, l, r),
                        BinOp::And | BinOp::Or => unreachable!(),
                    };
                    Ok((Some(val), None))
                }
            },
            CheckedExpr::Call {
                name,
                args,
                ret,
                async_spawn,
            } => {
                if *async_spawn {
                    let mut arg_vals = Vec::new();
                    for a in args {
                        let (v, _) = self.emit_expr(builder, a)?;
                        arg_vals.push(v.unwrap());
                    }
                    let nref = self
                        .module
                        .declare_func_in_func(self.future_new_id, builder.func);
                    let hcall = builder.ins().call(nref, &[]);
                    let handle = builder.inst_results(hcall)[0];

                    let nslots = (1 + arg_vals.len()) as i64;
                    let nbytes = builder.ins().iconst(types::I64, nslots * 8);
                    let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                    let acall = builder.ins().call(aref, &[nbytes]);
                    let env = builder.inst_results(acall)[0];
                    let flags = MemFlags::trusted();
                    builder.ins().store(flags, handle, env, 0);
                    for (i, v) in arg_vals.iter().enumerate() {
                        let off = builder.ins().iadd_imm(env, ((i as i64) + 1) * 8);
                        builder.ins().store(flags, *v, off, 0);
                    }

                    let wrap = async_wrap_name(name);
                    let wid = *self
                        .func_ids
                        .get(&wrap)
                        .ok_or_else(|| anyhow!("missing async wrap {wrap}"))?;
                    let wref = self.module.declare_func_in_func(wid, builder.func);
                    let ptr = builder.ins().func_addr(types::I64, wref);
                    let sref = self.module.declare_func_in_func(self.spawn_id, builder.func);
                    builder.ins().call(sref, &[ptr, env]);
                    Ok((Some(handle), None))
                } else {
                    let callee = *self
                        .func_ids
                        .get(name)
                        .ok_or_else(|| anyhow!("unknown fn {name}"))?;
                    let mut arg_vals = Vec::new();
                    for a in args {
                        let (v, _) = self.emit_expr(builder, a)?;
                        arg_vals.push(v.unwrap());
                    }
                    let fref = self.module.declare_func_in_func(callee, builder.func);
                    let call = builder.ins().call(fref, &arg_vals);
                    let nres = builder.inst_results(call).len();
                    if nres == 0 {
                        Ok((None, None))
                    } else {
                        let raw = builder.inst_results(call)[0];
                        let len = if *ret == Ty::String { Some(-1) } else { None };
                        Ok((Some(raw), len))
                    }
                }
            }
            CheckedExpr::ArrayLit { elems, len, .. } => {
                let size = builder.ins().iconst(types::I64, len * 8);
                let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                let call = builder.ins().call(aref, &[size]);
                let ptr = builder.inst_results(call)[0];
                let flags = MemFlags::trusted();
                for (i, e) in elems.iter().enumerate() {
                    let (v, _) = self.emit_expr(builder, e)?;
                    builder
                        .ins()
                        .store(flags, v.unwrap(), ptr, (i as i32) * 8);
                }
                Ok((Some(ptr), None))
            }
            CheckedExpr::Index {
                array,
                index,
                len,
                ..
            } => {
                let (arr, _) = self.emit_expr(builder, array)?;
                let (idx, _) = self.emit_expr(builder, index)?;
                let _ = len;
                let arr = arr.unwrap();
                let idx = idx.unwrap();
                let off = builder.ins().imul_imm(idx, 8);
                let ptr = builder.ins().iadd(arr, off);
                let flags = MemFlags::trusted();
                let val = builder.ins().load(types::I64, flags, ptr, 0);
                Ok((Some(val), None))
            }
            CheckedExpr::Await { expr, inner } => {
                let (h, _) = self.emit_expr(builder, expr)?;
                let href = self
                    .module
                    .declare_func_in_func(self.future_await_id, builder.func);
                let call = builder.ins().call(href, &[h.unwrap()]);
                let len = if *inner == Ty::String { Some(-1) } else { None };
                Ok((Some(builder.inst_results(call)[0]), len))
            }
            CheckedExpr::StdLog { args } => {
                self.emit_std_log(builder, args)?;
                Ok((None, None))
            }
            CheckedExpr::StdSleep { ms } => {
                let (v, _) = self.emit_expr(builder, ms)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.sleep_id, builder.func);
                builder.ins().call(fref, &[v.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::ChannelNew => {
                let fref = self
                    .module
                    .declare_func_in_func(self.channel_new_id, builder.func);
                let call = builder.ins().call(fref, &[]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::WaitGroupNew => {
                let fref = self
                    .module
                    .declare_func_in_func(self.waitgroup_new_id, builder.func);
                let call = builder.ins().call(fref, &[]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::ChannelSend { channel, value } => {
                let (c, _) = self.emit_expr(builder, channel)?;
                let (v, _) = self.emit_expr(builder, value)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.channel_send_id, builder.func);
                builder.ins().call(fref, &[c.unwrap(), v.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::ChannelRecv { channel } => {
                let (c, _) = self.emit_expr(builder, channel)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.channel_recv_id, builder.func);
                let call = builder.ins().call(fref, &[c.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::ChannelRecvFuture { channel } => {
                let (c, _) = self.emit_expr(builder, channel)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.channel_recv_future_id, builder.func);
                let call = builder.ins().call(fref, &[c.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::CpuSubmitNamed { fn_name } => {
                let nref = self
                    .module
                    .declare_func_in_func(self.future_new_id, builder.func);
                let hcall = builder.ins().call(nref, &[]);
                let handle = builder.inst_results(hcall)[0];

                let nbytes = builder.ins().iconst(types::I64, 8);
                let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                let acall = builder.ins().call(aref, &[nbytes]);
                let env = builder.inst_results(acall)[0];
                let flags = MemFlags::trusted();
                builder.ins().store(flags, handle, env, 0);

                let wrap = cpu_submit_wrap_name(fn_name);
                let wid = *self
                    .func_ids
                    .get(&wrap)
                    .ok_or_else(|| anyhow!("missing cpu submit wrap {wrap}"))?;
                let wref = self.module.declare_func_in_func(wid, builder.func);
                let ptr = builder.ins().func_addr(types::I64, wref);
                let sref = self.module.declare_func_in_func(self.spawn_id, builder.func);
                builder.ins().call(sref, &[ptr, env]);
                Ok((Some(handle), None))
            }
            CheckedExpr::CpuSubmitClosure { closure } => {
                let nref = self
                    .module
                    .declare_func_in_func(self.future_new_id, builder.func);
                let hcall = builder.ins().call(nref, &[]);
                let handle = builder.inst_results(hcall)[0];
                let (fat, _) = self.emit_expr(builder, closure)?;
                let fat = fat.ok_or_else(|| anyhow!("cpu submit closure produced no value"))?;

                let nbytes = builder.ins().iconst(types::I64, 16);
                let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                let acall = builder.ins().call(aref, &[nbytes]);
                let env = builder.inst_results(acall)[0];
                let flags = MemFlags::trusted();
                builder.ins().store(flags, handle, env, 0);
                builder.ins().store(flags, fat, env, 8);

                let wid = *self
                    .func_ids
                    .get("__cpu_submit_closure")
                    .ok_or_else(|| anyhow!("missing __cpu_submit_closure"))?;
                let wref = self.module.declare_func_in_func(wid, builder.func);
                let ptr = builder.ins().func_addr(types::I64, wref);
                let sref = self.module.declare_func_in_func(self.spawn_id, builder.func);
                builder.ins().call(sref, &[ptr, env]);
                Ok((Some(handle), None))
            }
            CheckedExpr::Closure {
                index,
                captures,
                ..
            } => {
                let wrap = format!("__closure_{index}");
                let wid = *self
                    .func_ids
                    .get(&wrap)
                    .ok_or_else(|| anyhow!("missing closure thunk {wrap}"))?;
                let wref = self.module.declare_func_in_func(wid, builder.func);
                let code = builder.ins().func_addr(types::I64, wref);

                let flags = MemFlags::trusted();
                let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                let env = if captures.is_empty() {
                    builder.ins().iconst(types::I64, 0)
                } else {
                    let nbytes = builder
                        .ins()
                        .iconst(types::I64, (captures.len() as i64) * 8);
                    let acall = builder.ins().call(aref, &[nbytes]);
                    let env = builder.inst_results(acall)[0];
                    for (i, cap) in captures.iter().enumerate() {
                        let var = self
                            .vars
                            .get(cap)
                            .ok_or_else(|| anyhow!("closure capture missing local {cap}"))?;
                        let val = builder.use_var(*var);
                        let off = builder.ins().iadd_imm(env, (i as i64) * 8);
                        builder.ins().store(flags, val, off, 0);
                    }
                    env
                };

                let fat_bytes = builder.ins().iconst(types::I64, 16);
                let fcall = builder.ins().call(aref, &[fat_bytes]);
                let fat = builder.inst_results(fcall)[0];
                builder.ins().store(flags, code, fat, 0);
                builder.ins().store(flags, env, fat, 8);
                Ok((Some(fat), None))
            }
            CheckedExpr::CallClosure { callee, args, ret } => {
                let (fat, _) = self.emit_expr(builder, callee)?;
                let fat = fat.ok_or_else(|| anyhow!("closure call has no callee value"))?;
                let flags = MemFlags::trusted();
                let code = builder.ins().load(types::I64, flags, fat, 0);
                let env = builder.ins().load(types::I64, flags, fat, 8);

                let mut call_args = vec![env];
                for a in args {
                    let (v, _) = self.emit_expr(builder, a)?;
                    call_args.push(v.ok_or_else(|| anyhow!("missing closure argument"))?);
                }

                let mut call_sig = Signature::new(CallConv::SystemV);
                call_sig.params.push(AbiParam::new(types::I64));
                for _ in args {
                    call_sig.params.push(AbiParam::new(types::I64));
                }
                if *ret != Ty::Void {
                    call_sig.returns.push(AbiParam::new(types::I64));
                }
                let sig_ref = builder.import_signature(call_sig);
                let call = builder.ins().call_indirect(sig_ref, code, &call_args);
                if *ret == Ty::Void {
                    Ok((None, None))
                } else {
                    Ok((Some(builder.inst_results(call)[0]), None))
                }
            }
            CheckedExpr::ResultOk { value } => {
                self.emit_tagged(builder, 0, value)
            }
            CheckedExpr::ResultErr { value } => {
                self.emit_tagged(builder, 1, value)
            }
            CheckedExpr::OptionSome { value } => {
                self.emit_tagged(builder, 0, value)
            }
            CheckedExpr::OptionNone => {
                let nbytes = builder.ins().iconst(types::I64, 16);
                let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                let acall = builder.ins().call(aref, &[nbytes]);
                let ptr = builder.inst_results(acall)[0];
                let flags = MemFlags::trusted();
                let tag = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                builder.ins().store(flags, tag, ptr, 0);
                builder.ins().store(flags, zero, ptr, 8);
                Ok((Some(ptr), None))
            }
            CheckedExpr::ChannelClose { channel } => {
                let (c, _) = self.emit_expr(builder, channel)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.channel_close_id, builder.func);
                builder.ins().call(fref, &[c.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::WaitGroupAdd { wg, delta } => {
                let (w, _) = self.emit_expr(builder, wg)?;
                let (d, _) = self.emit_expr(builder, delta)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.waitgroup_add_id, builder.func);
                builder.ins().call(fref, &[w.unwrap(), d.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::WaitGroupDone { wg } => {
                let (w, _) = self.emit_expr(builder, wg)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.waitgroup_done_id, builder.func);
                builder.ins().call(fref, &[w.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::WaitGroupWait { wg } => {
                let (w, _) = self.emit_expr(builder, wg)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.waitgroup_wait_id, builder.func);
                builder.ins().call(fref, &[w.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::WaitGroupWaitFuture { wg } => {
                let (w, _) = self.emit_expr(builder, wg)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.waitgroup_wait_future_id, builder.func);
                let call = builder.ins().call(fref, &[w.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::FutureJoin { left, right } => {
                let (l, _) = self.emit_expr(builder, left)?;
                let (r, _) = self.emit_expr(builder, right)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.future_join_id, builder.func);
                let call = builder.ins().call(fref, &[l.unwrap(), r.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::FutureRace { left, right } => {
                let (l, _) = self.emit_expr(builder, left)?;
                let (r, _) = self.emit_expr(builder, right)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.future_race_id, builder.func);
                let call = builder.ins().call(fref, &[l.unwrap(), r.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::FutureReady { value } => {
                let (v, _) = self.emit_expr(builder, value)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.future_ready_id, builder.func);
                let call = builder.ins().call(fref, &[v.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::ChannelBuffered { capacity } => {
                let (n, _) = self.emit_expr(builder, capacity)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.channel_buffered_id, builder.func);
                let call = builder.ins().call(fref, &[n.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::MutexNew { initial } => {
                let (v, _) = self.emit_expr(builder, initial)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.mutex_new_id, builder.func);
                let call = builder.ins().call(fref, &[v.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::MutexLock { mutex } => {
                let (m, _) = self.emit_expr(builder, mutex)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.mutex_lock_id, builder.func);
                builder.ins().call(fref, &[m.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::MutexUnlock { mutex } => {
                let (m, _) = self.emit_expr(builder, mutex)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.mutex_unlock_id, builder.func);
                builder.ins().call(fref, &[m.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::MutexGet { mutex } => {
                let (m, _) = self.emit_expr(builder, mutex)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.mutex_get_id, builder.func);
                let call = builder.ins().call(fref, &[m.unwrap()]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::MutexSet { mutex, value } => {
                let (m, _) = self.emit_expr(builder, mutex)?;
                let (v, _) = self.emit_expr(builder, value)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.mutex_set_id, builder.func);
                builder.ins().call(fref, &[m.unwrap(), v.unwrap()]);
                Ok((None, None))
            }
            CheckedExpr::RwLockNew { initial } => {
                self.emit_runtime_call(builder, self.rwlock_new_id, &[initial], None)
            }
            CheckedExpr::RwLockReadLock { lock } => {
                self.emit_runtime_call(builder, self.rwlock_read_lock_id, &[lock], None)
            }
            CheckedExpr::RwLockReadUnlock { lock } => {
                self.emit_runtime_call(builder, self.rwlock_read_unlock_id, &[lock], None)
            }
            CheckedExpr::RwLockWriteLock { lock } => {
                self.emit_runtime_call(builder, self.rwlock_write_lock_id, &[lock], None)
            }
            CheckedExpr::RwLockWriteUnlock { lock } => {
                self.emit_runtime_call(builder, self.rwlock_write_unlock_id, &[lock], None)
            }
            CheckedExpr::RwLockGet { lock } => {
                self.emit_runtime_call(builder, self.rwlock_get_id, &[lock], None)
            }
            CheckedExpr::RwLockSet { lock, value } => {
                self.emit_runtime_call(builder, self.rwlock_set_id, &[lock, value], None)
            }
            CheckedExpr::ParallelMap { list, fn_name } => {
                let (l, _) = self.emit_expr(builder, list)?;
                let fid = *self
                    .func_ids
                    .get(fn_name)
                    .ok_or_else(|| anyhow!("missing parallel map fn {fn_name}"))?;
                let fref = self.module.declare_func_in_func(fid, builder.func);
                let ptr = builder.ins().func_addr(types::I64, fref);
                let rt = self
                    .module
                    .declare_func_in_func(self.parallel_map_int_id, builder.func);
                let call = builder.ins().call(rt, &[l.unwrap(), ptr]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::HttpGet { url } => {
                self.emit_runtime_call(builder, self.http_get_id, &[url], None)
            }
            CheckedExpr::TaskYield => {
                let fref = self
                    .module
                    .declare_func_in_func(self.task_yield_id, builder.func);
                builder.ins().call(fref, &[]);
                Ok((None, None))
            }
            CheckedExpr::CancelTokenNew => {
                let fref = self
                    .module
                    .declare_func_in_func(self.cancel_token_new_id, builder.func);
                let call = builder.ins().call(fref, &[]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::CancelTokenCancel { token } => {
                self.emit_runtime_call(builder, self.cancel_token_cancel_id, &[token], None)
            }
            CheckedExpr::CancelTokenIsCancelled { token } => {
                self.emit_runtime_call(builder, self.cancel_token_is_cancelled_id, &[token], None)
            }
            CheckedExpr::ListNew => {
                let fref = self
                    .module
                    .declare_func_in_func(self.list_new_id, builder.func);
                let call = builder.ins().call(fref, &[]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::ListPush { list, value } => {
                self.emit_runtime_call(builder, self.list_push_id, &[list, value], None)
            }
            CheckedExpr::ListGet { list, index, elem } => {
                let slen = if *elem == Ty::String { Some(-1) } else { None };
                self.emit_runtime_call(builder, self.list_get_id, &[list, index], slen)
            }
            CheckedExpr::ListSet { list, index, value } => {
                self.emit_runtime_call(builder, self.list_set_id, &[list, index, value], None)
            }
            CheckedExpr::ListLen { list } => {
                self.emit_runtime_call(builder, self.list_len_id, &[list], None)
            }
            CheckedExpr::StdPanic { msg } => {
                self.emit_runtime_call(builder, self.panic_id, &[msg], None)
            }
            CheckedExpr::StdProcessExit { code } => {
                self.emit_runtime_call(builder, self.process_exit_id, &[code], None)
            }
            CheckedExpr::StdTimeNowMs => {
                let fref = self
                    .module
                    .declare_func_in_func(self.time_now_ms_id, builder.func);
                let call = builder.ins().call(fref, &[]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::StdEnvArgs => {
                let fref = self
                    .module
                    .declare_func_in_func(self.env_args_id, builder.func);
                let call = builder.ins().call(fref, &[]);
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::StdEnvGet { name } => {
                self.emit_runtime_call(builder, self.env_get_id, &[name], None)
            }
            CheckedExpr::StdEnvSet { name, value } => {
                self.emit_runtime_call(builder, self.env_set_id, &[name, value], None)
            }
            CheckedExpr::StdFsReadToString { path } => {
                self.emit_runtime_call(builder, self.fs_read_to_string_id, &[path], None)
            }
            CheckedExpr::StdFsWriteString { path, contents } => {
                self.emit_runtime_call(builder, self.fs_write_string_id, &[path, contents], None)
            }
            CheckedExpr::StdStringLen { s } => {
                self.emit_runtime_call(builder, self.string_len_id, &[s], None)
            }
            CheckedExpr::StdStringConcat { a, b } => {
                self.emit_runtime_call(builder, self.string_concat_id, &[a, b], Some(-1))
            }
            CheckedExpr::StdStringSlice { s, start, end } => {
                self.emit_runtime_call(builder, self.string_slice_id, &[s, start, end], Some(-1))
            }
            CheckedExpr::StdStringContains { hay, needle } => {
                self.emit_runtime_call(builder, self.string_contains_id, &[hay, needle], None)
            }
            CheckedExpr::StdStringFromInt { n } => {
                self.emit_runtime_call(builder, self.string_from_int_id, &[n], Some(-1))
            }
            CheckedExpr::StdStringParseInt { s } => {
                self.emit_runtime_call(builder, self.string_parse_int_id, &[s], None)
            }
            CheckedExpr::SerdeEncode {
                format,
                value,
                schema,
            } => {
                use stk_ast::SerdeFormat;
                let fmt = match format {
                    SerdeFormat::Json => 0,
                    SerdeFormat::Yaml => 1,
                    SerdeFormat::Toml => 2,
                    SerdeFormat::Toon => 3,
                };
                let fmt_v = builder.ins().iconst(types::I64, fmt);
                let (schema_ptr, _) = self.emit_expr(
                    builder,
                    &CheckedExpr::StringLit(schema.clone()),
                )?;
                let (val, _) = self.emit_expr(builder, value)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.serde_encode_id, builder.func);
                let call = builder.ins().call(
                    fref,
                    &[fmt_v, schema_ptr.unwrap(), val.unwrap()],
                );
                Ok((Some(builder.inst_results(call)[0]), Some(-1)))
            }
            CheckedExpr::SerdeDecode {
                format,
                text,
                schema,
                ..
            } => {
                use stk_ast::SerdeFormat;
                let fmt = match format {
                    SerdeFormat::Json => 0,
                    SerdeFormat::Yaml => 1,
                    SerdeFormat::Toml => 2,
                    SerdeFormat::Toon => 3,
                };
                let fmt_v = builder.ins().iconst(types::I64, fmt);
                let (schema_ptr, _) = self.emit_expr(
                    builder,
                    &CheckedExpr::StringLit(schema.clone()),
                )?;
                let (txt, _) = self.emit_expr(builder, text)?;
                let fref = self
                    .module
                    .declare_func_in_func(self.serde_decode_id, builder.func);
                let call = builder.ins().call(
                    fref,
                    &[fmt_v, schema_ptr.unwrap(), txt.unwrap()],
                );
                Ok((Some(builder.inst_results(call)[0]), None))
            }
            CheckedExpr::AsyncBlock {
                index,
                captures,
                ..
            } => {
                let nref = self
                    .module
                    .declare_func_in_func(self.future_new_id, builder.func);
                let hcall = builder.ins().call(nref, &[]);
                let handle = builder.inst_results(hcall)[0];

                let nslots = (1 + captures.len()) as i64;
                let nbytes = builder.ins().iconst(types::I64, nslots * 8);
                let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
                let acall = builder.ins().call(aref, &[nbytes]);
                let env = builder.inst_results(acall)[0];
                let flags = MemFlags::trusted();
                builder.ins().store(flags, handle, env, 0);
                for (i, cap) in captures.iter().enumerate() {
                    let var = self
                        .vars
                        .get(cap)
                        .ok_or_else(|| anyhow!("async block capture missing local {cap}"))?;
                    let val = builder.use_var(*var);
                    let off = builder.ins().iadd_imm(env, ((i as i64) + 1) * 8);
                    builder.ins().store(flags, val, off, 0);
                }

                let wrap = format!("__async_block_{index}");
                let wid = *self
                    .func_ids
                    .get(&wrap)
                    .ok_or_else(|| anyhow!("missing async block wrap {wrap}"))?;
                let wref = self.module.declare_func_in_func(wid, builder.func);
                let ptr = builder.ins().func_addr(types::I64, wref);
                let sref = self.module.declare_func_in_func(self.spawn_id, builder.func);
                builder.ins().call(sref, &[ptr, env]);
                Ok((Some(handle), None))
            }
        }
    }

    fn icmp_i64(
        &self,
        builder: &mut FunctionBuilder,
        cc: IntCC,
        a: Value,
        b: Value,
    ) -> Value {
        let cmp = builder.ins().icmp(cc, a, b);
        builder.ins().uextend(types::I64, cmp)
    }

    /// Evaluate `args` left-to-right and call a runtime helper with the i64 ABI.
    /// `str_len` is the string-length metadata of the result (`None` = not a string).
    fn emit_runtime_call(
        &mut self,
        builder: &mut FunctionBuilder,
        callee: FuncId,
        args: &[&CheckedExpr],
        str_len: Option<i64>,
    ) -> Result<(Option<Value>, Option<i64>)> {
        let mut vals = Vec::with_capacity(args.len());
        for a in args {
            let (v, _) = self.emit_expr(builder, a)?;
            vals.push(v.ok_or_else(|| anyhow!("void used as runtime argument"))?);
        }
        let fref = self.module.declare_func_in_func(callee, builder.func);
        let call = builder.ins().call(fref, &vals);
        if builder.inst_results(call).is_empty() {
            Ok((None, None))
        } else {
            Ok((Some(builder.inst_results(call)[0]), str_len))
        }
    }

    fn bits_to_f64(builder: &mut FunctionBuilder, v: Value) -> Value {
        builder.ins().bitcast(types::F64, MemFlags::new(), v)
    }

    fn f64_to_bits(builder: &mut FunctionBuilder, v: Value) -> Value {
        builder.ins().bitcast(types::I64, MemFlags::new(), v)
    }

    fn fcmp_i64(builder: &mut FunctionBuilder, cc: FloatCC, a: Value, b: Value) -> Value {
        let cmp = builder.ins().fcmp(cc, a, b);
        builder.ins().uextend(types::I64, cmp)
    }

    fn emit_and(
        &mut self,
        builder: &mut FunctionBuilder,
        left: &CheckedExpr,
        right: &CheckedExpr,
    ) -> Result<(Option<Value>, Option<i64>)> {
        let (l, _) = self.emit_expr(builder, left)?;
        let l = l.unwrap();
        let rhs_b = builder.create_block();
        let short_b = builder.create_block();
        let join = builder.create_block();
        builder.append_block_param(join, types::I64);

        builder.ins().brif(l, rhs_b, &[], short_b, &[]);

        builder.switch_to_block(short_b);
        builder.seal_block(short_b);
        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().jump(join, &[zero]);

        builder.switch_to_block(rhs_b);
        builder.seal_block(rhs_b);
        let (r, _) = self.emit_expr(builder, right)?;
        builder.ins().jump(join, &[r.unwrap()]);

        builder.switch_to_block(join);
        builder.seal_block(join);
        Ok((Some(builder.block_params(join)[0]), None))
    }

    fn emit_or(
        &mut self,
        builder: &mut FunctionBuilder,
        left: &CheckedExpr,
        right: &CheckedExpr,
    ) -> Result<(Option<Value>, Option<i64>)> {
        let (l, _) = self.emit_expr(builder, left)?;
        let l = l.unwrap();
        let rhs_b = builder.create_block();
        let short_b = builder.create_block();
        let join = builder.create_block();
        builder.append_block_param(join, types::I64);

        builder.ins().brif(l, short_b, &[], rhs_b, &[]);

        builder.switch_to_block(short_b);
        builder.seal_block(short_b);
        let one = builder.ins().iconst(types::I64, 1);
        builder.ins().jump(join, &[one]);

        builder.switch_to_block(rhs_b);
        builder.seal_block(rhs_b);
        let (r, _) = self.emit_expr(builder, right)?;
        builder.ins().jump(join, &[r.unwrap()]);

        builder.switch_to_block(join);
        builder.seal_block(join);
        Ok((Some(builder.block_params(join)[0]), None))
    }

    fn emit_tagged(
        &mut self,
        builder: &mut FunctionBuilder,
        tag: i64,
        value: &CheckedExpr,
    ) -> Result<(Option<Value>, Option<i64>)> {
        let (v, _) = self.emit_expr(builder, value)?;
        let nbytes = builder.ins().iconst(types::I64, 16);
        let aref = self.module.declare_func_in_func(self.alloc_id, builder.func);
        let acall = builder.ins().call(aref, &[nbytes]);
        let ptr = builder.inst_results(acall)[0];
        let flags = MemFlags::trusted();
        let t = builder.ins().iconst(types::I64, tag);
        builder.ins().store(flags, t, ptr, 0);
        builder.ins().store(flags, v.unwrap(), ptr, 8);
        Ok((Some(ptr), None))
    }

    fn emit_std_log(
        &mut self,
        builder: &mut FunctionBuilder,
        args: &[CheckedExpr],
    ) -> Result<()> {
        let (fmt_ptr, fmt_len) = self.emit_expr(builder, &args[0])?;
        let fmt_ptr = fmt_ptr.unwrap();
        let fmt_len = builder.ins().iconst(types::I64, fmt_len.unwrap_or(0));
        let n = (args.len() - 1) as i64;
        let n_val = builder.ins().iconst(types::I64, n);

        let max = 8i32;
        let slot_vals = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (max as u32) * 8,
            0,
        ));
        let slot_lens = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (max as u32) * 8,
            0,
        ));
        let slot_kinds = builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
            cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
            (max as u32) * 8,
            0,
        ));

        for (i, a) in args.iter().skip(1).enumerate() {
            let is_float = expr_is_float(a);
            let (val, slen) = self.emit_expr(builder, a)?;
            let val = val.unwrap();
            let offset = (i as i32) * 8;
            builder.ins().stack_store(val, slot_vals, offset);
            // kinds: 0 = int, 1 = string, 2 = float bits
            let (kind, lenv) = if is_float {
                (
                    builder.ins().iconst(types::I64, 2),
                    builder.ins().iconst(types::I64, 0),
                )
            } else if let Some(len) = slen {
                (
                    builder.ins().iconst(types::I64, 1),
                    builder.ins().iconst(types::I64, len),
                )
            } else {
                (
                    builder.ins().iconst(types::I64, 0),
                    builder.ins().iconst(types::I64, 0),
                )
            };
            builder.ins().stack_store(lenv, slot_lens, offset);
            builder.ins().stack_store(kind, slot_kinds, offset);
        }

        let vals_ptr = builder.ins().stack_addr(types::I64, slot_vals, 0);
        let lens_ptr = builder.ins().stack_addr(types::I64, slot_lens, 0);
        let kinds_ptr = builder.ins().stack_addr(types::I64, slot_kinds, 0);
        let fref = self.module.declare_func_in_func(self.log_id, builder.func);
        builder.ins().call(
            fref,
            &[fmt_ptr, fmt_len, vals_ptr, lens_ptr, kinds_ptr, n_val],
        );
        Ok(())
    }
}
