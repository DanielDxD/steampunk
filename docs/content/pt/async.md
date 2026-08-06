---
title: Async e Future
description: async/await, Future.ready/join/race, blocos async e cpu.submit.
---

# Async e Future

## Modelo mental

| API | Papel |
|-----|--------|
| `async fn` | Computação que produz `Future<T>` |
| `await` | Suspende até o Future completar → `T` |
| `Future.ready` | Future já resolvido |
| `Future.join` | Espera dois `Future<int>` → `[int; 2]` |
| `Future.race` | Primeiro `Future<int>` a completar |
| `async { … }` | Bloco inline → `Future<int>` (MVP) |

> No MVP o runtime é **thread-backed** (não o scheduler M:N final). A API da linguagem já espelha a visão da SPEC.

## `await`

```stk
async fn main() {
    var n = await Future.ready(42)
    std.log("$1", n)
}
```

- Só em contexto `async`
- `await` em não-`Future` é erro de tipo

## Chamar `async fn`

```stk
async fn work() int {
    std.sleep(20)
    return 7
}

async fn main() {
    var f = work()          // Future<int>, não bloqueia
    var v = await f
    std.log("$1", v)
}
```

Exceto `main`, chamar `async fn` agenda o corpo e devolve um Future pendente.

## `join` e `race` (MVP: `Future<int>`)

```stk
async fn main() {
    var both = await Future.join(Future.ready(1), Future.ready(2))
    std.log("a=$1 b=$2", both[0], both[1])

    var winner = await Future.race(slow(), fast())
    std.log("winner=$1", winner)
}
```

`Future<string>` funciona via `async fn` que retorna `string` + `await`. `join` / `race` / `ready` no MVP ficam em `int`.

## Bloco `async`

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

O corpo captura locais e, no MVP, deve retornar `int`.

## Sleep

```stk
std.sleep(30)  // milissegundos; bloqueia a tarefa/thread atual
```

Útil em demos e para simular I/O / trabalho.
