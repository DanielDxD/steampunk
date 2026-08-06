---
title: Ejemplos prácticos
description: Catálogo de los ejemplos del repositorio y lo que cada uno demuestra.
---

# Ejemplos prácticos

Todos los archivos están en `examples/`. Ejecuta con:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/NOMBRE.stk
```

## Fundamentos

| Archivo | Demuestra |
|---------|-----------|
| `hello.stk` | `main` + `std.log` |
| `math.stk` | Aritmética y retorno |
| `greet.stk` | Parámetro `string` |
| `control.stk` | `if`, `while`, `for`, `match` |
| `modules_main.stk` | `@import ":modules/math"`, arrays, `const` |

## OOP

| Archivo | Demuestra |
|---------|-----------|
| `oop.stk` | Clases, propiedades, herencia múltiple, `super` |

## Async y Future

| Archivo | Demuestra |
|---------|-----------|
| `async.stk` | `async fn` + `await` |
| `async_block.stk` | `async { … }` |
| `async_string.stk` | `Future<string>` |
| `future_join.stk` | `Future.join` |
| `future_race.stk` | `Future.race` |
| `cpu_submit.stk` | `std.cpu.submit` con función nombrada |
| `closures.stk` | Closures, captura, submit inline |

## Concurrencia

| Archivo | Demuestra |
|---------|-----------|
| `spawn.stk` | `spawn` concurrente |
| `channel.stk` | Channel + WaitGroup + workers |
| `buffered.stk` | Channel buffered |
| `channel_string.stk` | `Channel<string>` |
| `await_recv.stk` | `await ch.recv()` |
| `await_wait.stk` | `await wg.wait()` |
| `mutex.stk` | `Mutex<int>` |

## Result / Option

| Archivo | Demuestra |
|---------|-----------|
| `result_option.stk` | `.ok`/`.err`/`.some`/`.none` + `match` |

## Mini-recetas

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

### Closure que captura y hace submit

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
