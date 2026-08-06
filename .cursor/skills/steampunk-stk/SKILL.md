---
name: steampunk-stk
description: >-
  Writes idiomatic Steampunk (.stk) source and examples from SPEC.md. Use when
  authoring main.stk, modules, classes, iclass, async/Future, spawn/goroutines,
  channels, tests/fixtures, or sample programs for the language.
---

# Writing Steampunk (`.stk`)

## Before writing

1. Skim the relevant section of [`SPEC.md`](../../../SPEC.md).
2. Mirror style from `main.stk` and SPEC §19 examples.
3. Never use `let`, TOML manifests, or `await` on `spawn`.

## Templates

### Entrypoint

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

### Async entrypoint

```stk
@import "std"

async fn main() {
    var body = await std.http.get("https://example.com")
    std.log("$1", body)
}
```

### Public module

```stk
pub class Math {
    pub fn sum(int a, int b) int {
        return a + b
    }
}
```

```stk
@import ":math"

fn main() {
    var m = new Math()
    std.log("$1", m.sum(1, 2))
}
```

### Class + visibility

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
}
```

### Inheritance / interface

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

### Goroutines + channel

```stk
@import "std"

fn producer(std.sync.Channel<int> ch) {
    ch.send(10)
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

## Checklist

- [ ] `@import` at top
- [ ] Params `tipo nome`; return after `)`
- [ ] Class members all `pub`/`priv`/`prot`
- [ ] `iclass` methods without `fn` / body
- [ ] Interface impl methods are `pub fn`
- [ ] `spawn` not assigned / not awaited
- [ ] Format strings use `$1`, `$2`, …
- [ ] Local modules use `:path`

## Anti-patterns

```stk
// BAD
let x = 1
var y = spawn work()
class C { var x int }          // missing visibility
iclass I { fn foo() }          // no fn in iclass
class B : A {}                 // : is interface; use :: for inherit
```
