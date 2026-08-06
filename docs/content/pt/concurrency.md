---
title: Concorrência
description: spawn, Channel, WaitGroup, Mutex e await em recv/wait.
---

# Concorrência

Steampunk trata concorrência como cidadão de primeira classe. O padrão idiomático mistura `spawn` (goroutine) com channels — no espírito do Go — e `async`/`await` quando o resultado é tipado como `Future`.

## `spawn`

Fire-and-forget: lança e retorna na hora (`void`):

```stk
spawn work(10)
spawn {
    std.log("rodando em paralelo")
}
```

- Captura locais do escopo
- Não devolve `Future` — sincronize com `Channel` / `WaitGroup`
- No MVP: thread OS por spawn (visão final: M:N)

## Channel

```stk
@import "std"

fn producer(std.sync.Channel<int> ch) {
    ch.send(10)
    ch.send(20)
    ch.close()
}

fn main() {
    var ch = std.sync.Channel<int>.new()
    spawn producer(ch)
    for v in ch {
        std.log("got=$1", v)
    }
}
```

| API | Descrição |
|-----|-----------|
| `Channel<T>.new()` | Fila ilimitada |
| `Channel<T>.buffered(n)` | Capacidade `n`; `send` bloqueia se cheio |
| `send` / `recv` / `close` | Comunicação |
| `for v in ch` | Drena até close |

`T` no MVP: `int` ou `string`.

### `await ch.recv()`

Em contexto `async`, `recv` tipa como `Future`:

```stk
async fn main() {
    var ch = std.sync.Channel<int>.new()
    spawn { ch.send(42) }
    var v = await ch.recv()
    std.log("got=$1", v)
}
```

Fora de `async`, `recv` continua síncrono.

## WaitGroup

```stk
var wg = std.sync.WaitGroup.new()
wg.add(1)
spawn {
    // trabalho
    wg.done()
}
wg.wait()
```

Em `async`, use `await wg.wait()` (`Future<void>`).

## Padrão workers + close

```stk
@import "std"

fn worker(int id, std.sync.Channel<int> out) {
    out.send(id * 2)
}

fn main() {
    var out = std.sync.Channel<int>.new()
    var wg = std.sync.WaitGroup.new()

    for i in 0..4 {
        wg.add(1)
        spawn {
            worker(i, out)
            wg.done()
        }
    }

    spawn {
        wg.wait()
        out.close()
    }

    var sum = 0
    for v in out {
        sum = sum + v
    }
    std.log("sum=$1", sum)
}
```

## Mutex (MVP: `Mutex<int>`)

```stk
var m = std.sync.Mutex<int>.new(0)
m.lock()
m.set(m.get() + 1)
m.unlock()
```

APIs: `new(initial)`, `lock`, `unlock`, `get`, `set`. Sem checagem de posse do lock neste corte.

## Channel de strings

```stk
var ch = std.sync.Channel<string>.new()
spawn { ch.send("hi"); ch.close() }
for s in ch {
    std.log("$1", s)
}
```
