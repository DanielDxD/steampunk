---
title: Result and Option
description: "std.Result<int, string>, std.Option<int>, and typed match."
---

# Result and Option

In the MVP there are no arbitrary generics: only the pairs below.

## `std.Result<int, string>`

```stk
@import "std"

fn parsePositive(int n) std.Result<int, string> {
    if n > 0 {
        return std.Result<int, string>.ok(n)
    }
    return std.Result<int, string>.err("non-positive")
}

fn main() {
    match parsePositive(7) {
        ok(n) => { std.log("ok=$1", n) }
        err(e) => { std.log("err=$1", e) }
    }
}
```

| Constructor | Meaning |
|-------------|---------|
| `.ok(v)` | Success (`int`) |
| `.err(e)` | Error (`string`) |

## `std.Option<int>`

```stk
var maybe = std.Option<int>.some(3)
match maybe {
    some(v) => { std.log("some=$1", v) }
    none => { std.log("empty") }
}

match std.Option<int>.none() {
    some(v) => { std.log("x=$1", v) }
    none => { std.log("empty") }
}
```

## Exhaustiveness

- `Result` → `ok` + `err` arms, or `_`
- `Option` → `some` + `none` arms, or `_`

## Still out of this slice

- `T?` / `null`
- `Result` / `Option` with other types
- The `?` operator
- `unwrap` / `expect`
