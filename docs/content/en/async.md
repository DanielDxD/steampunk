---
title: Async and Future
description: async/await, Future.ready/join/race, async blocks, and cpu.submit.
---

# Async and Future

## Mental model

| API | Role |
|-----|------|
| `async fn` | Computation that produces `Future<T>` |
| `await` | Suspends until the Future completes → `T` |
| `Future.ready` | Already-resolved Future |
| `Future.join` | Waits for two `Future<int>` → `[int; 2]` |
| `Future.race` | First `Future<int>` to complete |
| `async { … }` | Inline block → `Future<int>` (MVP) |

> In the MVP the runtime is **thread-backed** (not the final M:N scheduler). The language API already mirrors the SPEC vision.

## `await`

```stk
async fn main() {
    var n = await Future.ready(42)
    std.log("$1", n)
}
```

- Only in an `async` context
- `await` on a non-`Future` is a type error

## Calling an `async fn`

```stk
async fn work() int {
    std.sleep(20)
    return 7
}

async fn main() {
    var f = work()          // Future<int>, does not block
    var v = await f
    std.log("$1", v)
}
```

Except for `main`, calling an `async fn` schedules the body and returns a pending Future.

## `join` and `race` (MVP: `Future<int>`)

```stk
async fn main() {
    var both = await Future.join(Future.ready(1), Future.ready(2))
    std.log("a=$1 b=$2", both[0], both[1])

    var winner = await Future.race(slow(), fast())
    std.log("winner=$1", winner)
}
```

`Future<string>` works via an `async fn` that returns `string` + `await`. MVP `join` / `race` / `ready` stay on `int`.

## `async` block

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

The body captures locals and, in the MVP, must return `int`.

## Sleep

```stk
std.sleep(30)  // milliseconds; blocks the current task/thread
```

Useful in demos and to simulate I/O / work.
