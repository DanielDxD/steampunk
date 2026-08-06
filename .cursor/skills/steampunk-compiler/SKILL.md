---
name: steampunk-compiler
description: >-
  Implements and evolves the Steampunk compiler and runtime in Rust from SPEC.md.
  Use when building lexer, parser, AST, typechecker, codegen, async/Future,
  spawn/goroutines, .stkm manifests, .stkb/.stkmap deps, stdlib, or CLI
  (steampunk build/run).
---

# Steampunk Compiler

## Source of truth

1. Read [`SPEC.md`](../../../SPEC.md) for the feature being implemented.
2. Use [reference.md](reference.md) for a condensed checklist.
3. Do not invent syntax. Update SPEC first if the user requests a language change.

## Architecture (target)

```
source .stk / .stkm
    → lexer → parser → AST
    → module resolver (@import "pkg" | ":path")
    → typecheck (incl. .stkmap for deps)
    → lowering (MIR)
    → codegen (LLVM or Cranelift)
    → copy/link global .stkb into app binary
    → runtime (M:N scheduler, Future poll, goroutines)
```

Suggested crate layout (adapt if repo already differs):

```
compiler/
  crates/
    stk-span/        # SourceSpan, diagnostics
    stk-lexer/
    stk-parser/
    stk-ast/
    stk-resolve/     # imports, .stkm, dep cache
    stk-types/       # typecheck
    stk-mir/
    stk-codegen/
    stk-runtime/     # Future, spawn, channels
    stk-cli/         # steampunk binary
```

## Implementation workflow

Copy and track:

```
Task:
- [ ] SPEC section identified (§N)
- [ ] Grammar/tokens updated if needed
- [ ] AST nodes + parser tests
- [ ] Typecheck / resolve rules
- [ ] Codegen or runtime behavior
- [ ] Fixture .stk (or .stkm) under tests/
- [ ] Diagnostics cover the failure mode
```

### Phase alignment (SPEC §20)

| Work on… | Phase |
|----------|--------|
| Lexer, parser, AST, typecheck, codegen, `async`/`await`, `Future`, `spawn` | 0.1 |
| Channel, WaitGroup, `std.parallel` / `std.cpu.submit`, borrow, generics | 0.2 |
| `.stkm`, registry, `.stkb`+`.stkmap`, async I/O std | 0.3 |
| LSP via `.stkmap`, formatter | 0.4 |

## Non-negotiable semantics

| Topic | Rule |
|-------|------|
| Bindings | `var` / `const` only — **no `let`** |
| Class members | require `pub` \| `priv` \| `prot` |
| Inheritance | `class B :: A` |
| Interfaces | `iclass`; impl `class C : I`; methods in iclass **without** `fn` |
| Params | `tipo nome`; return type after `)` |
| `async fn f() T` | function type is `Future<T>` |
| `spawn` | goroutine; type `void`; no join handle |
| Manifest | `.stkm` only |
| Deps | global cache; typecheck/LSP from `.stkmap`; copy `.stkb` into app binary at compile time; never store deps in the project tree |

## Deps pipeline

1. Parse `manager.stkm` → dependency graph.
2. Resolve against **global** cache `~/.steampunk/deps/<name>/<ver>/` (or `$STEAMPUNK_HOME/deps/…`); download if missing. Same version is shared by all projects.
3. Typecheck/LSP read `.stkmap` from the global cache (no copy into the project).
4. Compile **only** local `.stk`.
5. At compile/link time, **copy/embed** each required `.stkb` into the application binary. Do not leave a project-local `deps/` folder.

## Testing

- Prefer golden/parser/typecheck tests with small `.stk` fixtures mirroring SPEC §19.
- Every accepted construct in SPEC should have at least one positive fixture; rejected forms (`let`, bare class fields, `await spawn`) negative fixtures.

## Done criteria

- Behavior matches the cited SPEC section.
- No silent divergence (if temporary, comment `// SPEC deviation:` and flag to user).
