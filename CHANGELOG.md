# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once stable releases begin. Pre-1.0 versions may include breaking changes.

## [Unreleased]

### Added

- Open-source project metadata: README, MIT license, contributing guide, code of conduct, security policy, and GitHub templates

## [0.1.0-draft] - 2026-08-06

### Added

- Language specification draft (`SPEC.md`)
- Rust compiler workspace (lexer, parser, types, Cranelift codegen, CLI, LSP stub)
- MVP language subset: OOP, async/`Future`, `spawn`, sync primitives, closures, stdlib basics
- Examples under `examples/`
- Multilingual docs site under `docs/` (pt / en / es)
- Project manifest format `.stkm` (`manager.stkm`)
