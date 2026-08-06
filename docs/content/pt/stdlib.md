---
title: Biblioteca padrão
description: APIs std disponíveis no MVP e visão da biblioteca completa.
---

# Biblioteca padrão

Importe sempre que for usar a std:

```stk
@import "std"
```

## Disponível no MVP

### Logging e tempo

| API | Descrição |
|-----|-----------|
| `std.log(fmt, …)` | Imprime com placeholders `$1`, `$2`, … |
| `std.sleep(ms)` | Espera milissegundos |

### Sync

| API | Descrição |
|-----|-----------|
| `std.sync.Channel<T>.new()` | Canal ilimitado (`int` \| `string`) |
| `std.sync.Channel<T>.buffered(n)` | Canal com capacidade |
| `std.sync.WaitGroup.new()` | Contador de conclusão |
| `std.sync.Mutex<int>.new(v)` | Mutex de `int` |

Métodos de channel: `send`, `recv`, `close`.  
WaitGroup: `add`, `done`, `wait`.  
Mutex: `lock`, `unlock`, `get`, `set`.

### Future (tipo de linguagem + helpers)

| API | Descrição |
|-----|-----------|
| `Future.ready(v)` | `Future<int>` pronto |
| `Future.join(a, b)` | Dois `Future<int>` → array `[int; 2]` |
| `Future.race(a, b)` | Primeiro a completar |

### CPU

| API | Descrição |
|-----|-----------|
| `std.cpu.submit(fn)` | `fn() int` nomeada ou closure → `Future<int>` |

### Result / Option

| API | Descrição |
|-----|-----------|
| `std.Result<int, string>.ok` / `.err` | Resultado tipado |
| `std.Option<int>.some` / `.none` | Opcional tipado |

## Planejado (SPEC / versões futuras)

- `std.parallel.map` / `forEach` / `reduce` / `invoke`
- `std.http`, `std.fs`, `std.net`, `std.time` async
- `std.sync.RwLock`, atomics
- Scheduler M:N com work-stealing
- `std.env`, `std.process.exit`, genéricos amplos

A especificação completa da linguagem está em `SPEC.md` na raiz do repositório.
