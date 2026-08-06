---
title: Concurrencia
description: spawn, Channel, WaitGroup, Mutex y await en recv/wait.
---

# Concurrencia

Steampunk trata la concurrencia como ciudadana de primera clase. El patrón idiomático mezcla `spawn` (goroutine) con channels — al espíritu de Go — y `async`/`await` cuando el resultado está tipado como `Future`.

## `spawn`

Fire-and-forget: lanza y retorna de inmediato (`void`):

```stk
spawn work(10)
spawn {
    std.log("corriendo en paralelo")
}
```

- Captura locales del scope
- No devuelve `Future` — sincroniza con `Channel` / `WaitGroup`
- En el MVP: thread OS por spawn (visión final: M:N)

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

| API | Descripción |
|-----|-------------|
| `Channel<T>.new()` | Cola ilimitada |
| `Channel<T>.buffered(n)` | Capacidad `n`; `send` bloquea si está lleno |
| `send` / `recv` / `close` | Comunicación |
| `for v in ch` | Drena hasta close |

`T` en el MVP: `int` o `string`.

### `await ch.recv()`

En contexto `async`, `recv` tipa como `Future`:

```stk
async fn main() {
    var ch = std.sync.Channel<int>.new()
    spawn { ch.send(42) }
    var v = await ch.recv()
    std.log("got=$1", v)
}
```

Fuera de `async`, `recv` sigue siendo síncrono.

## WaitGroup

```stk
var wg = std.sync.WaitGroup.new()
wg.add(1)
spawn {
    // trabajo
    wg.done()
}
wg.wait()
```

En `async`, usa `await wg.wait()` (`Future<void>`).

## Patrón workers + close

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

APIs: `new(initial)`, `lock`, `unlock`, `get`, `set`. Sin chequeo de posesión del lock en este corte.

## Channel de strings

```stk
var ch = std.sync.Channel<string>.new()
spawn { ch.send("hi"); ch.close() }
for s in ch {
    std.log("$1", s)
}
```
