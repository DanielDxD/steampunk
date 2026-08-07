use std::cell::Cell;
use std::collections::{HashMap, HashSet};

use stk_ast::*;
use stk_span::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Float,
    String,
    Bool,
    Void,
    Class(String),
    /// Interface type (`iclass`); values must be implementing classes.
    Interface(String),
    Array {
        elem: Box<Ty>,
        len: i64,
    },
    Future(Box<Ty>),
    Channel {
        elem: Box<Ty>,
    },
    WaitGroup,
    Mutex {
        elem: Box<Ty>,
    },
    RwLock {
        elem: Box<Ty>,
    },
    /// Opaque cancel token handle
    CancelToken,
    Result {
        ok: Box<Ty>,
        err: Box<Ty>,
    },
    Option {
        inner: Box<Ty>,
    },
    /// Dynamic list of any value type
    List {
        elem: Box<Ty>,
    },
    /// Closure / function value
    Fn {
        params: Vec<Ty>,
        ret: Box<Ty>,
    },
    HttpRequest,
    HttpResponse,
    HttpHeaders,
    HttpServer,
}

/// Named types resolved in O(1) average via hash sets.
#[derive(Clone, Copy)]
struct TypeNames<'a> {
    classes: &'a HashSet<String>,
    iclasses: &'a HashSet<String>,
}

/// Storable value type (not `void`).
#[inline]
fn is_value_ty(t: &Ty) -> bool {
    !matches!(t, Ty::Void)
}

impl Ty {
    fn from_ast_at(t: &TypeName, names: TypeNames<'_>, span: Span) -> Result<Self, Diagnostic> {
        match t {
            TypeName::Int => Ok(Ty::Int),
            TypeName::Float => Ok(Ty::Float),
            TypeName::String => Ok(Ty::String),
            TypeName::Bool => Ok(Ty::Bool),
            TypeName::Void => Ok(Ty::Void),
            TypeName::Class(name) => {
                if names.classes.contains(name) {
                    Ok(Ty::Class(name.clone()))
                } else if names.iclasses.contains(name) {
                    Ok(Ty::Interface(name.clone()))
                } else {
                    Err(Diagnostic::new(format!("unknown type '{name}'"), span))
                }
            }
            TypeName::Array { elem, len } => {
                let elem = Ty::from_ast_at(elem, names, span)?;
                if !is_value_ty(&elem) {
                    return Err(Diagnostic::new(
                        "array element type cannot be void",
                        span,
                    ));
                }
                if *len < 0 {
                    return Err(Diagnostic::new("array length must be >= 0", span));
                }
                Ok(Ty::Array {
                    elem: Box::new(elem),
                    len: *len,
                })
            }
            TypeName::Future(inner) => Ok(Ty::Future(Box::new(Ty::from_ast_at(
                inner, names, span,
            )?))),
            TypeName::Channel(elem) => {
                let elem = Ty::from_ast_at(elem, names, span)?;
                if !is_value_ty(&elem) {
                    return Err(Diagnostic::new(
                        "Channel element type cannot be void",
                        span,
                    ));
                }
                Ok(Ty::Channel {
                    elem: Box::new(elem),
                })
            }
            TypeName::WaitGroup => Ok(Ty::WaitGroup),
            TypeName::Mutex(elem) => {
                let elem = Ty::from_ast_at(elem, names, span)?;
                if !is_value_ty(&elem) {
                    return Err(Diagnostic::new(
                        "Mutex element type cannot be void",
                        span,
                    ));
                }
                Ok(Ty::Mutex {
                    elem: Box::new(elem),
                })
            }
            TypeName::RwLock(elem) => {
                let elem = Ty::from_ast_at(elem, names, span)?;
                if !is_value_ty(&elem) {
                    return Err(Diagnostic::new(
                        "RwLock element type cannot be void",
                        span,
                    ));
                }
                Ok(Ty::RwLock {
                    elem: Box::new(elem),
                })
            }
            TypeName::Result { ok, err } => {
                let ok = Ty::from_ast_at(ok, names, span)?;
                let err = Ty::from_ast_at(err, names, span)?;
                if !is_value_ty(&ok) || !is_value_ty(&err) {
                    return Err(Diagnostic::new(
                        "Result payloads cannot be void",
                        span,
                    ));
                }
                Ok(Ty::Result {
                    ok: Box::new(ok),
                    err: Box::new(err),
                })
            }
            TypeName::Option(inner) => {
                let inner = Ty::from_ast_at(inner, names, span)?;
                if !is_value_ty(&inner) {
                    return Err(Diagnostic::new(
                        "Option payload cannot be void",
                        span,
                    ));
                }
                Ok(Ty::Option {
                    inner: Box::new(inner),
                })
            }
            TypeName::List(elem) => {
                let elem = Ty::from_ast_at(elem, names, span)?;
                if !is_value_ty(&elem) {
                    return Err(Diagnostic::new(
                        "List element type cannot be void",
                        span,
                    ));
                }
                Ok(Ty::List {
                    elem: Box::new(elem),
                })
            }
            TypeName::HttpRequest => Ok(Ty::HttpRequest),
            TypeName::HttpResponse => Ok(Ty::HttpResponse),
            TypeName::HttpHeaders => Ok(Ty::HttpHeaders),
            TypeName::HttpServer => Ok(Ty::HttpServer),
        }
    }

    /// Stable short name for monomorphization mangling.
    fn mangle_name(&self) -> String {
        match self {
            Ty::Int => "int".into(),
            Ty::Float => "float".into(),
            Ty::String => "string".into(),
            Ty::Bool => "bool".into(),
            Ty::Void => "void".into(),
            Ty::Class(n) | Ty::Interface(n) => n.clone(),
            Ty::Array { elem, len } => format!("arr_{}_{}", elem.mangle_name(), len),
            Ty::Future(t) => format!("fut_{}", t.mangle_name()),
            Ty::Channel { elem } => format!("ch_{}", elem.mangle_name()),
            Ty::WaitGroup => "WaitGroup".into(),
            Ty::Mutex { elem } => format!("mu_{}", elem.mangle_name()),
            Ty::RwLock { elem } => format!("rw_{}", elem.mangle_name()),
            Ty::CancelToken => "CancelToken".into(),
            Ty::Result { ok, err } => format!("res_{}_{}", ok.mangle_name(), err.mangle_name()),
            Ty::Option { inner } => format!("opt_{}", inner.mangle_name()),
            Ty::List { elem } => format!("list_{}", elem.mangle_name()),
            Ty::Fn { params, ret } => {
                let ps: Vec<_> = params.iter().map(|p| p.mangle_name()).collect();
                format!("fn_{}_{}", ps.join("_"), ret.mangle_name())
            }
            Ty::HttpRequest => "HttpRequest".into(),
            Ty::HttpResponse => "HttpResponse".into(),
            Ty::HttpHeaders => "HttpHeaders".into(),
            Ty::HttpServer => "HttpServer".into(),
        }
    }
}

fn ty_result(ok: Ty, err: Ty) -> Ty {
    Ty::Result {
        ok: Box::new(ok),
        err: Box::new(err),
    }
}

fn ty_option(inner: Ty) -> Ty {
    Ty::Option {
        inner: Box::new(inner),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LitValue {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

impl LitValue {
    fn ty(&self) -> Ty {
        match self {
            LitValue::Int(_) => Ty::Int,
            LitValue::Float(_) => Ty::Float,
            LitValue::String(_) => Ty::String,
            LitValue::Bool(_) => Ty::Bool,
        }
    }

    fn to_checked(&self) -> CheckedExpr {
        match self {
            LitValue::Int(n) => CheckedExpr::IntLit(*n),
            LitValue::Float(f) => CheckedExpr::FloatLit(*f),
            LitValue::String(s) => CheckedExpr::StringLit(s.clone()),
            LitValue::Bool(b) => CheckedExpr::BoolLit(*b),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckedProgram {
    pub functions: Vec<CheckedFunction>,
    pub methods: Vec<CheckedMethod>,
    pub classes: HashMap<String, ClassInfo>,
    pub has_std_import: bool,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub is_pub: bool,
    pub module: String,
    pub size: i64,
    pub bases: Vec<String>,
    pub interfaces: Vec<String>,
    pub fields: HashMap<String, FieldInfo>,
    pub props: HashMap<String, PropInfo>,
    pub methods: HashMap<String, MethodInfo>,
    pub drop_symbol: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub offset: i64,
    pub ty: Ty,
    pub vis: Visibility,
    pub defining_class: String,
    pub default: Option<LitValue>,
    /// Wire name for serde (`@encodeProperty`); None → use field name.
    pub serde_name: Option<String>,
    pub serde_ignore: bool,
}

#[derive(Debug, Clone)]
pub struct PropInfo {
    pub ty: Ty,
    pub vis: Visibility,
    pub defining_class: String,
    pub getter_symbol: Option<String>,
    pub setter_symbol: Option<String>,
    pub serde_name: Option<String>,
    pub serde_ignore: bool,
}

#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub symbol: String,
    pub params: Vec<Ty>,
    pub param_defaults: Vec<Option<LitValue>>,
    pub ret: Ty,
    pub vis: Visibility,
    pub defining_class: String,
}

#[derive(Debug, Clone)]
pub struct CheckedFunction {
    pub name: String,
    pub is_async: bool,
    pub params: Vec<(String, Ty)>,
    /// Inner return type (T for async fn → Future<T>).
    pub return_ty: Ty,
    pub body: Vec<CheckedStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CheckedMethod {
    pub class_name: String,
    pub name: String,
    pub symbol: String,
    pub is_ctor: bool,
    pub params: Vec<(String, Ty)>,
    pub return_ty: Ty,
    pub body: Vec<CheckedStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum CheckedAssignTarget {
    Local {
        name: String,
    },
    Field {
        object: CheckedExpr,
        offset: i64,
        ty: Ty,
    },
    Setter {
        object: CheckedExpr,
        symbol: String,
    },
    Index {
        array: CheckedExpr,
        index: CheckedExpr,
        elem: Ty,
        len: i64,
    },
}

#[derive(Debug, Clone)]
pub enum CheckedStmt {
    VarDecl {
        name: String,
        ty: Ty,
        init: CheckedExpr,
    },
    Assign {
        target: CheckedAssignTarget,
        value: CheckedExpr,
    },
    Drop {
        object: CheckedExpr,
        symbol: String,
    },
    Spawn {
        /// Index of `__spawn_{index}` thunk.
        index: usize,
        body: Vec<CheckedStmt>,
        /// Outer locals packed into the spawn env (stable order).
        captures: Vec<String>,
    },
    Return {
        value: Option<CheckedExpr>,
    },
    Expr {
        expr: CheckedExpr,
    },
    If {
        arms: Vec<(CheckedExpr, Vec<CheckedStmt>)>,
        else_block: Option<Vec<CheckedStmt>>,
    },
    While {
        cond: CheckedExpr,
        body: Vec<CheckedStmt>,
    },
    ForRange {
        name: String,
        start: CheckedExpr,
        end: CheckedExpr,
        body: Vec<CheckedStmt>,
    },
    ForIn {
        name: String,
        iter: CheckedExpr,
        elem: Ty,
        body: Vec<CheckedStmt>,
    },
    Break,
    Continue,
    Match {
        scrutinee: CheckedExpr,
        arms: Vec<(CheckedPattern, Vec<CheckedStmt>)>,
    },
    /// `do { … } catch name { … }`
    DoCatch {
        body: Vec<CheckedStmt>,
        catch_name: String,
        catch_ty: Ty,
        catch_body: Vec<CheckedStmt>,
    },
}

#[derive(Debug, Clone)]
pub enum CheckedPattern {
    IntLit(i64),
    Wildcard,
    Ok { name: String },
    Err { name: String },
    Some { name: String },
    None,
}

#[derive(Debug, Clone)]
pub enum CheckedExpr {
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),
    BoolLit(bool),
    Local {
        name: String,
        ty: Ty,
    },
    SelfExpr {
        class: String,
    },
    New {
        class: String,
        size: i64,
        ctor_symbol: String,
        args: Vec<CheckedExpr>,
    },
    ArrayLit {
        elems: Vec<CheckedExpr>,
        elem_ty: Ty,
        len: i64,
    },
    Index {
        array: Box<CheckedExpr>,
        index: Box<CheckedExpr>,
        elem: Ty,
        len: i64,
    },
    FieldGet {
        object: Box<CheckedExpr>,
        offset: i64,
        ty: Ty,
    },
    MethodCall {
        object: Box<CheckedExpr>,
        symbol: String,
        args: Vec<CheckedExpr>,
        ret: Ty,
    },
    Await {
        expr: Box<CheckedExpr>,
        inner: Ty,
    },
    /// `try` / `try?` / `try!` over a `Result`
    Try {
        mode: TryMode,
        expr: Box<CheckedExpr>,
        ok_ty: Ty,
        err_ty: Ty,
    },
    Binary {
        op: BinOp,
        left: Box<CheckedExpr>,
        right: Box<CheckedExpr>,
        /// Result type of the expression.
        ty: Ty,
        /// Operand flavor for codegen (Int/Float/String/Bool).
        operand_ty: Ty,
    },
    Unary {
        op: UnOp,
        expr: Box<CheckedExpr>,
        ty: Ty,
    },
    Call {
        name: String,
        args: Vec<CheckedExpr>,
        ret: Ty,
        /// If set, this call produces a Future handle (async fn).
        async_spawn: bool,
    },
    StdLog {
        args: Vec<CheckedExpr>,
    },
    StdSleep {
        ms: Box<CheckedExpr>,
    },
    ChannelNew,
    WaitGroupNew,
    ChannelSend {
        channel: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    ChannelRecv {
        channel: Box<CheckedExpr>,
    },
    /// Async-context `ch.recv()` → `Future<int>`
    ChannelRecvFuture {
        channel: Box<CheckedExpr>,
    },
    ChannelClose {
        channel: Box<CheckedExpr>,
    },
    /// `std.cpu.submit(namedFn)` → `Future<int>`
    CpuSubmitNamed {
        fn_name: String,
    },
    /// `std.cpu.submit(fn() int {…})` or submit of a `Ty::Fn` value → `Future<int>`
    CpuSubmitClosure {
        closure: Box<CheckedExpr>,
    },
    /// Closure literal `fn(…) T {…}`
    Closure {
        index: usize,
        params: Vec<(String, Ty)>,
        ret: Ty,
        body: Vec<CheckedStmt>,
        captures: Vec<String>,
    },
    /// Call a function value / closure
    CallClosure {
        callee: Box<CheckedExpr>,
        args: Vec<CheckedExpr>,
        ret: Ty,
    },
    ResultOk {
        value: Box<CheckedExpr>,
    },
    ResultErr {
        value: Box<CheckedExpr>,
    },
    OptionSome {
        value: Box<CheckedExpr>,
    },
    OptionNone,
    WaitGroupAdd {
        wg: Box<CheckedExpr>,
        delta: Box<CheckedExpr>,
    },
    WaitGroupDone {
        wg: Box<CheckedExpr>,
    },
    WaitGroupWait {
        wg: Box<CheckedExpr>,
    },
    WaitGroupWaitFuture {
        wg: Box<CheckedExpr>,
    },
    FutureJoin {
        left: Box<CheckedExpr>,
        right: Box<CheckedExpr>,
    },
    FutureRace {
        left: Box<CheckedExpr>,
        right: Box<CheckedExpr>,
    },
    FutureReady {
        value: Box<CheckedExpr>,
    },
    ChannelBuffered {
        capacity: Box<CheckedExpr>,
    },
    MutexNew {
        initial: Box<CheckedExpr>,
    },
    MutexLock {
        mutex: Box<CheckedExpr>,
    },
    MutexUnlock {
        mutex: Box<CheckedExpr>,
    },
    MutexGet {
        mutex: Box<CheckedExpr>,
    },
    MutexSet {
        mutex: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    RwLockNew {
        initial: Box<CheckedExpr>,
    },
    RwLockReadLock {
        lock: Box<CheckedExpr>,
    },
    RwLockReadUnlock {
        lock: Box<CheckedExpr>,
    },
    RwLockWriteLock {
        lock: Box<CheckedExpr>,
    },
    RwLockWriteUnlock {
        lock: Box<CheckedExpr>,
    },
    RwLockGet {
        lock: Box<CheckedExpr>,
    },
    RwLockSet {
        lock: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    ParallelMap {
        list: Box<CheckedExpr>,
        fn_name: String,
    },
    /// Async HTTP client call → Future<Result<Response,string>>
    HttpClient {
        method: HttpClientMethod,
        url: Box<CheckedExpr>,
        body: Option<Box<CheckedExpr>>,
        headers: Option<Box<CheckedExpr>>,
    },
    HttpHeadersNew,
    HttpHeadersSet {
        headers: Box<CheckedExpr>,
        key: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    HttpHeadersGet {
        headers: Box<CheckedExpr>,
        key: Box<CheckedExpr>,
    },
    HttpResponseNew {
        kind: HttpResponseKind,
        status: Box<CheckedExpr>,
        body: Option<Box<CheckedExpr>>,
    },
    HttpResponseStatus {
        response: Box<CheckedExpr>,
    },
    HttpResponseBody {
        response: Box<CheckedExpr>,
    },
    HttpResponseSetHeader {
        response: Box<CheckedExpr>,
        key: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    HttpResponseHeader {
        response: Box<CheckedExpr>,
        key: Box<CheckedExpr>,
    },
    HttpRequestMethod {
        request: Box<CheckedExpr>,
    },
    HttpRequestPath {
        request: Box<CheckedExpr>,
    },
    HttpRequestBody {
        request: Box<CheckedExpr>,
    },
    HttpRequestQuery {
        request: Box<CheckedExpr>,
        name: Box<CheckedExpr>,
    },
    HttpRequestHeader {
        request: Box<CheckedExpr>,
        name: Box<CheckedExpr>,
    },
    HttpRequestParam {
        request: Box<CheckedExpr>,
        name: Box<CheckedExpr>,
    },
    HttpServerNew,
    HttpServerRoute {
        server: Box<CheckedExpr>,
        method: String,
        path: Box<CheckedExpr>,
        handler: Box<CheckedExpr>,
    },
    HttpServerListen {
        server: Box<CheckedExpr>,
        port: Box<CheckedExpr>,
    },
    TaskYield,
    CancelTokenNew,
    CancelTokenCancel {
        token: Box<CheckedExpr>,
    },
    CancelTokenIsCancelled {
        token: Box<CheckedExpr>,
    },
    ListNew,
    ListPush {
        list: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    ListGet {
        list: Box<CheckedExpr>,
        index: Box<CheckedExpr>,
        /// Element type, so codegen knows whether the result is a string pointer.
        elem: Ty,
    },
    ListSet {
        list: Box<CheckedExpr>,
        index: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    ListLen {
        list: Box<CheckedExpr>,
    },
    StdPanic {
        msg: Box<CheckedExpr>,
    },
    StdEnvArgs,
    StdEnvGet {
        name: Box<CheckedExpr>,
    },
    StdEnvSet {
        name: Box<CheckedExpr>,
        value: Box<CheckedExpr>,
    },
    StdProcessExit {
        code: Box<CheckedExpr>,
    },
    StdFsReadToString {
        path: Box<CheckedExpr>,
    },
    StdFsWriteString {
        path: Box<CheckedExpr>,
        contents: Box<CheckedExpr>,
    },
    StdTimeNowMs,
    StdStringLen {
        s: Box<CheckedExpr>,
    },
    StdStringConcat {
        a: Box<CheckedExpr>,
        b: Box<CheckedExpr>,
    },
    StdStringSlice {
        s: Box<CheckedExpr>,
        start: Box<CheckedExpr>,
        end: Box<CheckedExpr>,
    },
    StdStringContains {
        hay: Box<CheckedExpr>,
        needle: Box<CheckedExpr>,
    },
    StdStringFromInt {
        n: Box<CheckedExpr>,
    },
    StdStringParseInt {
        s: Box<CheckedExpr>,
    },
    AsyncBlock {
        index: usize,
        body: Vec<CheckedStmt>,
        captures: Vec<String>,
    },
    /// `std.{json,yaml,toml,toon}.encode(value)`
    SerdeEncode {
        format: SerdeFormat,
        value: Box<CheckedExpr>,
        schema: String,
    },
    /// `std.{json,yaml,toml,toon}.decode<T>(text)` → Result<T,string>
    SerdeDecode {
        format: SerdeFormat,
        text: Box<CheckedExpr>,
        schema: String,
        ty: Ty,
    },
}

#[derive(Clone)]
struct FuncSig {
    params: Vec<Ty>,
    defaults: Vec<Option<LitValue>>,
    /// Declared/inner return (not wrapped in Future).
    ret: Ty,
    is_async: bool,
    is_pub: bool,
    module: String,
}

struct IClassSig {
    methods: HashMap<String, (Vec<Ty>, Ty)>,
}

struct CheckCtx<'a> {
    sigs: &'a HashMap<String, FuncSig>,
    classes: &'a HashMap<String, ClassInfo>,
    type_names: TypeNames<'a>,
    /// Generic functions awaiting monomorphization at call sites.
    generic_fns: &'a HashMap<String, Function>,
    /// Emitted specialized functions (name → CheckedFunction).
    mono: &'a std::cell::RefCell<HashMap<String, CheckedFunction>>,
    /// Emitted specialized FuncSig entries shared with callers.
    mono_sigs: &'a std::cell::RefCell<HashMap<String, FuncSig>>,
    has_std: bool,
    current_class: Option<&'a str>,
    current_module: &'a str,
    in_async: bool,
    /// When `Some`, we are inside a `do` block; the cell holds the unified err type (`None` until first `try`).
    do_catch: Option<&'a Cell<Option<Ty>>>,
    /// Top-level `const` literals visible in the current module (folded at use sites).
    const_lits: HashMap<String, LitValue>,
    async_block_index: &'a Cell<usize>,
    spawn_index: &'a Cell<usize>,
    closure_index: &'a Cell<usize>,
}

const SLOT: i64 = 8;

pub fn typecheck(program: &Program, entry_module: &str) -> Result<CheckedProgram, Diagnostic> {
    let spawn_index = Cell::new(0usize);
    let async_block_index = Cell::new(0usize);
    let closure_index = Cell::new(0usize);
    let mut has_std_import = false;
    for imp in &program.imports {
        if imp.path == "std" {
            has_std_import = true;
        } else {
            return Err(Diagnostic::new(
                format!("unknown import '{}'; MVP only supports \"std\"", imp.path),
                imp.span,
            ));
        }
    }

    let mut class_names: HashSet<String> = HashSet::with_capacity(program.classes.len());
    let mut iclass_names: HashSet<String> = HashSet::with_capacity(program.iclasses.len());
    for c in &program.classes {
        if !c.type_params.is_empty() {
            return Err(Diagnostic::new(
                "generic classes are not yet supported (use concrete fields/types)",
                c.span,
            ));
        }
        if !class_names.insert(c.name.clone()) {
            return Err(Diagnostic::new(
                format!("duplicate class '{}'", c.name),
                c.span,
            ));
        }
    }
    for i in &program.iclasses {
        if class_names.contains(&i.name) || !iclass_names.insert(i.name.clone()) {
            return Err(Diagnostic::new(
                format!("duplicate type '{}'", i.name),
                i.span,
            ));
        }
    }
    let type_names = TypeNames {
        classes: &class_names,
        iclasses: &iclass_names,
    };

    let iclasses = build_iclasses(program, type_names)?;
    let (classes, synthetic_ctors) = build_classes(program, type_names, &iclasses)?;

    let mut sigs: HashMap<String, FuncSig> = HashMap::new();
    let mut generic_fns: HashMap<String, Function> = HashMap::new();
    for f in &program.functions {
        if sigs.contains_key(&f.name) || generic_fns.contains_key(&f.name) {
            return Err(Diagnostic::new(
                format!("duplicate function '{}'", f.name),
                f.span,
            ));
        }
        if !f.type_params.is_empty() {
            if f.is_async {
                return Err(Diagnostic::new(
                    "async generic functions are not yet supported",
                    f.span,
                ));
            }
            let mut seen = HashSet::new();
            for tp in &f.type_params {
                if !seen.insert(tp.clone()) {
                    return Err(Diagnostic::new(
                        format!("duplicate type parameter '{tp}'"),
                        f.span,
                    ));
                }
                if class_names.contains(tp) || iclass_names.contains(tp) {
                    return Err(Diagnostic::new(
                        format!("type parameter '{tp}' shadows a named type"),
                        f.span,
                    ));
                }
            }
            generic_fns.insert(f.name.clone(), f.clone());
            continue;
        }
        let (params, defaults) = check_params(&f.params, type_names)?;
        let ret = match &f.return_ty {
            Some(t) => Ty::from_ast_at(t, type_names, f.span)?,
            None => Ty::Void,
        };
        sigs.insert(
            f.name.clone(),
            FuncSig {
                params,
                defaults,
                ret,
                is_async: f.is_async,
                is_pub: f.is_pub,
                module: f.module.clone(),
            },
        );
    }

    if !sigs.contains_key("main") {
        return Err(Diagnostic::new(
            "program must define fn main() or async fn main()",
            Span::dummy(),
        ));
    }
    {
        let main_sig = &sigs["main"];
        if !main_sig.params.is_empty() || main_sig.ret != Ty::Void {
            let main_fn = program.functions.iter().find(|f| f.name == "main").unwrap();
            return Err(Diagnostic::new(
                "main must have no parameters and no return type",
                main_fn.span,
            ));
        }
        if main_sig.module != entry_module && !entry_module.is_empty() {
            let main_fn = program.functions.iter().find(|f| f.name == "main").unwrap();
            return Err(Diagnostic::new(
                "main must be defined in the entry file",
                main_fn.span,
            ));
        }
    }

    let mono = std::cell::RefCell::new(HashMap::new());
    let mono_sigs = std::cell::RefCell::new(HashMap::new());

    let mut functions = Vec::new();
    for f in &program.functions {
        if !f.type_params.is_empty() {
            continue; // specialized on demand
        }
        let ctx = CheckCtx {
            sigs: &sigs,
            classes: &classes,
            type_names,
            generic_fns: &generic_fns,
            mono: &mono,
            mono_sigs: &mono_sigs,
            has_std: has_std_import,
            current_class: None,
            current_module: f.module.as_str(),
            in_async: f.is_async,
            do_catch: None,
            const_lits: module_const_lits(&program.constants, f.module.as_str())?,
            async_block_index: &async_block_index,
            spawn_index: &spawn_index,
            closure_index: &closure_index,
        };
        functions.push(check_function(f, &ctx, &program.constants)?);
    }

    let mut methods = Vec::new();
    for c in &program.classes {
        for m in &c.methods {
            let mctx = CheckCtx {
                sigs: &sigs,
                classes: &classes,
                type_names,
                generic_fns: &generic_fns,
                mono: &mono,
                mono_sigs: &mono_sigs,
                has_std: has_std_import,
                current_class: Some(&c.name),
                current_module: c.module.as_str(),
                in_async: false,
                do_catch: None,
                const_lits: module_const_lits(&program.constants, c.module.as_str())?,
                async_block_index: &async_block_index,
                spawn_index: &spawn_index,
                closure_index: &closure_index,
            };
            let mut checked = check_method(c, m, &mctx)?;
            if checked.is_ctor {
                checked.body = prepend_field_defaults(&c.name, &classes, checked.body);
            }
            methods.push(checked);
        }
    }

    for c in &program.classes {
        for f in &c.fields {
            if let Some(acc) = &f.accessors {
                let mctx = CheckCtx {
                    sigs: &sigs,
                    classes: &classes,
                    type_names,
                    generic_fns: &generic_fns,
                    mono: &mono,
                    mono_sigs: &mono_sigs,
                    has_std: has_std_import,
                    current_class: Some(&c.name),
                    current_module: c.module.as_str(),
                    in_async: false,
                    do_catch: None,
                    const_lits: module_const_lits(&program.constants, c.module.as_str())?,
                    async_block_index: &async_block_index,
                    spawn_index: &spawn_index,
                    closure_index: &closure_index,
                };
                if let Some(getter) = &acc.getter {
                    let mut env = HashMap::new();
                    let mut owned = Vec::new();
                    let ret = Ty::from_ast_at(&f.ty, type_names, f.span)?;
                    let body = check_block(
                        &getter.stmts,
                        &mut env,
                        &mctx,
                        &ret,
                        0,
                        &mut owned,
                        &mut HashSet::new(),
                        true,
                    )?;
                    methods.push(CheckedMethod {
                        class_name: c.name.clone(),
                        name: format!("__get_{}", f.name),
                        symbol: format!("{}___get_{}", c.name, f.name),
                        is_ctor: false,
                        params: vec![],
                        return_ty: ret,
                        body,
                        span: f.span,
                    });
                }
                if let Some(setter) = &acc.setter {
                    let mut env = HashMap::new();
                    let pty = Ty::from_ast_at(&setter.param.ty, type_names, setter.param.span)?;
                    let expected = Ty::from_ast_at(&f.ty, type_names, f.span)?;
                    if !ty_assignable(&pty, &expected, &mctx) {
                        return Err(Diagnostic::new(
                            format!("setter parameter type must be {:?}", expected),
                            setter.param.span,
                        ));
                    }
                    env.insert(setter.param.name.clone(), pty.clone());
                    let mut owned = Vec::new();
                    let body = check_block(
                        &setter.body.stmts,
                        &mut env,
                        &mctx,
                        &Ty::Void,
                        0,
                        &mut owned,
                        &mut HashSet::new(),
                        true,
                    )?;
                    methods.push(CheckedMethod {
                        class_name: c.name.clone(),
                        name: format!("__set_{}", f.name),
                        symbol: format!("{}___set_{}", c.name, f.name),
                        is_ctor: false,
                        params: vec![(setter.param.name.clone(), pty)],
                        return_ty: Ty::Void,
                        body,
                        span: f.span,
                    });
                }
            }
        }
    }

    for mut syn in synthetic_ctors {
        syn.body = prepend_field_defaults(&syn.class_name, &classes, syn.body);
        methods.push(syn);
    }

    // Append monomorphized generic specializations
    let mono_map = mono.into_inner();
    for (_, cf) in mono_map {
        functions.push(cf);
    }

    Ok(CheckedProgram {
        functions,
        methods,
        classes,
        has_std_import,
    })
}

fn check_params(
    params: &[Param],
    names: TypeNames<'_>,
) -> Result<(Vec<Ty>, Vec<Option<LitValue>>), Diagnostic> {
    let mut tys = Vec::with_capacity(params.len());
    let mut defaults = Vec::with_capacity(params.len());
    let mut seen_default = false;
    for p in params {
        let ty = Ty::from_ast_at(&p.ty, names, p.span)?;
        if matches!(ty, Ty::Void) {
            return Err(Diagnostic::new("parameter cannot be void", p.span));
        }
        let def = match &p.default {
            Some(e) => {
                seen_default = true;
                let lit = lit_from_expr(e)?;
                if lit.ty() != ty {
                    return Err(Diagnostic::new(
                        "default value type mismatch",
                        e.span(),
                    ));
                }
                Some(lit)
            }
            None => {
                if seen_default {
                    return Err(Diagnostic::new(
                        "required parameter cannot follow defaulted parameter",
                        p.span,
                    ));
                }
                None
            }
        };
        tys.push(ty);
        defaults.push(def);
    }
    Ok((tys, defaults))
}

/// Locals referenced in `stmts` that are not declared inside that subtree.
fn free_locals(stmts: &[CheckedStmt]) -> Vec<String> {
    use std::collections::BTreeSet;
    let mut declared = HashSet::new();
    let mut used = BTreeSet::new();
    fn walk_stmts(
        stmts: &[CheckedStmt],
        declared: &mut HashSet<String>,
        used: &mut BTreeSet<String>,
    ) {
        for s in stmts {
            match s {
                CheckedStmt::VarDecl { name, init, .. } => {
                    walk_expr_free(init, declared, used);
                    declared.insert(name.clone());
                }
                CheckedStmt::Assign { target, value } => {
                    match target {
                        CheckedAssignTarget::Local { name, .. } => {
                            if !declared.contains(name) {
                                used.insert(name.clone());
                            }
                        }
                        CheckedAssignTarget::Field { object, .. }
                        | CheckedAssignTarget::Setter { object, .. } => {
                            walk_expr_free(object, declared, used);
                        }
                        CheckedAssignTarget::Index { array, index, .. } => {
                            walk_expr_free(array, declared, used);
                            walk_expr_free(index, declared, used);
                        }
                    }
                    walk_expr_free(value, declared, used);
                }
                CheckedStmt::Drop { object, .. } => walk_expr_free(object, declared, used),
                CheckedStmt::Spawn { body, .. } => walk_stmts(body, declared, used),
                CheckedStmt::Return { value } => {
                    if let Some(v) = value {
                        walk_expr_free(v, declared, used);
                    }
                }
                CheckedStmt::Expr { expr } => walk_expr_free(expr, declared, used),
                CheckedStmt::If { arms, else_block } => {
                    for (c, b) in arms {
                        walk_expr_free(c, declared, used);
                        walk_stmts(b, declared, used);
                    }
                    if let Some(eb) = else_block {
                        walk_stmts(eb, declared, used);
                    }
                }
                CheckedStmt::While { cond, body } => {
                    walk_expr_free(cond, declared, used);
                    walk_stmts(body, declared, used);
                }
                CheckedStmt::ForRange {
                    name,
                    start,
                    end,
                    body,
                } => {
                    walk_expr_free(start, declared, used);
                    walk_expr_free(end, declared, used);
                    let mut inner = declared.clone();
                    inner.insert(name.clone());
                    walk_stmts(body, &mut inner, used);
                }
                CheckedStmt::ForIn {
                    name, iter, body, ..
                } => {
                    walk_expr_free(iter, declared, used);
                    let mut inner = declared.clone();
                    inner.insert(name.clone());
                    walk_stmts(body, &mut inner, used);
                }
                CheckedStmt::Match { scrutinee, arms } => {
                    walk_expr_free(scrutinee, declared, used);
                    for (_, b) in arms {
                        walk_stmts(b, declared, used);
                    }
                }
                CheckedStmt::DoCatch {
                    body,
                    catch_name,
                    catch_body,
                    ..
                } => {
                    walk_stmts(body, declared, used);
                    let mut inner = declared.clone();
                    inner.insert(catch_name.clone());
                    walk_stmts(catch_body, &mut inner, used);
                }
                CheckedStmt::Break | CheckedStmt::Continue => {}
            }
        }
    }
    fn walk_expr_free(
        e: &CheckedExpr,
        declared: &HashSet<String>,
        used: &mut BTreeSet<String>,
    ) {
        match e {
            CheckedExpr::Local { name, .. } => {
                if !declared.contains(name) {
                    used.insert(name.clone());
                }
            }
            CheckedExpr::Binary { left, right, .. } => {
                walk_expr_free(left, declared, used);
                walk_expr_free(right, declared, used);
            }
            CheckedExpr::Unary { expr, .. }
            | CheckedExpr::Await { expr, .. }
            | CheckedExpr::Try { expr, .. } => {
                walk_expr_free(expr, declared, used);
            }
            CheckedExpr::Call { args, .. }
            | CheckedExpr::StdLog { args }
            | CheckedExpr::New { args, .. }
            | CheckedExpr::ArrayLit { elems: args, .. } => {
                for a in args {
                    walk_expr_free(a, declared, used);
                }
            }
            CheckedExpr::MethodCall { object, args, .. } => {
                walk_expr_free(object, declared, used);
                for a in args {
                    walk_expr_free(a, declared, used);
                }
            }
            CheckedExpr::FieldGet { object, .. } => walk_expr_free(object, declared, used),
            CheckedExpr::Index { array, index, .. } => {
                walk_expr_free(array, declared, used);
                walk_expr_free(index, declared, used);
            }
            CheckedExpr::StdSleep { ms } => walk_expr_free(ms, declared, used),
            CheckedExpr::ChannelSend { channel, value } => {
                walk_expr_free(channel, declared, used);
                walk_expr_free(value, declared, used);
            }
            CheckedExpr::ChannelRecv { channel }
            | CheckedExpr::ChannelRecvFuture { channel }
            | CheckedExpr::ChannelClose { channel } => walk_expr_free(channel, declared, used),
            CheckedExpr::CpuSubmitNamed { .. } => {}
            CheckedExpr::CpuSubmitClosure { closure } => {
                walk_expr_free(closure, declared, used);
            }
            CheckedExpr::Closure { captures, .. } => {
                for c in captures {
                    if !declared.contains(c) {
                        used.insert(c.clone());
                    }
                }
            }
            CheckedExpr::CallClosure { callee, args, .. } => {
                walk_expr_free(callee, declared, used);
                for a in args {
                    walk_expr_free(a, declared, used);
                }
            }
            CheckedExpr::ResultOk { value }
            | CheckedExpr::ResultErr { value }
            | CheckedExpr::OptionSome { value } => walk_expr_free(value, declared, used),
            CheckedExpr::OptionNone => {}
            CheckedExpr::WaitGroupAdd { wg, delta } => {
                walk_expr_free(wg, declared, used);
                walk_expr_free(delta, declared, used);
            }
            CheckedExpr::WaitGroupDone { wg }
            | CheckedExpr::WaitGroupWait { wg }
            | CheckedExpr::WaitGroupWaitFuture { wg } => {
                walk_expr_free(wg, declared, used);
            }
            CheckedExpr::FutureJoin { left, right }
            | CheckedExpr::FutureRace { left, right } => {
                walk_expr_free(left, declared, used);
                walk_expr_free(right, declared, used);
            }
            CheckedExpr::FutureReady { value } => walk_expr_free(value, declared, used),
            CheckedExpr::ChannelBuffered { capacity } => {
                walk_expr_free(capacity, declared, used)
            }
            CheckedExpr::MutexNew { initial } => walk_expr_free(initial, declared, used),
            CheckedExpr::MutexLock { mutex }
            | CheckedExpr::MutexUnlock { mutex }
            | CheckedExpr::MutexGet { mutex } => walk_expr_free(mutex, declared, used),
            CheckedExpr::MutexSet { mutex, value } => {
                walk_expr_free(mutex, declared, used);
                walk_expr_free(value, declared, used);
            }
            CheckedExpr::AsyncBlock { captures, .. } => {
                for c in captures {
                    if !declared.contains(c) {
                        used.insert(c.clone());
                    }
                }
            }
            CheckedExpr::SerdeEncode { value, .. } => walk_expr_free(value, declared, used),
            CheckedExpr::SerdeDecode { text, .. } => walk_expr_free(text, declared, used),
            _ => {}
        }
    }
    walk_stmts(stmts, &mut declared, &mut used);
    used.into_iter().collect()
}

fn lit_from_expr(e: &Expr) -> Result<LitValue, Diagnostic> {
    match e {
        Expr::IntLit { value, .. } => Ok(LitValue::Int(*value)),
        Expr::FloatLit { value, .. } => Ok(LitValue::Float(*value)),
        Expr::StringLit { value, .. } => Ok(LitValue::String(value.clone())),
        Expr::BoolLit { value, .. } => Ok(LitValue::Bool(*value)),
        _ => Err(Diagnostic::new(
            "default value must be a literal",
            e.span(),
        )),
    }
}

fn module_const_lits(
    constants: &[ConstDecl],
    module: &str,
) -> Result<HashMap<String, LitValue>, Diagnostic> {
    let mut out = HashMap::new();
    for c in constants {
        if c.module != module {
            continue;
        }
        if out.contains_key(&c.name) {
            return Err(Diagnostic::new(
                format!("duplicate const '{}'", c.name),
                c.span,
            ));
        }
        out.insert(c.name.clone(), lit_from_expr(&c.value)?);
    }
    Ok(out)
}

fn fill_args(
    provided: &[Expr],
    param_tys: &[Ty],
    defaults: &[Option<LitValue>],
    env: &HashMap<String, Ty>,
    ctx: &CheckCtx<'_>,
    what: &str,
    span: Span,
) -> Result<Vec<CheckedExpr>, Diagnostic> {
    if provided.len() > param_tys.len() {
        return Err(Diagnostic::new(
            format!("{what} expects at most {} args, got {}", param_tys.len(), provided.len()),
            span,
        ));
    }
    let mut out = Vec::with_capacity(param_tys.len());
    for (i, expected) in param_tys.iter().enumerate() {
        if i < provided.len() {
            let (ty, e) = check_expr(&provided[i], env, ctx)?;
            if !ty_assignable(&ty, expected, ctx) {
                return Err(Diagnostic::new(
                    format!("argument type mismatch in {what}"),
                    provided[i].span(),
                ));
            }
            out.push(e);
        } else if let Some(Some(lit)) = defaults.get(i) {
            out.push(lit.to_checked());
        } else {
            return Err(Diagnostic::new(
                format!("{what} missing required argument {}", i + 1),
                span,
            ));
        }
    }
    Ok(out)
}

fn prepend_field_defaults(
    class: &str,
    classes: &HashMap<String, ClassInfo>,
    body: Vec<CheckedStmt>,
) -> Vec<CheckedStmt> {
    let Some(cinfo) = classes.get(class) else {
        return body;
    };
    let mut inits = Vec::new();
    let mut fields: Vec<_> = cinfo.fields.values().collect();
    fields.sort_by_key(|f| f.offset);
    for f in fields {
        if let Some(def) = &f.default {
            inits.push(CheckedStmt::Assign {
                target: CheckedAssignTarget::Field {
                    object: CheckedExpr::SelfExpr {
                        class: class.to_string(),
                    },
                    offset: f.offset,
                    ty: f.ty.clone(),
                },
                value: def.to_checked(),
            });
        }
    }
    inits.extend(body);
    inits
}

fn build_iclasses(
    program: &Program,
    names: TypeNames<'_>,
) -> Result<HashMap<String, IClassSig>, Diagnostic> {
    let mut out = HashMap::with_capacity(program.iclasses.len());
    for i in &program.iclasses {
        let mut methods = HashMap::new();
        for m in &i.methods {
            if methods.contains_key(&m.name) {
                return Err(Diagnostic::new(
                    format!("duplicate iclass method '{}'", m.name),
                    m.span,
                ));
            }
            let (params, _) = check_params(&m.params, names)?;
            let ret = match &m.return_ty {
                Some(t) => Ty::from_ast_at(t, names, m.span)?,
                None => Ty::Void,
            };
            methods.insert(m.name.clone(), (params, ret));
        }
        out.insert(i.name.clone(), IClassSig { methods });
    }
    Ok(out)
}

fn build_classes(
    program: &Program,
    names: TypeNames<'_>,
    iclasses: &HashMap<String, IClassSig>,
) -> Result<(HashMap<String, ClassInfo>, Vec<CheckedMethod>), Diagnostic> {
    let by_name: HashMap<&str, &ClassDecl> = program
        .classes
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    let mut order = Vec::new();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for c in &program.classes {
        topo_visit(&c.name, &by_name, &mut visiting, &mut visited, &mut order)?;
    }

    let mut classes: HashMap<String, ClassInfo> = HashMap::new();
    let mut synthetic_ctors = Vec::new();

    for name in order {
        let c = by_name[name.as_str()];

        // Diamond check: collect ancestors from each base path
        let mut seen_ancestors: HashSet<String> = HashSet::new();
        for b in &c.bases {
            if !classes.contains_key(b) {
                return Err(Diagnostic::new(
                    format!("unknown base class '{b}'"),
                    c.span,
                ));
            }
            let mut path = Vec::new();
            collect_ancestors(b, &classes, &mut path);
            for a in path {
                if !seen_ancestors.insert(a.clone()) {
                    return Err(Diagnostic::new(
                        format!(
                            "diamond inheritance: '{}' appears via multiple paths in '{}'",
                            a, c.name
                        ),
                        c.span,
                    ));
                }
            }
        }

        let mut fields: HashMap<String, FieldInfo> = HashMap::new();
        let mut props: HashMap<String, PropInfo> = HashMap::new();
        let mut offset = 0i64;
        let mut methods: HashMap<String, MethodInfo> = HashMap::new();
        let mut ambiguous: HashSet<String> = HashSet::new();

        for b in &c.bases {
            let bi = classes.get(b).unwrap();
            for (fname, finfo) in &bi.fields {
                if fields.contains_key(fname) || props.contains_key(fname) {
                    return Err(Diagnostic::new(
                        format!("duplicate field '{fname}' from bases"),
                        c.span,
                    ));
                }
                let mut f = finfo.clone();
                f.offset += offset;
                fields.insert(fname.clone(), f);
            }
            for (pname, pinfo) in &bi.props {
                if fields.contains_key(pname) || props.contains_key(pname) {
                    return Err(Diagnostic::new(
                        format!("duplicate property '{pname}' from bases"),
                        c.span,
                    ));
                }
                props.insert(pname.clone(), pinfo.clone());
            }
            for (mname, minfo) in &bi.methods {
                if let Some(prev) = methods.get(mname) {
                    if prev.symbol != minfo.symbol {
                        ambiguous.insert(mname.clone());
                    }
                } else {
                    methods.insert(mname.clone(), minfo.clone());
                }
            }
            offset += bi.size;
        }

        // Own storage fields and properties
        for f in &c.fields {
            if fields.contains_key(&f.name) || props.contains_key(&f.name) {
                return Err(Diagnostic::new(
                    format!("duplicate member '{}'", f.name),
                    f.span,
                ));
            }
            let ty = Ty::from_ast_at(&f.ty, names, f.span)?;
            if matches!(ty, Ty::Void) {
                return Err(Diagnostic::new("field cannot be void", f.span));
            }

            let (serde_name, serde_ignore) = parse_field_serde_attrs(&f.attrs, f.span)?;

            if let Some(acc) = &f.accessors {
                let getter_symbol = acc
                    .getter
                    .as_ref()
                    .map(|_| format!("{}___get_{}", c.name, f.name));
                let setter_symbol = acc
                    .setter
                    .as_ref()
                    .map(|_| format!("{}___set_{}", c.name, f.name));
                props.insert(
                    f.name.clone(),
                    PropInfo {
                        ty,
                        vis: f.vis,
                        defining_class: c.name.clone(),
                        getter_symbol,
                        setter_symbol,
                        serde_name,
                        serde_ignore,
                    },
                );
            } else {
                let default = match &f.default {
                    Some(e) => {
                        let lit = lit_from_expr(e)?;
                        if lit.ty() != ty {
                            return Err(Diagnostic::new(
                                "field default type mismatch",
                                e.span(),
                            ));
                        }
                        Some(lit)
                    }
                    None => None,
                };
                fields.insert(
                    f.name.clone(),
                    FieldInfo {
                        offset,
                        ty,
                        vis: f.vis,
                        defining_class: c.name.clone(),
                        default,
                        serde_name,
                        serde_ignore,
                    },
                );
                offset += SLOT;
            }
        }

        for m in &c.methods {
            let (params, param_defaults) = check_params(&m.params, names)?;
            let ret = match &m.return_ty {
                Some(t) => Ty::from_ast_at(t, names, m.span)?,
                None => Ty::Void,
            };

            // `new` may change arity per class; other overrides must match.
            if m.name != "new" {
                if let Some(prev) = methods.get(&m.name) {
                    if prev.params != params || prev.ret != ret {
                        return Err(Diagnostic::new(
                            format!("method '{}' override must match base signature", m.name),
                            m.span,
                        ));
                    }
                }
            }
            ambiguous.remove(&m.name);

            if m.name == "new" {
                if ret != Ty::Class(c.name.clone()) {
                    return Err(Diagnostic::new(
                        format!("constructor 'new' must return {}", c.name),
                        m.span,
                    ));
                }
                if m.vis != Visibility::Pub {
                    return Err(Diagnostic::new("constructor 'new' must be pub", m.span));
                }
            }
            if m.name == "drop" {
                if !params.is_empty() || ret != Ty::Void {
                    return Err(Diagnostic::new(
                        "drop must be declared as fn drop() with no return type",
                        m.span,
                    ));
                }
            }

            methods.insert(
                m.name.clone(),
                MethodInfo {
                    symbol: format!("{}_{}", c.name, m.name),
                    params,
                    param_defaults,
                    ret,
                    vis: m.vis,
                    defining_class: c.name.clone(),
                },
            );
        }

        if !ambiguous.is_empty() {
            let names: Vec<_> = ambiguous.iter().cloned().collect();
            return Err(Diagnostic::new(
                format!(
                    "ambiguous inherited methods in '{}': {} — override or qualify with super.Base",
                    c.name,
                    names.join(", ")
                ),
                c.span,
            ));
        }

        for iface in &c.interfaces {
            let Some(isig) = iclasses.get(iface) else {
                return Err(Diagnostic::new(
                    format!("unknown iclass '{iface}'"),
                    c.span,
                ));
            };
            for (mname, (params, ret)) in &isig.methods {
                let Some(minfo) = methods.get(mname) else {
                    return Err(Diagnostic::new(
                        format!("class '{}' does not implement '{iface}.{mname}'", c.name),
                        c.span,
                    ));
                };
                if minfo.vis != Visibility::Pub {
                    return Err(Diagnostic::new(
                        format!("iclass method '{mname}' must be pub in class '{}'", c.name),
                        c.span,
                    ));
                }
                if &minfo.params != params || &minfo.ret != ret {
                    return Err(Diagnostic::new(
                        format!("method '{mname}' does not match iclass '{iface}' signature"),
                        c.span,
                    ));
                }
            }
        }

        let needs_ctor = fields.values().any(|f| f.default.is_none());
        if needs_ctor && !methods.contains_key("new") {
            return Err(Diagnostic::new(
                format!(
                    "class '{}' has fields without defaults and must define or inherit pub fn new",
                    c.name
                ),
                c.span,
            ));
        }

        if !methods.contains_key("new") && !needs_ctor {
            // Synthetic empty/defaulting constructor
            let symbol = format!("{}_new", c.name);
            methods.insert(
                "new".into(),
                MethodInfo {
                    symbol: symbol.clone(),
                    params: vec![],
                    param_defaults: vec![],
                    ret: Ty::Class(c.name.clone()),
                    vis: Visibility::Pub,
                    defining_class: c.name.clone(),
                },
            );
            synthetic_ctors.push(CheckedMethod {
                class_name: c.name.clone(),
                name: "new".into(),
                symbol,
                is_ctor: true,
                params: vec![],
                return_ty: Ty::Class(c.name.clone()),
                body: vec![CheckedStmt::Return {
                    value: Some(CheckedExpr::SelfExpr {
                        class: c.name.clone(),
                    }),
                }],
                span: c.span,
            });
        }

        let drop_symbol = methods.get("drop").map(|m| m.symbol.clone());
        let size = if offset == 0 { SLOT } else { offset };

        classes.insert(
            c.name.clone(),
            ClassInfo {
                name: c.name.clone(),
                is_pub: c.is_pub,
                module: c.module.clone(),
                size,
                bases: c.bases.clone(),
                interfaces: c.interfaces.clone(),
                fields,
                props,
                methods,
                drop_symbol,
            },
        );
    }

    Ok((classes, synthetic_ctors))
}

fn collect_ancestors(name: &str, classes: &HashMap<String, ClassInfo>, out: &mut Vec<String>) {
    out.push(name.to_string());
    if let Some(c) = classes.get(name) {
        for b in &c.bases {
            collect_ancestors(b, classes, out);
        }
    }
}

fn topo_visit(
    name: &str,
    by_name: &HashMap<&str, &ClassDecl>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    order: &mut Vec<String>,
) -> Result<(), Diagnostic> {
    if visited.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_string()) {
        let span = by_name.get(name).map(|c| c.span).unwrap_or_else(Span::dummy);
        return Err(Diagnostic::new(
            format!("cyclic inheritance involving '{name}'"),
            span,
        ));
    }
    let Some(c) = by_name.get(name) else {
        return Ok(());
    };
    for base in &c.bases {
        if !by_name.contains_key(base.as_str()) {
            return Err(Diagnostic::new(
                format!("unknown base class '{base}'"),
                c.span,
            ));
        }
        topo_visit(base, by_name, visiting, visited, order)?;
    }
    visiting.remove(name);
    visited.insert(name.to_string());
    order.push(name.to_string());
    Ok(())
}

fn check_function(
    f: &Function,
    ctx: &CheckCtx<'_>,
    constants: &[ConstDecl],
) -> Result<CheckedFunction, Diagnostic> {
    let ret = match &f.return_ty {
        Some(t) => Ty::from_ast_at(t, ctx.type_names, f.span)?,
        None => Ty::Void,
    };
    let mut env: HashMap<String, Ty> = HashMap::new();
    let mut immutable = HashSet::new();
    for c in constants {
        if c.module == ctx.current_module {
            let lit = lit_from_expr(&c.value)?;
            env.insert(c.name.clone(), lit.ty());
            immutable.insert(c.name.clone());
        }
    }
    let mut params = Vec::new();
    for p in &f.params {
        let ty = Ty::from_ast_at(&p.ty, ctx.type_names, p.span)?;
        if env.contains_key(&p.name) {
            return Err(Diagnostic::new(
                format!("duplicate parameter '{}'", p.name),
                p.span,
            ));
        }
        env.insert(p.name.clone(), ty.clone());
        params.push((p.name.clone(), ty));
    }
    let mut owned = Vec::new();
    let body = check_block(
        &f.body.stmts,
        &mut env,
        ctx,
        &ret,
        0,
        &mut owned,
        &mut immutable,
        true,
    )?;
    Ok(CheckedFunction {
        name: f.name.clone(),
        is_async: f.is_async,
        params,
        return_ty: ret,
        body,
        span: f.span,
    })
}

fn check_method(
    class: &ClassDecl,
    m: &MethodDecl,
    ctx: &CheckCtx<'_>,
) -> Result<CheckedMethod, Diagnostic> {
    let ret = match &m.return_ty {
        Some(t) => Ty::from_ast_at(t, ctx.type_names, m.span)?,
        None => Ty::Void,
    };
    let mut env: HashMap<String, Ty> = HashMap::new();
    let mut params = Vec::new();
    for p in &m.params {
        let ty = Ty::from_ast_at(&p.ty, ctx.type_names, p.span)?;
        if env.contains_key(&p.name) || p.name == "self" {
            return Err(Diagnostic::new(
                format!("duplicate or reserved parameter '{}'", p.name),
                p.span,
            ));
        }
        env.insert(p.name.clone(), ty.clone());
        params.push((p.name.clone(), ty));
    }
    let mut owned = Vec::new();
    let mut immutable = HashSet::new();
    let body = check_block(
        &m.body.stmts,
        &mut env,
        ctx,
        &ret,
        0,
        &mut owned,
        &mut immutable,
        true,
    )?;
    Ok(CheckedMethod {
        class_name: class.name.clone(),
        name: m.name.clone(),
        symbol: format!("{}_{}", class.name, m.name),
        is_ctor: m.name == "new",
        params,
        return_ty: ret,
        body,
        span: m.span,
    })
}

/// `owned` tracks (local_name, drop_symbol) for class values owned by this block.
fn check_block(
    stmts: &[Stmt],
    env: &mut HashMap<String, Ty>,
    ctx: &CheckCtx<'_>,
    fn_ret: &Ty,
    loop_depth: usize,
    owned: &mut Vec<(String, String)>,
    immutable: &mut HashSet<String>,
    drop_at_end: bool,
) -> Result<Vec<CheckedStmt>, Diagnostic> {
    let owned_start = owned.len();
    let mut out = Vec::new();
    let mut moved: HashSet<String> = HashSet::new();

    for stmt in stmts {
        // Before reassign of owned local, drop old value
        if let Stmt::Assign {
            target: AssignTarget::Local { name, .. },
            ..
        } = stmt
        {
            if let Some(pos) = owned.iter().position(|(n, _)| n == name) {
                if !moved.contains(name) {
                    let (n, sym) = owned[pos].clone();
                    out.push(CheckedStmt::Drop {
                        object: CheckedExpr::Local {
                            name: n,
                            ty: env.get(name).cloned().unwrap_or(Ty::Void),
                        },
                        symbol: sym,
                    });
                    owned.remove(pos);
                }
            }
        }

        let checked = check_stmt(
            stmt,
            env,
            ctx,
            fn_ret,
            loop_depth,
            owned,
            immutable,
            &mut moved,
        )?;

        // Track new owned locals
        if let CheckedStmt::VarDecl { name, ty, .. } = &checked {
            if let Ty::Class(cname) = ty {
                if let Some(sym) = ctx.classes.get(cname).and_then(|c| c.drop_symbol.clone()) {
                    owned.push((name.clone(), sym));
                }
            }
        }

        // Return transfers ownership of the returned local; drop other owned locals first.
        if let CheckedStmt::Return { value } = &checked {
            if let Some(CheckedExpr::Local { name, .. }) = value {
                moved.insert(name.clone());
                owned.retain(|(n, _)| n != name);
            }
            let to_drop: Vec<_> = owned[owned_start..].iter().rev().cloned().collect();
            for (name, sym) in to_drop {
                let ty = env.get(&name).cloned().unwrap_or(Ty::Void);
                out.push(CheckedStmt::Drop {
                    object: CheckedExpr::Local { name, ty },
                    symbol: sym,
                });
            }
            owned.truncate(owned_start);
            out.push(checked);
            continue;
        }

        out.push(checked);
    }

    if drop_at_end {
        let to_drop: Vec<_> = owned[owned_start..]
            .iter()
            .filter(|(n, _)| !moved.contains(n))
            .cloned()
            .collect();
        for (name, sym) in to_drop.into_iter().rev() {
            let ty = env.get(&name).cloned().unwrap_or(Ty::Void);
            out.push(CheckedStmt::Drop {
                object: CheckedExpr::Local { name, ty },
                symbol: sym,
            });
        }
        owned.truncate(owned_start);
    }

    Ok(out)
}

fn check_stmt(
    stmt: &Stmt,
    env: &mut HashMap<String, Ty>,
    ctx: &CheckCtx<'_>,
    fn_ret: &Ty,
    loop_depth: usize,
    owned: &mut Vec<(String, String)>,
    immutable: &mut HashSet<String>,
    moved: &mut HashSet<String>,
) -> Result<CheckedStmt, Diagnostic> {
    match stmt {
        Stmt::ConstDecl { name, value, span } => {
            if env.contains_key(name) {
                return Err(Diagnostic::new(
                    format!("'{name}' already declared"),
                    *span,
                ));
            }
            let lit = lit_from_expr(value)?;
            let ty = lit.ty();
            env.insert(name.clone(), ty);
            immutable.insert(name.clone());
            Ok(CheckedStmt::VarDecl {
                name: name.clone(),
                ty: lit.ty(),
                init: lit.to_checked(),
            })
        }
        Stmt::Spawn { body, span } => {
            let _ = span;
            let index = ctx.spawn_index.get();
            ctx.spawn_index.set(index + 1);
            let mut spawn_env = env.clone();
            let mut spawn_owned = Vec::new();
            let mut spawn_imm = immutable.clone();
            let body = match body {
                SpawnBody::Block(b) => check_block(
                    &b.stmts,
                    &mut spawn_env,
                    ctx,
                    &Ty::Void,
                    0,
                    &mut spawn_owned,
                    &mut spawn_imm,
                    true,
                )?,
                SpawnBody::Expr(e) => {
                    let (_, ce) = check_expr(e, env, ctx)?;
                    vec![CheckedStmt::Expr { expr: ce }]
                }
            };
            let captures = free_locals(&body);
            Ok(CheckedStmt::Spawn {
                index,
                body,
                captures,
            })
        }
        Stmt::VarDecl {
            name, ty, init, span, ..
        } => {
            if env.contains_key(name) {
                return Err(Diagnostic::new(
                    format!("variable '{name}' already declared"),
                    *span,
                ));
            }
            let (init_ty, init_e) = check_expr(init, env, ctx)?;
            let declared = match ty {
                Some(t) => Some(Ty::from_ast_at(t, ctx.type_names, *span)?),
                None => None,
            };
            let final_ty = match declared {
                Some(t) if !ty_assignable(&init_ty, &t, ctx) => {
                    return Err(Diagnostic::new(
                        format!("type mismatch in var '{name}'"),
                        *span,
                    ));
                }
                Some(t) => t,
                None => init_ty,
            };
            env.insert(name.clone(), final_ty.clone());
            Ok(CheckedStmt::VarDecl {
                name: name.clone(),
                ty: final_ty,
                init: init_e,
            })
        }
        Stmt::Assign {
            target, value, span,
        } => {
            let (target_ty, checked_target) =
                check_assign_target(target, env, ctx, immutable)?;
            let (val_ty, expr) = check_expr(value, env, ctx)?;
            if !ty_assignable(&val_ty, &target_ty, ctx) {
                return Err(Diagnostic::new(
                    format!("cannot assign {:?} to {:?}", val_ty, target_ty),
                    *span,
                ));
            }
            if let CheckedAssignTarget::Local { name } = &checked_target {
                if let Ty::Class(cname) = &val_ty {
                    if let Some(sym) = ctx.classes.get(cname).and_then(|c| c.drop_symbol.clone()) {
                        owned.push((name.clone(), sym));
                        moved.remove(name);
                    }
                }
            }
            Ok(CheckedStmt::Assign {
                target: checked_target,
                value: expr,
            })
        }
        Stmt::Return { value, span } => match (fn_ret, value) {
            (Ty::Void, None) => Ok(CheckedStmt::Return { value: None }),
            (Ty::Void, Some(_)) => Err(Diagnostic::new(
                "cannot return a value from void function",
                *span,
            )),
            (_, None) => Err(Diagnostic::new("missing return value", *span)),
            (expected, Some(v)) => {
                let (ty, expr) = check_expr(v, env, ctx)?;
                if !ty_assignable(&ty, expected, ctx) {
                    return Err(Diagnostic::new(
                        format!("return type mismatch: expected {:?}, got {:?}", expected, ty),
                        *span,
                    ));
                }
                Ok(CheckedStmt::Return { value: Some(expr) })
            }
        },
        Stmt::Expr { expr, .. } => {
            let (_, e) = check_expr(expr, env, ctx)?;
            Ok(CheckedStmt::Expr { expr: e })
        }
        Stmt::If {
            arms,
            else_block,
            ..
        } => {
            let mut checked_arms = Vec::new();
            for (cond, body) in arms {
                let (cty, cexpr) = check_expr(cond, env, ctx)?;
                if cty != Ty::Bool {
                    return Err(Diagnostic::new("if condition must be bool", cond.span()));
                }
                let mut branch_env = env.clone();
                let mut branch_owned = Vec::new();
                let body = check_block(
                    &body.stmts,
                    &mut branch_env,
                    ctx,
                    fn_ret,
                    loop_depth,
                    &mut branch_owned,
                    immutable,
                    true,
                )?;
                checked_arms.push((cexpr, body));
            }
            let else_checked = if let Some(eb) = else_block {
                let mut branch_env = env.clone();
                let mut branch_owned = Vec::new();
                Some(check_block(
                    &eb.stmts,
                    &mut branch_env,
                    ctx,
                    fn_ret,
                    loop_depth,
                    &mut branch_owned,
                    immutable,
                    true,
                )?)
            } else {
                None
            };
            Ok(CheckedStmt::If {
                arms: checked_arms,
                else_block: else_checked,
            })
        }
        Stmt::While { cond, body, .. } => {
            let (cty, cexpr) = check_expr(cond, env, ctx)?;
            if cty != Ty::Bool {
                return Err(Diagnostic::new("while condition must be bool", cond.span()));
            }
            let mut branch_env = env.clone();
            let mut branch_owned = Vec::new();
            let body = check_block(
                &body.stmts,
                &mut branch_env,
                ctx,
                fn_ret,
                loop_depth + 1,
                &mut branch_owned,
                immutable,
                true,
            )?;
            Ok(CheckedStmt::While {
                cond: cexpr,
                body,
            })
        }
        Stmt::ForRange {
            name,
            start,
            end,
            body,
            span,
        } => {
            let (st, se) = check_expr(start, env, ctx)?;
            let (et, ee) = check_expr(end, env, ctx)?;
            if st != Ty::Int || et != Ty::Int {
                return Err(Diagnostic::new("for range bounds must be int", *span));
            }
            let mut branch_env = env.clone();
            branch_env.insert(name.clone(), Ty::Int);
            let mut branch_owned = Vec::new();
            let body = check_block(
                &body.stmts,
                &mut branch_env,
                ctx,
                fn_ret,
                loop_depth + 1,
                &mut branch_owned,
                immutable,
                true,
            )?;
            Ok(CheckedStmt::ForRange {
                name: name.clone(),
                start: se,
                end: ee,
                body,
            })
        }
        Stmt::ForIn {
            name,
            iter,
            body,
            span,
        } => {
            let (ity, ie) = check_expr(iter, env, ctx)?;
            let Ty::Channel { elem } = ity else {
                return Err(Diagnostic::new(
                    "for-in currently supports Channel iterators only",
                    *span,
                ));
            };
            let mut branch_env = env.clone();
            branch_env.insert(name.clone(), (*elem).clone());
            let mut branch_owned = Vec::new();
            let body = check_block(
                &body.stmts,
                &mut branch_env,
                ctx,
                fn_ret,
                loop_depth + 1,
                &mut branch_owned,
                immutable,
                true,
            )?;
            Ok(CheckedStmt::ForIn {
                name: name.clone(),
                iter: ie,
                elem: (*elem).clone(),
                body,
            })
        }
        Stmt::Break { span } => {
            if loop_depth == 0 {
                return Err(Diagnostic::new("'break' outside of loop", *span));
            }
            Ok(CheckedStmt::Break)
        }
        Stmt::Continue { span } => {
            if loop_depth == 0 {
                return Err(Diagnostic::new("'continue' outside of loop", *span));
            }
            Ok(CheckedStmt::Continue)
        }
        Stmt::Match {
            scrutinee,
            arms,
            span,
        } => {
            let (sty, sexpr) = check_expr(scrutinee, env, ctx)?;
            if !matches!(
                sty,
                Ty::Int | Ty::Result { .. } | Ty::Option { .. }
            ) {
                return Err(Diagnostic::new(
                    "match scrutinee must be int, Result, or Option",
                    scrutinee.span(),
                ));
            }
            if arms.is_empty() {
                return Err(Diagnostic::new("match needs at least one arm", *span));
            }
            let mut seen_lits = HashSet::new();
            let mut has_wildcard = false;
            let mut has_ok = false;
            let mut has_err = false;
            let mut has_some = false;
            let mut has_none = false;
            let mut checked_arms = Vec::new();
            for (i, arm) in arms.iter().enumerate() {
                let (pat, bind): (CheckedPattern, Option<(String, Ty)>) = match &arm.pattern {
                    Pattern::Wildcard { .. } => {
                        if i != arms.len() - 1 {
                            return Err(Diagnostic::new(
                                "'_' must be the last match arm",
                                arm.span,
                            ));
                        }
                        has_wildcard = true;
                        (CheckedPattern::Wildcard, None)
                    }
                    Pattern::IntLit { value, .. } => {
                        if sty != Ty::Int {
                            return Err(Diagnostic::new(
                                "int pattern only matches int scrutinee",
                                arm.span,
                            ));
                        }
                        if !seen_lits.insert(*value) {
                            return Err(Diagnostic::new(
                                format!("duplicate match pattern '{value}'"),
                                arm.span,
                            ));
                        }
                        (CheckedPattern::IntLit(*value), None)
                    }
                    Pattern::Ok { name, .. } => {
                        let Ty::Result { ok, .. } = &sty else {
                            return Err(Diagnostic::new(
                                "ok(x) pattern only matches Result",
                                arm.span,
                            ));
                        };
                        has_ok = true;
                        (
                            CheckedPattern::Ok { name: name.clone() },
                            Some((name.clone(), *ok.clone())),
                        )
                    }
                    Pattern::Err { name, .. } => {
                        let Ty::Result { err, .. } = &sty else {
                            return Err(Diagnostic::new(
                                "err(x) pattern only matches Result",
                                arm.span,
                            ));
                        };
                        has_err = true;
                        (
                            CheckedPattern::Err { name: name.clone() },
                            Some((name.clone(), *err.clone())),
                        )
                    }
                    Pattern::Some { name, .. } => {
                        let Ty::Option { inner } = &sty else {
                            return Err(Diagnostic::new(
                                "some(x) pattern only matches Option",
                                arm.span,
                            ));
                        };
                        has_some = true;
                        (
                            CheckedPattern::Some { name: name.clone() },
                            Some((name.clone(), *inner.clone())),
                        )
                    }
                    Pattern::None { .. } => {
                        if !matches!(sty, Ty::Option { .. }) {
                            return Err(Diagnostic::new(
                                "none pattern only matches Option",
                                arm.span,
                            ));
                        }
                        has_none = true;
                        (CheckedPattern::None, None)
                    }
                };
                let mut branch_env = env.clone();
                if let Some((n, t)) = bind {
                    branch_env.insert(n, t);
                }
                let mut branch_owned = Vec::new();
                let body = check_block(
                    &arm.body.stmts,
                    &mut branch_env,
                    ctx,
                    fn_ret,
                    loop_depth,
                    &mut branch_owned,
                    immutable,
                    true,
                )?;
                checked_arms.push((pat, body));
            }
            match &sty {
                Ty::Int => {
                    if !has_wildcard {
                        return Err(Diagnostic::new(
                            "match must end with '_' wildcard arm",
                            *span,
                        ));
                    }
                }
                Ty::Result { .. } => {
                    if !(has_wildcard || (has_ok && has_err)) {
                        return Err(Diagnostic::new(
                            "Result match needs ok(...) and err(...), or '_'",
                            *span,
                        ));
                    }
                }
                Ty::Option { .. } => {
                    if !(has_wildcard || (has_some && has_none)) {
                        return Err(Diagnostic::new(
                            "Option match needs some(...) and none, or '_'",
                            *span,
                        ));
                    }
                }
                _ => {}
            }
            Ok(CheckedStmt::Match {
                scrutinee: sexpr,
                arms: checked_arms,
            })
        }
        Stmt::DoCatch {
            body,
            catch_name,
            catch_body,
            span,
        } => {
            let err_cell = Cell::new(None);
            let do_ctx = CheckCtx {
                sigs: ctx.sigs,
                classes: ctx.classes,
                type_names: ctx.type_names,
                generic_fns: ctx.generic_fns,
                mono: ctx.mono,
                mono_sigs: ctx.mono_sigs,
                has_std: ctx.has_std,
                current_class: ctx.current_class,
                current_module: ctx.current_module,
                in_async: ctx.in_async,
                do_catch: Some(&err_cell),
                const_lits: ctx.const_lits.clone(),
                async_block_index: ctx.async_block_index,
                spawn_index: ctx.spawn_index,
                closure_index: ctx.closure_index,
            };
            let mut body_env = env.clone();
            let mut body_owned = owned.clone();
            let mut body_imm = immutable.clone();
            let checked_body = check_block(
                &body.stmts,
                &mut body_env,
                &do_ctx,
                fn_ret,
                loop_depth,
                &mut body_owned,
                &mut body_imm,
                false,
            )?;
            let catch_ty = err_cell.into_inner().unwrap_or(Ty::String);
            let mut catch_env = env.clone();
            if catch_env.contains_key(catch_name) {
                return Err(Diagnostic::new(
                    format!("'{catch_name}' already declared"),
                    *span,
                ));
            }
            catch_env.insert(catch_name.clone(), catch_ty.clone());
            let mut catch_owned = owned.clone();
            let mut catch_imm = immutable.clone();
            let checked_catch = check_block(
                &catch_body.stmts,
                &mut catch_env,
                ctx,
                fn_ret,
                loop_depth,
                &mut catch_owned,
                &mut catch_imm,
                false,
            )?;
            Ok(CheckedStmt::DoCatch {
                body: checked_body,
                catch_name: catch_name.clone(),
                catch_ty,
                catch_body: checked_catch,
            })
        }
    }
}

fn check_assign_target(
    target: &AssignTarget,
    env: &HashMap<String, Ty>,
    ctx: &CheckCtx<'_>,
    immutable: &HashSet<String>,
) -> Result<(Ty, CheckedAssignTarget), Diagnostic> {
    match target {
        AssignTarget::Local { name, span } => {
            if immutable.contains(name) {
                return Err(Diagnostic::new(
                    format!("cannot assign to const '{name}'"),
                    *span,
                ));
            }
            let Some(ty) = env.get(name).cloned() else {
                return Err(Diagnostic::new(
                    format!("undeclared variable '{name}'"),
                    *span,
                ));
            };
            Ok((
                ty,
                CheckedAssignTarget::Local {
                    name: name.clone(),
                },
            ))
        }
        AssignTarget::Index {
            array,
            index,
            span,
        } => {
            let (aty, aexpr) = check_expr(array, env, ctx)?;
            let Ty::Array { elem, len } = aty else {
                return Err(Diagnostic::new("index assign requires an array", *span));
            };
            let (ity, iexpr) = check_expr(index, env, ctx)?;
            if ity != Ty::Int {
                return Err(Diagnostic::new("array index must be int", *span));
            }
            Ok((
                (*elem).clone(),
                CheckedAssignTarget::Index {
                    array: aexpr,
                    index: iexpr,
                    elem: (*elem).clone(),
                    len,
                },
            ))
        }
        AssignTarget::Field {
            object,
            field,
            span,
        } => {
            let (oty, oexpr) = check_expr(object, env, ctx)?;
            let Ty::Class(ref cname) = oty else {
                return Err(Diagnostic::new(
                    "field assignment requires a class instance",
                    *span,
                ));
            };
            let cinfo = ctx.classes.get(cname).ok_or_else(|| {
                Diagnostic::new(format!("unknown class '{cname}'"), *span)
            })?;
            if let Some(prop) = cinfo.props.get(field) {
                check_prop_vis(ctx, prop, *span)?;
                let Some(sym) = &prop.setter_symbol else {
                    return Err(Diagnostic::new(
                        format!("property '{field}' has no setter"),
                        *span,
                    ));
                };
                return Ok((
                    prop.ty.clone(),
                    CheckedAssignTarget::Setter {
                        object: oexpr,
                        symbol: sym.clone(),
                    },
                ));
            }
            let finfo = lookup_field(ctx, cname, field, *span)?;
            check_field_vis(ctx, &finfo, *span)?;
            Ok((
                finfo.ty.clone(),
                CheckedAssignTarget::Field {
                    object: oexpr,
                    offset: finfo.offset,
                    ty: finfo.ty.clone(),
                },
            ))
        }
    }
}

fn check_expr(
    expr: &Expr,
    env: &HashMap<String, Ty>,
    ctx: &CheckCtx<'_>,
) -> Result<(Ty, CheckedExpr), Diagnostic> {
    match expr {
        Expr::IntLit { value, .. } => Ok((Ty::Int, CheckedExpr::IntLit(*value))),
        Expr::FloatLit { value, .. } => Ok((Ty::Float, CheckedExpr::FloatLit(*value))),
        Expr::StringLit { value, .. } => Ok((Ty::String, CheckedExpr::StringLit(value.clone()))),
        Expr::BoolLit { value, .. } => Ok((Ty::Bool, CheckedExpr::BoolLit(*value))),
        Expr::SelfExpr { span } => {
            let Some(cname) = ctx.current_class else {
                return Err(Diagnostic::new(
                    "'self' is only valid inside class methods",
                    *span,
                ));
            };
            Ok((
                Ty::Class(cname.to_string()),
                CheckedExpr::SelfExpr {
                    class: cname.to_string(),
                },
            ))
        }
        Expr::SuperField { base, field, span } => {
            let self_e = CheckedExpr::SelfExpr {
                class: ctx
                    .current_class
                    .ok_or_else(|| {
                        Diagnostic::new("'super' is only valid inside class methods", *span)
                    })?
                    .to_string(),
            };
            let (ty, offset, _) = resolve_super_field(ctx, base.as_deref(), field, *span)?;
            Ok((
                ty.clone(),
                CheckedExpr::FieldGet {
                    object: Box::new(self_e),
                    offset,
                    ty,
                },
            ))
        }
        Expr::SuperMethod {
            base,
            method,
            args,
            span,
        } => {
            let cname = ctx.current_class.ok_or_else(|| {
                Diagnostic::new("'super' is only valid inside class methods", *span)
            })?;
            let minfo = resolve_super_method(ctx, base.as_deref(), method, *span)?;
            let args = fill_args(
                args,
                &minfo.params,
                &minfo.param_defaults,
                env,
                ctx,
                &format!("super.{method}"),
                *span,
            )?;
            Ok((
                minfo.ret.clone(),
                CheckedExpr::MethodCall {
                    object: Box::new(CheckedExpr::SelfExpr {
                        class: cname.to_string(),
                    }),
                    symbol: minfo.symbol.clone(),
                    args,
                    ret: minfo.ret.clone(),
                },
            ))
        }
        Expr::Ident { name, span } => {
            if let Some(lit) = ctx.const_lits.get(name) {
                return Ok((lit.ty(), lit.to_checked()));
            }
            let Some(ty) = env.get(name) else {
                return Err(Diagnostic::new(
                    format!("undeclared variable '{name}'"),
                    *span,
                ));
            };
            Ok((
                ty.clone(),
                CheckedExpr::Local {
                    name: name.clone(),
                    ty: ty.clone(),
                },
            ))
        }
        Expr::Group { expr, .. } => check_expr(expr, env, ctx),
        Expr::New {
            class_name,
            args,
            span,
        } => {
            let Some(cinfo) = ctx.classes.get(class_name) else {
                return Err(Diagnostic::new(
                    format!("unknown class '{class_name}'"),
                    *span,
                ));
            };
            if cinfo.module != ctx.current_module && !cinfo.is_pub {
                return Err(Diagnostic::new(
                    format!("class '{class_name}' is private to its module"),
                    *span,
                ));
            }
            let Some(ctor) = cinfo.methods.get("new") else {
                return Err(Diagnostic::new(
                    format!("class '{class_name}' has no constructor 'new'"),
                    *span,
                ));
            };
            let checked_args = fill_args(
                args,
                &ctor.params,
                &ctor.param_defaults,
                env,
                ctx,
                &format!("new {class_name}"),
                *span,
            )?;
            Ok((
                Ty::Class(class_name.clone()),
                CheckedExpr::New {
                    class: class_name.clone(),
                    size: cinfo.size,
                    ctor_symbol: ctor.symbol.clone(),
                    args: checked_args,
                },
            ))
        }
        Expr::ArrayLit { elems, span } => {
            if elems.is_empty() {
                return Err(Diagnostic::new("array literal cannot be empty", *span));
            }
            let mut checked = Vec::new();
            let mut elem_ty = None;
            for e in elems {
                let (ty, ce) = check_expr(e, env, ctx)?;
                if !is_value_ty(&ty) {
                    return Err(Diagnostic::new(
                        "array elements cannot be void",
                        e.span(),
                    ));
                }
                if let Some(ref et) = elem_ty {
                    if &ty != et {
                        return Err(Diagnostic::new(
                            "array elements must share one type",
                            e.span(),
                        ));
                    }
                } else {
                    elem_ty = Some(ty);
                }
                checked.push(ce);
            }
            let elem_ty = elem_ty.unwrap();
            let len = checked.len() as i64;
            Ok((
                Ty::Array {
                    elem: Box::new(elem_ty.clone()),
                    len,
                },
                CheckedExpr::ArrayLit {
                    elems: checked,
                    elem_ty,
                    len,
                },
            ))
        }
        Expr::Index {
            array,
            index,
            span,
        } => {
            let (aty, aexpr) = check_expr(array, env, ctx)?;
            let Ty::Array { elem, len } = aty else {
                return Err(Diagnostic::new("index requires an array", *span));
            };
            let (ity, iexpr) = check_expr(index, env, ctx)?;
            if ity != Ty::Int {
                return Err(Diagnostic::new("array index must be int", *span));
            }
            Ok((
                (*elem).clone(),
                CheckedExpr::Index {
                    array: Box::new(aexpr),
                    index: Box::new(iexpr),
                    elem: (*elem).clone(),
                    len,
                },
            ))
        }
        Expr::Await { expr, span } => {
            if !ctx.in_async {
                return Err(Diagnostic::new(
                    "'await' is only valid inside async fn or async block",
                    *span,
                ));
            }
            let (ty, e) = check_expr(expr, env, ctx)?;
            let Ty::Future(inner) = ty else {
                return Err(Diagnostic::new("'await' requires a Future", *span));
            };
            Ok((
                (*inner).clone(),
                CheckedExpr::Await {
                    expr: Box::new(e),
                    inner: (*inner).clone(),
                },
            ))
        }
        Expr::Try { mode, expr, span } => {
            let (ty, e) = check_expr(expr, env, ctx)?;
            let Ty::Result { ok, err } = ty else {
                return Err(Diagnostic::new(
                    "'try' requires a Result value",
                    *span,
                ));
            };
            match mode {
                TryMode::Unwrap => {
                    let Some(cell) = ctx.do_catch else {
                        return Err(Diagnostic::new(
                            "'try' (unwrap) is only valid inside a do { … } catch block; use try? or try!",
                            *span,
                        ));
                    };
                    match cell.take() {
                        None => cell.set(Some((*err).clone())),
                        Some(existing) => {
                            if existing != *err {
                                return Err(Diagnostic::new(
                                    format!(
                                        "do/catch error type mismatch: expected {}, found {}",
                                        existing.mangle_name(),
                                        err.mangle_name()
                                    ),
                                    *span,
                                ));
                            }
                            cell.set(Some(existing));
                        }
                    }
                    Ok((
                        (*ok).clone(),
                        CheckedExpr::Try {
                            mode: *mode,
                            expr: Box::new(e),
                            ok_ty: (*ok).clone(),
                            err_ty: (*err).clone(),
                        },
                    ))
                }
                TryMode::Option => Ok((
                    ty_option((*ok).clone()),
                    CheckedExpr::Try {
                        mode: *mode,
                        expr: Box::new(e),
                        ok_ty: (*ok).clone(),
                        err_ty: (*err).clone(),
                    },
                )),
                TryMode::Force => {
                    if !ctx.has_std {
                        return Err(Diagnostic::new(
                            "try! requires @import \"std\" (uses std.panic)",
                            *span,
                        ));
                    }
                    Ok((
                        (*ok).clone(),
                        CheckedExpr::Try {
                            mode: *mode,
                            expr: Box::new(e),
                            ok_ty: (*ok).clone(),
                            err_ty: (*err).clone(),
                        },
                    ))
                }
            }
        }
        Expr::AsyncBlock { body, span } => {
            let index = ctx.async_block_index.get();
            ctx.async_block_index.set(index + 1);
            let block_ctx = CheckCtx {
                sigs: ctx.sigs,
                classes: ctx.classes,
                type_names: ctx.type_names,
                generic_fns: ctx.generic_fns,
                mono: ctx.mono,
                mono_sigs: ctx.mono_sigs,
                has_std: ctx.has_std,
                current_class: ctx.current_class,
                current_module: ctx.current_module,
                in_async: true,
                do_catch: None,
                const_lits: ctx.const_lits.clone(),
                async_block_index: ctx.async_block_index,
                spawn_index: ctx.spawn_index,
                closure_index: ctx.closure_index,
            };
            let mut block_env = env.clone();
            let mut owned = Vec::new();
            let mut imm = HashSet::new();
            let checked_body = check_block(
                &body.stmts,
                &mut block_env,
                &block_ctx,
                &Ty::Int,
                0,
                &mut owned,
                &mut imm,
                true,
            )
            .map_err(|d| {
                if d.message.contains("return") {
                    Diagnostic::new(
                        format!("async block: {} (MVP requires return int)", d.message),
                        *span,
                    )
                } else {
                    d
                }
            })?;
            let captures = free_locals(&checked_body);
            Ok((
                Ty::Future(Box::new(Ty::Int)),
                CheckedExpr::AsyncBlock {
                    index,
                    body: checked_body,
                    captures,
                },
            ))
        }
        Expr::Closure {
            params,
            return_ty,
            body,
            span,
        } => {
            let index = ctx.closure_index.get();
            ctx.closure_index.set(index + 1);

            let mut param_tys = Vec::new();
            let mut param_names = Vec::new();
            for p in params {
                if p.default.is_some() {
                    return Err(Diagnostic::new(
                        "closure parameters cannot have defaults in MVP",
                        p.span,
                    ));
                }
                let pty = Ty::from_ast_at(&p.ty, ctx.type_names, p.span)?;
                if !is_value_ty(&pty) {
                    return Err(Diagnostic::new(
                        "closure parameter cannot be void",
                        p.span,
                    ));
                }
                param_names.push(p.name.clone());
                param_tys.push((p.name.clone(), pty));
            }
            let ret = match return_ty {
                Some(t) => Ty::from_ast_at(t, ctx.type_names, *span)?,
                None => Ty::Void,
            };

            let block_ctx = CheckCtx {
                sigs: ctx.sigs,
                classes: ctx.classes,
                type_names: ctx.type_names,
                generic_fns: ctx.generic_fns,
                mono: ctx.mono,
                mono_sigs: ctx.mono_sigs,
                has_std: ctx.has_std,
                current_class: ctx.current_class,
                current_module: ctx.current_module,
                in_async: false,
                do_catch: None,
                const_lits: ctx.const_lits.clone(),
                async_block_index: ctx.async_block_index,
                spawn_index: ctx.spawn_index,
                closure_index: ctx.closure_index,
            };
            let mut block_env = env.clone();
            for (n, t) in &param_tys {
                block_env.insert(n.clone(), t.clone());
            }
            let mut owned = Vec::new();
            let mut imm = HashSet::new();
            let checked_body = check_block(
                &body.stmts,
                &mut block_env,
                &block_ctx,
                &ret,
                0,
                &mut owned,
                &mut imm,
                true,
            )?;
            let param_set: HashSet<String> = param_names.into_iter().collect();
            let captures: Vec<String> = free_locals(&checked_body)
                .into_iter()
                .filter(|n| !param_set.contains(n))
                .collect();
            let fn_ty = Ty::Fn {
                params: param_tys.iter().map(|(_, t)| t.clone()).collect(),
                ret: Box::new(ret.clone()),
            };
            Ok((
                fn_ty,
                CheckedExpr::Closure {
                    index,
                    params: param_tys,
                    ret,
                    body: checked_body,
                    captures,
                },
            ))
        }
        Expr::FieldGet {
            object,
            field,
            span,
        } => {
            let (oty, oexpr) = check_expr(object, env, ctx)?;
            if let Ty::Array { len, .. } = &oty {
                if field == "len" {
                    return Ok((Ty::Int, CheckedExpr::IntLit(*len)));
                }
                return Err(Diagnostic::new(
                    "arrays only support .len",
                    *span,
                ));
            }
            let Ty::Class(ref cname) = oty else {
                return Err(Diagnostic::new(
                    "field access requires a class instance",
                    *span,
                ));
            };
            let cinfo = ctx.classes.get(cname).ok_or_else(|| {
                Diagnostic::new(format!("unknown class '{cname}'"), *span)
            })?;
            if let Some(prop) = cinfo.props.get(field) {
                check_prop_vis(ctx, prop, *span)?;
                let Some(sym) = &prop.getter_symbol else {
                    return Err(Diagnostic::new(
                        format!("property '{field}' has no getter"),
                        *span,
                    ));
                };
                return Ok((
                    prop.ty.clone(),
                    CheckedExpr::MethodCall {
                        object: Box::new(oexpr),
                        symbol: sym.clone(),
                        args: vec![],
                        ret: prop.ty.clone(),
                    },
                ));
            }
            let finfo = lookup_field(ctx, cname, field, *span)?;
            check_field_vis(ctx, &finfo, *span)?;
            Ok((
                finfo.ty.clone(),
                CheckedExpr::FieldGet {
                    object: Box::new(oexpr),
                    offset: finfo.offset,
                    ty: finfo.ty.clone(),
                },
            ))
        }
        Expr::MethodCall {
            object,
            method,
            args,
            span,
        } => {
            let (oty, oexpr) = check_expr(object, env, ctx)?;
            match oty {
                Ty::Channel { elem } => match method.as_str() {
                    "send" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                "Channel.send expects one argument",
                                *span,
                            ));
                        }
                        let (vty, ve) = check_expr(&args[0], env, ctx)?;
                        if !ty_assignable(&vty, elem.as_ref(), ctx) {
                            return Err(Diagnostic::new(
                                format!("Channel.send argument must be {:?}", elem),
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::ChannelSend {
                                channel: Box::new(oexpr),
                                value: Box::new(ve),
                            },
                        ))
                    }
                    "recv" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "Channel.recv takes no arguments",
                                *span,
                            ));
                        }
                        if ctx.in_async {
                            Ok((
                                Ty::Future(elem.clone()),
                                CheckedExpr::ChannelRecvFuture {
                                    channel: Box::new(oexpr),
                                },
                            ))
                        } else {
                            Ok((
                                (*elem).clone(),
                                CheckedExpr::ChannelRecv {
                                    channel: Box::new(oexpr),
                                },
                            ))
                        }
                    }
                    "close" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "Channel.close takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::ChannelClose {
                                channel: Box::new(oexpr),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown Channel method '{method}'"),
                        *span,
                    )),
                },
                Ty::WaitGroup => match method.as_str() {
                    "add" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                "WaitGroup.add expects one int argument",
                                *span,
                            ));
                        }
                        let (dty, de) = check_expr(&args[0], env, ctx)?;
                        if dty != Ty::Int {
                            return Err(Diagnostic::new(
                                "WaitGroup.add argument must be int",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::WaitGroupAdd {
                                wg: Box::new(oexpr),
                                delta: Box::new(de),
                            },
                        ))
                    }
                    "done" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "WaitGroup.done takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::WaitGroupDone {
                                wg: Box::new(oexpr),
                            },
                        ))
                    }
                    "wait" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "WaitGroup.wait takes no arguments",
                                *span,
                            ));
                        }
                        if ctx.in_async {
                            Ok((
                                Ty::Future(Box::new(Ty::Void)),
                                CheckedExpr::WaitGroupWaitFuture {
                                    wg: Box::new(oexpr),
                                },
                            ))
                        } else {
                            Ok((
                                Ty::Void,
                                CheckedExpr::WaitGroupWait {
                                    wg: Box::new(oexpr),
                                },
                            ))
                        }
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown WaitGroup method '{method}'"),
                        *span,
                    )),
                },
                Ty::Mutex { ref elem } => match method.as_str() {
                    "lock" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "Mutex.lock takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::MutexLock {
                                mutex: Box::new(oexpr),
                            },
                        ))
                    }
                    "unlock" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "Mutex.unlock takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::MutexUnlock {
                                mutex: Box::new(oexpr),
                            },
                        ))
                    }
                    "get" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "Mutex.get takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            *elem.clone(),
                            CheckedExpr::MutexGet {
                                mutex: Box::new(oexpr),
                            },
                        ))
                    }
                    "set" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                "Mutex.set expects one argument",
                                *span,
                            ));
                        }
                        let (vty, ve) = check_expr(&args[0], env, ctx)?;
                        if !ty_assignable(&vty, elem.as_ref(), ctx) {
                            return Err(Diagnostic::new(
                                "Mutex.set argument type mismatch",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::MutexSet {
                                mutex: Box::new(oexpr),
                                value: Box::new(ve),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown Mutex method '{method}'"),
                        *span,
                    )),
                },
                Ty::RwLock { ref elem } => match method.as_str() {
                    "readLock" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "RwLock.readLock takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::RwLockReadLock {
                                lock: Box::new(oexpr),
                            },
                        ))
                    }
                    "readUnlock" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "RwLock.readUnlock takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::RwLockReadUnlock {
                                lock: Box::new(oexpr),
                            },
                        ))
                    }
                    "writeLock" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "RwLock.writeLock takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::RwLockWriteLock {
                                lock: Box::new(oexpr),
                            },
                        ))
                    }
                    "writeUnlock" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "RwLock.writeUnlock takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::RwLockWriteUnlock {
                                lock: Box::new(oexpr),
                            },
                        ))
                    }
                    "get" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "RwLock.get takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            *elem.clone(),
                            CheckedExpr::RwLockGet {
                                lock: Box::new(oexpr),
                            },
                        ))
                    }
                    "set" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                "RwLock.set expects one argument",
                                *span,
                            ));
                        }
                        let (vty, ve) = check_expr(&args[0], env, ctx)?;
                        if !ty_assignable(&vty, elem.as_ref(), ctx) {
                            return Err(Diagnostic::new(
                                "RwLock.set argument type mismatch",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::RwLockSet {
                                lock: Box::new(oexpr),
                                value: Box::new(ve),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown RwLock method '{method}'"),
                        *span,
                    )),
                },
                Ty::CancelToken => match method.as_str() {
                    "cancel" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "CancellationToken.cancel takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::CancelTokenCancel {
                                token: Box::new(oexpr),
                            },
                        ))
                    }
                    "isCancelled" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "CancellationToken.isCancelled takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Bool,
                            CheckedExpr::CancelTokenIsCancelled {
                                token: Box::new(oexpr),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown CancellationToken method '{method}'"),
                        *span,
                    )),
                },
                Ty::List { ref elem } => match method.as_str() {
                    "push" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new("List.push expects one argument", *span));
                        }
                        let (vty, ve) = check_expr(&args[0], env, ctx)?;
                        if !ty_assignable(&vty, elem.as_ref(), ctx) {
                            return Err(Diagnostic::new(
                                "List.push argument type mismatch",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::ListPush {
                                list: Box::new(oexpr),
                                value: Box::new(ve),
                            },
                        ))
                    }
                    "get" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new("List.get expects one int index", *span));
                        }
                        let (ity, ie) = check_expr(&args[0], env, ctx)?;
                        if ity != Ty::Int {
                            return Err(Diagnostic::new(
                                "List.get index must be int",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            *elem.clone(),
                            CheckedExpr::ListGet {
                                list: Box::new(oexpr),
                                index: Box::new(ie),
                                elem: *elem.clone(),
                            },
                        ))
                    }
                    "set" => {
                        if args.len() != 2 {
                            return Err(Diagnostic::new(
                                "List.set expects (index, value)",
                                *span,
                            ));
                        }
                        let (ity, ie) = check_expr(&args[0], env, ctx)?;
                        if ity != Ty::Int {
                            return Err(Diagnostic::new(
                                "List.set index must be int",
                                args[0].span(),
                            ));
                        }
                        let (vty, ve) = check_expr(&args[1], env, ctx)?;
                        if !ty_assignable(&vty, elem.as_ref(), ctx) {
                            return Err(Diagnostic::new(
                                "List.set value type mismatch",
                                args[1].span(),
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::ListSet {
                                list: Box::new(oexpr),
                                index: Box::new(ie),
                                value: Box::new(ve),
                            },
                        ))
                    }
                    "len" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new("List.len takes no arguments", *span));
                        }
                        Ok((
                            Ty::Int,
                            CheckedExpr::ListLen {
                                list: Box::new(oexpr),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown List method '{method}'"),
                        *span,
                    )),
                },
                Ty::HttpHeaders => match method.as_str() {
                    "set" => {
                        if args.len() != 2 {
                            return Err(Diagnostic::new(
                                "Headers.set expects (key, value)",
                                *span,
                            ));
                        }
                        let (kt, ke) = check_expr(&args[0], env, ctx)?;
                        let (vt, ve) = check_expr(&args[1], env, ctx)?;
                        if kt != Ty::String || vt != Ty::String {
                            return Err(Diagnostic::new(
                                "Headers.set requires string key and value",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::HttpHeadersSet {
                                headers: Box::new(oexpr),
                                key: Box::new(ke),
                                value: Box::new(ve),
                            },
                        ))
                    }
                    "get" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new("Headers.get expects one string", *span));
                        }
                        let (kt, ke) = check_expr(&args[0], env, ctx)?;
                        if kt != Ty::String {
                            return Err(Diagnostic::new(
                                "Headers.get key must be string",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Option {
                                inner: Box::new(Ty::String),
                            },
                            CheckedExpr::HttpHeadersGet {
                                headers: Box::new(oexpr),
                                key: Box::new(ke),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown Headers method '{method}'"),
                        *span,
                    )),
                },
                Ty::HttpRequest => match method.as_str() {
                    "method" | "path" | "body" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                format!("Request.{method} takes no arguments"),
                                *span,
                            ));
                        }
                        let ce = match method.as_str() {
                            "method" => CheckedExpr::HttpRequestMethod {
                                request: Box::new(oexpr),
                            },
                            "path" => CheckedExpr::HttpRequestPath {
                                request: Box::new(oexpr),
                            },
                            _ => CheckedExpr::HttpRequestBody {
                                request: Box::new(oexpr),
                            },
                        };
                        Ok((Ty::String, ce))
                    }
                    "query" | "header" | "param" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                format!("Request.{method} expects one string"),
                                *span,
                            ));
                        }
                        let (nt, ne) = check_expr(&args[0], env, ctx)?;
                        if nt != Ty::String {
                            return Err(Diagnostic::new(
                                format!("Request.{method} name must be string"),
                                args[0].span(),
                            ));
                        }
                        match method.as_str() {
                            "param" => Ok((
                                Ty::String,
                                CheckedExpr::HttpRequestParam {
                                    request: Box::new(oexpr),
                                    name: Box::new(ne),
                                },
                            )),
                            "query" => Ok((
                                Ty::Option {
                                    inner: Box::new(Ty::String),
                                },
                                CheckedExpr::HttpRequestQuery {
                                    request: Box::new(oexpr),
                                    name: Box::new(ne),
                                },
                            )),
                            _ => Ok((
                                Ty::Option {
                                    inner: Box::new(Ty::String),
                                },
                                CheckedExpr::HttpRequestHeader {
                                    request: Box::new(oexpr),
                                    name: Box::new(ne),
                                },
                            )),
                        }
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown Request method '{method}'"),
                        *span,
                    )),
                },
                Ty::HttpResponse => match method.as_str() {
                    "status" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "Response.status takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Int,
                            CheckedExpr::HttpResponseStatus {
                                response: Box::new(oexpr),
                            },
                        ))
                    }
                    "body" => {
                        if !args.is_empty() {
                            return Err(Diagnostic::new(
                                "Response.body takes no arguments",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::String,
                            CheckedExpr::HttpResponseBody {
                                response: Box::new(oexpr),
                            },
                        ))
                    }
                    "setHeader" => {
                        if args.len() != 2 {
                            return Err(Diagnostic::new(
                                "Response.setHeader expects (key, value)",
                                *span,
                            ));
                        }
                        let (kt, ke) = check_expr(&args[0], env, ctx)?;
                        let (vt, ve) = check_expr(&args[1], env, ctx)?;
                        if kt != Ty::String || vt != Ty::String {
                            return Err(Diagnostic::new(
                                "Response.setHeader requires strings",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::HttpResponseSetHeader {
                                response: Box::new(oexpr),
                                key: Box::new(ke),
                                value: Box::new(ve),
                            },
                        ))
                    }
                    "header" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                "Response.header expects one string",
                                *span,
                            ));
                        }
                        let (kt, ke) = check_expr(&args[0], env, ctx)?;
                        if kt != Ty::String {
                            return Err(Diagnostic::new(
                                "Response.header key must be string",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Option {
                                inner: Box::new(Ty::String),
                            },
                            CheckedExpr::HttpResponseHeader {
                                response: Box::new(oexpr),
                                key: Box::new(ke),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown Response method '{method}'"),
                        *span,
                    )),
                },
                Ty::HttpServer => match method.as_str() {
                    "get" | "post" | "put" | "delete" | "patch" => {
                        if args.len() != 2 {
                            return Err(Diagnostic::new(
                                format!("Server.{method} expects (path, handler)"),
                                *span,
                            ));
                        }
                        let (pty, pe) = check_expr(&args[0], env, ctx)?;
                        if pty != Ty::String {
                            return Err(Diagnostic::new(
                                "route path must be string",
                                args[0].span(),
                            ));
                        }
                        let (hty, he) = check_expr(&args[1], env, ctx)?;
                        let handler_ty = Ty::Fn {
                            params: vec![Ty::HttpRequest],
                            ret: Box::new(Ty::HttpResponse),
                        };
                        if !ty_assignable(&hty, &handler_ty, ctx) {
                            return Err(Diagnostic::new(
                                "handler must be fn(Request) Response",
                                args[1].span(),
                            ));
                        }
                        Ok((
                            Ty::Void,
                            CheckedExpr::HttpServerRoute {
                                server: Box::new(oexpr),
                                method: method.to_ascii_uppercase(),
                                path: Box::new(pe),
                                handler: Box::new(he),
                            },
                        ))
                    }
                    "listen" => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                "Server.listen expects one port int",
                                *span,
                            ));
                        }
                        let (pty, pe) = check_expr(&args[0], env, ctx)?;
                        if pty != Ty::Int {
                            return Err(Diagnostic::new(
                                "listen port must be int",
                                args[0].span(),
                            ));
                        }
                        Ok((
                            Ty::Future(Box::new(ty_result(Ty::Int, Ty::String))),
                            CheckedExpr::HttpServerListen {
                                server: Box::new(oexpr),
                                port: Box::new(pe),
                            },
                        ))
                    }
                    _ => Err(Diagnostic::new(
                        format!("unknown Server method '{method}'"),
                        *span,
                    )),
                },
                Ty::Class(ref cname) => {
                    let minfo = lookup_method(ctx, cname, method, *span)?;
                    check_method_vis(ctx, &minfo, *span)?;
                    let checked_args = fill_args(
                        args,
                        &minfo.params,
                        &minfo.param_defaults,
                        env,
                        ctx,
                        &format!("{cname}.{method}"),
                        *span,
                    )?;
                    Ok((
                        minfo.ret.clone(),
                        CheckedExpr::MethodCall {
                            object: Box::new(oexpr),
                            symbol: minfo.symbol.clone(),
                            args: checked_args,
                            ret: minfo.ret.clone(),
                        },
                    ))
                }
                _ => Err(Diagnostic::new(
                    "method call requires a class instance or std handle type",
                    *span,
                )),
            }
        }
        Expr::Unary { op, expr, span } => {
            let (ty, e) = check_expr(expr, env, ctx)?;
            match op {
                UnOp::Not => {
                    if ty != Ty::Bool {
                        return Err(Diagnostic::new("'!' requires bool operand", *span));
                    }
                    Ok((
                        Ty::Bool,
                        CheckedExpr::Unary {
                            op: *op,
                            expr: Box::new(e),
                            ty: Ty::Bool,
                        },
                    ))
                }
                UnOp::Neg => {
                    if ty == Ty::Int {
                        Ok((
                            Ty::Int,
                            CheckedExpr::Unary {
                                op: *op,
                                expr: Box::new(e),
                                ty: Ty::Int,
                            },
                        ))
                    } else if ty == Ty::Float {
                        Ok((
                            Ty::Float,
                            CheckedExpr::Unary {
                                op: *op,
                                expr: Box::new(e),
                                ty: Ty::Float,
                            },
                        ))
                    } else {
                        Err(Diagnostic::new(
                            "unary '-' requires int or float operand",
                            *span,
                        ))
                    }
                }
            }
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
            ..
        } => {
            let (lt, le) = check_expr(left, env, ctx)?;
            let (rt, re) = check_expr(right, env, ctx)?;
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                    // No implicit int→float conversion: both operands must share a flavor.
                    if lt == Ty::Float && rt == Ty::Float {
                        if *op == BinOp::Rem {
                            return Err(Diagnostic::new(
                                "'%' is not supported for float operands",
                                *span,
                            ));
                        }
                        return Ok((
                            Ty::Float,
                            CheckedExpr::Binary {
                                op: *op,
                                left: Box::new(le),
                                right: Box::new(re),
                                ty: Ty::Float,
                                operand_ty: Ty::Float,
                            },
                        ));
                    }
                    if lt != Ty::Int || rt != Ty::Int {
                        if matches!(lt, Ty::Int | Ty::Float) && matches!(rt, Ty::Int | Ty::Float) {
                            return Err(Diagnostic::new(
                                "arithmetic operands must both be int or both be float (no implicit conversion)",
                                *span,
                            ));
                        }
                        return Err(Diagnostic::new(
                            "arithmetic operators require int or float operands",
                            *span,
                        ));
                    }
                    Ok((
                        Ty::Int,
                        CheckedExpr::Binary {
                            op: *op,
                            left: Box::new(le),
                            right: Box::new(re),
                            ty: Ty::Int,
                            operand_ty: Ty::Int,
                        },
                    ))
                }
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    if matches!(lt, Ty::Int | Ty::Float)
                        && matches!(rt, Ty::Int | Ty::Float)
                        && lt != rt
                    {
                        return Err(Diagnostic::new(
                            "comparison operands must both be int or both be float (no implicit conversion)",
                            *span,
                        ));
                    }
                    let operand_ty = if lt == Ty::Int && rt == Ty::Int {
                        Ty::Int
                    } else if lt == Ty::Float && rt == Ty::Float {
                        Ty::Float
                    } else if lt == Ty::String
                        && rt == Ty::String
                        && matches!(op, BinOp::Eq | BinOp::Ne)
                    {
                        Ty::String
                    } else if lt == Ty::Bool
                        && rt == Ty::Bool
                        && matches!(op, BinOp::Eq | BinOp::Ne)
                    {
                        Ty::Bool
                    } else {
                        return Err(Diagnostic::new(
                            "comparison operators require matching numeric, string, or bool operands",
                            *span,
                        ));
                    };
                    Ok((
                        Ty::Bool,
                        CheckedExpr::Binary {
                            op: *op,
                            left: Box::new(le),
                            right: Box::new(re),
                            ty: Ty::Bool,
                            operand_ty,
                        },
                    ))
                }
                BinOp::And | BinOp::Or => {
                    if lt != Ty::Bool || rt != Ty::Bool {
                        return Err(Diagnostic::new(
                            "logical operators require bool operands",
                            *span,
                        ));
                    }
                    Ok((
                        Ty::Bool,
                        CheckedExpr::Binary {
                            op: *op,
                            left: Box::new(le),
                            right: Box::new(re),
                            ty: Ty::Bool,
                            operand_ty: Ty::Bool,
                        },
                    ))
                }
            }
        }
        Expr::Call { callee, args, span } => match callee {
            Callee::StdLog { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.log requires @import \"std\"",
                        *span,
                    ));
                }
                if args.is_empty() {
                    return Err(Diagnostic::new(
                        "std.log requires a format string",
                        *span,
                    ));
                }
                let mut checked_args = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let (ty, e) = check_expr(a, env, ctx)?;
                    if i == 0 && ty != Ty::String {
                        return Err(Diagnostic::new(
                            "std.log first argument must be a string",
                            a.span(),
                        ));
                    }
                    if i > 0
                        && ty != Ty::Int
                        && ty != Ty::String
                        && ty != Ty::Bool
                        && ty != Ty::Float
                    {
                        return Err(Diagnostic::new(
                            "std.log arguments must be int, float, string, or bool",
                            a.span(),
                        ));
                    }
                    checked_args.push(e);
                }
                Ok((
                    Ty::Void,
                    CheckedExpr::StdLog {
                        args: checked_args,
                    },
                ))
            }
            Callee::StdSleep { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.sleep requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "std.sleep expects one int argument (milliseconds)",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::Int {
                    return Err(Diagnostic::new(
                        "std.sleep argument must be int",
                        args[0].span(),
                    ));
                }
                Ok((
                    Ty::Void,
                    CheckedExpr::StdSleep { ms: Box::new(e) },
                ))
            }
            Callee::StdResultOk { ok, err, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.Result requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new("Result.ok expects one value", *span));
                }
                let ok_ty = Ty::from_ast_at(ok, ctx.type_names, *span)?;
                let err_ty = Ty::from_ast_at(err, ctx.type_names, *span)?;
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if !ty_assignable(&ty, &ok_ty, ctx) {
                    return Err(Diagnostic::new(
                        "Result.ok value type mismatch",
                        args[0].span(),
                    ));
                }
                Ok((
                    ty_result(ok_ty, err_ty),
                    CheckedExpr::ResultOk {
                        value: Box::new(e),
                    },
                ))
            }
            Callee::StdResultErr { ok, err, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.Result requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new("Result.err expects one value", *span));
                }
                let ok_ty = Ty::from_ast_at(ok, ctx.type_names, *span)?;
                let err_ty = Ty::from_ast_at(err, ctx.type_names, *span)?;
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if !ty_assignable(&ty, &err_ty, ctx) {
                    return Err(Diagnostic::new(
                        "Result.err value type mismatch",
                        args[0].span(),
                    ));
                }
                Ok((
                    ty_result(ok_ty, err_ty),
                    CheckedExpr::ResultErr {
                        value: Box::new(e),
                    },
                ))
            }
            Callee::StdOptionSome { inner, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.Option requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new("Option.some expects one value", *span));
                }
                let inner_ty = Ty::from_ast_at(inner, ctx.type_names, *span)?;
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != inner_ty {
                    return Err(Diagnostic::new(
                        "Option.some value type mismatch",
                        args[0].span(),
                    ));
                }
                Ok((
                    ty_option(inner_ty),
                    CheckedExpr::OptionSome {
                        value: Box::new(e),
                    },
                ))
            }
            Callee::StdOptionNone { inner, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.Option requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new("Option.none takes no arguments", *span));
                }
                let inner_ty = Ty::from_ast_at(inner, ctx.type_names, *span)?;
                Ok((ty_option(inner_ty), CheckedExpr::OptionNone))
            }
            Callee::StdCpuSubmit { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.cpu.submit requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "std.cpu.submit expects one fn() int (name or closure)",
                        *span,
                    ));
                }
                // Named global: bare Ident that is a sync fn() int
                if let Expr::Ident { name, span: nspan } = &args[0] {
                    if env.get(name).is_none() {
                        let Some(sig) = ctx.sigs.get(name) else {
                            return Err(Diagnostic::new(
                                format!("unknown function '{name}'"),
                                *nspan,
                            ));
                        };
                        if sig.is_async {
                            return Err(Diagnostic::new(
                                "std.cpu.submit requires a sync fn() T (not async)",
                                *nspan,
                            ));
                        }
                        if !sig.params.is_empty() {
                            return Err(Diagnostic::new(
                                "std.cpu.submit requires fn() T (zero parameters)",
                                *nspan,
                            ));
                        }
                        if !is_value_ty(&sig.ret) {
                            return Err(Diagnostic::new(
                                "std.cpu.submit requires fn() T where T is a value type",
                                *nspan,
                            ));
                        }
                        return Ok((
                            Ty::Future(Box::new(sig.ret.clone())),
                            CheckedExpr::CpuSubmitNamed {
                                fn_name: name.clone(),
                            },
                        ));
                    }
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                let Ty::Fn { params, ret } = ty else {
                    return Err(Diagnostic::new(
                        "std.cpu.submit expects fn() T (name or closure)",
                        args[0].span(),
                    ));
                };
                if !params.is_empty() || !is_value_ty(&ret) {
                    return Err(Diagnostic::new(
                        "std.cpu.submit requires fn() T where T is a value type",
                        args[0].span(),
                    ));
                }
                Ok((
                    Ty::Future(ret),
                    CheckedExpr::CpuSubmitClosure {
                        closure: Box::new(e),
                    },
                ))
            }
            Callee::Value { expr } => {
                let (ty, callee) = check_expr(expr, env, ctx)?;
                let Ty::Fn { params, ret } = ty else {
                    return Err(Diagnostic::new(
                        "only function values can be called",
                        expr.span(),
                    ));
                };
                if args.len() != params.len() {
                    return Err(Diagnostic::new(
                        format!(
                            "closure expects {} argument(s), got {}",
                            params.len(),
                            args.len()
                        ),
                        *span,
                    ));
                }
                let mut checked_args = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let (aty, ae) = check_expr(a, env, ctx)?;
                    if aty != params[i] {
                        return Err(Diagnostic::new(
                            format!("argument type mismatch: expected {:?}", params[i]),
                            a.span(),
                        ));
                    }
                    checked_args.push(ae);
                }
                Ok((
                    (*ret).clone(),
                    CheckedExpr::CallClosure {
                        callee: Box::new(callee),
                        args: checked_args,
                        ret: (*ret).clone(),
                    },
                ))
            }
            Callee::StdChannelNew { elem, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.sync.Channel requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new(
                        "Channel.new() takes no arguments",
                        *span,
                    ));
                }
                let elem_ty = Ty::from_ast_at(elem, ctx.type_names, *span)?;
                Ok((
                    Ty::Channel {
                        elem: Box::new(elem_ty),
                    },
                    CheckedExpr::ChannelNew,
                ))
            }
            Callee::StdChannelBuffered { elem, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.sync.Channel requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "Channel.buffered expects one int capacity",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::Int {
                    return Err(Diagnostic::new(
                        "Channel.buffered capacity must be int",
                        args[0].span(),
                    ));
                }
                let elem_ty = Ty::from_ast_at(elem, ctx.type_names, *span)?;
                Ok((
                    Ty::Channel {
                        elem: Box::new(elem_ty),
                    },
                    CheckedExpr::ChannelBuffered {
                        capacity: Box::new(e),
                    },
                ))
            }
            Callee::StdWaitGroupNew { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.sync.WaitGroup requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new(
                        "WaitGroup.new() takes no arguments",
                        *span,
                    ));
                }
                Ok((Ty::WaitGroup, CheckedExpr::WaitGroupNew))
            }
            Callee::StdMutexNew { elem, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.sync.Mutex requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "Mutex.new expects one initial value",
                        *span,
                    ));
                }
                let elem_ty = Ty::from_ast_at(elem, ctx.type_names, *span)?;
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != elem_ty {
                    return Err(Diagnostic::new(
                        "Mutex.new initial value type mismatch",
                        args[0].span(),
                    ));
                }
                Ok((
                    Ty::Mutex {
                        elem: Box::new(elem_ty),
                    },
                    CheckedExpr::MutexNew {
                        initial: Box::new(e),
                    },
                ))
            }
            Callee::FutureJoin { .. } => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "Future.join expects two Future<T> arguments",
                        *span,
                    ));
                }
                let (lt, le) = check_expr(&args[0], env, ctx)?;
                let (rt, re) = check_expr(&args[1], env, ctx)?;
                let (Ty::Future(a), Ty::Future(b)) = (&lt, &rt) else {
                    return Err(Diagnostic::new(
                        "Future.join requires two Future values",
                        *span,
                    ));
                };
                if a != b || !is_value_ty(a) {
                    return Err(Diagnostic::new(
                        "Future.join requires two Future<T> with the same value type T",
                        *span,
                    ));
                }
                let elem = *a.clone();
                let ret = Ty::Future(Box::new(Ty::Array {
                    elem: Box::new(elem),
                    len: 2,
                }));
                Ok((
                    ret,
                    CheckedExpr::FutureJoin {
                        left: Box::new(le),
                        right: Box::new(re),
                    },
                ))
            }
            Callee::FutureRace { .. } => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "Future.race expects two Future<T> arguments",
                        *span,
                    ));
                }
                let (lt, le) = check_expr(&args[0], env, ctx)?;
                let (rt, re) = check_expr(&args[1], env, ctx)?;
                let (Ty::Future(a), Ty::Future(b)) = (&lt, &rt) else {
                    return Err(Diagnostic::new(
                        "Future.race requires two Future values",
                        *span,
                    ));
                };
                if a != b || !is_value_ty(a) {
                    return Err(Diagnostic::new(
                        "Future.race requires two Future<T> with the same value type T",
                        *span,
                    ));
                }
                Ok((
                    Ty::Future(a.clone()),
                    CheckedExpr::FutureRace {
                        left: Box::new(le),
                        right: Box::new(re),
                    },
                ))
            }
            Callee::FutureReady { .. } => {
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "Future.ready expects one argument",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if !is_value_ty(&ty) {
                    return Err(Diagnostic::new(
                        "Future.ready requires a value type (not void)",
                        args[0].span(),
                    ));
                }
                Ok((
                    Ty::Future(Box::new(ty)),
                    CheckedExpr::FutureReady {
                        value: Box::new(e),
                    },
                ))
            }
            Callee::StdPanic { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.panic requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new("std.panic expects one string", *span));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::String {
                    return Err(Diagnostic::new(
                        "std.panic message must be string",
                        args[0].span(),
                    ));
                }
                Ok((Ty::Void, CheckedExpr::StdPanic { msg: Box::new(e) }))
            }
            Callee::StdEnvArgs { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.env requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new("std.env.args takes no arguments", *span));
                }
                Ok((
                    Ty::List {
                        elem: Box::new(Ty::String),
                    },
                    CheckedExpr::StdEnvArgs,
                ))
            }
            Callee::StdEnvGet { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.env requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new("std.env.get expects one string", *span));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::String {
                    return Err(Diagnostic::new(
                        "std.env.get name must be string",
                        args[0].span(),
                    ));
                }
                Ok((
                    ty_option(Ty::String),
                    CheckedExpr::StdEnvGet { name: Box::new(e) },
                ))
            }
            Callee::StdEnvSet { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.env requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "std.env.set expects (name, value)",
                        *span,
                    ));
                }
                let (nty, ne) = check_expr(&args[0], env, ctx)?;
                let (vty, ve) = check_expr(&args[1], env, ctx)?;
                if nty != Ty::String || vty != Ty::String {
                    return Err(Diagnostic::new(
                        "std.env.set expects two strings",
                        *span,
                    ));
                }
                Ok((
                    Ty::Void,
                    CheckedExpr::StdEnvSet {
                        name: Box::new(ne),
                        value: Box::new(ve),
                    },
                ))
            }
            Callee::StdProcessExit { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.process requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "std.process.exit expects one int",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::Int {
                    return Err(Diagnostic::new(
                        "std.process.exit code must be int",
                        args[0].span(),
                    ));
                }
                Ok((
                    Ty::Void,
                    CheckedExpr::StdProcessExit { code: Box::new(e) },
                ))
            }
            Callee::StdFsReadToString { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.fs requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "std.fs.readToString expects one path string",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::String {
                    return Err(Diagnostic::new(
                        "std.fs.readToString path must be string",
                        args[0].span(),
                    ));
                }
                Ok((
                    ty_result(Ty::String, Ty::String),
                    CheckedExpr::StdFsReadToString { path: Box::new(e) },
                ))
            }
            Callee::StdFsWriteString { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.fs requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "std.fs.writeString expects (path, contents)",
                        *span,
                    ));
                }
                let (pty, pe) = check_expr(&args[0], env, ctx)?;
                let (cty, ce) = check_expr(&args[1], env, ctx)?;
                if pty != Ty::String || cty != Ty::String {
                    return Err(Diagnostic::new(
                        "std.fs.writeString expects two strings",
                        *span,
                    ));
                }
                Ok((
                    ty_result(Ty::Int, Ty::String),
                    CheckedExpr::StdFsWriteString {
                        path: Box::new(pe),
                        contents: Box::new(ce),
                    },
                ))
            }
            Callee::StdTimeNowMs { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.time requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new("std.time.nowMs takes no arguments", *span));
                }
                Ok((Ty::Int, CheckedExpr::StdTimeNowMs))
            }
            Callee::StdStringLen { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.string requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new("std.string.len expects one string", *span));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::String {
                    return Err(Diagnostic::new(
                        "std.string.len expects string",
                        args[0].span(),
                    ));
                }
                Ok((Ty::Int, CheckedExpr::StdStringLen { s: Box::new(e) }))
            }
            Callee::StdStringConcat { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.string requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "std.string.concat expects two strings",
                        *span,
                    ));
                }
                let (a, ae) = check_expr(&args[0], env, ctx)?;
                let (b, be) = check_expr(&args[1], env, ctx)?;
                if a != Ty::String || b != Ty::String {
                    return Err(Diagnostic::new(
                        "std.string.concat expects two strings",
                        *span,
                    ));
                }
                Ok((
                    Ty::String,
                    CheckedExpr::StdStringConcat {
                        a: Box::new(ae),
                        b: Box::new(be),
                    },
                ))
            }
            Callee::StdStringSlice { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.string requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 3 {
                    return Err(Diagnostic::new(
                        "std.string.slice expects (s, start, end)",
                        *span,
                    ));
                }
                let (st, se) = check_expr(&args[0], env, ctx)?;
                let (a, ae) = check_expr(&args[1], env, ctx)?;
                let (b, be) = check_expr(&args[2], env, ctx)?;
                if st != Ty::String || a != Ty::Int || b != Ty::Int {
                    return Err(Diagnostic::new(
                        "std.string.slice expects (string, int, int)",
                        *span,
                    ));
                }
                Ok((
                    Ty::String,
                    CheckedExpr::StdStringSlice {
                        s: Box::new(se),
                        start: Box::new(ae),
                        end: Box::new(be),
                    },
                ))
            }
            Callee::StdStringContains { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.string requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "std.string.contains expects two strings",
                        *span,
                    ));
                }
                let (a, ae) = check_expr(&args[0], env, ctx)?;
                let (b, be) = check_expr(&args[1], env, ctx)?;
                if a != Ty::String || b != Ty::String {
                    return Err(Diagnostic::new(
                        "std.string.contains expects two strings",
                        *span,
                    ));
                }
                Ok((
                    Ty::Bool,
                    CheckedExpr::StdStringContains {
                        hay: Box::new(ae),
                        needle: Box::new(be),
                    },
                ))
            }
            Callee::StdStringFromInt { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.string requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "std.string.fromInt expects one int",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::Int {
                    return Err(Diagnostic::new(
                        "std.string.fromInt expects int",
                        args[0].span(),
                    ));
                }
                Ok((
                    Ty::String,
                    CheckedExpr::StdStringFromInt { n: Box::new(e) },
                ))
            }
            Callee::StdStringParseInt { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.string requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "std.string.parseInt expects one string",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::String {
                    return Err(Diagnostic::new(
                        "std.string.parseInt expects string",
                        args[0].span(),
                    ));
                }
                Ok((
                    ty_result(Ty::Int, Ty::String),
                    CheckedExpr::StdStringParseInt { s: Box::new(e) },
                ))
            }
            Callee::StdListNew { elem, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.List requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new("List.new takes no arguments", *span));
                }
                let elem_ty = Ty::from_ast_at(elem, ctx.type_names, *span)?;
                if !is_value_ty(&elem_ty) {
                    return Err(Diagnostic::new(
                        "List element type cannot be void",
                        *span,
                    ));
                }
                Ok((
                    Ty::List {
                        elem: Box::new(elem_ty),
                    },
                    CheckedExpr::ListNew,
                ))
            }
            Callee::StdRwLockNew { elem, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.sync.RwLock requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new("RwLock.new expects one argument", *span));
                }
                let elem_ty = Ty::from_ast_at(elem, ctx.type_names, *span)?;
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != elem_ty {
                    return Err(Diagnostic::new(
                        "RwLock.new initial value type mismatch",
                        args[0].span(),
                    ));
                }
                Ok((
                    Ty::RwLock {
                        elem: Box::new(elem_ty),
                    },
                    CheckedExpr::RwLockNew {
                        initial: Box::new(e),
                    },
                ))
            }
            Callee::StdParallelMap { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.parallel requires @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "std.parallel.map expects (List<int>, fnName)",
                        *span,
                    ));
                }
                let (lty, le) = check_expr(&args[0], env, ctx)?;
                let Ty::List { elem } = &lty else {
                    return Err(Diagnostic::new(
                        "std.parallel.map first arg must be List<int>",
                        args[0].span(),
                    ));
                };
                if **elem != Ty::Int {
                    return Err(Diagnostic::new(
                        "std.parallel.map requires List<int>",
                        args[0].span(),
                    ));
                }
                let Expr::Ident { name, span: nspan } = &args[1] else {
                    return Err(Diagnostic::new(
                        "std.parallel.map MVP expects a bare function name",
                        args[1].span(),
                    ));
                };
                let Some(sig) = ctx.sigs.get(name) else {
                    return Err(Diagnostic::new(
                        format!("unknown function '{name}'"),
                        *nspan,
                    ));
                };
                if sig.params != vec![Ty::Int] || sig.ret != Ty::Int || sig.is_async {
                    return Err(Diagnostic::new(
                        "std.parallel.map requires sync fn(int) int",
                        *nspan,
                    ));
                }
                Ok((
                    Ty::List {
                        elem: Box::new(Ty::Int),
                    },
                    CheckedExpr::ParallelMap {
                        list: Box::new(le),
                        fn_name: name.clone(),
                    },
                ))
            }
            Callee::StdHttpClient { method, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.http requires @import \"std\"",
                        *span,
                    ));
                }
                let needs_body = matches!(
                    method,
                    HttpClientMethod::Post | HttpClientMethod::Put | HttpClientMethod::Patch
                );
                let (url_e, body_e, headers_e) = if needs_body {
                    if args.len() < 2 || args.len() > 3 {
                        return Err(Diagnostic::new(
                            "std.http post/put/patch expects (url, body) or (url, body, Headers)",
                            *span,
                        ));
                    }
                    let (uty, ue) = check_expr(&args[0], env, ctx)?;
                    let (bty, be) = check_expr(&args[1], env, ctx)?;
                    if uty != Ty::String || bty != Ty::String {
                        return Err(Diagnostic::new(
                            "url and body must be strings",
                            *span,
                        ));
                    }
                    let headers = if args.len() == 3 {
                        let (hty, he) = check_expr(&args[2], env, ctx)?;
                        if hty != Ty::HttpHeaders {
                            return Err(Diagnostic::new(
                                "third argument must be Headers",
                                args[2].span(),
                            ));
                        }
                        Some(Box::new(he))
                    } else {
                        None
                    };
                    (ue, Some(Box::new(be)), headers)
                } else {
                    if args.is_empty() || args.len() > 2 {
                        return Err(Diagnostic::new(
                            "std.http get/delete expects (url) or (url, Headers)",
                            *span,
                        ));
                    }
                    let (uty, ue) = check_expr(&args[0], env, ctx)?;
                    if uty != Ty::String {
                        return Err(Diagnostic::new("url must be string", args[0].span()));
                    }
                    let headers = if args.len() == 2 {
                        let (hty, he) = check_expr(&args[1], env, ctx)?;
                        if hty != Ty::HttpHeaders {
                            return Err(Diagnostic::new(
                                "second argument must be Headers",
                                args[1].span(),
                            ));
                        }
                        Some(Box::new(he))
                    } else {
                        None
                    };
                    (ue, None, headers)
                };
                Ok((
                    Ty::Future(Box::new(ty_result(Ty::HttpResponse, Ty::String))),
                    CheckedExpr::HttpClient {
                        method: *method,
                        url: Box::new(url_e),
                        body: body_e,
                        headers: headers_e,
                    },
                ))
            }
            Callee::StdHttpHeadersNew { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.http requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new("Headers.new takes no arguments", *span));
                }
                Ok((Ty::HttpHeaders, CheckedExpr::HttpHeadersNew))
            }
            Callee::StdHttpResponseNew { kind, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.http requires @import \"std\"",
                        *span,
                    ));
                }
                match kind {
                    HttpResponseKind::Empty => {
                        if args.len() != 1 {
                            return Err(Diagnostic::new(
                                "Response.empty expects (status)",
                                *span,
                            ));
                        }
                        let (sty, se) = check_expr(&args[0], env, ctx)?;
                        if sty != Ty::Int {
                            return Err(Diagnostic::new("status must be int", args[0].span()));
                        }
                        Ok((
                            Ty::HttpResponse,
                            CheckedExpr::HttpResponseNew {
                                kind: *kind,
                                status: Box::new(se),
                                body: None,
                            },
                        ))
                    }
                    HttpResponseKind::Text | HttpResponseKind::Json => {
                        if args.len() != 2 {
                            return Err(Diagnostic::new(
                                "Response.text/json expects (status, body)",
                                *span,
                            ));
                        }
                        let (sty, se) = check_expr(&args[0], env, ctx)?;
                        let (bty, be) = check_expr(&args[1], env, ctx)?;
                        if sty != Ty::Int || bty != Ty::String {
                            return Err(Diagnostic::new(
                                "status must be int and body string",
                                *span,
                            ));
                        }
                        Ok((
                            Ty::HttpResponse,
                            CheckedExpr::HttpResponseNew {
                                kind: *kind,
                                status: Box::new(se),
                                body: Some(Box::new(be)),
                            },
                        ))
                    }
                }
            }
            Callee::StdHttpServerNew { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.http requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new("Server.new takes no arguments", *span));
                }
                Ok((Ty::HttpServer, CheckedExpr::HttpServerNew))
            }
            Callee::StdTaskYield { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.task requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new("std.task.yield takes no arguments", *span));
                }
                Ok((Ty::Void, CheckedExpr::TaskYield))
            }
            Callee::StdCancelTokenNew { .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "std.task requires @import \"std\"",
                        *span,
                    ));
                }
                if !args.is_empty() {
                    return Err(Diagnostic::new(
                        "CancellationToken.new takes no arguments",
                        *span,
                    ));
                }
                Ok((Ty::CancelToken, CheckedExpr::CancelTokenNew))
            }
            Callee::StdSerdeEncode { format, .. } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "serde APIs require @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "encode expects one value argument",
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if !is_serializable_ty(&ty) {
                    return Err(Diagnostic::new(
                        format!("type {:?} is not serializable", ty),
                        args[0].span(),
                    ));
                }
                let schema = build_serde_schema(&ty, ctx.classes, *span)?;
                Ok((
                    Ty::String,
                    CheckedExpr::SerdeEncode {
                        format: *format,
                        value: Box::new(e),
                        schema,
                    },
                ))
            }
            Callee::StdSerdeDecode {
                format,
                type_arg,
                ..
            } => {
                if !ctx.has_std {
                    return Err(Diagnostic::new(
                        "serde APIs require @import \"std\"",
                        *span,
                    ));
                }
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "decode expects one string argument",
                        *span,
                    ));
                }
                let Some(ta) = type_arg else {
                    return Err(Diagnostic::new(
                        "decode requires an explicit type argument: decode<T>(…)",
                        *span,
                    ));
                };
                let target = Ty::from_ast_at(ta, ctx.type_names, *span)?;
                if !is_serializable_ty(&target) {
                    return Err(Diagnostic::new(
                        format!("type {:?} is not serializable", target),
                        *span,
                    ));
                }
                let (ty, e) = check_expr(&args[0], env, ctx)?;
                if ty != Ty::String {
                    return Err(Diagnostic::new(
                        "decode expects a string",
                        args[0].span(),
                    ));
                }
                let schema = build_serde_schema(&target, ctx.classes, *span)?;
                Ok((
                    ty_result(target.clone(), Ty::String),
                    CheckedExpr::SerdeDecode {
                        format: *format,
                        text: Box::new(e),
                        schema,
                        ty: target,
                    },
                ))
            }
            Callee::Func { name, type_args: _, .. } => {
                // Local / captured function value shadows global name.
                if let Some(Ty::Fn { params, ret }) = env.get(name).cloned() {
                    if args.len() != params.len() {
                        return Err(Diagnostic::new(
                            format!(
                                "closure expects {} argument(s), got {}",
                                params.len(),
                                args.len()
                            ),
                            *span,
                        ));
                    }
                    let mut checked_args = Vec::new();
                    for (i, a) in args.iter().enumerate() {
                        let (aty, ae) = check_expr(a, env, ctx)?;
                        if aty != params[i] {
                            return Err(Diagnostic::new(
                                format!("argument type mismatch: expected {:?}", params[i]),
                                a.span(),
                            ));
                        }
                        checked_args.push(ae);
                    }
                    return Ok((
                        (*ret).clone(),
                        CheckedExpr::CallClosure {
                            callee: Box::new(CheckedExpr::Local {
                                name: name.clone(),
                                ty: Ty::Fn {
                                    params: params.clone(),
                                    ret: ret.clone(),
                                },
                            }),
                            args: checked_args,
                            ret: (*ret).clone(),
                        },
                    ));
                }
                // Concrete or already-monomorphized function
                let existing = ctx.sigs.get(name).cloned().or_else(|| {
                    ctx.mono_sigs.borrow().get(name).cloned()
                });
                let (call_name, sig) = if let Some(s) = existing {
                    (name.clone(), s)
                } else if ctx.generic_fns.contains_key(name) {
                    specialize_generic_call(name, args, env, ctx, *span)?
                } else {
                    return Err(Diagnostic::new(
                        format!("unknown function '{name}'"),
                        *span,
                    ));
                };
                if sig.module != ctx.current_module && !sig.is_pub {
                    return Err(Diagnostic::new(
                        format!("function '{name}' is private to its module"),
                        *span,
                    ));
                }
                let checked_args = fill_args(
                    args,
                    &sig.params,
                    &sig.defaults,
                    env,
                    ctx,
                    &format!("function '{name}'"),
                    *span,
                )?;
                if sig.is_async {
                    Ok((
                        Ty::Future(Box::new(sig.ret.clone())),
                        CheckedExpr::Call {
                            name: call_name,
                            args: checked_args,
                            ret: sig.ret.clone(),
                            async_spawn: true,
                        },
                    ))
                } else {
                    Ok((
                        sig.ret.clone(),
                        CheckedExpr::Call {
                            name: call_name,
                            args: checked_args,
                            ret: sig.ret.clone(),
                            async_spawn: false,
                        },
                    ))
                }
            }
        },
    }
}

fn resolve_super_method<'a>(
    ctx: &'a CheckCtx<'a>,
    base: Option<&str>,
    method: &str,
    span: Span,
) -> Result<&'a MethodInfo, Diagnostic> {
    let cname = ctx
        .current_class
        .ok_or_else(|| Diagnostic::new("'super' outside method", span))?;
    let cinfo = ctx
        .classes
        .get(cname)
        .ok_or_else(|| Diagnostic::new("internal: missing class", span))?;

    if let Some(b) = base {
        if !cinfo.bases.iter().any(|x| x == b) {
            return Err(Diagnostic::new(
                format!("'{b}' is not a direct base of '{cname}'"),
                span,
            ));
        }
        let binfo = ctx.classes.get(b).unwrap();
        return binfo.methods.get(method).ok_or_else(|| {
            Diagnostic::new(format!("base '{b}' has no method '{method}'"), span)
        });
    }

    let mut found: Vec<&MethodInfo> = Vec::new();
    for b in &cinfo.bases {
        if let Some(m) = ctx.classes.get(b).and_then(|bi| bi.methods.get(method)) {
            if !found.iter().any(|x| x.symbol == m.symbol) {
                found.push(m);
            }
        }
    }
    match found.len() {
        0 => Err(Diagnostic::new(
            format!("no super method '{method}'"),
            span,
        )),
        1 => Ok(found[0]),
        _ => Err(Diagnostic::new(
            format!(
                "ambiguous super.{method}; use super.Base.{method}"
            ),
            span,
        )),
    }
}

fn resolve_super_field(
    ctx: &CheckCtx<'_>,
    base: Option<&str>,
    field: &str,
    span: Span,
) -> Result<(Ty, i64, Visibility), Diagnostic> {
    let cname = ctx
        .current_class
        .ok_or_else(|| Diagnostic::new("'super' outside method", span))?;
    let cinfo = ctx.classes.get(cname).unwrap();

    if let Some(b) = base {
        if !cinfo.bases.iter().any(|x| x == b) {
            return Err(Diagnostic::new(
                format!("'{b}' is not a direct base of '{cname}'"),
                span,
            ));
        }
        // Field offsets in child already include base offset
        let finfo = lookup_field(ctx, cname, field, span)?;
        if finfo.defining_class != b
            && !is_subclass(ctx, &finfo.defining_class, b)
            && finfo.defining_class != *b
        {
            // allow if field comes from that base's layout
        }
        let binfo = ctx.classes.get(b).unwrap();
        if let Some(f) = binfo.fields.get(field) {
            // Recompute offset in child: find field in child fields
            let child_f = cinfo.fields.get(field).ok_or_else(|| {
                Diagnostic::new(format!("field '{field}' not in layout"), span)
            })?;
            return Ok((child_f.ty.clone(), child_f.offset, f.vis));
        }
        return Err(Diagnostic::new(
            format!("base '{b}' has no field '{field}'"),
            span,
        ));
    }

    let mut found = Vec::new();
    for b in &cinfo.bases {
        if let Some(f) = ctx.classes.get(b).and_then(|bi| bi.fields.get(field)) {
            found.push((b.as_str(), f));
        }
    }
    match found.len() {
        0 => Err(Diagnostic::new(format!("no super field '{field}'"), span)),
        1 => {
            let child_f = cinfo.fields.get(field).unwrap();
            Ok((child_f.ty.clone(), child_f.offset, found[0].1.vis))
        }
        _ => Err(Diagnostic::new(
            format!("ambiguous super.{field}; use super.Base.{field}"),
            span,
        )),
    }
}

fn lookup_field<'a>(
    ctx: &'a CheckCtx<'a>,
    class: &str,
    field: &str,
    span: Span,
) -> Result<&'a FieldInfo, Diagnostic> {
    let Some(cinfo) = ctx.classes.get(class) else {
        return Err(Diagnostic::new(format!("unknown class '{class}'"), span));
    };
    cinfo.fields.get(field).ok_or_else(|| {
        Diagnostic::new(format!("class '{class}' has no field '{field}'"), span)
    })
}

fn lookup_method<'a>(
    ctx: &'a CheckCtx<'a>,
    class: &str,
    method: &str,
    span: Span,
) -> Result<&'a MethodInfo, Diagnostic> {
    let Some(cinfo) = ctx.classes.get(class) else {
        return Err(Diagnostic::new(format!("unknown class '{class}'"), span));
    };
    cinfo.methods.get(method).ok_or_else(|| {
        Diagnostic::new(
            format!("class '{class}' has no method '{method}'"),
            span,
        )
    })
}


fn parse_field_serde_attrs(
    attrs: &[Attribute],
    span: Span,
) -> Result<(Option<String>, bool), Diagnostic> {
    let mut serde_name = None;
    let mut serde_ignore = false;
    for a in attrs {
        match a.name.as_str() {
            "ignore" => {
                if !a.args.is_empty() {
                    return Err(Diagnostic::new("@ignore takes no arguments", a.span));
                }
                if serde_name.is_some() {
                    return Err(Diagnostic::new(
                        "cannot combine @ignore and @encodeProperty",
                        a.span,
                    ));
                }
                serde_ignore = true;
            }
            "encodeProperty" => {
                if a.args.len() != 1 {
                    return Err(Diagnostic::new(
                        "@encodeProperty expects one string argument",
                        a.span,
                    ));
                }
                let Expr::StringLit { value, .. } = &a.args[0] else {
                    return Err(Diagnostic::new(
                        "@encodeProperty argument must be a string literal",
                        a.span,
                    ));
                };
                if serde_ignore {
                    return Err(Diagnostic::new(
                        "cannot combine @ignore and @encodeProperty",
                        a.span,
                    ));
                }
                serde_name = Some(value.clone());
            }
            other => {
                return Err(Diagnostic::new(
                    format!("unknown decorator '@{other}'"),
                    a.span,
                ));
            }
        }
    }
    let _ = span;
    Ok((serde_name, serde_ignore))
}

fn is_serializable_ty(ty: &Ty) -> bool {
    match ty {
        Ty::Int | Ty::Float | Ty::String | Ty::Bool => true,
        Ty::Class(_) => true,
        Ty::List { elem } | Ty::Option { inner: elem } => is_serializable_ty(elem),
        _ => false,
    }
}

/// Compact schema for runtime serde (see runtime `stk_serde_*`).
fn build_serde_schema(
    ty: &Ty,
    classes: &HashMap<String, ClassInfo>,
    span: Span,
) -> Result<String, Diagnostic> {
    match ty {
        Ty::Int => Ok("i".into()),
        Ty::Float => Ok("f".into()),
        Ty::String => Ok("s".into()),
        Ty::Bool => Ok("b".into()),
        Ty::Option { inner } => Ok(format!("o({})", build_serde_schema(inner, classes, span)?)),
        Ty::List { elem } => Ok(format!("L({})", build_serde_schema(elem, classes, span)?)),
        Ty::Class(name) => {
            let Some(cinfo) = classes.get(name) else {
                return Err(Diagnostic::new(format!("unknown class '{name}'"), span));
            };
            let mut parts = Vec::new();
            let mut fields: Vec<_> = cinfo.fields.iter().collect();
            fields.sort_by_key(|(_, f)| f.offset);
            for (fname, finfo) in fields {
                if finfo.serde_ignore {
                    continue;
                }
                if !is_serializable_ty(&finfo.ty) {
                    return Err(Diagnostic::new(
                        format!(
                            "field '{fname}' of type {:?} is not serializable (use @ignore)",
                            finfo.ty
                        ),
                        span,
                    ));
                }
                let wire = finfo
                    .serde_name
                    .clone()
                    .unwrap_or_else(|| fname.clone());
                let sub = build_serde_schema(&finfo.ty, classes, span)?;
                parts.push(format!("{wire}:{sub}:{}", finfo.offset));
            }
            for (pname, pinfo) in &cinfo.props {
                if pinfo.serde_ignore {
                    continue;
                }
                return Err(Diagnostic::new(
                    format!(
                        "property '{pname}' cannot be serialized yet (use a storage field)"
                    ),
                    span,
                ));
            }
            Ok(format!("C{}({})", cinfo.size, parts.join(",")))
        }
        other => Err(Diagnostic::new(
            format!("type {:?} is not serializable", other),
            span,
        )),
    }
}

fn is_subclass(ctx: &CheckCtx<'_>, child: &str, ancestor: &str) -> bool {
    if child == ancestor {
        return true;
    }
    let mut stack = vec![child];
    let mut seen = HashSet::new();
    while let Some(cur) = stack.pop() {
        if !seen.insert(cur.to_string()) {
            continue;
        }
        let Some(cinfo) = ctx.classes.get(cur) else {
            continue;
        };
        for b in &cinfo.bases {
            if b == ancestor {
                return true;
            }
            stack.push(b.as_str());
        }
    }
    false
}

#[inline]
fn implements_iface(ctx: &CheckCtx<'_>, class: &str, iface: &str) -> bool {
    let Some(cinfo) = ctx.classes.get(class) else {
        return false;
    };
    if cinfo.interfaces.iter().any(|i| i == iface) {
        return true;
    }
    for b in &cinfo.bases {
        if implements_iface(ctx, b, iface) {
            return true;
        }
    }
    false
}

/// Nominal assignability: exact match, subclass → base, class → implemented iclass.
#[inline]
fn ty_assignable(got: &Ty, expected: &Ty, ctx: &CheckCtx<'_>) -> bool {
    if got == expected {
        return true;
    }
    match (got, expected) {
        (Ty::Class(c), Ty::Class(e)) => is_subclass(ctx, c, e),
        (Ty::Class(c), Ty::Interface(i)) => implements_iface(ctx, c, i),
        _ => false,
    }
}

fn ty_to_type_name(t: &Ty) -> Result<TypeName, String> {
    Ok(match t {
        Ty::Int => TypeName::Int,
        Ty::Float => TypeName::Float,
        Ty::String => TypeName::String,
        Ty::Bool => TypeName::Bool,
        Ty::Void => TypeName::Void,
        Ty::Class(n) | Ty::Interface(n) => TypeName::Class(n.clone()),
        Ty::Array { elem, len } => TypeName::Array {
            elem: Box::new(ty_to_type_name(elem)?),
            len: *len,
        },
        Ty::Future(inner) => TypeName::Future(Box::new(ty_to_type_name(inner)?)),
        Ty::Channel { elem } => TypeName::Channel(Box::new(ty_to_type_name(elem)?)),
        Ty::WaitGroup => TypeName::WaitGroup,
        Ty::Mutex { elem } => TypeName::Mutex(Box::new(ty_to_type_name(elem)?)),
        Ty::RwLock { elem } => TypeName::RwLock(Box::new(ty_to_type_name(elem)?)),
        Ty::Result { ok, err } => TypeName::Result {
            ok: Box::new(ty_to_type_name(ok)?),
            err: Box::new(ty_to_type_name(err)?),
        },
        Ty::Option { inner } => TypeName::Option(Box::new(ty_to_type_name(inner)?)),
        Ty::List { elem } => TypeName::List(Box::new(ty_to_type_name(elem)?)),
        Ty::HttpRequest => TypeName::HttpRequest,
        Ty::HttpResponse => TypeName::HttpResponse,
        Ty::HttpHeaders => TypeName::HttpHeaders,
        Ty::HttpServer => TypeName::HttpServer,
        Ty::CancelToken | Ty::Fn { .. } => {
            return Err("cannot use this type as a generic type argument".into());
        }
    })
}

fn apply_type_binds(tn: &TypeName, binds: &HashMap<String, Ty>, tparams: &HashSet<String>) -> Result<TypeName, String> {
    match tn {
        TypeName::Class(n) if tparams.contains(n) => {
            let Some(t) = binds.get(n) else {
                return Err(format!("unbound type parameter '{n}'"));
            };
            ty_to_type_name(t)
        }
        TypeName::Array { elem, len } => Ok(TypeName::Array {
            elem: Box::new(apply_type_binds(elem, binds, tparams)?),
            len: *len,
        }),
        TypeName::Future(inner) => Ok(TypeName::Future(Box::new(apply_type_binds(
            inner, binds, tparams,
        )?))),
        TypeName::Channel(elem) => Ok(TypeName::Channel(Box::new(apply_type_binds(
            elem, binds, tparams,
        )?))),
        TypeName::Mutex(elem) => Ok(TypeName::Mutex(Box::new(apply_type_binds(
            elem, binds, tparams,
        )?))),
        TypeName::RwLock(elem) => Ok(TypeName::RwLock(Box::new(apply_type_binds(
            elem, binds, tparams,
        )?))),
        TypeName::Result { ok, err } => Ok(TypeName::Result {
            ok: Box::new(apply_type_binds(ok, binds, tparams)?),
            err: Box::new(apply_type_binds(err, binds, tparams)?),
        }),
        TypeName::Option(inner) => Ok(TypeName::Option(Box::new(apply_type_binds(
            inner, binds, tparams,
        )?))),
        TypeName::List(elem) => Ok(TypeName::List(Box::new(apply_type_binds(
            elem, binds, tparams,
        )?))),
        other => Ok(other.clone()),
    }
}

fn infer_type_binds(
    pattern: &TypeName,
    concrete: &Ty,
    tparams: &HashSet<String>,
    binds: &mut HashMap<String, Ty>,
    ctx: &CheckCtx<'_>,
    span: Span,
) -> Result<(), Diagnostic> {
    match pattern {
        TypeName::Class(n) if tparams.contains(n) => {
            if let Some(prev) = binds.get(n) {
                if prev != concrete {
                    return Err(Diagnostic::new(
                        format!(
                            "conflicting inferences for type parameter '{n}': {:?} vs {:?}",
                            prev, concrete
                        ),
                        span,
                    ));
                }
            } else {
                if !is_value_ty(concrete) {
                    return Err(Diagnostic::new(
                        format!("type parameter '{n}' cannot be void"),
                        span,
                    ));
                }
                binds.insert(n.clone(), concrete.clone());
            }
            Ok(())
        }
        TypeName::List(elem) => {
            let Ty::List { elem: c } = concrete else {
                return Err(Diagnostic::new("expected List type argument", span));
            };
            infer_type_binds(elem, c, tparams, binds, ctx, span)
        }
        TypeName::Option(inner) => {
            let Ty::Option { inner: c } = concrete else {
                return Err(Diagnostic::new("expected Option type argument", span));
            };
            infer_type_binds(inner, c, tparams, binds, ctx, span)
        }
        TypeName::Future(inner) => {
            let Ty::Future(c) = concrete else {
                return Err(Diagnostic::new("expected Future type argument", span));
            };
            infer_type_binds(inner, c, tparams, binds, ctx, span)
        }
        TypeName::Channel(elem) => {
            let Ty::Channel { elem: c } = concrete else {
                return Err(Diagnostic::new("expected Channel type argument", span));
            };
            infer_type_binds(elem, c, tparams, binds, ctx, span)
        }
        TypeName::Mutex(elem) => {
            let Ty::Mutex { elem: c } = concrete else {
                return Err(Diagnostic::new("expected Mutex type argument", span));
            };
            infer_type_binds(elem, c, tparams, binds, ctx, span)
        }
        TypeName::RwLock(elem) => {
            let Ty::RwLock { elem: c } = concrete else {
                return Err(Diagnostic::new("expected RwLock type argument", span));
            };
            infer_type_binds(elem, c, tparams, binds, ctx, span)
        }
        TypeName::Result { ok, err } => {
            let Ty::Result {
                ok: cok,
                err: cerr,
            } = concrete
            else {
                return Err(Diagnostic::new("expected Result type argument", span));
            };
            infer_type_binds(ok, cok, tparams, binds, ctx, span)?;
            infer_type_binds(err, cerr, tparams, binds, ctx, span)
        }
        TypeName::Array { elem, len } => {
            let Ty::Array {
                elem: c,
                len: clen,
            } = concrete
            else {
                return Err(Diagnostic::new("expected array type argument", span));
            };
            if *len != *clen {
                return Err(Diagnostic::new("array length mismatch in generic", span));
            }
            infer_type_binds(elem, c, tparams, binds, ctx, span)
        }
        other => {
            let expected = Ty::from_ast_at(other, ctx.type_names, span)?;
            if !ty_assignable(concrete, &expected, ctx) {
                return Err(Diagnostic::new(
                    format!("type mismatch: expected {:?}, got {:?}", expected, concrete),
                    span,
                ));
            }
            Ok(())
        }
    }
}

fn specialize_generic_call(
    name: &str,
    args: &[Expr],
    env: &HashMap<String, Ty>,
    ctx: &CheckCtx<'_>,
    span: Span,
) -> Result<(String, FuncSig), Diagnostic> {
    let gfn = ctx.generic_fns.get(name).unwrap();
    if gfn.module != ctx.current_module && !gfn.is_pub {
        return Err(Diagnostic::new(
            format!("function '{name}' is private to its module"),
            span,
        ));
    }
    let tparams: HashSet<String> = gfn.type_params.iter().cloned().collect();
    if args.len() != gfn.params.len() {
        // defaults on generics: require all args for MVP
        return Err(Diagnostic::new(
            format!(
                "generic function '{name}' expects {} argument(s), got {}",
                gfn.params.len(),
                args.len()
            ),
            span,
        ));
    }

    let mut binds = HashMap::new();
    let mut arg_tys = Vec::with_capacity(args.len());
    for (p, a) in gfn.params.iter().zip(args.iter()) {
        let (aty, _) = check_expr(a, env, ctx)?;
        infer_type_binds(&p.ty, &aty, &tparams, &mut binds, ctx, a.span())?;
        arg_tys.push(aty);
    }
    for tp in &gfn.type_params {
        if !binds.contains_key(tp) {
            return Err(Diagnostic::new(
                format!("cannot infer type parameter '{tp}' for '{name}'"),
                span,
            ));
        }
    }

    let mut mangled = name.to_string();
    for tp in &gfn.type_params {
        mangled.push('$');
        mangled.push_str(&binds[tp].mangle_name());
    }

    {
        let map = ctx.mono_sigs.borrow();
        if let Some(sig) = map.get(&mangled).cloned() {
            return Ok((mangled, sig));
        }
    }

    // Rewrite AST types and typecheck specialization
    let mut specialized = gfn.clone();
    specialized.name = mangled.clone();
    specialized.type_params.clear();
    for p in &mut specialized.params {
        p.ty = apply_type_binds(&p.ty, &binds, &tparams).map_err(|m| {
            Diagnostic::new(m, span)
        })?;
    }
    if let Some(rt) = &specialized.return_ty {
        specialized.return_ty = Some(apply_type_binds(rt, &binds, &tparams).map_err(|m| {
            Diagnostic::new(m, span)
        })?);
    }

    let (params, defaults) = check_params(&specialized.params, ctx.type_names)?;
    let ret = match &specialized.return_ty {
        Some(t) => Ty::from_ast_at(t, ctx.type_names, specialized.span)?,
        None => Ty::Void,
    };
    // Validate args against inferred params (assignability)
    for (i, (got, exp)) in arg_tys.iter().zip(params.iter()).enumerate() {
        if !ty_assignable(got, exp, ctx) {
            return Err(Diagnostic::new(
                format!("argument type mismatch in function '{name}'"),
                args[i].span(),
            ));
        }
    }

    let sig = FuncSig {
        params: params.clone(),
        defaults: defaults.clone(),
        ret: ret.clone(),
        is_async: false,
        is_pub: specialized.is_pub,
        module: specialized.module.clone(),
    };
    ctx.mono_sigs.borrow_mut().insert(mangled.clone(), sig.clone());

    let checked = check_function(&specialized, ctx, &[])?;
    ctx.mono.borrow_mut().insert(mangled.clone(), checked);

    Ok((mangled, sig))
}

fn check_field_vis(ctx: &CheckCtx<'_>, field: &FieldInfo, span: Span) -> Result<(), Diagnostic> {
    match field.vis {
        Visibility::Pub => Ok(()),
        Visibility::Priv => {
            if ctx.current_class == Some(field.defining_class.as_str()) {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    format!("field is private to '{}'", field.defining_class),
                    span,
                ))
            }
        }
        Visibility::Prot => {
            let Some(cur) = ctx.current_class else {
                return Err(Diagnostic::new(
                    format!("field is protected in '{}'", field.defining_class),
                    span,
                ));
            };
            if is_subclass(ctx, cur, &field.defining_class) {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    format!("field is protected in '{}'", field.defining_class),
                    span,
                ))
            }
        }
    }
}

fn check_prop_vis(ctx: &CheckCtx<'_>, prop: &PropInfo, span: Span) -> Result<(), Diagnostic> {
    match prop.vis {
        Visibility::Pub => Ok(()),
        Visibility::Priv => {
            if ctx.current_class == Some(prop.defining_class.as_str()) {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    format!("property is private to '{}'", prop.defining_class),
                    span,
                ))
            }
        }
        Visibility::Prot => {
            let Some(cur) = ctx.current_class else {
                return Err(Diagnostic::new(
                    format!("property is protected in '{}'", prop.defining_class),
                    span,
                ));
            };
            if is_subclass(ctx, cur, &prop.defining_class) {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    format!("property is protected in '{}'", prop.defining_class),
                    span,
                ))
            }
        }
    }
}

fn check_method_vis(ctx: &CheckCtx<'_>, method: &MethodInfo, span: Span) -> Result<(), Diagnostic> {
    match method.vis {
        Visibility::Pub => Ok(()),
        Visibility::Priv => {
            if ctx.current_class == Some(method.defining_class.as_str()) {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    format!("method is private to '{}'", method.defining_class),
                    span,
                ))
            }
        }
        Visibility::Prot => {
            let Some(cur) = ctx.current_class else {
                return Err(Diagnostic::new(
                    format!("method is protected in '{}'", method.defining_class),
                    span,
                ));
            };
            if is_subclass(ctx, cur, &method.defining_class) {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    format!("method is protected in '{}'", method.defining_class),
                    span,
                ))
            }
        }
    }
}
