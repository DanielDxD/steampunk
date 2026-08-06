# Steampunk compiler (v0.1 — mínimo usável)

Rust workspace that compiles `.stk` to native code via Cranelift (JIT `run` / AOT `build`).

## Commands

```bash
# From repo root
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/hello.stk
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- build examples/math.stk --out build/math
cargo test --manifest-path compiler/Cargo.toml
```

- `run` — Cranelift JIT in memory, then execute `main`
- `build` — object file + system linker → native binary

## Language subset (v0.1)

- Types: `int`, `float`, `string`, `bool`, `void`, `[T; N]`, `Future<T>`
- Bindings: `var` / `const` (no `let`)
- Control: `if` / `while` / `for` / `match` / `break` / `continue`
- OOP: `class` / `iclass`, `pub`/`priv`/`prot`, inheritance `::`, interfaces `:`
- Async: `async fn`, `await`, `async {…}`, `spawn`, `Future.ready|join|race`
- Sync: `Channel`/`Mutex`/`WaitGroup` (`int`|`string`), `std.cpu.submit`
- Closures: `fn(params) Ret {…}`
- Stdlib: `log`, `panic`, `env`, `process`, `fs` (sync), `time`, `string`, `List<T>`, `Result`/`Option` (any value `T`, including class/iclass)
- Generics: `fn identity<T>(T x) T` with monomorphization; `class Box<T>` still pending
- Typecheck: O(1) named-type lookup; class → base / iclass assignability

See repo root `SPEC.md` §8 (MVP box) and §20 roadmap.
