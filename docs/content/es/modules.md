---
title: Módulos e imports
description: "@import, visibilidad pub y organización de proyectos .stk."
---

# Módulos e imports

## Directiva `@import`

Los imports van al inicio del archivo, antes de las declaraciones:

```stk
@import "std"            // biblioteca estándar
@import ":modules/math"  // módulo local
```

| Forma | Significado |
|-------|-------------|
| `"nombre"` | Paquete / std (MVP: solo `"std"`) |
| `":ruta"` | Archivo local relativo a la raíz del proyecto / entry |

## Visibilidad de módulo

| Modificador | Alcance |
|-------------|---------|
| (omitido) | Privado al archivo |
| `pub` | Visible para quien importe |

Se aplica a `fn`, `class`, `iclass`, `const` de nivel de archivo.

`fn main` / `async fn main` **no** necesita `pub` — es punto de entrada, no export.

## Ejemplo práctico

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

## Reglas

1. Ciclos de import son error de compilación
2. Cada `.stk` es una unidad de módulo
3. Solo miembros `pub` cruzan la frontera del módulo
4. En el MVP, imports además de `"std"` y módulos locales `:…` no están habilitados
