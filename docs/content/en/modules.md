---
title: Modules and imports
description: "@import, pub visibility, and organizing .stk projects."
---

# Modules and imports

## The `@import` directive

Imports go at the top of the file, before declarations:

```stk
@import "std"            // standard library
@import ":modules/math"  // local module
```

| Form | Meaning |
|------|---------|
| `"name"` | Package / std (MVP: only `"std"`) |
| `":path"` | Local file relative to the project / entry root |

## Module visibility

| Modifier | Scope |
|----------|-------|
| (omitted) | Private to the file |
| `pub` | Visible to importers |

Applies to file-level `fn`, `class`, `iclass`, and `const`.

`fn main` / `async fn main` does **not** need `pub` — it is an entry point, not an export.

## Practical example

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

## Rules

1. Import cycles are a compile error
2. Each `.stk` file is a module unit
3. Only `pub` members cross the module boundary
4. In the MVP, imports beyond `"std"` and local `:…` modules are not enabled
