# Security Policy

## Supported versions

Steampunk is pre-1.0 (`0.1.0-draft`). Security fixes are applied on the default
development branch; there are no long-term support releases yet.

| Version | Supported |
|---------|-----------|
| 0.1.x (draft) | Yes (best effort) |
| older / forks | No |

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Instead, report privately:

1. If the repository has [GitHub Security Advisories](https://docs.github.com/en/code-security/security-advisories) enabled, use **Report a vulnerability**.
2. Otherwise, email the maintainer: **leandrodonato** (GitHub username) via GitHub’s private contact options for the account that owns this repository.

Include:

- Description of the issue and impact
- Steps to reproduce or a proof of concept (if safe)
- Affected component (compiler, runtime, stdlib, docs site, etc.)
- Whether you are aware of public disclosure or active exploitation

You should receive an acknowledgment within a few days. We will coordinate a fix and disclosure timeline with you. Please give us reasonable time to release a patch before public discussion.

## Scope

In scope: the Steampunk compiler, runtime, CLI, LSP, and standard library in this repository, plus the docs site under `docs/` when it processes untrusted input.

Out of scope: third-party dependencies (report upstream when possible), issues that require unrealistic local privilege, and speculative reports without a plausible exploit path.
