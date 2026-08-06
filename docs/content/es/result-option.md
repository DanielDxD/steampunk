---
title: Result y Option
description: "std.Result<int, string>, std.Option<int> y match tipado."
---

# Result y Option

En el MVP no hay genéricos arbitrarios: solo los pares de abajo.

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

| Constructor | Significado |
|-------------|-------------|
| `.ok(v)` | Éxito (`int`) |
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

## Exhaustividad

- `Result` → brazos `ok` + `err`, o `_`
- `Option` → brazos `some` + `none`, o `_`

## Aún fuera de este corte

- `T?` / `null`
- `Result` / `Option` con otros tipos
- Operador `?`
- `unwrap` / `expect`
