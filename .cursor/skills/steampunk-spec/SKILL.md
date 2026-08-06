---
name: steampunk-spec
description: >-
  Evolves the Steampunk language specification in SPEC.md consistently with
  existing design decisions. Use when changing syntax, semantics, stdlib,
  manifest (.stkm), dependency artifacts, roadmap, or when the user revises
  language rules.
---

# Evolving `SPEC.md`

## Rules

1. **SPEC is normative** for the language until the compiler implements a section.
2. Preserve decisions in §21 unless the user explicitly overrides them.
3. Keep examples compilable against the described rules (same file may show multiple snippets as comments/sections).
4. After a SPEC change, update related project skills/rules if they hard-code the old rule:
   - `.cursor/skills/steampunk-compiler/`
   - `.cursor/skills/steampunk-stk/`
   - `.cursor/rules/steampunk-*.mdc`

## Workflow

```
- [ ] Identify section(s) to change
- [ ] State the decision in one sentence (user-facing)
- [ ] Update prose + examples + EBNF (§18) if syntax changes
- [ ] Update §21 design decisions list if it's a core rule
- [ ] Update §20 roadmap if it shifts a phase deliverable
- [ ] Sync skills/rules cheat-sheets
```

## Locked decisions (do not “fix” casually)

| Decision | Spec |
|----------|------|
| No `let` | §6, §21 |
| Class members `pub`/`priv`/`prot` | §4.3, §9 |
| Inherit `::` / iface `:` / `iclass` | §9 |
| `async` → `Future<T>` | §7–8 |
| `spawn` = goroutine (`void`) | §8.3 |
| Manifest `.stkm` not TOML | §17 |
| Deps `.stkb` + `.stkmap` in **global** cache; copy into app binary only at compile time | §17.3 |
| Compiler in Rust | §1 |

## When user contradicts SPEC

1. Confirm they want a **language change**.
2. Edit SPEC first.
3. Then adjust compiler/fixtures/skills.

## Style

- Portuguese is OK for prose (matches current SPEC).
- Code samples in `stk` / `stkm` fences.
- Prefer tables for modifiers and APIs.
- Keep version note at top (`0.1.0-draft` until release).
