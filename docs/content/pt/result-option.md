---
title: Result e Option
description: "std.Result<int, string>, std.Option<int> e match tipado."
---

# Result e Option

No MVP não há genéricos arbitrários: só os pares abaixo.

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

| Construtor | Significado |
|------------|-------------|
| `.ok(v)` | Sucesso (`int`) |
| `.err(e)` | Erro (`string`) |

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

## Exhaustividade

- `Result` → braços `ok` + `err`, ou `_`
- `Option` → braços `some` + `none`, ou `_`

## Ainda fora deste corte

- `T?` / `null`
- `Result` / `Option` com outros tipos
- Operador `?`
- `unwrap` / `expect`
