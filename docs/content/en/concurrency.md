---
title: Concurrency
description: spawn, Channel, WaitGroup, Mutex, and await on recv/wait.
---

# Concurrency

Steampunk treats concurrency as a first-class citizen. The idiomatic pattern mixes `spawn` (goroutine) with channels — in the spirit of Go — and `async`/`await` when the result is typed as `Future`.

## `spawn`

Fire-and-forget: launches and returns immediately (`void`):

```stk
spawn work(10)
spawn {
    std.log("running in parallel")
}
```

- Captures locals from the scope
- Does not return a `Future` — synchronize with `Channel` / `WaitGroup`
- In the MVP: one OS thread per spawn (final vision: M:N)

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

| API | Description |
|-----|-------------|
| `Channel<T>.new()` | Unbounded queue |
| `Channel<T>.buffered(n)` | Capacity `n`; `send` blocks when full |
| `send` / `recv` / `close` | Communication |
| `for v in ch` | Drains until close |

MVP `T`: `int` or `string`.

### `await ch.recv()`

In an `async` context, `recv` types as a `Future`:

```stk
async fn main() {
    var ch = std.sync.Channel<int>.new()
    spawn { ch.send(42) }
    var v = await ch.recv()
    std.log("got=$1", v)
}
```

Outside `async`, `recv` remains synchronous.

## WaitGroup

```stk
var wg = std.sync.WaitGroup.new()
wg.add(1)
spawn {
    // work
    wg.done()
}
wg.wait()
```

In `async`, use `await wg.wait()` (`Future<void>`).

## Workers + close pattern

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

APIs: `new(initial)`, `lock`, `unlock`, `get`, `set`. No lock-ownership checking in this slice.

## String channel

```stk
var ch = std.sync.Channel<string>.new()
spawn { ch.send("hi"); ch.close() }
for s in ch {
    std.log("$1", s)
}
```
