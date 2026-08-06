---
title: Visão geral
description: Steampunk é uma linguagem compilada, estaticamente tipada e async-first — com concorrência e paralelismo como pilares.
---

# Visão geral

**Steampunk** (extensão `.stk`) é uma linguagem orientada a objetos, tipada estaticamente e compilada para código nativo. O design equilibra desempenho previsível com DX clara: módulos explícitos, erros amigáveis e async no núcleo.

## Pilares

- Compilação AOT / JIT para nativo (compilador em Rust + Cranelift)
- Tipagem estática com inferência local (`var x = 10`)
- OOP com `class`, herança (`::`), interfaces (`iclass`)
- `async` / `await` e tipo `Future<T>`
- `spawn` no estilo goroutine + `Channel` / `WaitGroup` / `Mutex`
- Closures e `std.cpu.submit` para trabalho CPU
- Módulos via `@import`

## Olá, mundo

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

## Filosofia

| Conceito | Papel |
|----------|--------|
| Concorrência | Muitas tarefas sobrepostas (`async`, `spawn`, channels) |
| Paralelismo | Trabalho CPU em vários núcleos (`std.cpu.submit`) |
| Tipagem | Erros cedo, inferência onde o tipo é óbvio |
| Módulos | Imports explícitos; só `pub` cruza fronteiras |

> Esta documentação cobre a linguagem e o que o **MVP v0.1** do compilador já executa. Detalhes do recorte atual estão em [Status do MVP](./mvp).

## Próximo passo

Siga para [Começando](./getting-started) para instalar o compilador e rodar os exemplos do repositório.
