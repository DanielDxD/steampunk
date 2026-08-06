---
title: Getting started
description: How to compile and run Steampunk programs with the MVP compiler.
---

# Getting started

## Requirements

- Rust (stable toolchain) and Cargo
- A C linker (`cc`, `clang`, or `gcc`) for native `build`
- macOS or Linux (tested in current development)

## Clone and run

From the repository root:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/hello.stk
```

Other useful examples:

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/math.stk
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/closures.stk
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/channel.stk
```

## `run` vs `build`

| Command | What it does |
|---------|--------------|
| `run file.stk` | Cranelift JIT in memory and runs `main` |
| `build file.stk --out path` | Native object + link → binary |

```bash
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- \
  build examples/math.stk --out build/math
./build/math
```

## Entry point

Every executable needs exactly one `main`:

```stk
@import "std"

fn main() {
    std.log("sync main")
}
```

Or async (allows `await` at entry):

```stk
@import "std"

async fn main() {
    var n = await Future.ready(42)
    std.log("n=$1", n)
}
```

## Typical project layout

```text
my-app/
  main.stk
  modules/
    math.stk
```

```stk
@import "std"
@import ":modules/math"

fn main() {
    std.log("$1", add(2, 3))
}
```

The `:` prefix in `@import ":modules/math"` resolves local modules from the project / entry root.
