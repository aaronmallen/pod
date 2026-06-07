---
id: "0008"
title: Assets Data Path
status: active
tags: [architecture, assets, database, pagination, search, sde, ui]
created: 2026-06-06
---

# ADR-0008: Assets Data Path

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The v0.5 Assets rewrite (spec `wwyvxlsm`, S6) must serve character and corporation assets at 1B+ rows across 40+
characters with **all** filtering, sorting, pagination, and hierarchy navigation pushed to the database — the v0.4
screen's in-memory filtering was its most expensive failure (post-mortem `snntmuyv` #2). Four data-path decisions
gate every downstream schema and query task and are settled here:

1. The asset **hierarchy** (`container_id` / `depth` / `is_container`) lives as **persisted columns written by the
   asset sync each cycle**, not as a derived SQL view — an explicit, reconciled departure from the canonical-data-
   model preference for deriving composites, justified by post-mortem #3/#4 and ADR-0002's completeness contract.
2. The flat Inventory table paginates by **keyset on `(sort_col, item_id)`**, not offset/limit, over a fixed set of
   **DB-indexable sort columns**.
3. Text search uses **SQLite `LIKE '%term%'` over an SDE-joined `type_name`**, not an FTS5 virtual table.
4. The Abyssals tab **overrides** the prototype's equal-height `chunks(cols)` card grid: cards are grouped by item
   type, a forced row break separates groups, and each card sizes to its own content. This is recorded here as an
   **intentional, signed-off departure from prototype-as-UI-truth**.

The DDL implied by decision (1) is presented below for sign-off so the downstream schema tasks can edit the
create-table migrations to an approved shape (no ALTER migration — per the in-place-migration rule).

## Context

The v0.4 Assets screen failed structurally, and the post-mortem (`snntmuyv`) locks six constraints on the rewrite.
Three of them bear directly on the data path and force decisions the spec left open:

- **#2 — in-memory filtering does not scale.** With millions of assets, search only saw loaded rows; items in
  collapsed/unexpanded containers were undiscoverable. The lesson is explicit: *implement database-level search and
  pagination from the start; never build memory-based filtering — it breaks with lazy-loading.*
- **#3 — container toggle was inverted** (a `HashSet::insert` return-value bug). Orthogonal to the data path but it
  motivates persisting *whether a row is a container* so the renderer can draw an expand caret without holding the
  child set in memory.
- **#4 — lazy-loaded items nested under the wrong parent** because they were written with `container_id: 0`, which
  the tree read as top-level. The lesson: *hierarchy rendering requires explicit parent ids on every row, even
  lazy-loaded ones.*

The 0.5.0 prototype reproduces exactly the forbidden path: its controller
(`tmp/scratch/0.5.0-prototype.1/src/controllers/assets.rs`) loads every asset row into a `Vec`, computes nesting in
memory (`compute_depth` / `build_is_container_set`), and filters via `AssetFilterQuery::matches` predicates. The
prototype is the visual/behavioral reference for layout and the filter grammar, but its **data path is explicitly
being replaced** with a DB-driven one. That replacement raises four questions the spec flagged as open and that the
rest of S6 cannot proceed without:

- **Hierarchy home** (Open Question 3): stored columns written by sync per cycle vs. a derived SQL view. The
  canonical data model (`canonical-data-model`) favors one home per fact and deriving composites via views; the
  post-mortem favors explicit persisted parent tracking. These pull in opposite directions and must be reconciled.
- **Pagination** (Open Question 1): keyset `(sort_col, item_id)` vs. offset/limit, plus the exact set of columns a
  user can sort by that must therefore be DB-indexable.
- **Search index** (Open Question 2): `LIKE '%term%'` over an SDE-joined `type_name` view vs. an FTS5 virtual table
  kept in sync with the SDE seed.
- **Abyssals layout**: the spec signs off a specific override of the prototype's card grid; prototype-as-UI-truth is
  a standing project rule (`stick-to-the-design-never-invent-ui`), so the departure must be recorded explicitly.

Metadata (`type_name` / `group_name` / `category` / `volume`) is **not** in tension and is settled by ADR-0006: it
is never denormalized onto asset rows; it resolves by joining `asset.type_id` through the SDE-seeded
`item_types → item_groups → item_categories` chain inside the SQL query, so a row is always returned fully populated
(post-mortem #5). This ADR builds on that join rather than re-deciding it.

## Decision

### 1. Hierarchy lives in persisted columns, written by sync each cycle

`character_assets` and `corporation_assets` gain three explicit hierarchy columns — `container_id`, `depth`,
`is_container` — **persisted**, written by the asset sync job on each cycle, **not** derived by a SQL view at read
time.

- `container_id` — the real parent `item_id` when the asset is inside another item (ESI `location_type == "item"`);
  **`NULL` when the asset sits directly in a station/structure/space** — never `0` (the exact bug of post-mortem
  #4). Modeled as nullable rather than a `0` sentinel so "top level" is a first-class state and a join on
  `container_id` cannot accidentally match a real `item_id` of 0.
- `depth` — integer nesting depth, `0` at a hangar/station root, written by sync, never recomputed at render
  (replacing the prototype's `compute_depth` walk).
- `is_container` — boolean, true when other rows declare this item as their `container_id`. Persisting it lets the
  tree draw an expand caret per row without loading the child set into memory (post-mortem #3's renderer need).

**Reconciliation with the canonical data model.** The canonical model's preference for deriving composites via views
holds where a composite is a pure, cheap function of other columns *available on the same row*. The asset hierarchy
is not that. `container_id` is a fact ESI reports per asset (the `location_id` + `location_type` pair), so it is a
base fact with one home — the asset row — not a derived composite; persisting it is *consistent* with the canonical
model, not an exception to it. `depth` and `is_container` *are* derivable (a recursive CTE over `container_id`), and
this is the genuine tension. We persist them anyway, for reasons the canonical model's "derive via view" guidance
does not cover:

- **ADR-0002 completeness contract.** Sync owns all ESI calls and parent resolution and must write rows that are
  *complete and renderable on their own*. A row whose nesting is only knowable by a recursive read-time CTE is not
  complete by that contract; computing it at sync time, once, and storing it keeps the row self-contained.
- **Render must not pay a recursive cost.** A derived view would force a recursive CTE (or per-expand re-walk) into
  the hot pagination/expansion path at 1B+ rows. The post-mortem's whole thesis is that the read path must be cheap
  and DB-indexable; a recursive composite undermines that. Persisting `depth`/`is_container` makes the read path a
  plain indexed lookup.
- **Post-mortem #3/#4 favor explicit persisted parent tracking** as the direct fix for the wrong-nesting and
  caret-drawing failures. This ADR follows that lean.

The sync job is the **single writer** of all three columns and recomputes them each cycle from the freshly synced
asset set (it already owns parent resolution), so they never drift from `container_id`. They are added by **editing
the original create-table migrations** (`m0000000019_create_character_assets`,
`m0000000047_create_corporation_assets` in the prototype lineage) — never an ALTER migration, per the
edit-migrations-in-place rule for this unpublished project.

#### Proposed DDL (presented for sign-off)

Applies to **both** `character_assets` and `corporation_assets` (substitute the owner column —
`character_id` / `corporation_id`):

```sql
-- added to the existing CREATE TABLE (edited in place, not ALTER):
container_id  INTEGER NULL,                       -- real parent item_id; NULL at station/structure/space root
depth         INTEGER NOT NULL DEFAULT 0,         -- nesting depth, 0 at a hangar/station root
is_container  INTEGER NOT NULL DEFAULT 0,         -- 1 when some row declares this item_id as its container_id

-- indexes supporting on-demand child fetch and roll-up aggregates:
CREATE INDEX ix_<table>_owner_container ON <table>(<owner_id>, container_id);
CREATE INDEX ix_<table>_container       ON <table>(container_id);
```

Notes for the schema task:

- SQLite has no native `BOOLEAN`; `is_container` is `INTEGER` storing `0`/`1` (matching the project's existing
  boolean convention — see `0011` migration `0020`).
- `container_id` is **not** a foreign key to `<table>(item_id)`: parent and child can arrive in any order within a
  sync cycle, which ADR-0003's skip-until-parent ordering tolerates precisely because there is no enforced FK on
  intra-asset references; a self-FK would reintroduce the ordering hazard ADR-0003 removed.
- The composite `(owner_id, container_id)` index serves the lazy-expansion query ("children of item X for this
  owner") and the per-node roll-up aggregates; the bare `(container_id)` index serves the ancestor-chain
  auto-expand query that walks up `container_id` regardless of owner scope.
- The search-supporting index over the SDE-joined `type_name` is **not** added here — see decision (3), which keeps
  `type_name` out of the asset table entirely.

### 2. Pagination: keyset on `(sort_col, item_id)` over a fixed, indexable sort-column set

The flat Inventory table paginates by **keyset** (seek), not offset/limit. Each page request carries the active sort
column and the `(last_sort_value, last_item_id)` cursor of the previous page; the query is
`WHERE (sort_col, item_id) > (:cursor_val, :cursor_item) ORDER BY sort_col, item_id LIMIT :page`, with `item_id` as
the deterministic tiebreaker that makes the order total even when the sort column has duplicates (many assets share
a `type_name`, `group`, `volume`, or unit price).

Keyset is chosen over offset/limit because:

- **It is O(page), not O(offset).** Offset/limit must scan and discard `offset` rows; at 1B+ rows a deep page is a
  full-prefix scan. Keyset seeks directly into the index. The post-mortem's scale target makes this decisive.
- **It is stable under live re-sync.** Assets are re-synced underneath an open screen; rows inserted/removed above
  the current position shift every offset, causing skipped or duplicated rows with offset/limit. A keyset cursor is
  anchored to a value, not a position, so it stays correct across concurrent writes.

**DB-indexable sort columns.** The user may sort the flat table by exactly this set (from the spec's Open Question 1
and the prototype's column header set); each must back a keyset index that ends in `item_id`:

| Sort key   | Backing value                                             |
|------------|-----------------------------------------------------------|
| name       | SDE-joined `type_name` (`type_id → item_types`)           |
| group      | SDE-joined `group_name` (`type_id → item_groups`)         |
| qty        | `quantity` (asset column)                                 |
| volume     | `type.volume * quantity` — packaged volume, SDE `volume`  |
| unit price | latest unit price for `type_id` (Market Prices spec)      |
| value      | `quantity * unit_price` (derived from the two above)      |
| owner      | `character_id` / `corporation_id` (+ resolved owner name) |
| location   | resolved location/structure/station name (name cache)     |

`qty` is the only sort key that is a plain asset column; the rest derive from joins. Because `name`, `group`,
`volume`, `unit price`, and `value` derive from **joined** SDE/price/location data rather
than from asset columns, their keyset indexes cannot be plain single-table column indexes. The schema task should
materialize the sortable view (asset ⋈ SDE type/group ⋈ price ⋈ location) and back each sort key with an index on
`(sort_expr, item_id)` over that join — or, where SQLite cannot index a cross-table expression directly, persist the
join's sort key into the view/materialization the pagination query reads. The **set above is the contract**; how
each index is physically realized is a schema-task detail, but every column in the set must be DB-orderable without
loading rows into memory.

### 3. Search: `LIKE '%term%'` over an SDE-joined `type_name`, not FTS5

Free-text search compiles to **SQLite `LIKE '%term%'`** against the SDE-joined `type_name`, evaluated at the DB
layer. We do **not** introduce an FTS5 virtual table.

Rationale, per the spec's own framing:

- **The corpus is the SDE type dictionary, which is already a seeded, indexed table.** `type_name` comes from the
  `type_id → item_types` join (ADR-0006). FTS5's advantage is fast *token/substring* search over large free text;
  EVE type names are short, structured labels, and the search target is "does this type name contain the term," for
  which `LIKE '%term%'` over the joined name is adequate at the row counts involved (the distinct *type* count is
  tens of thousands, not the 1B asset rows — search filters on type, then the asset scan is bounded by the
  hierarchy/owner indexes).
- **FTS5 adds a second index that must be kept in sync with the SDE seed.** An FTS5 table over SDE type names is a
  derived copy that the SDE seed (ADR-0006, splash-owned, version-gated, `upsert_many`) would have to populate and
  re-populate on every re-seed, plus guard for staleness. That is exactly the "an index to keep in sync with the SDE
  seed" cost the spec flagged. `LIKE` over the live joined name has **no separate index to maintain** — it reads the
  same `item_types` rows the seed already owns, so it cannot drift from the seed.
- **The structured grammar, not raw substring speed, is the hard part** and it is unaffected by this choice. The
  free-text term is only the bare-token case; the structured keys (`name`/`n`, `group`/`g`, `category`/`cat`,
  `region`/`r`, `constellation`/`c`, `system`/`s`, `location`/`loc`, `owner` incl. `me`, `type:`
  bpc/bpo/singleton/stack), `-` negation, comma = OR, multiple tokens = AND, and quoted phrases are re-implemented
  as a **SQL WHERE-clause compiler** (reusing `AssetFilterQuery::parse` as the parser→AST stage; only the in-memory
  `matches`/`match_*` predicates are retired). A free-text token becomes one `type_name LIKE '%term%'` clause inside
  that compiler; the rest become joins/predicates on owner, location, and SDE category/group. FTS5 would only
  accelerate the bare-token clause while complicating the negation and AND/OR composition (FTS5 `MATCH` does not
  compose cleanly with arbitrary `WHERE` predicates), so it is a poor fit for the compiler regardless.

**Consequence to record:** if profiling later shows the `LIKE` scan is the bottleneck at extreme scale, the
mitigation is a generated/`COLLATE NOCASE` index on `item_types(type_name)` (a single seed-owned table, cheaply
re-derivable), **not** an FTS5 table — FTS5 remains explicitly out of scope (see Future Work). The
auto-expand-on-hit capability (find ancestor containers holding a match by walking `container_id` chains in SQL) is
built on this same WHERE compiler and is independent of the index choice.

### 4. Abyssals card grid: grouped, forced row break, natural height — a signed-off override of the prototype

The Abyssals tab **overrides** the prototype's layout. The prototype
(`tmp/scratch/0.5.0-prototype.1/crates/ui/src/views/assets/abyssals_tab/card_grid.rs`) chunks all filtered cards
into rows of `N` columns regardless of type (`visible_items.chunks(cols)`), which forces every card in a row to the
tallest card's height. Abyssal modules have a **variable stat-row count per module family**, so the chunked layout
stretches short cards to match tall ones and interleaves unrelated families in a row.

The replacement layout is:

- **Group cards by item type** (the abyssal source module / family).
- **Insert a forced row break between groups** so a group never shares a row with another type.
- **Size each card to its own content** (natural height) — **no equal-height stretch** across mixed types.

This is recorded as an **intentional, signed-off departure from prototype-as-UI-truth**. Prototype-as-UI-truth is a
standing rule for this project (`stick-to-the-design-never-invent-ui`): where a 0.5.0-prototype view exists, it is
the UI truth and treatments are not to be invented. This ADR is the explicit exception the rule allows: the spec's
Phase 4 signs off the override by name, the rationale (mixed-family stat-row counts make equal-height stretch a
visual defect, not a feature) is on record here, and the override is scoped strictly to `abyssals_tab/card_grid.rs`.
Implementations must cite this ADR where they diverge from the prototype's `chunks(cols)` so the divergence is never
mistaken for drift. The human sign-off was unavailable interactively; the spec's written sign-off in Phase 4 is
treated as the authority and this ADR ratifies it.

### 5. Terminal-inaccessible structures: a bounded exception to the completeness contract

ADR-0002's completeness contract requires sync to resolve every referenced location before persisting an asset row, so
`AssetSync`'s Finished event means "fully displayable." A player structure that returns **403/404 from every available
structure-read grant** cannot be resolved — yet dropping the asset would hide an owned holding, and leaving it
unresolved would keep the sync chip In Progress forever. The bounded exception: such a structure is recorded as
**terminally inaccessible for the owning subject** in the durable, per-`(owner_id, owner_type, structure_id)`
`inaccessible_structures` marker (migration `0007`), and the asset is **still persisted**. The asset view renders that
location as the literal "Inaccessible Structure". The marker is per-subject because a structure one character cannot
read may be readable by another owner who has docking access. This keeps the contract honest — every persisted row is
renderable (with a real name or the explicit inaccessible label), and a 403/404 structure neither drops an asset nor
hangs the chip.

## Affected Areas

- `character_assets` / `corporation_assets` create-table migrations (prototype lineage
  `m0000000019_create_character_assets`, `m0000000047_create_corporation_assets`) — gain `container_id`, `depth`,
  `is_container` and the two hierarchy indexes, edited **in place** (no ALTER). DDL above is the approved shape.
- The asset **sync job** — becomes the single writer of `container_id`/`depth`/`is_container`, recomputing them each
  cycle from the synced asset set (it already owns parent resolution; ADR-0002/ADR-0003).
- The assets **read/query layer** — keyset pagination over the fixed sort-column set; the structured-filter
  WHERE-clause compiler (reusing `AssetFilterQuery::parse`); the SDE-join sortable view backing the indexable sort
  keys; the `LIKE`-on-`type_name` free-text clause; the ancestor-chain auto-expand query.
- The assets **UI** — the location tree reads `is_container`/`depth` for carets and indentation and fetches children
  by `container_id` on demand; the flat table consumes keyset pages; the **Abyssals tab** uses the grouped,
  natural-height card layout instead of `chunks(cols)`.
- Read-only consumers of the **Market Prices & Financial Aggregation** spec's outputs (unit price for the price/value
  sort keys) and the structure/station name cache (the location sort key) — joined, not owned here.

## Consequences

### Positive

- The read path is fully DB-indexable: pagination, sort, hierarchy navigation, and search all run as indexed SQL
  with no full-asset-set materialization, directly answering post-mortem #2 at the 1B-row target.
- Persisted `container_id`/`depth`/`is_container` make every row self-contained and renderable on its own
  (ADR-0002), fix the `container_id: 0` mis-nesting (#4), and let the tree draw carets without holding child sets
  (#3) — and avoid a recursive CTE in the hot path.
- Keyset pagination is stable under live re-sync and cheap at any depth, where offset/limit degrades and skips/dupes
  rows.
- `LIKE` over the seeded `type_name` adds **no** index to keep in sync with the SDE seed; search cannot drift from
  the seed because it reads the seed's own `item_types` rows.
- The abyssals override is on record with rationale, so the divergence from the prototype is auditable and not
  mistaken for UI drift.

### Negative

- `depth`/`is_container` are **persisted derived state**: sync must recompute them each cycle and a sync bug could
  let them drift from `container_id`. Mitigated by sync being their single writer and recomputing from the full
  synced set each cycle; a diagnostic asserting `is_container` ⇔ "some row has this `container_id`" is cheap to add.
- The indexable sort set includes join-derived keys (name, group, volume, price, value) that cannot be plain
  single-column indexes; the schema task must materialize/index the asset⋈SDE⋈price⋈location sort view, which is
  more work than indexing raw columns.
- `LIKE '%term%'` is a leading-wildcard scan and is not index-accelerated by a B-tree; acceptable because it filters
  on the tens-of-thousands-row type dictionary, not the asset rows, but it bounds how cheap free-text search can get
  without the (deferred) collation-index mitigation.
- This ADR is one of several Phase-1 sign-off ADRs; the DDL here is binding on the schema tasks and changing the
  hierarchy shape later would mean re-editing the create-table migrations (acceptable while unpublished).

## Open Questions

- The remaining S6 open questions are **out of this ADR's scope** and stay with their own tasks: the exact corp
  role(s) that gate corp-asset visibility (needs the corp-roles model from org sync) and whether the 90-day Tracker
  NAV series is produced by the Market Prices & Financial Aggregation spec or computed here from
  `AssetValueSummary` snapshots. This ADR settles only the four data-path decisions above.
- Whether the `unit price` / `value` keyset sort keys need a recomputed materialization on every price refresh, or
  can join live against the price table at page time — a profiling call for the schema/query task, not a blocker on
  the shape decided here.

## Future Work

- **FTS5 is explicitly deferred, not adopted.** If `LIKE` profiling shows a bottleneck at extreme scale, the first
  mitigation is a `COLLATE NOCASE` (or generated) index on the single seed-owned `item_types(type_name)` table, kept
  current by the SDE seed; FTS5 would only be revisited if even that proves insufficient, and would then need its
  own ADR covering seed-sync of the FTS index.
- A self-referential foreign key on `container_id → item_id` could be reconsidered if/when ADR-0003's intra-sync
  ordering gains a general deferred-FK guarantee for self-references; today it is deliberately omitted to preserve
  skip-until-parent tolerance.

## References

- ADR-0002 — Sync/Render Separation (`0002-sync-render-separation.md`). Sync owns ESI calls and parent resolution
  and must write complete, renderable rows; the persisted hierarchy columns realize that completeness contract.
- ADR-0013 — Image Assets — Committed Item Icons and Synced Portraits/Logos (`0013-committed-item-icon-set.md`). The
  icon `(type_id, variant) → (type_id, "icon")` fallback (post-mortem #6) lives in that path, not this ADR.
- ADR-0003 — Canonical Data Model (`0003-canonical-data-model.md`). Why `container_id` is **not**
  a self-FK: skip-until-parent ordering tolerates any intra-cycle arrival order only because intra-asset references
  carry no enforced FK.
- ADR-0006 — Static and Reference Data (`0006-static-and-reference-data.md`). The source of the `type_name` / `group` /
  `category` / `volume` joins this ADR's search and sort keys read; the seed this ADR deliberately does **not** add
  an FTS index to.
- Spec — "S6: Assets" (gest artifact `wwyvxlsm`): Open Questions 1–3 (pagination, search index, hierarchy home) and
  the Phase-4 signed-off abyssals card-grid override.
- Post-mortem (LOCKED) — "Pod v0.4.8 Hotfix: Lessons Learned" (gest artifact `snntmuyv`): #2 DB-level search/
  pagination, #3 explicit container state, #4 explicit parent tracking, #5 complete metadata.
- Prototype override target — `tmp/scratch/0.5.0-prototype.1/crates/ui/src/views/assets/abyssals_tab/card_grid.rs`
  (`visible_items.chunks(cols)`).
- Project rule — prototype-as-UI-truth (`stick-to-the-design-never-invent-ui`): this ADR is the explicit, scoped
  exception the rule permits.
