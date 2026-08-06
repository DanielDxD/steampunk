---
title: Classes e OOP
description: class, new, self/super, herança, propriedades get/set e drop.
---

# Classes e OOP

## Declaração e `new`

```stk
@import "std"

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
    std.log("x=$1 y=$2", p.x, p.y)
}
```

- `new Tipo(args)` aloca e chama o construtor `new`
- Construtor devolve `self` tipado como a classe

## Visibilidade de membros

Todo campo e método **deve** declarar `pub`, `priv` ou `prot`:

| Modificador | Escopo |
|-------------|--------|
| `pub` | Qualquer código que veja o tipo |
| `priv` | Só a própria classe |
| `prot` | Classe + subclasses |

## Propriedades (`get` / `set`)

```stk
class Named {
    priv var _label string = ""

    pub var label string {
        get { return self._label }
        set(string v) { self._label = v }
    }

    pub fn new(string label = "x") Named {
        self.label = label
        return self
    }
}
```

`obj.label` chama o getter; `obj.label = v` chama o setter. O storage fica no `priv var`.

## Herança

`::` herda implementação (múltiplas bases permitidas; diamante é erro):

```stk
class Bird :: Named, Flyer {
    pub fn new(string label) Bird {
        self.label = label
        return self
    }

    pub fn speak() {
        super.Flyer.fly()
        std.log("bird=$1", self.label)
    }
}
```

Com ambiguidade: `super.Base.membro`.

## Interfaces (`iclass`)

```stk
iclass IGreeter {
    greet(string name)
}

class Person : IGreeter {
    pub fn greet(string name) {
        std.log("hi $1", name)
    }
}
```

- Na `iclass`, assinaturas **sem** `fn` e sem corpo
- Implementação usa `:` e métodos `pub`
- Pode combinar: `class C :: Base : IFoo {}`

## Destructor `drop`

```stk
pub fn drop() {
    std.log("liberando")
}
```

Para locais de classe com `drop`, o compilador chama ao sair do escopo (ownership transferida em `return` não chama `drop` no callee).
