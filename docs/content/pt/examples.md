---
title: Exemplos práticos
description: Catálogo dos exemplos do repositório e o que cada um demonstra.
---

# Exemplos práticos

Todos os arquivos estão em `examples/`. Rode com:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/NOME.stk
```

## Fundamentos

| Arquivo | Demonstra |
|---------|-----------|
| `hello.stk` | `main` + `std.log` |
| `math.stk` | Aritmética e retorno |
| `greet.stk` | Parâmetro `string` |
| `control.stk` | `if`, `while`, `for`, `match` |
| `modules_main.stk` | `@import ":modules/math"`, arrays, `const` |

## OOP

| Arquivo | Demonstra |
|---------|-----------|
| `oop.stk` | Classes, propriedades, herança múltipla, `super` |

## Async e Future

| Arquivo | Demonstra |
|---------|-----------|
| `async.stk` | `async fn` + `await` |
| `async_block.stk` | `async { … }` |
| `async_string.stk` | `Future<string>` |
| `future_join.stk` | `Future.join` |
| `future_race.stk` | `Future.race` |
| `cpu_submit.stk` | `std.cpu.submit` com função nomeada |
| `closures.stk` | Closures, captura, submit inline |

## Concorrência

| Arquivo | Demonstra |
|---------|-----------|
| `spawn.stk` | `spawn` concorrente |
| `channel.stk` | Channel + WaitGroup + workers |
| `buffered.stk` | Channel buffered |
| `channel_string.stk` | `Channel<string>` |
| `await_recv.stk` | `await ch.recv()` |
| `await_wait.stk` | `await wg.wait()` |
| `mutex.stk` | `Mutex<int>` |

## Result / Option

| Arquivo | Demonstra |
|---------|-----------|
| `result_option.stk` | `.ok`/`.err`/`.some`/`.none` + `match` |

## Mini-receitas

### Pipeline async + channel

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

### Closure capturando e submetendo

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
