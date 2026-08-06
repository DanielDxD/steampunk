---
title: Módulos e imports
description: "@import, visibilidade pub e organização de projetos .stk."
---

# Módulos e imports

## Diretiva `@import`

Imports ficam no topo do arquivo, antes das declarações:

```stk
@import "std"            // biblioteca padrão
@import ":modules/math"  // módulo local
```

| Forma | Significado |
|-------|-------------|
| `"nome"` | Pacote / std (MVP: só `"std"`) |
| `":caminho"` | Arquivo local relativo à raiz do projeto / entry |

## Visibilidade de módulo

| Modificador | Escopo |
|-------------|--------|
| (omitido) | Privado ao arquivo |
| `pub` | Visível para quem importar |

Aplica-se a `fn`, `class`, `iclass`, `const` de nível de arquivo.

`fn main` / `async fn main` **não** precisa de `pub` — é ponto de entrada, não export.

## Exemplo prático

`modules/math.stk`:

```stk
pub fn add(int a, int b) int {
    return a + b
}
```

`modules_main.stk`:

```stk
@import "std"
@import ":modules/math"

const FACTOR = 2

fn main() {
    var xs = [10, 20, 30]
    var s = add(xs[0], xs[1])
    std.log("sum=$1 len=$2 factor=$3", s, xs.len, FACTOR)
}
```

## Regras

1. Ciclos de import são erro de compilação
2. Cada `.stk` é uma unidade de módulo
3. Só membros `pub` cruzam a fronteira do módulo
4. No MVP, imports além de `"std"` e módulos locais `:…` não estão habilitados
