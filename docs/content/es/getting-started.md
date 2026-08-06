---
title: Empezando
description: Cómo compilar y ejecutar programas Steampunk con el compilador MVP.
---

# Empezando

## Requisitos

- Rust (toolchain estable) y Cargo
- Un linker C (`cc`, `clang` o `gcc`) para `build` nativo
- macOS o Linux (probado en el desarrollo actual)

## Clonar y ejecutar

Desde la raíz del repositorio:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/hello.stk
```

Otros ejemplos útiles:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/math.stk
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/closures.stk
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/channel.stk
```

## `run` vs `build`

| Comando | Qué hace |
|---------|----------|
| `run archivo.stk` | JIT Cranelift en memoria y ejecuta `main` |
| `build archivo.stk --out ruta` | Objeto nativo + link → binario |

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- \
  build examples/math.stk --out build/math
./build/math
```

## Punto de entrada

Todo ejecutable necesita exactamente un `main`:

```stk
@import "std"

fn main() {
    std.log("sync main")
}
```

O asíncrono (permite `await` en el entry):

```stk
@import "std"

async fn main() {
    var n = await Future.ready(42)
    std.log("n=$1", n)
}
```

## Estructura típica de proyecto

```text
mi-app/
  main.stk
  modules/
    math.stk
```

```stk
@import "std"
@import ":modules/math"

fn main() {
    std.log("$1", add(2, 3))
}
```

El prefijo `:` en `@import ":modules/math"` resuelve módulos locales a partir de la raíz del proyecto / entry.
