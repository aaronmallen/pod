---
id: "0026"
title: Saved Build-Plan Node Identity
status: active
tags: [data-model, industry, persistence]
created: 2026-06-14
---

# ADR-0026: Saved Build-Plan Node Identity

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

A saved Industry build plan persists the planner's recursive build tree: a top-level product plus, for every material
the user chose to produce in-house, a nested node carrying its own material-efficiency (ME), time-efficiency (TE), and
facility. We persist this normalized across two tables — `industry_plans` (the parent: product type-id, runs, root
facility, saved-at) and `industry_plan_nodes` (one row per tree node) — rather than as a JSON blob, because pod has no
JSON-blob persistence convention. Each node row identifies itself by a **materialized path**: the `/`-joined chain of
material type-ids walked from the root down (the root node is the empty string `''`). This mirrors exactly how the
planner addresses nodes at runtime — `NodeConfig` keys children by material type-id and reaches a node by walking a
`Vec<i64>` of type-ids from the root — so a saved plan rehydrates straight back into the planner's tree model.

## Context

The planner's in-memory tree is `NodeConfig`, where each node holds `me`, `te`, and an optional `facility_system`, and
`children: BTreeMap<i64, NodeConfig>` keyed by the material type-id built in-house. A node is addressed at runtime by a
path slice of type-ids: `path = [A, B]` reaches `root.children[A].children[B]`. The root node is the product itself,
reached by the empty path. Run counts are not stored per node — every sub-build's runs are derived from parent demand —
so only the root's `runs` is authoritative.

Persisting a tree of arbitrary depth normalized requires a way to encode each node's position. Two options:

1. **Parent-pointer rows** — each node stores its own surrogate id plus a `parent_id` and the material type-id it
   represents. Rehydration walks the pointer graph, and reconstructing a node's runtime path requires either a
   recursive self-join or an in-memory second pass to chase parents back to the root.
2. **Materialized path** — each node stores the full `/`-joined chain of type-ids from the root. The node's runtime
   address is the row itself; no surrogate node ids, no parent pointers, no recursion.

Because the planner *already* keys and addresses nodes by exactly this path-of-type-ids, option 2 makes the stored
identity and the runtime identity the same value.

## Decision

Identify each `industry_plan_nodes` row by a **materialized-path text column** — the `/`-joined material type-ids from
the root, with the root node stored as the empty string. A `UNIQUE(plan_id, path)` index enforces one row per node.

- **Snapshot** flattens the `NodeConfig` tree depth-first, emitting one node per visited path (root included).
- **Load** reads the rows back and rebuilds the tree by processing nodes shortest-path-first, so each parent exists
  before its children are inserted; the rebuild is therefore independent of the row order returned by the query.
- Encoding/decoding is a pure split/join on `/`. Type-ids are positive integers, so `/` is an unambiguous separator and
  the empty string is an unambiguous root sentinel.

The parent `industry_plans` row redundantly records the root facility (an acceptance requirement and a convenient
list-view column); the per-node rows remain authoritative for tree rehydration. Child rows cascade-delete with their
parent plan.

## Consequences

- Rehydration is a single ordered scan with no recursive self-join and no parent-chasing; the stored path *is* the
  runtime path the planner consumes.
- Node identity is self-contained — there are no surrogate node ids to allocate, reference, or keep stable across saves.
- Moving a subtree would mean rewriting the paths of all its descendants, but the planner has no move operation
  (nodes are broken-down or collapsed in place), so this never arises.
- The separator and root-sentinel encoding rely on type-ids being non-negative integers; a non-integer or negative key
  would break the scheme, but type-ids are always positive in the EVE data model.
