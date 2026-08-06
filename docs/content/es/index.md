---
title: Visión general
description: Steampunk es un lenguaje compilado, estáticamente tipado y async-first — con concurrencia y paralelismo como pilares.
---

# Visión general

**Steampunk** (extensión `.stk`) es un lenguaje orientado a objetos, tipado estáticamente y compilado a código nativo. El diseño equilibra rendimiento predecible con DX clara: módulos explícitos, errores amigables y async en el núcleo.

## Pilares

- Compilación AOT / JIT a nativo (compilador en Rust + Cranelift)
- Tipado estático con inferencia local (`var x = 10`)
- OOP con `class`, herencia (`::`), interfaces (`iclass`)
- `async` / `await` y tipo `Future<T>`
- `spawn` al estilo goroutine + `Channel` / `WaitGroup` / `Mutex`
- Closures y `std.cpu.submit` para trabajo CPU
- Módulos vía `@import`

## Hola, mundo

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

## Filosofía

| Concepto | Rol |
|----------|-----|
| Concurrencia | Muchas tareas superpuestas (`async`, `spawn`, channels) |
| Paralelismo | Trabajo CPU en varios núcleos (`std.cpu.submit`) |
| Tipado | Errores tempranos, inferencia donde el tipo es obvio |
| Módulos | Imports explícitos; solo `pub` cruza fronteras |

> Esta documentación cubre el lenguaje y lo que el **MVP v0.1** del compilador ya ejecuta. Detalles del recorte actual están en [Estado del MVP](./mvp).

## Siguiente paso

Sigue a [Empezando](./getting-started) para instalar el compilador y ejecutar los ejemplos del repositorio.
