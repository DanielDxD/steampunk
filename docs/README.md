# Steampunk Docs

Site de documentação da linguagem Steampunk com suporte a **português**, **inglês** e **espanhol**.

## Desenvolvimento

```bash
cd docs
bun install
bun dev
```

Abra [http://localhost:3000](http://localhost:3000) — redireciona para `/pt`.

| Locale | URL |
|--------|-----|
| Português | `/pt` |
| English | `/en` |
| Español | `/es` |

## Conteúdo

Markdown em `content/{pt,en,es}/`. Cada página usa frontmatter:

```yaml
---
title: Título
description: Resumo curto
---
```

A navegação lateral é definida em `src/lib/nav.ts` — mantenha os slugs alinhados aos arquivos `.md`.

## Build

```bash
bun run build
bun start
```
