use stk_span::Span;

#[derive(Debug, Clone)]
pub struct Program {
    pub imports: Vec<Import>,
    pub constants: Vec<ConstDecl>,
    pub functions: Vec<Function>,
    pub classes: Vec<ClassDecl>,
    pub iclasses: Vec<IClassDecl>,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub value: Expr,
    pub span: Span,
    /// Module file key (set by loader); empty for single-file.
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Pub,
    Priv,
    Prot,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub is_pub: bool,
    pub name: String,
    /// Type parameters (`class Box<T>`). Empty if non-generic.
    pub type_params: Vec<String>,
    pub bases: Vec<String>,
    pub interfaces: Vec<String>,
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<MethodDecl>,
    pub span: Span,
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub args: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: String,
    pub ty: TypeName,
    /// Literal default for storage fields (`= false`). Ignored when accessors present.
    pub default: Option<Expr>,
    /// Property accessors — no storage slot when present.
    pub accessors: Option<PropAccessors>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct PropAccessors {
    pub getter: Option<Block>,
    pub setter: Option<SetterDecl>,
}

#[derive(Debug, Clone)]
pub struct SetterDecl {
    pub param: Param,
    pub body: Block,
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub vis: Visibility,
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeName>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IClassDecl {
    pub is_pub: bool,
    pub name: String,
    pub methods: Vec<IClassMethod>,
    pub span: Span,
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct IClassMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeName>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub is_pub: bool,
    pub is_async: bool,
    pub name: String,
    /// Type parameters (`fn identity<T>(…)`). Empty if non-generic.
    pub type_params: Vec<String>,
    pub params: Vec<Param>,
    pub return_ty: Option<TypeName>,
    pub body: Block,
    pub span: Span,
    pub module: String,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub ty: TypeName,
    pub name: String,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeName {
    Int,
    Float,
    String,
    Bool,
    Void,
    Class(String),
    /// Fixed-size array `[elem; len]`
    Array { elem: Box<TypeName>, len: i64 },
    /// `Future<T>` (T may be Void)
    Future(Box<TypeName>),
    /// `std.sync.Channel<T>` (T = any value type)
    Channel(Box<TypeName>),
    /// `std.sync.WaitGroup`
    WaitGroup,
    /// `std.sync.Mutex<T>` (T = any value type)
    Mutex(Box<TypeName>),
    /// `std.sync.RwLock<T>` (T = any value type)
    RwLock(Box<TypeName>),
    /// `std.Result<Ok, Err>` (any value payloads)
    Result {
        ok: Box<TypeName>,
        err: Box<TypeName>,
    },
    /// `std.Option<T>` (any value payload)
    Option(Box<TypeName>),
    /// `std.List<T>` (T = any value type)
    List(Box<TypeName>),
    /// `std.http.Request`
    HttpRequest,
    /// `std.http.Response`
    HttpResponse,
    /// `std.http.Headers`
    HttpHeaders,
    /// `std.http.Server`
    HttpServer,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    Local { name: String, span: Span },
    Field {
        object: Expr,
        field: String,
        span: Span,
    },
    Index {
        array: Expr,
        index: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Stmt {
    VarDecl {
        name: String,
        ty: Option<TypeName>,
        init: Expr,
        span: Span,
    },
    ConstDecl {
        name: String,
        value: Expr,
        span: Span,
    },
    Assign {
        target: AssignTarget,
        value: Expr,
        span: Span,
    },
    Spawn {
        body: SpawnBody,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
    If {
        arms: Vec<(Expr, Block)>,
        else_block: Option<Block>,
        span: Span,
    },
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    ForRange {
        name: String,
        start: Expr,
        end: Expr,
        body: Block,
        span: Span,
    },
    ForIn {
        name: String,
        iter: Expr,
        body: Block,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Match {
        scrutinee: Expr,
        arms: Vec<MatchArm>,
        span: Span,
    },
    /// `do { … } catch name { … }`
    DoCatch {
        body: Block,
        catch_name: String,
        catch_body: Block,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    IntLit { value: i64, span: Span },
    Wildcard { span: Span },
    Ok { name: String, span: Span },
    Err { name: String, span: Span },
    Some { name: String, span: Span },
    None { span: Span },
}

#[derive(Debug, Clone)]
pub enum SpawnBody {
    Block(Block),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit {
        value: i64,
        span: Span,
    },
    FloatLit {
        value: f64,
        span: Span,
    },
    StringLit {
        value: String,
        span: Span,
    },
    BoolLit {
        value: bool,
        span: Span,
    },
    Ident {
        name: String,
        span: Span,
    },
    SelfExpr {
        span: Span,
    },
    SuperField {
        /// `None` = unqualified `super.field`; `Some(B)` = `super.B.field`
        base: Option<String>,
        field: String,
        span: Span,
    },
    SuperMethod {
        base: Option<String>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    New {
        class_name: String,
        args: Vec<Expr>,
        span: Span,
    },
    ArrayLit {
        elems: Vec<Expr>,
        span: Span,
    },
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    FieldGet {
        object: Box<Expr>,
        field: String,
        span: Span,
    },
    MethodCall {
        object: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        span: Span,
    },
    Await {
        expr: Box<Expr>,
        span: Span,
    },
    /// `try expr` | `try? expr` | `try! expr`
    Try {
        mode: TryMode,
        expr: Box<Expr>,
        span: Span,
    },
    /// `async { … }` → `Future<T>`
    AsyncBlock {
        body: Block,
        span: Span,
    },
    /// `fn(params) Ret { … }`
    Closure {
        params: Vec<Param>,
        return_ty: Option<TypeName>,
        body: Block,
        span: Span,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Callee,
        args: Vec<Expr>,
        span: Span,
    },
    Group {
        expr: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Callee {
    Func {
        name: String,
        /// Explicit type args at call site (`f<T>(…)`).
        type_args: Vec<TypeName>,
        span: Span,
    },
    /// Call a function value / closure: `expr(args)`
    Value { expr: Box<Expr> },
    StdLog { span: Span },
    /// `std.sleep(ms)` / `std.time.sleepMs(ms)`
    StdSleep { span: Span },
    /// `std.time.nowMs()`
    StdTimeNowMs { span: Span },
    /// `std.panic(msg)`
    StdPanic { span: Span },
    /// `std.env.args()` → `List<string>`
    StdEnvArgs { span: Span },
    /// `std.env.get(name)` → `Option<string>`
    StdEnvGet { span: Span },
    /// `std.env.set(name, value)`
    StdEnvSet { span: Span },
    /// `std.process.exit(code)`
    StdProcessExit { span: Span },
    /// `std.fs.readToString(path)` → `Result<string, string>`
    StdFsReadToString { span: Span },
    /// `std.fs.writeString(path, contents)` → `Result<int, string>`
    StdFsWriteString { span: Span },
    /// `std.string.len(s)`
    StdStringLen { span: Span },
    /// `std.string.concat(a, b)`
    StdStringConcat { span: Span },
    /// `std.string.slice(s, start, end)`
    StdStringSlice { span: Span },
    /// `std.string.contains(hay, needle)`
    StdStringContains { span: Span },
    /// `std.string.fromInt(n)`
    StdStringFromInt { span: Span },
    /// `std.string.parseInt(s)` → `Result<int, string>`
    StdStringParseInt { span: Span },
    /// `std.List<T>.new()`
    StdListNew { elem: Box<TypeName>, span: Span },
    /// `std.cpu.submit(fn)` — `fn() T` → `Future<T>` (any value T)
    StdCpuSubmit { span: Span },
    /// `std.Result<Ok, Err>.ok(v)` / `.err(e)`
    StdResultOk {
        ok: Box<TypeName>,
        err: Box<TypeName>,
        span: Span,
    },
    StdResultErr {
        ok: Box<TypeName>,
        err: Box<TypeName>,
        span: Span,
    },
    /// `std.Option<T>.some(v)` / `.none()`
    StdOptionSome { inner: Box<TypeName>, span: Span },
    StdOptionNone { inner: Box<TypeName>, span: Span },
    /// `std.sync.Channel<T>.new()`
    StdChannelNew { elem: Box<TypeName>, span: Span },
    /// `std.sync.Channel<T>.buffered(n)`
    StdChannelBuffered { elem: Box<TypeName>, span: Span },
    /// `std.sync.WaitGroup.new()`
    StdWaitGroupNew { span: Span },
    /// `std.sync.Mutex<T>.new(initial)`
    StdMutexNew { elem: Box<TypeName>, span: Span },
    /// `std.sync.RwLock<T>.new(initial)`
    StdRwLockNew { elem: Box<TypeName>, span: Span },
    /// `std.parallel.map(list, fn)`
    StdParallelMap { span: Span },
    /// `std.http.get|post|put|delete|patch(...)`
    StdHttpClient {
        method: HttpClientMethod,
        span: Span,
    },
    /// `std.http.Headers.new()`
    StdHttpHeadersNew { span: Span },
    /// `std.http.Response.text|json|empty(...)`
    StdHttpResponseNew {
        kind: HttpResponseKind,
        span: Span,
    },
    /// `std.http.Server.new()`
    StdHttpServerNew { span: Span },
    /// `std.task.yield()`
    StdTaskYield { span: Span },
    /// `std.task.CancellationToken.new()`
    StdCancelTokenNew { span: Span },
    /// `std.json|yaml|toml|toon.encode(value)`
    StdSerdeEncode {
        format: SerdeFormat,
        span: Span,
    },
    /// `std.json|yaml|toml|toon.decode<T>(text)` — type_arg optional if inferred later
    StdSerdeDecode {
        format: SerdeFormat,
        type_arg: Option<Box<TypeName>>,
        span: Span,
    },
    /// `Future.join(a, b)` — two `Future<T>` → `Future<[T; 2]>` (any value T)
    FutureJoin { span: Span },
    /// `Future.race(a, b)` — two `Future<T>` → `Future<T>`
    FutureRace { span: Span },
    /// `Future.ready(v)` — T → `Future<T>` (any value T)
    FutureReady { span: Span },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpClientMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpResponseKind {
    Text,
    Json,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerdeFormat {
    Json,
    Yaml,
    Toml,
    Toon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryMode {
    /// `try expr` — unwrap inside `do`, propagate err to `catch`
    Unwrap,
    /// `try? expr` → `Option<T>`
    Option,
    /// `try! expr` — panic on err
    Force,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLit { span, .. }
            | Expr::FloatLit { span, .. }
            | Expr::StringLit { span, .. }
            | Expr::BoolLit { span, .. }
            | Expr::Ident { span, .. }
            | Expr::SelfExpr { span }
            | Expr::SuperField { span, .. }
            | Expr::SuperMethod { span, .. }
            | Expr::New { span, .. }
            | Expr::ArrayLit { span, .. }
            | Expr::Index { span, .. }
            | Expr::FieldGet { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::Await { span, .. }
            | Expr::Try { span, .. }
            | Expr::AsyncBlock { span, .. }
            | Expr::Closure { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Group { span, .. } => *span,
        }
    }
}
