---
title: MVP status
description: What the v0.1 compiler already does and what remains in the future SPEC.
---

# MVP status

Documented version: **0.1.0-draft**. Compiler in Rust (Cranelift). Current runtime: **thread-backed**.

## Already works

- `@import "std"` and local modules `@import ":path"`
- `fn` / `async fn` / `async fn main` / `fn main`
- `var`, `const`, `int` / `string` / `bool`, fixed arrays
- Control: `if`, `while`, `for` (range and channel), `match`
- Classes, `::` inheritance, properties, `drop`, `iclass` (as in the examples)
- `Future.ready` / `join` / `race` (`int`), `async { … }` → `Future<int>`
- `spawn`, `Channel<int|string>`, `WaitGroup`, `Mutex<int>`
- `await ch.recv()`, `await wg.wait()` in async context
- Closures + `std.cpu.submit` (`fn() int`)
- `std.Result<int, string>` / `std.Option<int>`

## Still out / partial

| Item | Notes |
|------|-------|
| M:N scheduler | Final vision; MVP uses OS threads |
| `std.parallel.*` | Planned |
| Real generics | Only MVP pairs (`Result`/`Option`/`Channel`) |
| `T?` / `null` / `?` | Out |
| String `join`/`race`/`ready` | Out; `Future<string>` via async fn |
| Mutex other types | `int` only |
| Async closures / Fn in fields | Out |
| Async I/O (`http`, `fs`) | Out |
| Float / fixed-width integers | Full SPEC; not in this slice |
| Full borrow checker | Post-MVP evolution |

## Where to go deeper

- Normative specification: `SPEC.md` at the root
- Compiler: `compiler/README.md`
- Runnable examples: `examples/`

This user documentation tracks the MVP and will grow as the language grows.
