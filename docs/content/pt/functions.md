---
title: Funções e closures
description: Funções nomeadas, async, parâmetros com default e closures como valores.
---

# Funções e closures

## Funções nomeadas

Parâmetros usam **tipo antes do nome**. Retorno vem após a lista; omitido = `void`.

```stk
fn add(int a, int b) int {
    return a + b
}

fn greet(string name = "world") {
    std.log("Hi $1", name)
}
```

- Defaults só com literais (`int` / `string` / `bool`)
- Params com default vêm depois dos obrigatórios
- `pub fn` exporta o símbolo do módulo

## Funções `async`

`async fn` retorna `Future<T>`, onde `T` é o tipo anotado:

```stk
async fn fetchAnswer() int {
    std.sleep(10)
    return 42
}

async fn main() {
    var n = await fetchAnswer()
    std.log("$1", n)
}
```

Regras importantes:

1. `await` só dentro de `async fn` ou bloco `async { … }`
2. Chamar `async fn` **não** bloqueia — devolve um `Future`
3. `async fn main` é o entry assíncrono do programa

## Closures

Closures são expressões `fn` sem nome. Viram valores do tipo função e podem capturar locais:

```stk
@import "std"

async fn main() {
    var base = 40
    var f = fn() int { return base + 2 }
    var r = await std.cpu.submit(f)
    std.log("stored=$1", r)

    var g = fn(int n) int { return n + base }
    std.log("call=$1", g(2))

    var r2 = await std.cpu.submit(fn() int { return 41 + 1 })
    std.log("inline=$1", r2)
}
```

### MVP de closures

| Permitido | Detalhe |
|-----------|---------|
| Params | `int`, `string`, `bool` (0 ou N) |
| Retorno | `int`, `string`, `void` |
| Captura | Locais livres do escopo (como `spawn`) |
| Chamada | `f(args)` quando `f` é valor função |
| `cpu.submit` | só `fn() int` (nome global ou closure) |

Ainda fora: closures `async`, `Fn` em campos de classe, `std.parallel.map`.

## `std.cpu.submit`

Agenda trabalho síncrono pesado numa thread OS e devolve `Future<int>`:

```stk
fn heavy() int {
    std.sleep(30)
    return 42
}

async fn main() {
    var a = await std.cpu.submit(heavy)
    var b = await std.cpu.submit(fn() int { return heavy() })
    std.log("$1 $2", a, b)
}
```

Use `submit` para CPU-bound; use `spawn` para concorrência estilo goroutine.
