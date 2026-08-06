---
title: Começando
description: Como compilar e executar programas Steampunk com o compilador MVP.
---

# Começando

## Requisitos

- Rust (toolchain estável) e Cargo
- Um linker C (`cc`, `clang` ou `gcc`) para `build` nativo
- macOS ou Linux (testado no desenvolvimento atual)

## Clonar e rodar

A partir da raiz do repositório:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/hello.stk
```

Outros exemplos úteis:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/math.stk
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/closures.stk
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/channel.stk
```

## `run` vs `build`

| Comando | O que faz |
|---------|-----------|
| `run arquivo.stk` | JIT Cranelift em memória e executa `main` |
| `build arquivo.stk --out caminho` | Objeto nativo + link → binário |

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- \
  build examples/math.stk --out build/math
./build/math
```

## Ponto de entrada

Todo executável precisa de exatamente um `main`:

```stk
@import "std"

fn main() {
    std.log("sync main")
}
```

Ou assíncrono (permite `await` no entry):

```stk
@import "std"

async fn main() {
    var n = await Future.ready(42)
    std.log("n=$1", n)
}
```

## Estrutura típica de projeto

```text
meu-app/
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

O prefixo `:` em `@import ":modules/math"` resolve módulos locais a partir da raiz do projeto / entry.
