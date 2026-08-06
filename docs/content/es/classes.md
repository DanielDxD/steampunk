---
title: Clases y OOP
description: class, new, self/super, herencia, propiedades get/set y drop.
---

# Clases y OOP

## Declaración y `new`

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

- `new Tipo(args)` reserva memoria y llama al constructor `new`
- El constructor devuelve `self` tipado como la clase

## Visibilidad de miembros

Todo campo y método **debe** declarar `pub`, `priv` o `prot`:

| Modificador | Alcance |
|-------------|---------|
| `pub` | Cualquier código que vea el tipo |
| `priv` | Solo la propia clase |
| `prot` | Clase + subclases |

## Propiedades (`get` / `set`)

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

`obj.label` llama al getter; `obj.label = v` llama al setter. El storage queda en el `priv var`.

## Herencia

`::` hereda implementación (múltiples bases permitidas; diamante es error):

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

Con ambigüedad: `super.Base.miembro`.

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

- En la `iclass`, firmas **sin** `fn` y sin cuerpo
- La implementación usa `:` y métodos `pub`
- Se puede combinar: `class C :: Base : IFoo {}`

## Destructor `drop`

```stk
pub fn drop() {
    std.log("liberando")
}
```

Para locales de clase con `drop`, el compilador llama al salir del scope (ownership transferida en `return` no llama `drop` en el callee).
