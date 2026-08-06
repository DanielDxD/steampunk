# Contributing to Steampunk

Thanks for helping improve Steampunk. This document covers how to work on the language, compiler, and docs.

## Source of truth

- Language rules: [`SPEC.md`](SPEC.md)
- If SPEC and implementation disagree, **align the code to the SPEC**, or update the SPEC first with an explicit decision
- Compiler & toolchain: Rust under [`compiler/`](compiler/)
- User programs: `.stk`; project manifest: `.stkm` (never TOML)

## Locked language decisions

Do not “fix” these casually (see SPEC §21):

| Rule | Notes |
|------|--------|
| No `let` | Use `var` / `const` only |
| Class members | Always `pub` / `priv` / `prot` |
| Inheritance / interfaces | `::` inherit, `:` implement, `iclass` for interfaces |
| `async` | Returns `Future<T>` |
| `spawn` | Goroutine style; returns `void` |
| Manifest | `.stkm` (e.g. `manager.stkm`) |
| Deps | `.stkb` + `.stkmap` in global cache |

## Setup

1. Install a recent stable Rust toolchain and a C linker (`cc` / `clang` / `gcc`)
2. Clone the repo and run tests:

```bash
cargo test --manifest-path compiler/Cargo.toml
cargo run -p stk-cli --manifest-path compiler/Cargo.toml -- run examples/hello.stk
```

3. Docs site (optional): Bun + Next.js in [`docs/`](docs/)

```bash
cd docs && bun install && bun dev
```

## Workflow

1. Open an issue (or discuss on an existing one) for non-trivial changes
2. Keep PRs focused — one concern per PR when practical
3. For language changes: edit `SPEC.md` first, then compiler / examples / skills
4. Prefer the roadmap in SPEC §20 when choosing what to implement next
5. Add or update examples under `examples/` and compiler tests when behavior changes

## Code style

- Rust: idiomatic Rust 2021; match existing crate layout (`stk-lexer`, `stk-parser`, …)
- Steampunk: follow SPEC and the project Cursor skills (`.cursor/skills/steampunk-*`)
- Do not invent syntax or keywords that contradict the SPEC

## Commit messages

Use clear, imperative subjects focused on **why** (e.g. `fix await on Channel.recv for string payloads`).

## Pull requests

- Describe the change and how you tested it
- Link related issues
- Ensure `cargo test --manifest-path compiler/Cargo.toml` passes
- Do not commit secrets, local build artifacts, or `target/` / `.next/`

## Conduct

By participating, you agree to follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Security

Please report vulnerabilities privately — see [SECURITY.md](SECURITY.md).
