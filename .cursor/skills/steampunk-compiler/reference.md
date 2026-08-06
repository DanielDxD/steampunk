# Steampunk compiler — quick reference

Canonical details: repo root `SPEC.md`.

## Keywords

```
fn async await spawn pub priv prot class iclass new
var const return if else while for break continue match
true false null self super as import module defer
```

`@import` is a directive, not an expression keyword.

## Tokens / syntax highlights

| Construct | Form |
|-----------|------|
| Import std/dep | `@import "name"` |
| Import local | `@import ":path/to/mod"` |
| Fn | `fn name(int a) int { return a }` |
| Async fn | `async fn name() string { return await … }` |
| Class | `pub class C { pub var x int  pub fn new() C {…} }` |
| Inherit | `class B :: A {}` |
| Interface | `iclass I { method(string s) }` |
| Impl iface | `class C : I { pub fn method(string s) {…} }` |
| Both | `class C :: Base : IFoo, IBar {}` |
| Spawn | `spawn foo()` / `spawn { … }` → `void` |
| Format | `"x=$1 y=$2", a, b` |
| Float | `3.14`, type `float` |

## Visibility

- Top-level: omit = module-private; `pub` = export.
- Class fields/methods: **must** be `pub` | `priv` | `prot`.
- `iclass` method impls must be `pub`.

## Types (v0.1 mínimo usável)

`bool` `int` `float` `string` `void`  
`[T; N]` · `Future<T>` · `std.Result` / `std.Option` / `std.List<T>` (any value T: primitives, class, iclass)  
(`i8`…`u64`/`f32`/`char`/`T?` planejados)

## Stdlib v0.1

| API | Notes |
|-----|-------|
| `std.log` | `$n` placeholders |
| `std.panic(msg)` | abort |
| `std.env.args/get/set` | args → `List<string>` |
| `std.process.exit` | |
| `std.fs.readToString` / `writeString` | sync → `Result` |
| `std.time.sleepMs` / `nowMs` | `std.sleep` alias |
| `std.string.*` | len, concat, slice, contains, fromInt, parseInt |
| `std.List<T>` | new, push, get, set, len |
| `std.sync.Channel/WaitGroup/Mutex` | int\|string elems |
| `std.cpu.submit` | `fn() int\|string` → `Future<T>` |

## Manifest `.stkm` (0.3)

```stkm
name = "App"
version = "1.0.0"
private = true
description = "…"

scripts
    .declare("start", "steampunk run main.stk")

dependencies
    .use("dep1", version = "^1.0.1")
```

## Artifacts

| Ext | Role |
|-----|------|
| `.stk` | Source |
| `.stkm` | Manifest |
| `.stkb` | Precompiled lib in **global** cache; copied into app binary at build |
| `.stkmap` | API sourcemap (types + LSP) from global cache |

Cache: `~/.steampunk/deps/<name>/<ver>/` (shared across projects). Not stored in the project tree.

## Runtime

- v0.1: thread-backed (OS threads)
- v0.2+: M:N work-stealing scheduler
- `await` suspends current task
- `spawn` = goroutine (channel/WaitGroup for sync)
- CPU parallel: `std.cpu.submit`; `std.parallel.*` in 0.2
