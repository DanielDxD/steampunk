---
title: Async y Future
description: async/await, Future.ready/join/race, bloques async y cpu.submit.
---

# Async y Future

## Modelo mental

| API | Rol |
|-----|-----|
| `async fn` | Computación que produce `Future<T>` |
| `await` | Suspende hasta que el Future complete → `T` |
| `Future.ready` | Future ya resuelto |
| `Future.join` | Espera dos `Future<int>` → `[int; 2]` |
| `Future.race` | Primer `Future<int>` en completar |
| `async { … }` | Bloque inline → `Future<int>` (MVP) |

> En el MVP el runtime es **thread-backed** (no el scheduler M:N final). La API del lenguaje ya refleja la visión de la SPEC.

## `await`

```stk
async fn main() {
    var n = await Future.ready(42)
    std.log("$1", n)
}
```

- Solo en contexto `async`
- `await` en no-`Future` es error de tipo

## Llamar `async fn`

```stk
async fn work() int {
    std.sleep(20)
    return 7
}

async fn main() {
    var f = work()          // Future<int>, no bloquea
    var v = await f
    std.log("$1", v)
}
```

Excepto `main`, llamar `async fn` agenda el cuerpo y devuelve un Future pendiente.

## `join` y `race` (MVP: `Future<int>`)

```stk
async fn main() {
    var both = await Future.join(Future.ready(1), Future.ready(2))
    std.log("a=$1 b=$2", both[0], both[1])

    var winner = await Future.race(slow(), fast())
    std.log("winner=$1", winner)
}
```

`Future<string>` funciona vía `async fn` que retorna `string` + `await`. `join` / `race` / `ready` en el MVP quedan en `int`.

## Bloque `async`

```stk
async fn main() {
    var base = 5
    var f = async {
        var x = await Future.ready(base + 2)
        return x
    }
    std.log("block=$1", await f)
}
```

El cuerpo captura locales y, en el MVP, debe retornar `int`.

## Sleep

```stk
std.sleep(30)  // milisegundos; bloquea la tarea/thread actual
```

Útil en demos y para simular I/O / trabajo.
