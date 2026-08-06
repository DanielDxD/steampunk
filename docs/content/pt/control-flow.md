---
title: Controle de fluxo
description: if, while, for, match e padrões ok/err/some/none.
---

# Controle de fluxo

## `if` / `else`

A condição deve ser `bool`:

```stk
if n > 0 {
    std.log("positivo")
} else if n == 0 {
    std.log("zero")
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

`break` e `continue` são suportados.

## `for` em intervalo

`0..n` é exclusivo no fim (como em Rust):

```stk
for i in 0..3 {
    std.log("i=$1", i)  // 0, 1, 2
}
```

## `for` em channel

Drena até `close()`:

```stk
for v in ch {
    std.log("got=$1", v)
}
```

## `match`

### Literais e wildcard

```stk
fn describe(int n) {
    match n {
        0 => { std.log("zero") }
        1 => { std.log("um") }
        _ => { std.log("outro") }
    }
}
```

Para `int`, o compilador exige cobertura completa — geralmente com `_`.

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

- `Result` exige braços `ok` + `err` (ou `_`)
- `Option` exige `some` + `none` (ou `_`)

Veja [Result e Option](./result-option) para a API completa.
