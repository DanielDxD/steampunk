---
title: Basic syntax
description: Comments, literals, types, variables, and mutability in Steampunk.
---

# Basic syntax

## Comments

```stk
// line

/*
  block
*/
```

## Identifiers

- Start with a letter or `_`
- Case-sensitive
- Conventions: types in `PascalCase`, functions/vars in `camelCase`, consts in `SCREAMING_SNAKE`

## Literals (MVP)

| Type | Examples |
|------|----------|
| `int` | `42`, `0`, `-7` |
| `string` | `"Hello"`, `"line\n"` |
| `bool` | `true`, `false` |

Strings accept escapes: `\n`, `\t`, `\r`, `\\`, `\"`.

## Primitive types in the MVP

The current compiler stably types:

- `int`, `string`, `bool`, `void`
- Fixed arrays `[T; N]` (elements `int` / `string` / `bool`)
- `Future<T>`, `Channel<T>` (today `T ∈ {int, string}`)
- `std.Result<int, string>`, `std.Option<int>`
- Function / closure: `fn(…) Ret`

The full SPEC plans more widths (`i32`, `float`, etc.); see [MVP status](./mvp).

## Variables

There is no `let`. Use `var` (mutable) and `const` (compile-time):

```stk
var counter = 0
counter = counter + 1

const MAX = 128
var name string = "Ada"
```

`var` infers the type from the initializer when possible.

## Arithmetic and comparison operators

```stk
var a = 10 + 3
var b = a * 2
var ok = b >= 20
var same = "x" == "x"
```

Operators: `+ - * / %`, `== != < <= > >=`, `&& ||`, `!` (logical negation), unary `-`.

## Arrays

```stk
var xs = [10, 20, 30]
std.log("len=$1 first=$2", xs.len, xs[0])
```

- Literals cannot be empty in the MVP
- `.len` is an array property
- Out-of-range index is a runtime error

## Logging

```stk
@import "std"

fn main() {
    std.log("text only")
    std.log("value=$1 name=$2", 42, "Ada")
}
```

Placeholders `$1`, `$2`, … match arguments in order.
