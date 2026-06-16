---
id: "0026"
title: Saved Build-Plan Type Identity
status: active
tags: [data-model, industry, persistence]
created: 2026-06-14
---

# ADR-0026: Saved Build-Plan Type Identity

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

A saved Industry build plan persists the user's build *intent*, keyed by item **type**: a top-level product plus, for
every item the user configured, its material-efficiency (ME), time-efficiency (TE), and facility, and a flag marking
which items they chose to produce in-house. We persist this normalized across two tables — `industry_plans` (the
parent: product type-id, runs, root facility, saved-at) and `industry_plan_types` (one row per distinct type) — rather
than as a JSON blob, because pod has no JSON-blob persistence convention. The recursive build tree the planner computes
is **derived offline**, not stored: at load time the planner walks recipes from the product and descends into a child
only when that child's type is flagged `built`, pulling each type's ME/TE/facility from its row. There are no per-node
rows and no per-node identity.

## Context

The planner's user-facing state is per **type**, not per tree position: a `settings: BTreeMap<i64, TypeSettings>`
(ME/TE/facility for each configured type, the root product included) plus a `built: BTreeSet<i64>` of the types produced
in-house. The computed `BuildPlan`/`BuildNode` tree is derived from these by `assemble()`, which walks the product's
recipe and recurses into a material only when its type is in `built`. A single type can therefore appear at many
positions in the tree (e.g. a component consumed by several sub-assemblies), but it has exactly one ME/TE/facility
setting — editing it applies to every occurrence — and one build-vs-buy decision.

Persisting this requires encoding the per-type intent, not the shape of the derived tree. The earlier design stored one
row per *node* of the computed tree, identified by a materialized path (the `/`-joined chain of type-ids from the root).
That mirrored an older runtime model where the planner keyed a `NodeConfig` tree by path and a type built by several
jobs produced duplicate nodes. The per-type model supersedes it: storing per-node rows would duplicate a type's
settings across every position it occupies and reintroduce the path keying the runtime no longer uses.

## Decision

Identify each `industry_plan_types` row by its **`type_id`**. A `UNIQUE(plan_id, type_id)` index enforces one row per
type per plan. Each row carries that type's `me`, `te`, `facility_system`, and a `built` flag (the root product is a row
with `built = 0`; every in-house type is a row with `built = 1`).

- **Snapshot** flattens the planner's `settings` map and `built` set into one row per type: each type in `settings`
  emits a row, with `built = 1` when that type is in `built`. The root product is always emitted.
- **Load** reads the rows back, rebuilding the `settings` map directly and collecting the `built`-flagged type-ids into
  the `built` set. The build tree is then derived by `assemble()` — no row order or recursive self-join is involved.
- The flatten/rehydrate is a flat per-type scan; there is no tree structure to encode or reconstruct in storage.

The parent `industry_plans` row redundantly records the root facility (an acceptance requirement and a convenient
list-view column); the per-type rows remain authoritative. Type rows cascade-delete with their parent plan.

## Consequences

- A type's settings are stored exactly once regardless of how many positions it occupies in the derived tree, matching
  the per-type editing model — there is no per-node duplication to keep consistent.
- Rehydration is a flat scan with no recursive self-join and no parent-chasing; the derived tree is recomputed from the
  product's recipes plus the `built` set, so it always reflects the current recipe data.
- There is no stored notion of tree position. A type that becomes unreachable in the recipe graph (e.g. a recipe
  changed) simply never gets walked into; its stale settings row is harmless and ignored.
- Identity is a plain integer `type_id`; there are no surrogate node ids or materialized paths to allocate or keep
  stable across saves.
