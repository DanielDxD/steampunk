# Steampunk (`.stk`) — Especificação da Linguagem

**Versão:** 0.1.0-draft  
**Compilador:** escrito em Rust  
**Extensão de arquivo:** `.stk`  
**Paradigma:** orientada a objetos, tipagem estática, compilada, async-first  
**Objetivo de design:** extrema performance com excelente DX; paralelismo e concorrência como pilares

---

## 1. Visão geral

Steampunk é uma linguagem compilada, estaticamente tipada e orientada a objetos. O compilador gera código nativo com foco em desempenho previsível, enquanto a sintaxe prioriza clareza, módulos explícitos e erros amigáveis.

Concorrência (muitas tarefas sobrepostas, I/O) e paralelismo (uso de múltiplos núcleos de CPU) são cidadãos de primeira classe: `async`/`await`, tipo `Future<T>`, `spawn` no estilo goroutine e APIs de paralelismo de dados na `std`.

Características centrais:

- Compilação ahead-of-time (AOT) para binário nativo
- Tipagem estática com inferência local onde o tipo é óbvio
- Orientação a objetos com classes, herança e interfaces (`iclass`)
- Async nativo: funções `async` retornam `Future<T>`
- `spawn` lança goroutines leves (modelo M:N); sincronização via channels
- Runtime com scheduler work-stealing para concorrência e paralelismo
- Sistema de módulos com imports explícitos (`@import`)
- Biblioteca padrão (`std`) sempre disponível via import
- Sem garbage collector obrigatório na v0.1 — modelo de ownership inspirado em Rust (ver §13)

---

## 2. Convenções léxicas

### 2.1 Comentários

```stk
// Comentário de linha

/*
  Comentário de bloco
*/
```

### 2.2 Identificadores

- Começam com letra ou `_`, seguidos de letras, dígitos ou `_`
- Case-sensitive
- Convenções recomendadas:
  - tipos/classes/interfaces: `PascalCase` (`MyModule`, `IFoo`)
  - funções/métodos/variáveis: `camelCase` (`sumResult`, `getName`)
  - constantes: `SCREAMING_SNAKE` (`MAX_SIZE`)

### 2.3 Palavras-chave

```
fn        async     await     spawn     pub
priv      prot      class     iclass    new
var       const     return    if        else
while     for       break     continue  match
do        try       catch
true      false     null      self      super
as        import    module    defer
```

`@import` é uma diretiva de preprocessamento/compilação (atributo de módulo), não uma palavra-chave de expressão.

### 2.4 Literais

| Tipo     | Exemplos                          |
|----------|-----------------------------------|
| Inteiro  | `42`, `0`, `-7`, `1_000_000`      |
| Float    | `3.14`, `0.5`, `-1.0`             |
| Bool     | `true`, `false`                   |
| String   | `"Hello"`, `"linha\n"`            |
| Char     | `'a'`, `'\n'`                     |
| Null     | `null` (apenas em tipos opcionais)|

Strings suportam escapes: `\n`, `\t`, `\r`, `\\`, `\"`, `\$`.

---

## 3. Programa e ponto de entrada

Todo executável deve definir `fn main()` em algum arquivo raiz (por convenção `main.stk`).

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

- `main` não recebe argumentos na v0.1 (args de CLI virão via `std.env`)
- `main` pode ser síncrona (`fn main()`) ou assíncrona (`async fn main()`)
- `async fn main()` retorna `Future<void>`; o runtime inicia o scheduler e faz poll até completar
- Saída de processo usa `std.process.exit(code)`
- O compilador exige exatamente um `main` no grafo de compilação de um binário

---

## 4. Módulos e imports

### 4.1 Diretiva `@import`

```stk
@import "std"           // biblioteca padrão / pacote externo
@import ":module"       // módulo local na raiz do projeto
@import ":modules/math" // módulo local em subpasta
```

Regras:

1. **Pacote nomeado** (`"nome"`): resolve em dependências do manifesto `.stkm` ou na stdlib. O compilador e o LSP usam o **artefato pré-compilado** da dep (binário + sourcemap) — não recompilam o código-fonte da dependência (ver §17).
2. **Módulo local** (`":caminho"`): o `:` indica a raiz do projeto. Resolução MVP: primeiro o diretório do arquivo **entry**, depois o ancestral com `manager.stkm` (se existir). Pastas usam `/`.
3. Imports devem aparecer no topo do arquivo, antes de declarações.
4. Só membros `pub` de um módulo são visíveis fora dele. (`fn main` / `async fn main` no entry **não** precisa de `pub` — é o ponto de entrada, não um export.)
5. Importar um arquivo local traz ao escopo os tipos/funções públicos exportados por ele. Importar um pacote usa a API pública descrita no sourcemap do artefato.

Exemplo de módulo (`module.stk`):

```stk
pub class MyModule {
    pub fn sum(int a, int b) int {
        return a + b
    }
}
```

Uso (`main.stk`):

```stk
@import "std"
@import ":module"

fn main() {
    var myMod = new MyModule()
    var sumResult = myMod.sum(10, 40)
    std.log("O resultado é $1", sumResult)
}
```

### 4.2 Unidade de compilação

- Cada arquivo `.stk` é uma unidade de módulo.
- O nome do tipo principal público costuma espelhar o arquivo, mas não é obrigatório.
- Ciclos de import são erro de compilação.

### 4.3 Visibilidade

#### Módulo / top-level

| Modificador | Escopo |
|-------------|--------|
| (omitido)   | Privado ao arquivo/módulo |
| `pub`       | Visível para quem importar o módulo |

Aplica-se a: `class`, `iclass`, `fn` e `const` de nível de arquivo.

#### Membros de classe (métodos e propriedades)

Métodos e propriedades (`var` de instância) usam **obrigatoriamente** um destes modificadores:

| Modificador | Nome       | Escopo |
|-------------|------------|--------|
| `pub`       | public     | Acessível de qualquer código que veja o tipo |
| `priv`      | private    | Acessível apenas dentro da própria classe |
| `prot`      | protected  | Acessível na própria classe e em subclasses |

```stk
pub class Account {
    priv var balance int
    prot var status string
    pub var owner string

    pub fn new(string owner) Account {
        self.owner = owner
        self.balance = 0
        self.status = "active"
        return self
    }

    pub fn deposit(int amount) {
        self.balance = self.balance + amount
    }

    prot fn setStatus(string status) {
        self.status = status
    }

    priv fn assertPositive(int amount) {
        if amount <= 0 {
            std.panic("amount must be > 0")
        }
    }
}

class VipAccount :: Account {
    pub fn freeze() {
        self.setStatus("frozen")   // ok: setStatus é prot
        // self.balance = 0        // erro: balance é priv
    }
}
```

Regras:

1. Todo campo e método de classe deve declarar `pub`, `priv` ou `prot`.
2. `priv` não é visível em subclasses (nem via `super`).
3. `prot` é visível em subclasses da hierarquia, no mesmo ou em outro módulo.
4. Implementações de métodos de `iclass` devem ser `pub` (o contrato da interface é público).
5. Visibilidade de módulo (`pub class`) é independente da visibilidade dos membros.

### 4.4 Decorators

Decorators anotam **membros** de `class`/`struct` (campos e propriedades). Sintaxe:

```stk
@name
@name("arg")
@name(1, true)
```

Ordem no membro: decorators → `pub`|`priv`|`prot` → `var` …

```stk
struct User {
    @encodeProperty("user_name")
    pub var name string
    pub var age int
    @ignore
    pub var password string
}
```

Regras:

1. `@import` permanece **diretiva de módulo** só no topo do arquivo — não é decorator de membro.
2. Decorators de usuário definidos em Steampunk **não** existem na v0.1; só built-ins.
3. Built-ins de serialização (valem para `std.json` / `yaml` / `toml` / `toon`):

| Decorator | Efeito |
|-----------|--------|
| `@encodeProperty(string wire)` | Nome da chave no wire (encode e decode) |
| `@ignore` | Campo excluído do encode/decode |

4. `@ignore` e `@encodeProperty` juntos no mesmo membro = erro.
5. Decorators em métodos = erro.
6. Sem `@encodeProperty`, o nome wire é o identificador do campo.

---

## 5. Tipos

### 5.1 Tipos primitivos

| Tipo     | Descrição                          |
|----------|------------------------------------|
| `bool`   | Booleano                           |
| `int`    | Inteiro com sinal, largura da plataforma (padrão `i64` em 64-bit) |
| `i8` `i16` `i32` `i64` | Inteiros com sinal de largura fixa |
| `u8` `u16` `u32` `u64` | Inteiros sem sinal                 |
| `float`  | Ponto flutuante padrão (`f64`)     |
| `f32` `f64` | Floats de largura fixa          |
| `string` | Sequência UTF-8 imutável           |
| `char`   | Code point Unicode escalar         |
| `void`   | Ausência de valor (só retorno)     |

### 5.2 Tipos compostos

```stk
// Array de tamanho fixo
var xs = [1, 2, 3]          // tipo: [int; 3]

// Slice / lista dinâmica (stdlib)
var ys = std.List<int>.new()

// Opcional
var maybe int? = null

// Resultado (stdlib)
var r = std.Result<int, string>.ok(10)

// Função
var op = fn(int a, int b) int { return a + b }

// Future — valor assíncrono ainda não resolvido
var pending Future<int> = fetchCount()
```

### 5.3 Inferência

`var` infere o tipo pela inicialização. Anotação explícita é permitida:

```stk
var x = 10           // int
var y int = 10       // int explícito
var name = "Ada"     // string
```

### 5.4 Tipos de valor (regra geral)

Qualquer posição de **tipo de valor** — propriedades/`var` de classe, parâmetros, retornos, elementos de `List`/`Channel`/`Mutex`/`RwLock`/array, payloads de `Result`/`Option`, parâmetro de `Future<T>`, argumentos de tipo genérico — aceita:

| Categoria | Exemplos |
|-----------|----------|
| Primitivos | `bool`, `int`, `float`, `string` (e larguras fixas quando existirem) |
| Compostos | `[T; N]`, `Future<T>`, `std.List<T>`, `std.Result<A,B>`, `std.Option<T>` |
| Nomeados | `class` e `iclass` (interfaces como tipo de parâmetro/campo/retorno; valor concreto deve implementar a `iclass`) |
| Função | `fn(…) Ret` (closures) |

**Não** é tipo de valor: `void` (só retorno explícito de função). Usar `void` como elemento/payload é erro de compilação.

O typechecker resolve nomes contra o conjunto de `class`/`iclass` do programa em **O(1)** médio (hash set), usa atribuição nominal com subtipagem (`class` → base / `iclass` implementada) e evita reparse.

---

## 6. Variáveis e mutabilidade

Não existe `let`. Bindings locais e campos usam apenas `var`. Constantes de compile-time usam `const`.

| Declaração | Mutável | Observação                |
|------------|---------|---------------------------|
| `var`      | sim     | reatribuível              |
| `const`    | não     | constante de compile-time |

```stk
var counter = 0
counter = counter + 1

const MAX_CONNECTIONS = 128
```

Campos de classe usam `var` e seguem os modificadores de visibilidade (ver §9).

---

## 7. Funções

### 7.1 Sintaxe

```stk
fn nome(tipo param1, tipo param2 = literal) TipoRetorno {
    return expressao
}
```

- Parâmetros: `tipo nome` (tipo antes do nome)
- Defaults opcionais: `tipo nome = literal` (`int` / `string` / `bool`); params com default vêm **depois** dos obrigatórios
- Em chamadas, argumentos omitidos são preenchidos pelo compilador (sem named args)
- Tipo de retorno: após a lista de parâmetros
- Se omitido o retorno, assume-se `void`
- `return` sem valor só é válido em funções `void`

```stk
fn add(int a, int b) int {
    return a + b
}

fn greet(string name = "world") {
    std.log("Hi $1", name)
}
```

### 7.2 Funções públicas

```stk
pub fn exported() {
    // ...
}
```

### 7.3 Funções assíncronas

Funções `async` **sempre retornam** `Future<T>`, onde `T` é o tipo resolvido anotado (ou inferido) na assinatura.

```stk
async fn fetchCount() int {
    var body = await std.http.get("http://example.com/count")
    return std.string.parseInt(body)
}
// tipo efetivo: () -> Future<int>
```

Regras:

1. O modificador `async` antecede `fn` (em métodos: `pub async fn …`, `priv async fn …` ou `prot async fn …`).
2. O tipo escrito após os parâmetros é o tipo **dentro** do `Future`, não `Future` aninhado.
3. `return expr` numa `async fn` completa o `Future` com `expr` tipado como `T`.
4. `await` só é permitido dentro de `async fn` (ou blocos `async`).
5. Chamar uma `async fn` **não** bloqueia: devolve um `Future<T>` imediatamente.
6. Métodos de classe podem ser `async` da mesma forma.

```stk
pub class ApiClient {
    pub async fn getUser(string id) User {
        var raw = await std.http.get("/users/" + id)
        return User.parse(raw)
    }
}
```

### 7.4 Overload

Não suportado. Use nomes distintos ou genéricos (`fn f<T>(…)`).

---

## 8. Async, `Future`, paralelismo e concorrência

Steampunk trata concorrência e paralelismo como parte do núcleo da linguagem — não como biblioteca opcional.

> **MVP do compilador (v0.1 — mínimo usável):** o runtime atual é **thread-backed**, não o scheduler M:N completo da visão final.
> - Tipos de valor em propriedades, params, retornos, arrays, closures, `List` / `Channel` / `Mutex` / `RwLock` / `Result` / `Option` / `Future`: **qualquer tipo de valor** (primitivos + `class` / `iclass`), exceto `void` como elemento/payload.
> - `Future.ready` / `join` / `race` e `std.cpu.submit` aceitam qualquer tipo de valor (ABI `i64`/ponteiro).
> - Chamar `async fn` (exceto `main`) agenda o corpo numa thread OS e devolve um `Future` pendente; `await` espera o complete.
> - `spawn` / `spawn { … }` (fire-and-forget) com **worker pool**; captura locais; o driver drena spawns antes de sair.
> - `Channel` / `WaitGroup` / `Mutex` / `RwLock`; em `async`, `ch.recv()` / `wg.wait()` tipam como `Future`.
> - Expressão `async { … }` → `Future<int>` (MVP do bloco; `async fn` pode retornar qualquer `T`).
> - Closures `fn(params) Ret {…}` com params/retorno de qualquer tipo de valor (`void` só no retorno).
> - Entry: `fn main` ou `async fn main`.
> - `float` (f64): sem promoção implícita `int`→`float`; `%` só para `int`.
> - `std.string`, `std.env`, `std.process`, `std.fs`, `std.time`, `std.panic`, `std.http` client+server (`http://`), serde (`json`/`yaml`/`toml`/`toon`), `std.task.yield` / `CancellationToken`.
> - `std.parallel.map(List<int>, fnName)` aplica `fn(int) int` em paralelo.
> - `struct` = sinônimo de `class`. Genéricos de usuário (`fn f<T>`, `class Box<T>`) com monomorphização.
> - CLI: `deps`, `script`, `test`, `fmt`; LSP: `steampunk-lsp`.
> - Ainda **parcial**: borrow checker (`&`/`&mut`); scheduler M:N cooperativo; HTTPS; constraints `T: Iface` em genéricos.
| Conceito        | Uso principal                                      |
|-----------------|----------------------------------------------------|
| Concorrência    | Muitas tarefas sobrepostas (I/O, timers, rede)     |
| Paralelismo     | Trabalho CPU-bound em vários núcleos               |
| `Future<T>`     | Resultado assíncrono ainda não disponível          |
| `async`/`await` | Expressar e compor operações assíncronas           |
| `spawn`         | Goroutine: lança execução concorrente leve (`void`) |
| `Channel`       | Comunicação / sincronização entre goroutines       |
| `std.parallel`  | Paralelismo de dados / fan-out em thread pool      |

### 8.1 Tipo `Future<T>`

`Future<T>` representa uma computação que produz um `T` (ou falha via `std.Result` se o `T` for um Result). É um tipo da linguagem / std, pollável pelo runtime.

Operações essenciais:

| API | Descrição |
|-----|-----------|
| `await future` | Suspende a tarefa corrente até o `Future` completar; tipo da expressão = `T` |
| `future.map(fn)` | Transforma `Future<A>` → `Future<B>` sem bloquear |
| `Future.join(a, b)` | Espera vários futures; retorna `Future<(A, B)>` |
| `Future.race(a, b)` | Completa com o primeiro que terminar |
| `Future.ready(value)` | Future já resolvido |
| `Future.from(fn)` | Empacota trabalho lazy num Future |

```stk
async fn loadAll() (string, string) {
    var home = std.http.get("/")
    var about = std.http.get("/about")
    return await Future.join(home, about)
}
```

### 8.2 `await`

```stk
var value = await someFuture
```

- Avalia um `Future<T>` e produz `T`
- Só em contexto `async`
- Não bloqueia a thread do SO: a tarefa é suspensa e o scheduler executa outras
- `await` em valor que não é `Future` é erro de tipo

### 8.3 `spawn` — goroutines

`spawn` funciona como o `go` do Go: lança uma **goroutine** (tarefa concorrente leve) e retorna imediatamente. A expressão `spawn` tem tipo `void` — **não** devolve `Future` nem handle de join.

```stk
spawn work(10)
spawn {
    std.log("rodando em paralelo")
}
```

Semântica (espelhando goroutines):

1. **Fire-and-forget:** `spawn` só inicia a execução; o chamador segue sem esperar.
2. **Leve / M:N:** milhares ou milhões de goroutines são multiplexadas em poucas threads OS pelo runtime (scheduler work-stealing).
3. **Qualquer callable ou bloco:** `spawn nome(args)`, `spawn asyncFn(args)` ou `spawn { … }`.
4. **Sem valor de retorno pelo `spawn`:** resultados e sincronização usam `Channel`, `WaitGroup` ou estado compartilhado protegido — não `await` no `spawn`.
5. **Independência:** a goroutine vive até o corpo terminar (ou o processo encerrar); panics numa goroutine podem derrubar o processo (política v0.1: abort), salvo recuperação futura na std.
6. **Preempção cooperativa em pontos de await/I/O:** corpos síncronos longos devem ceder (`std.task.yield`) ou ir para `std.cpu.submit` / `std.parallel`.

Comunicação via channel (padrão idiomático, como em Go):

```stk
@import "std"

fn producer(std.sync.Channel<int> ch) {
    ch.send(10)
    ch.send(20)
    ch.close()
}

async fn main() {
    var ch = std.sync.Channel<int>.new()

    spawn producer(ch)

    for v in ch {
        std.log("got $1", v)
    }
}
```

Várias goroutines + agregação:

```stk
@import "std"

fn worker(int id, std.sync.Channel<int> out) {
    out.send(id * 2)
}

async fn main() {
    var out = std.sync.Channel<int>.new()
    var wg = std.sync.WaitGroup.new()

    for i in 0..4 {
        wg.add(1)
        spawn {
            worker(i, out)
            wg.done()
        }
    }

    spawn {
        await wg.wait()
        out.close()
    }

    var sum = 0
    for v in out {
        sum = sum + v
    }
    std.log("sum=$1", sum)
}
```

Composição com `Future` (caminho separado):

- Use `async`/`await` e `Future.join` quando quiser tipar e aguardar resultados sem channels.
- Use `spawn` quando quiser o estilo goroutine: lançar, comunicar por channel, não acoplar ao tipo `Future`.

```stk
// Future: composição tipada
var titles = await Future.join(fetchTitle(a), fetchTitle(b))

// Goroutine: spawn + channel
var ch = std.sync.Channel<string>.new()
spawn { ch.send(await fetchTitle(a)) }
spawn { ch.send(await fetchTitle(b)) }
```

Cancelamento cooperativo via `std.task.CancellationToken` (v0.2+).

### 8.4 Paralelismo (CPU)

Para trabalho CPU-bound, use o pool de threads / work-stealing via `std.parallel` — distinto de async I/O puro:

```stk
@import "std"

fn heavy(int n) int {
    // computação intensiva
    return n * n
}

async fn main() {
    var nums = [1, 2, 3, 4, 5, 6, 7, 8]
    var squares = await std.parallel.map(nums, heavy)
    std.log("done $1 items", squares.len())
}
```

APIs planejadas:

| API | Papel |
|-----|--------|
| `std.parallel.map` | Mapeia coleção em paralelo; retorna `Future<List<R>>` |
| `std.parallel.forEach` | Side-effects em paralelo; retorna `Future<void>` |
| `std.parallel.reduce` | Redução paralela associativa |
| `std.parallel.invoke(a, b, …)` | Executa N closures CPU em paralelo; `Future<(…)>` |
| `std.cpu.submit(fn)` | Enfileira closure síncrona no pool; retorna `Future<T>` |

`std.cpu.submit` é a ponte explícita para trabalho pesado no pool de threads (não confundir com `spawn`/goroutine):

```stk
async fn main() {
    var result = await std.cpu.submit(fn() int {
        return heavyCompute()
    })
    std.log("$1", result)
}
```

### 8.5 Sincronização

Goroutines compartilham memória; a forma idiomática de coordenar é **channel** (como em Go). Locks existem para estado compartilhado explícito.

| Tipo | Uso |
|------|-----|
| `std.sync.Channel<T>` | Comunicação e sincronização entre goroutines |
| `std.sync.WaitGroup` | Esperar um conjunto de goroutines terminar |
| `std.sync.Mutex<T>` | Exclusão mútua |
| `std.sync.RwLock<T>` | Muitos leitores / um escritor |
| `std.sync.Atomic*` | Contadores / flags lock-free |

```stk
var ch = std.sync.Channel<int>.new()

spawn {
    ch.send(42)
}

async fn consumer() {
    var v = await ch.recv()
    std.log("got $1", v)
}
```

Canais:

- `send` / `recv` bloqueiam a goroutine atual (com yield ao runtime), não a thread OS inteira quando possível
- `recv` em contexto `async` também pode ser escrito como `await ch.recv()` → `Future<T>`
- `close()` sinaliza fim da transmissão; iterar `for v in ch` drena até o close
- Buffer opcional: `Channel<int>.buffered(n)`

### 8.6 Modelo de runtime

- Goroutines são unidades leves; scheduler **M:N** (muitas goroutines → N threads OS), work-stealing, N ≈ núcleos por padrão
- `spawn` enfileira a goroutine e retorna na hora (igual `go f()`)
- Yield cooperativo em `await`, I/O, operações de channel e `std.task.yield`
- Trabalho CPU longo sem yield deve usar `std.cpu.submit` / `std.parallel` para não monopolizar o worker
- I/O da std (`std.http`, `std.fs`, `std.net`, `std.time`) é async e retorna `Future`
- `Future` e goroutines convivem: async tipado vs. concorrência estilo Go

### 8.7 Blocos `async`

Expressão/bloco que produz um `Future` inline:

```stk
var f = async {
    var a = await loadA()
    var b = await loadB()
    return a + b
}
var total = await f
```

### 8.8 Regras de tipagem (resumo)

```
async fn f(…) T          ⇒  tipo da fn: (…) -> Future<T>
await Future<T>          ⇒  T
spawn CallOrBlock        ⇒  void        (goroutine; não é Future)
async { … return T }     ⇒  Future<T>
```

- `spawn` é um **statement** (ou expressão tipo `void`); não se escreve `var x = spawn …` para obter resultado.
- Chamar `async fn` sem `await` deixa um `Future` vivo; futures não observados podem emitir warning (`unused_future`).
- Goroutine cujo resultado importa deve publicar em `Channel` (ou completar um `WaitGroup`).

---

## 9. Classes e orientação a objetos

### 9.1 Declaração

```stk
pub class MyModule {
    pub fn sum(int a, int b) int {
        return a + b
    }
}
```

### 9.2 Instanciação

```stk
var myMod = new MyModule()
```

`new Tipo(args...)` aloca e chama o construtor.

### 9.3 Construtor

Convenção: construtor nomeado `new` no tipo, invocado via `new Tipo(...)`.  
Se não houver `new` e todos os campos de armazenamento tiverem default (ou não houver campos), o compilador sintetiza `new()`.

```stk
class Point {
    pub var x int = 0
    pub var y int = 0

    pub fn new(int x, int y = 0) Point {
        self.x = x
        self.y = y
        return self
    }
}

fn main() {
    var p = new Point(3)
}
```

Defaults de campo são aplicados no início de `new` (antes do corpo). Subclasses podem redefinir `new` com outra aridade.

### 9.4 Campos e propriedades

Campos exigem `pub`, `priv` ou `prot`. Podem ter default literal:

```stk
pub var canFly bool = false
```

Propriedades com `get` / `set` **não** ocupam slot; o storage fica em outro `priv var`:

```stk
class Counter {
    priv var _value int = 0

    pub var value int {
        get { return self._value }
        set(int v) { self._value = v }
    }

    pub fn new() Counter {
        return self
    }
}
```

- Pelo menos um de `get` / `set` é obrigatório no bloco
- `obj.value` chama o getter; `obj.value = x` chama o setter
- A visibilidade do `var` aplica-se ao uso externo dos accessors

### 9.5 `self` e `super`

- `self` — referência à instância atual
- `super.membro` / `super.metodo(...)` — membro na(s) superclasse(s)
- Com herança múltipla, se o membro for ambíguo: `super.Base.membro` / `super.Base.metodo(...)`

### 9.6 Herança

Usa `::` (herança de implementação). Aceita **múltiplas** bases:

```stk
class Foo {}
class Bar {}
class Baz :: Foo, Bar {}
```

Regras:

- Layout LTR: campos das bases na ordem de declaração, depois os próprios
- Diamante (mesmo ancestral por dois caminhos) é **erro**
- Métodos herdados com o mesmo nome e símbolos distintos sem override na classe → **erro** (ambiguidade)
- Override (exceto `new`) deve coincidir na assinatura; visibilidade: `prot` pode virar `pub`; `pub` permanece `pub`
- Classes de módulo podem ser `pub`

### 9.6.1 Destructor (`drop`)

```stk
pub fn drop() {
    // liberar recursos / log
}
```

- Assinatura fixa: `fn drop()` sem retorno
- Para `var` locais de tipo classe que define (ou herda) `drop`, o compilador insere a chamada ao sair do scope e antes de reassign
- Retornar o local transfere a ownership (não chama `drop` no callee)
- Campos de classe **não** disparam `drop` automático dos filhos neste corte

### 9.7 Interfaces (`iclass`)

Interfaces declaram contratos sem implementação. Usa-se a palavra-chave `iclass`.

```stk
iclass IFoo {
    getName(string name)
}
```

Na `iclass`, métodos são declarados **sem** corpo e **sem** a palavra `fn` — apenas assinatura.

Implementação de interface usa `:` (um dois-pontos):

```stk
class Foo : IFoo {
    pub fn getName(string name) {
        std.log(name)
    }
}
```

Regras:

- Uma classe pode implementar várias interfaces: `class C : IFoo, IBar {}`
- Pode combinar herança e interfaces: `class C :: Base : IFoo, IBar {}`
- Todos os métodos da `iclass` devem ser implementados (erro de compilação se faltar)
- Métodos de interface são implicitamente públicos no contrato; a implementação na classe deve usar `pub fn`

### 9.8 Resumo dos operadores de tipo

| Sintaxe                 | Significado                 |
|-------------------------|-----------------------------|
| `class B :: A`          | B herda de A                |
| `class C :: A, B`       | herança múltipla            |
| `class C : I`           | C implementa interface I    |
| `class C :: A, B : I`   | herança múltipla + interface |

---

## 10. Controle de fluxo

```stk
if condicao {
    // ...
} else if outra {
    // ...
} else {
    // ...
}

while condicao {
    // break / continue
}

for item in colecao {
    // ...
}

for i in 0..10 {
    // intervalo exclusivo no fim: 0..9
}

match valor {
    0 => { std.log("zero") }
    1 => { std.log("um") }
    _ => { std.log("outro") }
}
```

Condições de `if`/`while` devem ser `bool` (sem truthiness implícita).

---

## 11. Operadores

Precedência (maior → menor):

1. Chamada/membro: `()`, `.`, `new`
2. Async/prefixo: `await`, `try` / `try?` / `try!`
3. Unários: `-`, `!`, `&`
4. Multiplicativos: `*`, `/`, `%`
5. Aditivos: `+`, `-`
6. Comparação: `<`, `<=`, `>`, `>=`
7. Igualdade: `==`, `!=`
8. Lógicos: `&&`, `||`
9. Atribuição: `=`, `+=`, `-=`, `*=`, `/=`

Operadores lógicos fazem short-circuit.

Operandos numéricos não sofrem conversão implícita: aritmética e comparação exigem
`int` com `int` ou `float` com `float`. `%` é exclusivo de `int`.

---

## 12. Strings e formatação

`std.log` (e APIs de formatação da std) usam placeholders posicionais `$n`:

```stk
std.log("O resultado é $1", sumResult)
std.log("$1 + $2 = $3", a, b, a + b)
```

- `$1` é o primeiro argumento após a format string, `$2` o segundo, etc.
- `$$` produz um `$` literal
- Placeholder sem argumento correspondente é erro de compilação quando os tipos são conhecidos estaticamente

Concatenação: `+` entre `string`, ou `std.string.format(...)`.

---

## 13. Memória e performance

Metas alinhadas ao compilador Rust:

- Sem GC stop-the-world na runtime padrão
- Ownership e borrowing em nível de linguagem
- Classes por padrão em heap via `new`
- Tipos primitivos e `struct` (0.2+) em stack quando possível
- Inlining agressivo e monomorfização de genéricos
- Zero-cost abstractions como princípio de design das APIs `std`

### 13.1 Modelo v0.1 (mínimo usável)

- `new` retorna referência owning à instância
- Atribuição de objetos move por padrão (não copia profunda)
- Regras conservadoras; sem borrow checker completo

### 13.2 Borrow checker (v0.2)

- Empréstimo compartilhado: `&T` (somente leitura, aliasing permitido)
- Empréstimo exclusivo: `&mut T` (escrita, sem aliases)
- Não é permitido mutar enquanto existir `&T` ativo; nem ter dois `&mut T`
- Lifetimes elididas na maioria dos casos; anotações explícitas só quando a elisão falhar
- `drop` de classe corre no fim do escopo do owning binding (já parcialmente na 0.1)

---

## 14. Biblioteca padrão (`std`)

Import:

```stk
@import "std"
```

Módulos da stdlib:

| API | Status v0.1 | Função |
|-----|-------------|--------|
| `std.log` | pronto | Log formatado para stdout/stderr (`$1`, `$2`, …) |
| `std.env` | pronto | `args()` → `List<string>`; `get(name)` → `Option<string>`; `set(name, value)` |
| `std.process` | pronto | `exit(code)` |
| `std.fs` | sync pronto | `readToString(path)` → `Result<string, string>`; `writeString(path, contents)` → `Result<int, string>` (`ok(0)`); async na 0.3 |
| `std.time` | sync pronto | `sleepMs(ms)`; `nowMs()` → epoch ms; `std.sleep` é alias de `sleepMs` |
| `std.string` | pronto | `len` / `concat` / `slice` / `contains` / `fromInt` / `parseInt` |
| `std.List<T>` | pronto | Lista dinâmica de qualquer tipo de valor `T` |
| `std.Result` / `std.Option` | pronto | Qualquer payload de valor; `.ok`/`.err`/`.some`/`.none` + `match` |
| `std.panic` | pronto | Abort com mensagem |
| `Future<T>` | pronto | Qualquer `T` de valor |
| `std.cpu` | pronto | `submit(fn() T)` → `Future<T>` para qualquer `T` de valor |
| `std.sync` | pronto | `Channel<T>` / `Mutex<T>` / `RwLock<T>` / `WaitGroup` |
| `std.parallel` | 0.2 | `map` / `reduce` sobre `List`/`array` via pool CPU |
| `std.task` | 0.4 | `yield`, `CancellationToken` |
| `std.http` | pronto (http://) | Client async + `Server` REST (ver §14.2); HTTPS fora do MVP |
| `std.net` | 0.3 | TCP básico async |
| `std.json` | pronto | `encode(T) string`; `decode<T>(string) Result<T,string>` |
| `std.yaml` | pronto | idem (YAML 1.2 subset ↔ mesmo modelo) |
| `std.toml` | pronto | idem (TOML; sem `null` — ver abaixo) |
| `std.toon` | pronto | idem ([TOON](http://github.com/toon-format/spec) ↔ modelo JSON) |

### 14.1 Serialização tipada

```stk
var text = std.json.encode(user)
match std.json.decode<User>(text) {
    ok(u) => { /* … */ }
    err(e) => { std.log("$1", e) }
}
```

- `encode` monomorphiza pelo tipo do argumento; gera serializer em compile-time.
- `decode<T>` exige type arg explícito **ou** tipo esperado no contexto (`var u User = std.json.decode(text)`).
- Tipos serializáveis: `bool`/`int`/`float`/`string`, `class`/`struct`, `List<T>`, `Option<T>`, aninhados.
- `Option.none`: JSON/YAML/TOON → `null` ou chave omitida; TOML → chave omitida no encode; chave ausente → `none` no decode.
- Campos `@ignore` e membros não-serializáveis (`Future`, channels, funções, …) não entram; estes últimos em campo sem `@ignore` = erro de compilação.

### 14.2 HTTP client e server (REST, `http://`)

I/O async: client e `listen` retornam `Future`. Só **`http://`** neste MVP (HTTPS = erro em runtime).

**Tipos**

| Tipo | API |
|------|-----|
| `std.http.Headers` | `new()`; `set(k,v)`; `get(k) → Option<string>` |
| `std.http.Request` | `method()` / `path()` / `body()` → `string`; `query(name)` / `header(name)` → `Option<string>`; `param(name)` → `string` (path `:id`, vazio se ausente) |
| `std.http.Response` | `text(status, body)` / `json(status, body)` / `empty(status)`; `status()` / `body()`; `setHeader(k,v)` / `header(k) → Option<string>` |
| `std.http.Server` | `new()`; `get\|post\|put\|delete\|patch(path, handler)`; `listen(port) → Future<Result<int,string>>` |

**Client** — todos → `Future<Result<Response, string>>`:

| API | Args |
|-----|------|
| `get` / `delete` | `(url)` ou `(url, Headers)` |
| `post` / `put` / `patch` | `(url, body)` ou `(url, body, Headers)` |

**Server** — handlers MVP: `fn(Request) Response` (síncronos). Path params: `/users/:id`. `listen` completa com `err` em falha de bind; em sucesso o future permanece pendente enquanto o servidor aceita conexões (`ok(0)` só se o servidor encerrar limpo).

```stk
@import "std"

async fn main() {
    var app = std.http.Server.new()
    app.get("/health", fn(std.http.Request _req) std.http.Response {
        return std.http.Response.text(200, "ok")
    })
    app.get("/users/:id", fn(std.http.Request req) std.http.Response {
        return std.http.Response.json(200, std.string.concat("{\"id\":\"", std.string.concat(req.param("id"), "\"}")))
    })
    match await app.listen(8080) {
        ok(_) => {}
        err(e) => { std.log("listen: $1", e) }
    }
}
```

Exemplo mínimo:

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

Exemplo async:

```stk
@import "std"

async fn main() {
    var body = await std.http.get("http://example.com")
    std.log("$1", body)
}
```

---

## 15. Erros

v0.1 usa retorno explícito com `std.Result` / `std.Option` — **sem exceções de stack**. `match` continua válido; `do` / `try` / `catch` é sugar sobre `Result` (estilo Swift).

```stk
fn parseInt(string s) std.Result<int, string> {
    // ...
}

fn main() {
    match parseInt("42") {
        ok(n) => { std.log("n=$1", n) }
        err(e) => { std.log("erro: $1", e) }
    }
}
```

### 15.1 `do` / `try` / `catch`

```stk
@import "std"

async fn main() {
    do {
        var res = try await std.http.get("http://127.0.0.1:59999/")
        std.log("status=$1", res.status())
    } catch e {
        std.log("http: $1", e)
    }
}
```

| Forma | Semântica |
|-------|-----------|
| `do { … } catch name { … }` | Bloco; `try` sem sucesso salta para `catch` com o valor `err` |
| `try expr` | `expr` deve ser `Result<T,E>`; produz `T`; só dentro de `do` |
| `try? expr` | `Result<T,E>` → `Option<T>` (`ok`→`some`, `err`→`none`); **não** exige `do` |
| `try! expr` | Em `err`, chama `std.panic` com a mensagem (se `E` for `string`) ou `"try! failed"`; produz `T`; **não** exige `do` |

- Todos os `try` (unwrap) no mesmo `do` devem compartilhar o mesmo tipo de erro `E` (o tipo de `name` no `catch`).
- `try await …` é válido: `await` produz o `Result`, `try` desembrulha.
- `break` / `continue` / `return` dentro de `do`/`catch` seguem as regras normais do bloco envolvente.

`std.panic(string msg)` aborta o processo em erros irrecuperáveis de programação.

```stk
@import "std"

fn main() {
    std.panic("unreachable")
}
```

---

## 16. Genéricos (v0.2+)

Sintaxe:

```stk
class Box<T> {
    priv var value T

    pub fn new(T value) Box<T> {
        self.value = value
        return self
    }

    pub fn get() T {
        return self.value
    }
}

fn identity<T>(T x) T {
    return x
}
```

Regras:

1. Parâmetros de tipo em `class`, `iclass`, `fn` e `struct`: `<T, U, …>`.
2. Constraints via interfaces: `fn sort<T: Comparable>(std.List<T> items)`.
3. Compilação por **monomorphização** (uma cópia por instanciação concreta).
4. `std.List<T>`, `std.Result<T,E>`, `std.Option<T>`, `Channel<T>`, `Mutex<T>`, `RwLock<T>`, `Future<T>` aceitam qualquer tipo de valor `T` (builtin parametrizado + monomorphização de genéricos de usuário).
5. Inferência de args de tipo a partir do uso quando não ambígua; senão exigir anotação.
6. Type args em **chamadas**: `f<T>(args)` (ex.: `std.json.decode<User>(s)`).

---

## 17. Manifesto (`.stkm`) e dependências

O manifesto do projeto **não é TOML**. É um arquivo Steampunk Manifest com extensão `.stkm` (ex.: `manager.stkm` na raiz).

### 17.1 Layout de projeto

```
meu-projeto/
├── manager.stkm          # manifesto (nome, scripts, deps)
├── main.stk              # entrypoint
├── module.stk            # @import ":module"
├── modules/
│   └── math.stk          # @import ":modules/math"
└── SPEC.md

# Cache GLOBAL (fora do projeto) — compartilhado entre apps:
~/.steampunk/deps/
└── dep1/1.0.1/
    ├── dep1.stkb
    └── dep1.stkmap
```

Dependências **não** ficam na pasta do projeto. Moram num cache global do toolchain e são reutilizadas por qualquer projeto que peça a mesma versão.

### 17.2 Sintaxe do manifesto

Campos de metadados usam `chave = valor`. Blocos (`scripts`, `dependencies`) agrupam chamadas encadeadas com `.método(...)`.

```stkm
name = "My App"
version = "1.0.0"
private = true
description = "My Application manifest"

scripts
    .declare("start", "steampunk run main.stk")
    .declare("build", "steampunk build main.stk --out build/app")

dependencies
    .use("dep1", version = "^1.0.1")
    .use("dep2", version = "^2.4.3")
```

| Elemento | Função |
|----------|--------|
| `name` | Nome do pacote / app |
| `version` | Versão semver do projeto |
| `private` | Se `true`, não é publicável no registry |
| `description` | Descrição curta |
| `scripts` + `.declare(nome, comando)` | Scripts invocáveis via CLI (`steampunk run:start`, etc.) |
| `dependencies` + `.use(nome, version = "…")` | Dependência externa com range semver |

Campos opcionais planejados: `entry = "main.stk"`, `authors`, `license`.

### 17.3 Artefatos de dependência (binário + sourcemap)

Dependências **não são recompiladas** a cada build do app e **não são armazenadas no diretório do projeto**. Cada pacote é um artefato pré-compilado + sourcemap, guardado num **cache global** reutilizável.

#### Cache global

| Item | Valor |
|------|--------|
| Local padrão | `~/.steampunk/deps/<nome>/<versão>/` |
| Override | variável de ambiente `STEAMPUNK_HOME` → `$STEAMPUNK_HOME/deps/…` |
| Compartilhamento | A mesma versão de uma dep é baixada **uma vez** e reutilizada por todos os projetos da máquina |

```
~/.steampunk/deps/
└── dep1/
    └── 1.0.1/
        ├── dep1.stkb       # binário da biblioteca
        └── dep1.stkmap     # sourcemap (API + símbolos para tooling)
```

| Arquivo | Papel |
|---------|--------|
| `.stkb` | Biblioteca já compilada. Permanece só no cache global até o build. |
| `.stkmap` | Sourcemap da API pública: módulos, tipos, assinaturas, docs, mapeamento símbolo→nome. Typecheck, autocomplete e go-to-definition **sem** fonte e **sem** recompilar a dep. |

#### Cópia apenas em tempo de compilação

No build do app, o compilador:

1. Resolve as deps do `manager.stkm` contra o cache global (baixa o que faltar).
2. Lê `.stkmap` para typecheck / LSP (sem copiar para o projeto).
3. Compila somente os `.stk` locais.
4. **Copia/incorpora** o `.stkb` da biblioteca **no binário final da aplicação** (link estático / embedding no artefato de saída).

Não há `node_modules` / pasta de deps no projeto. O diretório do app continua só com fontes + `manager.stkm`; o binário publicado/entregue já contém o código das libs necessárias.

Fluxo resumido:

1. `steampunk deps` (ou o primeiro build) resolve `dependencies`, popula o **cache global**, valida checksums.
2. LSP/typecheck leem `.stkmap` do cache global.
3. `steampunk build` copia os `.stkb` usados **só durante a compilação/link** para produzir o binário do app.
4. `steampunk publish` gera o par `.stkb` + `.stkmap` e publica no registry (consumidores gravam no cache global).

Regras:

- Projeto A e projeto B com `dep1@1.0.1` → um único artefato em `~/.steampunk/deps/dep1/1.0.1/`.
- Mudança no app → recompila só o app; reutiliza `.stkb` global (cópia de novo no link).
- Bump de versão → outro diretório de versão no cache global; sem rebuild a partir de fonte.
- O projeto **nunca** versiona nem commita bins de deps.
- `std` segue o mesmo modelo (artefato + sourcemap no toolchain / cache).
- Fontes de deps opcionais só para debug; nunca requisito de build ou autocomplete.

### 17.4 Relação com `@import`

```stk
@import "dep1"    // resolve via manager.stkm → artefato .stkb + .stkmap
@import "std"     // stdlib do toolchain (mesmo formato de artefato)
@import ":module" // fonte local; compilado junto com o projeto
```

---

## 18. Gramática (EBNF simplificada)

```ebnf
Program        = { ImportDir | Decl } ;

ImportDir      = "@import" StringLit ;

Decl           = FunctionDecl | ClassDecl | IClassDecl | ConstDecl ;

FunctionDecl   = [ "pub" ] [ "async" ] "fn" Ident "(" ParamList ")" [ Type ] Block ;
ParamList      = [ Param { "," Param } ] ;
Param          = Type Ident [ "=" Literal ] ;

ClassDecl      = [ "pub" ] "class" Ident [ "::" TypeList ] [ ":" TypeList ] ClassBody ;
IClassDecl     = [ "pub" ] "iclass" Ident IClassBody ;
TypeList       = Ident { "," Ident } ;

Visibility     = "pub" | "priv" | "prot" ;

ClassBody      = "{" { FieldDecl | MethodDecl } "}" ;
IClassBody     = "{" { InterfaceMethod } "}" ;
InterfaceMethod= [ "async" ] Ident "(" ParamList ")" [ Type ] ;
MethodDecl     = Visibility [ "async" ] "fn" Ident "(" ParamList ")" [ Type ] Block ;
FieldDecl      = { Decorator } Visibility "var" Ident Type ( [ "=" Literal ] | PropBody ) ;
Decorator      = "@" Ident [ "(" [ DecoratorArg { "," DecoratorArg } ] ")" ] ;
DecoratorArg   = StringLit | IntLit | BoolLit ;
PropBody       = "{" { "get" Block | "set" "(" Param ")" Block } "}" ;

Block          = "{" { Stmt } "}" ;
Stmt           = VarDecl | ReturnStmt | IfStmt | WhileStmt | ForStmt
               | MatchStmt | DoCatchStmt | SpawnStmt | ExprStmt | Block ;

VarDecl        = "var" Ident [ Type ] "=" Expr ;
ReturnStmt     = "return" [ Expr ] ;
SpawnStmt      = "spawn" ( CallExpr | Block ) ;
DoCatchStmt    = "do" Block "catch" Ident Block ;

Expr           = Assignment | LogicOr ;
Prefix         = "await" Prefix | TryPrefix | Unary ;
TryPrefix      = "try" [ "?" | "!" ] Prefix ;
Primary        = Ident | Literal | "new" Ident "(" ArgList ")"
               | "self" | "super" | "(" Expr ")" | FnLiteral
               | AsyncBlock ;
AsyncBlock     = "async" Block ;
CallOrMember   = Primary { "." Ident | "(" ArgList ")" } ;
```

A gramática completa acompanhará o parser no compilador Rust.

---

## 19. Exemplos canônicos

### 19.1 Hello World

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

### 19.2 Módulo local e chamada de método

```stk
@import "std"
@import ":module"

fn main() {
    var myMod = new MyModule()
    var sumResult = myMod.sum(10, 40)
    std.log("O resultado é $1", sumResult)
}
```

```stk
// module.stk
pub class MyModule {
    pub fn sum(int a, int b) int {
        return a + b
    }
}
```

### 19.3 Herança e interfaces

```stk
class Foo {}

class Bar :: Foo {}

iclass INamed {
    getName(string name)
}

class Person : INamed {
    pub fn getName(string name) {
        std.log(name)
    }
}
```

### 19.4 Herança + interface

```stk
iclass Drawable {
    draw()
}

class Shape {
    prot var tag string

    pub fn area() float {
        return 0.0
    }
}

class Circle :: Shape : Drawable {
    priv var radius float

    pub fn new(float radius) Circle {
        self.radius = radius
        self.tag = "circle"
        return self
    }

    pub fn area() float {
        return 3.14159 * self.radius * self.radius
    }

    pub fn draw() {
        std.log("circle r=$1", self.radius)
    }
}
```

### 19.5 Async, `Future`, goroutines e paralelismo

```stk
@import "std"

async fn fetchTitle(string url) string {
    var body = await std.http.get(url)
    return body
}

async fn main() {
    // Future: composição tipada
    var titles = await Future.join(
        fetchTitle("http://a.example"),
        fetchTitle("http://b.example")
    )
    std.log("$1 | $2", titles.0, titles.1)

    // spawn = goroutine (estilo Go); sync via channel
    var ch = std.sync.Channel<string>.new()
    spawn {
        ch.send(await fetchTitle("http://c.example"))
    }
    spawn {
        ch.send(await fetchTitle("http://d.example"))
    }
    std.log("first=$1", await ch.recv())
    std.log("second=$1", await ch.recv())

    // paralelismo de dados (CPU)
    var nums = [1, 2, 3, 4, 5, 6, 7, 8]
    var squares = await std.parallel.map(nums, fn(int n) int {
        return n * n
    })
    std.log("parallel items=$1", squares.len())
}
```

---

## 20. Roadmap do compilador

| Fase | Entrega |
|------|---------|
| 0.1 | Frontend + async thread-backed (worker pool); OOP; `struct`; closures; `float`; Result/Option/List; env/process/fs/time/string/panic; Channel/Mutex/Future int\|string; `std.cpu.submit` — **mínimo usável** |
| 0.2 | Worker pool; parallel.map; RwLock; struct; genéricos fn + type-args em calls; decorators `@encodeProperty`/`@ignore`; `std.json`/`yaml`/`toml`/`toon` encode/decode tipado |
| 0.3 | Manifesto `.stkm` (`deps`/`script`/`test`); cache global stub; `std.http` client+server REST (`http://`); formatter `fmt` |
| 0.4 | LSP mínimo (`steampunk-lsp`); `std.task.yield` + `CancellationToken` |

---

## 21. Decisões de design

Das capturas em `main.stk` e decisões de núcleo:

1. Compilada; compilador em Rust; foco em performance + DX; OOP.
2. `@import "std"` para stdlib; `@import ":path"` para módulos locais.
3. Funções: `fn nome(tipo arg) Retorno`.
4. Classes de módulo com `pub` opcional; membros de classe usam `pub` / `priv` / `prot`.
5. Instanciação com `new Tipo()`.
6. Variáveis com `var` (não existe `let`).
7. Formatação posicional `$1`, `$2`, … em `std.log`.
8. Herança com `::` (`class Bar :: Foo`).
9. Interfaces com `iclass`; implementação com `:` (`class Foo : IFoo`).
10. Assinaturas em `iclass` sem corpo e sem `fn`.
11. `async fn` retorna `Future<T>`; `await` suspende a tarefa corrente.
12. `spawn` lança goroutines (como `go` em Go): retorna `void`; sync via `Channel` / `WaitGroup`.
13. Paralelismo CPU via `std.parallel` / `std.cpu.submit`; runtime M:N work-stealing.
14. Manifesto do projeto é `.stkm` (não TOML), ex.: `manager.stkm`.
15. Dependências pré-compiladas (`.stkb` + `.stkmap`) ficam em cache **global** (`~/.steampunk/deps`); reutilizadas entre projetos na mesma versão; copiadas para o binário do app só em tempo de compilação/link.
16. Decorators de membro (`@name` / `@name(…)`); built-ins de serde `@encodeProperty` / `@ignore`; `@import` não é decorator.
17. Serialização tipada via `std.json` / `std.yaml` / `std.toml` / `std.toon` com monomorphização.
18. `std.http` REST: client async (`Future<Result<Response,string>>`) + `Server` estilo Express; só `http://` no MVP; handlers `fn(Request) Response`.
19. Erros: `Result`/`Option` sem exceções; `do`/`try`/`catch` e `try?`/`try!` são sugar sobre `Result` (estilo Swift).

---

*Documento vivo — a implementação no compilador Rust é a fonte da verdade quando divergir desta draft.*
