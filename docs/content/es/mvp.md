---
title: Estado del MVP
description: Lo que el compilador v0.1 ya hace y lo que aún está en la SPEC futura.
---

# Estado del MVP

Versión documentada: **0.1.0-draft**. Compilador en Rust (Cranelift). Runtime actual: **thread-backed**.

## Ya funciona

- `@import "std"` y módulos locales `@import ":path"`
- `fn` / `async fn` / `async fn main` / `fn main`
- `var`, `const`, `int` / `string` / `bool`, arrays fijos
- Control: `if`, `while`, `for` (rango y channel), `match`
- Clases, herencia `::`, propiedades, `drop`, `iclass` (según ejemplos)
- `Future.ready` / `join` / `race` (`int`), `async { … }` → `Future<int>`
- `spawn`, `Channel<int|string>`, `WaitGroup`, `Mutex<int>`
- `await ch.recv()`, `await wg.wait()` en contexto async
- Closures + `std.cpu.submit` (`fn() int`)
- `std.Result<int, string>` / `std.Option<int>`

## Aún fuera / parcial

| Ítem | Notas |
|------|-------|
| Scheduler M:N | Visión final; MVP usa threads OS |
| `std.parallel.*` | Planeado |
| Genéricos reales | Solo pares MVP (`Result`/`Option`/`Channel`) |
| `T?` / `null` / `?` | Fuera |
| `join`/`race`/`ready` string | Fuera; `Future<string>` vía async fn |
| Mutex otros tipos | Solo `int` |
| Closures async / Fn en fields | Fuera |
| I/O async (`http`, `fs`) | Fuera |
| Float / enteros de ancho fijo | SPEC completa; no en el recorte actual |
| Borrow checker completo | Evolución post-MVP |

## Dónde profundizar

- Especificación normativa: `SPEC.md` en la raíz
- Compilador: `compiler/README.md`
- Ejemplos ejecutables: `examples/`

Esta documentación de usuario acompaña el MVP y se ampliará conforme crezca el lenguaje.
