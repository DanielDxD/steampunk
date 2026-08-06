---
title: Practical examples
description: Catalog of repository examples and what each one demonstrates.
---

# Practical examples

All files live under `examples/`. Run with:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/NAME.stk
```

## Fundamentals

| File | Demonstrates |
|------|--------------|
| `hello.stk` | `main` + `std.log` |
| `math.stk` | Arithmetic and return |
| `greet.stk` | `string` parameter |
| `control.stk` | `if`, `while`, `for`, `match` |
| `modules_main.stk` | `@import ":modules/math"`, arrays, `const` |

## OOP

| File | Demonstrates |
|------|--------------|
| `oop.stk` | Classes, properties, multiple inheritance, `super` |

## Async and Future

| File | Demonstrates |
|------|--------------|
| `async.stk` | `async fn` + `await` |
| `async_block.stk` | `async { … }` |
| `async_string.stk` | `Future<string>` |
| `future_join.stk` | `Future.join` |
| `future_race.stk` | `Future.race` |
| `cpu_submit.stk` | `std.cpu.submit` with a named function |
| `closures.stk` | Closures, capture, inline submit |

## Concurrency

| File | Demonstrates |
|------|--------------|
| `spawn.stk` | Concurrent `spawn` |
| `channel.stk` | Channel + WaitGroup + workers |
| `buffered.stk` | Buffered channel |
| `channel_string.stk` | `Channel<string>` |
| `await_recv.stk` | `await ch.recv()` |
| `await_wait.stk` | `await wg.wait()` |
| `mutex.stk` | `Mutex<int>` |

## Result / Option

| File | Demonstrates |
|------|--------------|
| `result_option.stk` | `.ok`/`.err`/`.some`/`.none` + `match` |

## Mini recipes

### Async pipeline + channel

```stk
@import "std"

async fn main() {
    var ch = std.sync.Channel<int>.new()
    spawn {
        ch.send(1)
        ch.send(2)
        ch.close()
    }
    var sum = 0
    for v in ch {
        sum = sum + v
    }
    std.log("sum=$1", sum)
}
```

### Capturing closure and submit

```stk
@import "std"

async fn main() {
    var factor = 6
    var r = await std.cpu.submit(fn() int {
        return factor * 7
    })
    std.log("r=$1", r)
}
```
