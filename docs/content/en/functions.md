---
title: Functions and closures
description: Named functions, async, default parameters, and closures as values.
---

# Functions and closures

## Named functions

Parameters use **type before the name**. The return type comes after the parameter list; omitted means `void`.

```stk
fn add(int a, int b) int {
    return a + b
}

fn greet(string name = "world") {
    std.log("Hi $1", name)
}
```

- Defaults only with literals (`int` / `string` / `bool`)
- Parameters with defaults come after required ones
- `pub fn` exports the symbol from the module

## `async` functions

`async fn` returns `Future<T>`, where `T` is the annotated type:

```stk
async fn fetchAnswer() int {
    std.sleep(10)
    return 42
}

async fn main() {
    var n = await fetchAnswer()
    std.log("$1", n)
}
```

Important rules:

1. `await` only inside `async fn` or an `async { … }` block
2. Calling an `async fn` does **not** block — it returns a `Future`
3. `async fn main` is the program’s async entry point

## Closures

Closures are nameless `fn` expressions. They become function-typed values and can capture locals:

```stk
@import "std"

async fn main() {
    var base = 40
    var f = fn() int { return base + 2 }
    var r = await std.cpu.submit(f)
    std.log("stored=$1", r)

    var g = fn(int n) int { return n + base }
    std.log("call=$1", g(2))

    var r2 = await std.cpu.submit(fn() int { return 41 + 1 })
    std.log("inline=$1", r2)
}
```

### Closure MVP

| Allowed | Detail |
|---------|--------|
| Params | `int`, `string`, `bool` (0 or N) |
| Return | `int`, `string`, `void` |
| Capture | Free locals from the scope (like `spawn`) |
| Call | `f(args)` when `f` is a function value |
| `cpu.submit` | only `fn() int` (global name or closure) |

Still out: `async` closures, `Fn` in class fields, `std.parallel.map`.

## `std.cpu.submit`

Schedules heavy synchronous work on an OS thread and returns `Future<int>`:

```stk
fn heavy() int {
    std.sleep(30)
    return 42
}

async fn main() {
    var a = await std.cpu.submit(heavy)
    var b = await std.cpu.submit(fn() int { return heavy() })
    std.log("$1 $2", a, b)
}
```

Use `submit` for CPU-bound work; use `spawn` for goroutine-style concurrency.
