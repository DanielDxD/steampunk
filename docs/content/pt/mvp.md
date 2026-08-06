---
title: Status do MVP
description: O que o compilador v0.1 já faz e o que ainda está na SPEC futura.
---

# Status do MVP

Versão documentada: **0.1.0-draft**. Compilador em Rust (Cranelift). Runtime atual: **thread-backed**.

## Já funciona

- `@import "std"` e módulos locais `@import ":path"`
- `fn` / `async fn` / `async fn main` / `fn main`
- `var`, `const`, `int` / `string` / `bool`, arrays fixos
- Controle: `if`, `while`, `for` (range e channel), `match`
- Classes, herança `::`, propriedades, `drop`, `iclass` (conforme exemplos)
- `Future.ready` / `join` / `race` (`int`), `async { … }` → `Future<int>`
- `spawn`, `Channel<int|string>`, `WaitGroup`, `Mutex<int>`
- `await ch.recv()`, `await wg.wait()` em contexto async
- Closures + `std.cpu.submit` (`fn() int`)
- `std.Result<int, string>` / `std.Option<int>`

## Ainda fora / parcial

| Item | Notas |
|------|--------|
| Scheduler M:N | Visão final; MVP usa threads OS |
| `std.parallel.*` | Planejado |
| Genéricos reais | Só pares MVP (`Result`/`Option`/`Channel`) |
| `T?` / `null` / `?` | Fora |
| `join`/`race`/`ready` string | Fora; `Future<string>` via async fn |
| Mutex outros tipos | Só `int` |
| Closures async / Fn em fields | Fora |
| I/O async (`http`, `fs`) | Fora |
| Float / inteiros de largura fixa | SPEC completa; não no recorte atual |
| Borrow checker completo | Evolução pós-MVP |

## Onde aprofundar

- Especificação normativa: `SPEC.md` na raiz
- Compilador: `compiler/README.md`
- Exemplos executáveis: `examples/`

Esta documentação de usuário acompanha o MVP e será expandida conforme a linguagem crescer.
