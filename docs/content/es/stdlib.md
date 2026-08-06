---
title: Biblioteca estándar
description: APIs std disponibles en el MVP y visión de la biblioteca completa.
---

# Biblioteca estándar

Importa siempre que vayas a usar la std:

```stk
@import "std"
```

## Disponible en el MVP

### Logging y tiempo

| API | Descripción |
|-----|-------------|
| `std.log(fmt, …)` | Imprime con placeholders `$1`, `$2`, … |
| `std.sleep(ms)` | Espera milisegundos |

### Sync

| API | Descripción |
|-----|-------------|
| `std.sync.Channel<T>.new()` | Canal ilimitado (`int` \| `string`) |
| `std.sync.Channel<T>.buffered(n)` | Canal con capacidad |
| `std.sync.WaitGroup.new()` | Contador de finalización |
| `std.sync.Mutex<int>.new(v)` | Mutex de `int` |

Métodos de channel: `send`, `recv`, `close`.  
WaitGroup: `add`, `done`, `wait`.  
Mutex: `lock`, `unlock`, `get`, `set`.

### Future (tipo de lenguaje + helpers)

| API | Descripción |
|-----|-------------|
| `Future.ready(v)` | `Future<int>` listo |
| `Future.join(a, b)` | Dos `Future<int>` → array `[int; 2]` |
| `Future.race(a, b)` | Primero en completar |

### CPU

| API | Descripción |
|-----|-------------|
| `std.cpu.submit(fn)` | `fn() int` nombrada o closure → `Future<int>` |

### Result / Option

| API | Descripción |
|-----|-------------|
| `std.Result<int, string>.ok` / `.err` | Resultado tipado |
| `std.Option<int>.some` / `.none` | Opcional tipado |

## Planeado (SPEC / versiones futuras)

- `std.parallel.map` / `forEach` / `reduce` / `invoke`
- `std.http`, `std.fs`, `std.net`, `std.time` async
- `std.sync.RwLock`, atomics
- Scheduler M:N con work-stealing
- `std.env`, `std.process.exit`, genéricos amplios

La especificación completa del lenguaje está en `SPEC.md` en la raíz del repositorio.
