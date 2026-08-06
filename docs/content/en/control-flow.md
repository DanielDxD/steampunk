---
title: Control flow
description: if, while, for, match, and ok/err/some/none patterns.
---

# Control flow

## `if` / `else`

The condition must be `bool`:

```stk
if n > 0 {
    std.log("positive")
} else if n == 0 {
    std.log("zero")
} else {
    std.log("negative")
}
```

## `while`

```stk
var i = 0
while i < 3 {
    std.log("i=$1", i)
    i = i + 1
}
```

`break` and `continue` are supported.

## `for` over a range

`0..n` is exclusive at the end (like Rust):

```stk
for i in 0..3 {
    std.log("i=$1", i)  // 0, 1, 2
}
```

## `for` over a channel

Drains until `close()`:

```stk
for v in ch {
    std.log("got=$1", v)
}
```

## `match`

### Literals and wildcard

```stk
fn describe(int n) {
    match n {
        0 => { std.log("zero") }
        1 => { std.log("one") }
        _ => { std.log("other") }
    }
}
```

For `int`, the compiler requires full coverage — usually with `_`.

### Result / Option

```stk
match parsePositive(7) {
    ok(n) => { std.log("ok=$1", n) }
    err(e) => { std.log("err=$1", e) }
}

match std.Option<int>.some(3) {
    some(v) => { std.log("some=$1", v) }
    none => { std.log("empty") }
}
```

- `Result` requires `ok` + `err` arms (or `_`)
- `Option` requires `some` + `none` (or `_`)

See [Result and Option](./result-option) for the full API.
