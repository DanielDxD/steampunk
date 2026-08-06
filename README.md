# Steampunk

**Steampunk** (`.stk`) is a statically typed, object-oriented, ahead-of-time compiled language focused on native performance and strong DX. Concurrency and parallelism are first-class: `async`/`await`, `Future<T>`, goroutine-style `spawn`, and CPU parallelism APIs in `std`.

The compiler is written in **Rust** and targets native code via Cranelift (`run` = JIT, `build` = AOT binary).

> **Status:** `0.1.0-draft` — usable MVP. Language rules live in [`SPEC.md`](SPEC.md). When SPEC and code disagree, align code to the SPEC (or update the SPEC first).

## Features (v0.1)

- AOT / JIT compilation to native code
- Static typing with local inference where obvious
- Classes, inheritance (`::`), interfaces (`iclass` / `:`)
- `async fn` → `Future<T>`, `await`, `spawn` (returns `void`)
- Sync primitives: `Channel`, `Mutex`, `RwLock`, `WaitGroup`
- Modules via `@import`; project manifest `manager.stkm` (not TOML)
- Stdlib: log, env, process, fs, time, string, List, Result/Option, HTTP GET (MVP), and more

## Requirements

- Rust (stable) and Cargo
- A C linker (`cc`, `clang`, or `gcc`) for `build`
- macOS or Linux (current development targets)

## Quick start

From the repository root:

```bash
# JIT-compile and run
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/hello.stk

# Native binary
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- \
  build examples/hello.stk --out build/hello
./build/hello
```

Hello World:

```stk
@import "std"

fn main() {
    std.log("Hello World")
}
```

More samples under [`examples/`](examples/).

## CLI

Binary name: `steampunk` (crate `stk-cli`).

| Command | Description |
|---------|-------------|
| `run [file]` | Cranelift JIT in memory, then execute `main` |
| `build [file] -o path` | Object file + system linker → native binary |
| `deps` | Resolve `manager.stkm` dependencies into the global cache |
| `script <name>` | Run a script declared in `manager.stkm` |
| `test` | Run `*_test.stk` / `fn test_*` under a directory |
| `fmt <paths…>` | Format `.stk` sources (MVP) |

Install a local binary:

```bash
cargo install --path compiler/crates/stk-cli
steampunk run examples/hello.stk
```

## Project layout

```text
.
├── SPEC.md           # Language specification (source of truth)
├── main.stk          # Sample entry
├── manager.stkm      # Project manifest
├── examples/         # Language & stdlib demos
├── compiler/         # Rust workspace (lexer → codegen → CLI / LSP)
└── docs/             # Multilingual docs site (pt / en / es)
```

## Documentation

| Resource | Link |
|----------|------|
| Language spec | [`SPEC.md`](SPEC.md) |
| Compiler notes | [`compiler/README.md`](compiler/README.md) |
| Docs site | [`docs/`](docs/) — `cd docs && bun install && bun dev` |
| Contributing | [`CONTRIBUTING.md`](CONTRIBUTING.md) |
| Security | [`SECURITY.md`](SECURITY.md) |
| Code of conduct | [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) |

## Development

```bash
# Compiler tests
cargo test --manifest-path compiler/Cargo.toml

# Run the sample app via manager.stkm scripts
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- script start
```

Roadmap (compiler phases): see [`SPEC.md` §20](SPEC.md).

## Design highlights

- No `let` — only `var` and `const`
- Class members require `pub` / `priv` / `prot`
- `spawn` returns `void` (sync via channels / wait groups)
- Dependencies: precompiled `.stkb` + `.stkmap` in a **global** cache (`~/.steampunk/deps`), linked into the app binary at build time

## License

Licensed under the [MIT License](LICENSE).
