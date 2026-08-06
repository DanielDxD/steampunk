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

- **To develop / compile the toolchain:** Rust (stable) + Cargo, and a C linker for local `steampunk build`
- **To use a Release binary:** nothing else — download `steampunk` for your OS (see [Releases](#releases))

## Releases

GitHub Actions builds **standalone** CLI binaries when you push a version tag:

```bash
git tag v0.1.0
git push origin master     # commit com o workflow precisa estar no default branch
git push origin v0.1.0     # só então a tag dispara o Release
```

Se a tag já existir e o workflow não tiver rodado, apague e reenvie:

```bash
git push origin :refs/tags/v0.1.0
git push origin v0.1.0
```

Ou rode manualmente em **Actions → Release → Run workflow** (campo `tag`).

That runs [`.github/workflows/release.yml`](.github/workflows/release.yml) and publishes a [GitHub Release](https://github.com/OWNER/REPO/releases) with:

| Target | Notes |
|--------|--------|
| `x86_64-unknown-linux-musl` | Fully static Linux x86_64 |
| `aarch64-unknown-linux-musl` | Fully static Linux ARM64 |
| `x86_64-apple-darwin` | macOS Intel |
| `aarch64-apple-darwin` | macOS Apple Silicon |
| `x86_64-pc-windows-msvc` | Windows (CRT linked statically) |

After download:

```bash
# Linux / macOS
tar -xzf steampunk-v0.1.0-*.tar.gz
sudo mv steampunk-v0.1.0-*/steampunk /usr/local/bin/
steampunk run examples/hello.stk

# Windows (PowerShell)
Expand-Archive steampunk-v0.1.0-x86_64-pc-windows-msvc.zip
# put steampunk.exe on PATH, then:
steampunk.exe run examples\hello.stk
```

Build locally (one platform) without CI:

```bash
cargo build --release -p stk-cli --manifest-path compiler/Cargo.toml
# → compiler/target/release/steampunk
```

**Dependency note:** the *CLI* needs no Rust/Cargo on the user’s machine.  
`steampunk run` (JIT) is self-contained. `steampunk build` (AOT) still invokes a system C linker (`cc` / `clang` / `gcc` / MSVC) on that machine to finish the native binary — that is a toolchain limitation of AOT today, not of packaging.

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
