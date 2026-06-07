# Architecture Decision Records (ADRs)

## When to Write an ADR

Write an ADR when making decisions that:

- Affect the overall structure or architecture of the codebase
- Establish patterns or conventions that other code should follow
- Have long-term implications that future contributors need to understand
- Represent a significant trade-off between competing concerns

Unlike RFCs which gather feedback before committing to an approach, ADRs document decisions that have already been made.

## ADR Structure

Each ADR describes:

- **Context**: The circumstances and forces at play when the decision was made
- **Decision**: The change or approach that was chosen
- **Consequences**: The resulting effects, both positive and negative

## Status Lifecycle

| Status                           | Meaning                                                |
|----------------------------------|--------------------------------------------------------|
| ![Active][badge-active]          | Currently enforced                                     |
| ![Superseded][badge-superseded]  | Replaced by another ADR (update `superseded-by` field) |
| ![Deprecated][badge-deprecated]  | No longer followed, kept for historical reference      |

## ID Assignment

ADR IDs are **not** assigned during drafting. Drafts use `id: draft` and `# ADR-DRAFT: Title`. The ID is assigned at
approval time by checking existing ADRs in `docs/design/`.

The existing records are grouped by **domain** rather than strict chronology (foundational architecture → data model →
auth → static/reference data → domain-feature ADRs). A new ADR normally takes the next free trailing number; only
reorganize/renumber the set when a deliberate consolidation pass calls for it, and when you do, update every citation
(`docs/`, `src/`, `migrations/`, the README index) in one atomic change, since ADR numbers are referenced throughout the
codebase.

## Using the Template

The template below shows all possible sections and frontmatter fields. **Omit any section or frontmatter field that does
not apply to the decision.** For example, if the decision introduces no new dependencies, omit the Dependencies section
entirely. Only add `superseded-by` when the ADR is actually superseded. Do not include empty or placeholder sections.

## Template

```markdown
---
id: draft
title: ADR Title
status: active
tags: []
created: YYYY-MM-DD
superseded-by:
---

# ADR-<ID>: Title

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

One paragraph explaining the decision.

## Context

Why is this decision needed? What problem does it solve?

## Decision

What we're going to do. Technical details, syntax, semantics, etc.

## Affected Areas

Which parts of the codebase, infrastructure, or workflows this decision impacts.

- ...

## Dependencies

New dependencies introduced by this decision.

| Dependency | Version | Purpose |
|------------|---------|---------|
| -          | -       | -       |

## Consequences

### Positive

- ...

### Negative

- ...

## Open Questions

- Question 1?

## Future Work

Things explicitly out of scope, for future ADRs.

## References

- Related ADRs, discussions, external resources
```

[badge-active]: https://img.shields.io/badge/Active-green?style=for-the-badge
[badge-deprecated]: https://img.shields.io/badge/Deprecated-red?style=for-the-badge
[badge-superseded]: https://img.shields.io/badge/XXXX-black?style=for-the-badge&label=Superseded&labelColor=orange
