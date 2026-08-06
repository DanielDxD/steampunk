---
title: Sintaxe básica
description: Comentários, literais, tipos, variáveis e mutabilidade em Steampunk.
---

# Sintaxe básica

## Comentários

```stk
// linha

/*
  bloco
*/
```

## Identificadores

- Começam com letra ou `_`
- Case-sensitive
- Convenções: tipos em `PascalCase`, funções/vars em `camelCase`, consts em `SCREAMING_SNAKE`

## Literais (MVP)

| Tipo | Exemplos |
|------|----------|
| `int` | `42`, `0`, `-7` |
| `string` | `"Hello"`, `"linha\n"` |
| `bool` | `true`, `false` |

Strings aceitam escapes: `\n`, `\t`, `\r`, `\\`, `\"`.

## Tipos primitivos no MVP

O compilador atual tipa de forma estável:

- `int`, `string`, `bool`, `void`
- Arrays fixos `[T; N]` (elementos `int` / `string` / `bool`)
- `Future<T>`, `Channel<T>` (hoje `T ∈ {int, string}`)
- `std.Result<int, string>`, `std.Option<int>`
- Função / closure: `fn(…) Ret`

A SPEC completa prevê mais larguras (`i32`, `float`, etc.); veja [Status do MVP](./mvp).

## Variáveis

Não existe `let`. Use `var` (mutável) e `const` (compile-time):

```stk
var counter = 0
counter = counter + 1

const MAX = 128
var name string = "Ada"
```

`var` infere o tipo pela inicialização quando possível.

## Operadores aritméticos e comparação

```stk
var a = 10 + 3
var b = a * 2
var ok = b >= 20
var same = "x" == "x"
```

Operadores: `+ - * / %`, `== != < <= > >=`, `&& ||`, `!` (negação lógica), `-` unário.

## Arrays

```stk
var xs = [10, 20, 30]
std.log("len=$1 first=$2", xs.len, xs[0])
```

- Literal não pode ser vazio no MVP
- `.len` é propriedade do array
- Índice fora do intervalo é erro em runtime

## Logging

```stk
@import "std"

fn main() {
    std.log("só texto")
    std.log("valor=$1 nome=$2", 42, "Ada")
}
```

Placeholders `$1`, `$2`, … correspondem aos argumentos na ordem.
