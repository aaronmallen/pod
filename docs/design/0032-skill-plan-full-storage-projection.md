---
id: "0032"
title: Skill Plans Store the Full Skill Set, Project Per-Character
status: active
tags: [skills]
created: 2026-06-22
---

# ADR-0032: Skill plans store the full skill set, project per-character

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

A stored skill plan is the complete, character-agnostic set of skill levels and prerequisites required to finish the
plan, independent of who authored or imports it. Every per-character behavior (what the plan displays, injector/time
counts, attribute-remap recommendations) is a projection computed at view time against the current character's trained
levels. Storage is full; the character is a lens.

## Context

Plans were authored against the creating character: seeding subtracted the author's already-trained skills
(`expand_wishes` in [`src/features/skills/plan_math.rs`](../../src/features/skills/plan_math.rs)), so a stored plan was
an author-filtered subset rather than the complete plan. This produced two defects:

- The plan view surfaced already-trained levels as dimmed "already trained" skipped rows, reading as if the plan
  re-trains skills the character has.
- Exporting and importing a plan onto a different character dropped the low levels the author already had, yielding an
  incomplete plan. Import paths were inconsistent: JSON stored verbatim, text re-filtered against the importer's trained
  skills.

`compute_plan` already re-projects stored entries against `synced_trained_level` and emits zero-cost "skipped" steps for
trained levels: the projection machinery exists; only storage was lossy.

## Decision

Storage holds the full set; the character is a projection applied at view time.

- **Seed stores full.** Seeding no longer subtracts the author's trained levels. Prerequisite expansion is retained but
  driven without the trained filter.
- **All math consumes the projection.** Display, injector, time, and attribute/remap math all consume the projected
  needed-only steps from `compute_plan`. Already-trained levels are omitted from the view and contribute zero to every
  calculation.
- **Import/export unify on full storage.** A single canonical "persist a plan model onto a target character" routine is
  shared by file-import and any other persist path (e.g. cross-character copy). Text import re-expands full
  prerequisites and all levels rather than filtering against the importer.
- **Existing plans are repaired on load** by an idempotent, app-side re-expansion. Prerequisite expansion needs the
  runtime catalog, so this is not a SQL migration.

## Affected Areas

- [`src/features/skills/plan_math.rs`](../../src/features/skills/plan_math.rs) (`expand_wishes`, `schedule_skill`,
  `compute_plan`, `injector_yield`)
- [`src/features/skill_plan_editor.rs`](../../src/features/skill_plan_editor.rs) (seeding callers, `computed()`, export)
- `src/features/skill_plan_editor/` submodules (`entry_row`, `plan_entry_list`, `import_export`, `summary`,
  `remap_divider`, `remap_insertion`)
- `src/features/skills/` optimizer and attributes
- Cross-character copy (Manage Plans window) reuses the shared persist routine.

## Consequences

### Positive

- Plans are portable: export/import/copy across characters reproduces the complete plan; each character then sees only
  what they still need.
- A single source of truth for "persist a plan onto a character": copy and file-import are guaranteed identical.
- The view never shows a level the current character already trained.

### Negative

- Stored plans grow (they now include trained low levels), and existing plans need a one-time on-load repair with a
  heuristic to detect under-expansion.
- A subtle correctness contract: every calculation must consume projected needed-only steps, never raw stored entries.

## References

- Spec: skill plans store full skill set, project per-character (gest `tzkvrsyn`)
- Spec: Manage Plans, cross-character copy (gest `rkutqorr`)
- [`migrations/0027_create_skill_plans.sql`](../../migrations/0027_create_skill_plans.sql)
