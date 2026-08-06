---
title: Control de flujo
description: if, while, for, match y patrones ok/err/some/none.
---

# Control de flujo

## `if` / `else`

La condición debe ser `bool`:

```stk
if n > 0 {
    std.log("positivo")
} else if n == 0 {
    std.log("cero")
} else {
    std.log("negativo")
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

`break` y `continue` están soportados.

## `for` en rango

`0..n` es exclusivo al final (como en Rust):

```stk
for i in 0..3 {
    std.log("i=$1", i)  // 0, 1, 2
}
```

## `for` en channel

Drena hasta `close()`:

```stk
for v in ch {
    std.log("got=$1", v)
}
```

## `match`

### Literales y wildcard

```stk
fn describe(int n) {
    match n {
        0 => { std.log("cero") }
        1 => { std.log("uno") }
        _ => { std.log("otro") }
    }
}
```

Para `int`, el compilador exige cobertura completa — generalmente con `_`.

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

- `Result` exige brazos `ok` + `err` (o `_`)
- `Option` exige `some` + `none` (o `_`)

Véase [Result y Option](./result-option) para la API completa.
