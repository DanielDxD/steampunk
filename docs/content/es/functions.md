---
title: Funciones y closures
description: Funciones nombradas, async, parámetros con default y closures como valores.
---

# Funciones y closures

## Funciones nombradas

Los parámetros usan **tipo antes del nombre**. El retorno va después de la lista; omitido = `void`.

```stk
fn add(int a, int b) int {
    return a + b
}

fn greet(string name = "world") {
    std.log("Hi $1", name)
}
```

- Defaults solo con literales (`int` / `string` / `bool`)
- Params con default van después de los obligatorios
- `pub fn` exporta el símbolo del módulo

## Funciones `async`

`async fn` retorna `Future<T>`, donde `T` es el tipo anotado:

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

Reglas importantes:

1. `await` solo dentro de `async fn` o bloque `async { … }`
2. Llamar `async fn` **no** bloquea — devuelve un `Future`
3. `async fn main` es el entry asíncrono del programa

## Closures

Las closures son expresiones `fn` sin nombre. Se convierten en valores de tipo función y pueden capturar locales:

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

### MVP de closures

| Permitido | Detalle |
|-----------|---------|
| Params | `int`, `string`, `bool` (0 o N) |
| Retorno | `int`, `string`, `void` |
| Captura | Locales libres del scope (como `spawn`) |
| Llamada | `f(args)` cuando `f` es valor función |
| `cpu.submit` | solo `fn() int` (nombre global o closure) |

Aún fuera: closures `async`, `Fn` en campos de clase, `std.parallel.map`.

## `std.cpu.submit`

Agenda trabajo síncrono pesado en un thread OS y devuelve `Future<int>`:

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

Usa `submit` para CPU-bound; usa `spawn` para concurrencia estilo goroutine.
