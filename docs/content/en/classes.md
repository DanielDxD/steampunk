---
title: Classes and OOP
description: class, new, self/super, inheritance, get/set properties, and drop.
---

# Classes and OOP

## Declaration and `new`

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

- `new Type(args)` allocates and calls the `new` constructor
- The constructor returns `self` typed as the class

## Member visibility

Every field and method **must** declare `pub`, `priv`, or `prot`:

| Modifier | Scope |
|----------|-------|
| `pub` | Any code that can see the type |
| `priv` | Only the class itself |
| `prot` | Class + subclasses |

## Properties (`get` / `set`)

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

`obj.label` calls the getter; `obj.label = v` calls the setter. Storage lives in the `priv var`.

## Inheritance

`::` inherits implementation (multiple bases allowed; diamond is an error):

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

With ambiguity: `super.Base.member`.

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

- In an `iclass`, signatures have **no** `fn` and no body
- Implementation uses `:` and `pub` methods
- You can combine: `class C :: Base : IFoo {}`

## Destructor `drop`

```stk
pub fn drop() {
    std.log("releasing")
}
```

For class locals with `drop`, the compiler calls it on scope exit (ownership transferred via `return` does not call `drop` in the callee).
