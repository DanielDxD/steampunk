---
title: Standard library
description: std APIs available in the MVP and the full library vision.
---

# Standard library

Always import when using std:

```stk
@import "std"
```

## Available in the MVP

### Logging and time

| API | Description |
|-----|-------------|
| `std.log(fmt, …)` | Prints with `$1`, `$2`, … placeholders |
| `std.sleep(ms)` | Waits milliseconds |

### Sync

| API | Description |
|-----|-------------|
| `std.sync.Channel<T>.new()` | Unbounded channel (`int` \| `string`) |
| `std.sync.Channel<T>.buffered(n)` | Channel with capacity |
| `std.sync.WaitGroup.new()` | Completion counter |
| `std.sync.Mutex<int>.new(v)` | Mutex over `int` |

Channel methods: `send`, `recv`, `close`.  
WaitGroup: `add`, `done`, `wait`.  
Mutex: `lock`, `unlock`, `get`, `set`.

### Future (language type + helpers)

| API | Description |
|-----|-------------|
| `Future.ready(v)` | Ready `Future<int>` |
| `Future.join(a, b)` | Two `Future<int>` → `[int; 2]` array |
| `Future.race(a, b)` | First to complete |

### CPU

| API | Description |
|-----|-------------|
| `std.cpu.submit(fn)` | Named or closure `fn() int` → `Future<int>` |

### Result / Option

| API | Description |
|-----|-------------|
| `std.Result<int, string>.ok` / `.err` | Typed result |
| `std.Option<int>.some` / `.none` | Typed optional |

## Planned (SPEC / future versions)

- `std.parallel.map` / `forEach` / `reduce` / `invoke`
- Async `std.http`, `std.fs`, `std.net`, `std.time`
- `std.sync.RwLock`, atomics
- M:N scheduler with work-stealing
- `std.env`, `std.process.exit`, broad generics

The full language specification is in `SPEC.md` at the repository root.
