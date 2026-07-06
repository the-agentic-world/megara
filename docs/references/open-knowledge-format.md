---
type: Concept
title: Open Knowledge Format
description: Notes for applying OKF v0.1 to Megara user-facing knowledge docs.
timestamp: 2026-07-06
tags: [okf, docs, knowledge]
---

# Open Knowledge Format

Open Knowledge Format, or OKF, is a Markdown-based knowledge format described by Google Cloud for portable knowledge sharing. The current public reference is v0.1, and the practical shape is simple: Markdown files carry YAML frontmatter, a bundle has entry files such as `index.md` and `log.md`, and source material should be traceable through citations.

Megara applies OKF only to user-facing knowledge bundles. The default root is `docs/`; a user can choose another root with `megara docs --root` arguments. Runtime state, skills, and the Megara repository's bundled harness source are deliberately outside this rule.

## Megara Rules

- User-requested durable knowledge belongs in an OKF bundle.
- The default OKF bundle root is `docs/`.
- `index.md` and `log.md` are reserved bundle files.
- Concept Markdown files must have YAML frontmatter and a non-empty `type`.
- Recommended frontmatter fields are `title`, `description`, `tags`, and `timestamp`.
- `.megara/**` runtime state, artifacts, and cache files are not OKF knowledge docs.
- `.agents/skills/**` skill files are not OKF knowledge docs.
- `harness/**` product harness source files in the Megara repository are not OKF knowledge docs.

## References

- [Google Cloud: how the Open Knowledge Format can improve data sharing](https://cloud.google.com/blog/products/data-analytics/how-the-open-knowledge-format-can-improve-data-sharing)
- [OKF v0.1 specification](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
- [OKF README](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/README.md)
