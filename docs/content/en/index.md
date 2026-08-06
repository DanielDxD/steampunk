---
title: Overview
description: Steampunk is a compiled, statically typed, async-first language — with concurrency and parallelism as first-class pillars.
---

# Overview

**Steampunk** (`.stk` extension) is an object-oriented, statically typed language compiled to native code. The design balances predictable performance with clear DX: explicit modules, friendly errors, and async at the core.

## Pillars

- AOT / JIT compilation to native (Rust + Cranelift compiler)
- Static typing with local inference (`var x = 10`)
- OOP with `class`, inheritance (`::`), interfaces (`iclass`)
- `async` / `await` and the `Future<T>` type
- Goroutine-style `spawn` + `Channel` / `WaitGroup` / `Mutex`
- Closures and `std.cpu.submit` for CPU work
- Modules via `@import`

## Hello, world

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

## Philosophy

| Concept | Role |
|---------|------|
| Concurrency | Many overlapping tasks (`async`, `spawn`, channels) |
| Parallelism | CPU work across cores (`std.cpu.submit`) |
| Typing | Fail early; infer where the type is obvious |
| Modules | Explicit imports; only `pub` crosses boundaries |

> This documentation covers the language and what the **MVP v0.1** compiler already runs. Details of the current slice are in [MVP status](./mvp).

## Next step

Continue to [Getting started](./getting-started) to install the compiler and run the repository examples.
