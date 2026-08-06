---
title: Sintaxis básica
description: Comentarios, literales, tipos, variables y mutabilidad en Steampunk.
---

# Sintaxis básica

## Comentarios

```stk
// línea

/*
  bloque
*/
```

## Identificadores

- Empiezan con letra o `_`
- Case-sensitive
- Convenciones: tipos en `PascalCase`, funciones/vars en `camelCase`, consts en `SCREAMING_SNAKE`

## Literales (MVP)

| Tipo | Ejemplos |
|------|----------|
| `int` | `42`, `0`, `-7` |
| `string` | `"Hello"`, `"línea\n"` |
| `bool` | `true`, `false` |

Las strings aceptan escapes: `\n`, `\t`, `\r`, `\\`, `\"`.

## Tipos primitivos en el MVP

El compilador actual tipa de forma estable:

- `int`, `string`, `bool`, `void`
- Arrays fijos `[T; N]` (elementos `int` / `string` / `bool`)
- `Future<T>`, `Channel<T>` (hoy `T ∈ {int, string}`)
- `std.Result<int, string>`, `std.Option<int>`
- Función / closure: `fn(…) Ret`

La SPEC completa prevé más anchos (`i32`, `float`, etc.); véase [Estado del MVP](./mvp).

## Variables

No existe `let`. Usa `var` (mutable) y `const` (compile-time):

```stk
var counter = 0
counter = counter + 1

const MAX = 128
var name string = "Ada"
```

`var` infiere el tipo por la inicialización cuando es posible.

## Operadores aritméticos y comparación

```stk
var a = 10 + 3
var b = a * 2
var ok = b >= 20
var same = "x" == "x"
```

Operadores: `+ - * / %`, `== != < <= > >=`, `&& ||`, `!` (negación lógica), `-` unario.

## Arrays

```stk
var xs = [10, 20, 30]
std.log("len=$1 first=$2", xs.len, xs[0])
```

- El literal no puede estar vacío en el MVP
- `.len` es propiedad del array
- Índice fuera de rango es error en runtime

## Logging

```stk
@import "std"

fn main() {
    std.log("solo texto")
    std.log("valor=$1 nombre=$2", 42, "Ada")
}
```

Los placeholders `$1`, `$2`, … corresponden a los argumentos en orden.
