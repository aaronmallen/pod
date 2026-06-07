---
id: "0006"
title: Static and Reference Data
status: active
tags: [architecture, bootstrap, database, sde, splash, store, esi, name-resolution, sync]
created: 2026-06-06
---

# ADR-0006: Static and Reference Data

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod provisions reference data two ways, recorded here as one domain. **(1) SDE static-data seeding** —
the bulk baseline (item categories/groups/market-groups/types — and therefore skills, since a skill is
a category-16 item type — plus certificates, ship masteries, and lore: factions/races/bloodlines)
comes from CCP's **Static Data Export (SDE)** as a **splash-owned first-run bootstrap step**:
version-gated, downloaded **stream-and-discard** (no on-disk cache), recorded by a **marker row** in
`pod.db`, and inserted in **strict foreign-key order with `foreign_keys` left on**. **(2) Universe name
resolution** — the id-to-name mapping for **dynamic** entities (characters, corporations, alliances,
and other non-static ids) that have no SDE home lives behind ESI's `POST /universe/names/`, owned by a
**single shared, caller-agnostic resolver** (`sync/jobs/names.rs::resolve_names`) that batches,
deduplicates, and gracefully partitions unresolvable ids; **factions are excluded** and resolve from
the seeded SDE faction table, and resolved names are **persisted denormalized onto each consuming job's
rows** (no shared name-cache table). SDE seeding fits neither [ADR-0002](0002-sync-render-separation.md)
nor [ADR-0003](0003-canonical-data-model.md) and forms a distinct third write path; it also realizes the
universe/reference-data baseline anticipated by ADR-0003's Foreign Keys Future Work.

---

## Part 1 — SDE Static-Data Seeding

### Context

Two write paths exist today. ADR-0002 (Sync/Render Separation) describes the **sync engine**, which
follows ESI dependency chains and writes complete records for the subjects it discovers from
`credential::all` / `character::all`. [ADR-0003](0003-canonical-data-model.md) (App-Owned Data)
describes the **feature** write path, for data the user authors inside Pod, keyed on
application-allocated ids and excluded from sync. The SDE belongs to neither:

- It is **not credential-scoped ESI data.** The SDE is a complete, game-wide static export published by
  CCP, not a per-character ESI response. No character's credentials "discover" it, and the sync engine's
  subject-discovery model has no hook for "the whole game's item dictionary." Routing it through sync
  would mean inventing a subjectless job whose completeness contract is "all of EVE," which the
  sync model is not built for.
- It is **not app-owned.** Every SDE row is keyed on its **EVE entity id** (a type id, certificate id,
  faction id), which is the exact opposite of ADR-0003's application-allocated ids. The user does not
  author it and must never mutate it; it is canonical upstream reference data.

The functional gap is concrete. Reference tables are currently populated **only lazily**, one id at a
time over ESI by `sync/jobs/resolve.rs`. Consequences:

- Item and skill **names** are unavailable until a resolve cycle happens to fetch that id.
- **Certificates** and **ship masteries** have no source at all over ESI — the skill-plan picker's
  cert/mastery tabs are stuck on "Coming soon — needs resolvers."
- There is no **bulk baseline**: a screen that wants the full skill catalog or item dictionary cannot
  get it from the trickle of lazily-resolved ids.

The splash screen, meanwhile, already exists as the app's first-run gate but is cosmetic —
`run_bootstrap` is hardcoded sleeps and the splash→main transition fires on an animation, not on real
readiness. A first-run bulk load is exactly the kind of work a splash should gate on.

Finally, ADR-0003 (Foreign Keys Within Sync Scope) dropped foreign keys on references into data Pod
does not sync (item types, NPC corporations, stations, solar systems) and named, in its Future Work, a
"dedicated universe/reference sync" that would let some of those become real foreign keys again. The
SDE seed is the reference-data baseline that Future Work anticipated.

### Decision

Load the SDE as a **splash-owned first-run bootstrap step** — a distinct, third-write-path category
alongside sync (ADR-0002) and app-owned features (ADR-0003), with its own rules.

#### Ownership: the splash bootstrap, not sync, not a feature mutation

The seed runs inside the splash feature's bootstrap (`run_bootstrap`), over the splash's existing
`mpsc` / `Task::stream` plumbing, emitting a real per-stage progress label ("Seeding item types…",
"Seeding certificates…", …). The `store::Database` handle is opened in `app::init` and threaded into
the bootstrap task; the splash does not open its own connection. The **splash→main transition gates on
genuine seed + init completion**, not on the expand animation. The sync engine neither runs nor
discovers this work, and no feature `update` triggers it; it is a one-time bootstrap, not an ongoing
write path.

#### Version-gated: do the work only when the data is stale

A KV marker table (`app_metadata`) holds an `sde_version` row whose value is a **composite version**,
`{sdeBuild}+pod-{CARGO_PKG_VERSION}`. On startup the bootstrap compares the recorded value against the
current one:

- **First run, a newer SDE build, or a newer Pod version** → download and seed.
- **Recorded version current** → skip the download and seed entirely and proceed to main immediately.

Folding the Pod version into the marker means a Pod release that changes how the SDE maps into the
schema re-seeds even against an unchanged SDE build, without a separate migration to force it.

#### Stream-and-discard download (ADR-0007 compliance)

The SDE is a CCP **static export, not an ESI endpoint**, so it must **not** route through `esi.rs`; a
dedicated `src/clients/sde.rs` fetches the zip via the uncached HTTP path
(`http::Client::get_bytes_uncached`), extracts it to a **temp directory** on `spawn_blocking`, seeds
from the extracted YAML, and **discards both the zip and the extract** when seeding finishes. Nothing
SDE-derived persists on disk except the seeded database rows and the `sde_version` marker. This honors
[ADR-0007](0007-user-configurable-storage-paths.md)'s invariant that **Pod keeps no separate on-disk
cache** — transient data lives in the database (here, the seeded rows), not in a relocatable cache
directory. The seeded rows live in `pod.db` and therefore relocate with the database under ADR-0007's
move-on-change semantics, as one unit, with no extra store to move.

#### Strict foreign-key-ordered seed, FKs never disabled

The seed writes through normalized tables with **real foreign keys** (new migration `0020`:
`certificates`, `certificate_skills` → `item_types`, `ship_masteries` → `item_types`/`certificates`,
plus the `app_metadata` KV table; lore reuses the existing `factions`/`races`/`bloodlines` tables).
Because these are enforced foreign keys, the seed **inserts in strict dependency order** — categories →
groups → market-groups → item types (which seeds skills, category 16), then certificates →
certificate-skills → ship masteries, then lore (races before bloodlines; bloodlines after item types,
since `bloodlines.ship_type_id` → `item_types`).

Crucially, this is done with **`foreign_keys` left ON** for the whole seed — the store opens with
`foreign_keys(true)` and the seed **never issues `PRAGMA foreign_keys = OFF`**. Where a single
transaction needs to insert both ends of a relationship, it may use **deferred** FK checks (checked at
commit, as ADR-0003 already relies on for the org stack), but enforcement is never turned off. Bulk
`upsert_many` writers replace the current single-row/per-row-transaction idiom so that seeding tens of
thousands of item types is fast, without weakening integrity.

#### Scope of the seed

In scope: the four item tables (preserving the embedded `dogma_attributes` JSON so `resolve.rs` keeps
deriving `skill_metadata`), certificates, certificate-skills, ship masteries, and opted-in lore
(factions/races/bloodlines). Explicitly **out of scope**: all universe/space data (regions,
constellations, solar systems, stars, planets, stargates), abyssal/dogma-definition tables, and skill
prerequisite parsing.

### Consequences

#### Positive

- A real bulk baseline exists on first run: full item/skill catalog, certificates, and ship masteries
  are present, unblocking the skill-plan picker's cert/mastery tabs and any screen needing the whole
  dictionary.
- The seed is a clean third category with its own rules, so it neither distorts the sync model
  (ADR-0002) with a subjectless "all of EVE" job nor masquerades as app-owned data (ADR-0003) despite
  being EVE-id-keyed and user-immutable.
- Version-gating keeps subsequent launches fast: an up-to-date DB skips download and seed and reaches
  main immediately; the splash only does real work when the data is actually stale.
- Stream-and-discard keeps ADR-0007's "no separate on-disk cache" property intact — the only persistent
  artifacts are the seeded rows and the marker, all inside `pod.db` and relocatable with it.
- Integrity is preserved: the seed runs with `foreign_keys(true)` throughout and inserts in dependency
  order, so the new normalized tables get genuine referential integrity (advancing ADR-0003's Future
  Work for reference data) rather than the denormalized-id workaround.

#### Negative

- First run has a noticeable delay: a multi-tens-of-MB download plus a bulk seed of tens of thousands
  of rows must complete before the main UI appears. The splash must communicate this; minimum-display
  time must be reconciled with a possibly-long seed.
- A third write path touches the database (sync, features, **and** the splash seed). They stay on
  separate concerns — the seed owns reference tables, writes once at bootstrap, and is idempotent via
  `upsert_many` — but a reader must now know that reference rows can originate from either the seed or
  the lazy resolver.
- The seed depends on the SDE's on-disk YAML layout (e.g. `find_sde_root` keyed on `categories.yaml`,
  and the presence of `certificates.yaml` / `masteries.yaml`); an upstream packaging change could break
  parsing. Optional files are guarded and skipped rather than fatal.
- Strict FK-ordered insertion couples the seed to the schema's dependency graph; adding a new seeded
  table requires placing it correctly in the order.

### Open Questions

- Does the targeted SDE distribution ship `certificates.yaml` and `masteries.yaml` inside the zip? The
  seed guards both with an existence check and skips silently if absent.
- The minimum splash display time versus a possibly-long first-run seed — how to present a long initial
  bootstrap without a jarringly brief or stalled splash.

### Future Work

- This seed realizes the **reference-data** half of ADR-0003's Foreign Keys Future Work (a dedicated
  universe/reference baseline). The **universe/space** half — regions, constellations, solar systems,
  stations — remains out of scope here and would let further ADR-0003 keys (`corporations` → `stations`,
  `factions` → `solar_systems`) become enforceable foreign keys when seeded.
- Skill **prerequisite** parsing and surfacing (deferred to the skills-screen epic); the seed only
  preserves the dogma JSON today.
- Refreshing the seed **outside** the splash (e.g. a settings-triggered re-seed) rather than only on a
  version-gated first run.

---

## Part 2 — Universe Name Resolution

### Context

Standings, contacts, and contracts (S4 Character Detail) and mail (S7) all reference entities —
characters, corporations, alliances, and other non-static ids — by numeric id. To render
"Pilot X has +5 standing" the screen needs the *name* behind an id, but:

- The current tree had **no `/universe/names/` path**. `src/clients/esi/universe.rs` carried only GET
  lookups (regions, systems, stations, types) plus the exact-name `POST /universe/ids/` and the
  character-scoped type-ahead `search`. None map a *set of arbitrary ids* to names.
- ADR-0002 forbids the render path from calling ESI: each row must be **complete and renderable on
  write**. So a name has to be resolved during sync and stored, not fetched at draw time.
- Part 1 already seeds factions (and other SDE reference data) into local tables. Factions therefore
  have a static home and must **not** be round-tripped through ESI.
- The same resolver is independently required by **two specs** — S4 (standings/contacts/contracts) and
  S7 (mail sender/recipient names). Implementing it twice would duplicate the batching/partitioning
  logic and the all-or-nothing 404 hazard.

`POST /universe/names/` has two operational constraints that any consumer must handle:

1. **Per-request cap.** ESI rejects a request carrying more than 1000 ids.
2. **All-or-nothing resolution.** If *any* id in the request cannot be resolved (e.g. a since-deleted
   character), ESI returns `404` for the **entire** request, with no indication of which id was bad.

Constraint 2 is the sharp edge: a caller cannot treat a 404 as "this batch failed," because one stale
id would then erase the names of every other (perfectly resolvable) entity in the same standings or
contacts set.

### Decision

#### One shared resolver, generic over caller

A single function lives next to the existing reference-data resolver
(`src/sync/jobs/resolve.rs`), mirroring its fetch-on-miss shape but for names:

```rust
// src/sync/jobs/names.rs
pub(crate) async fn resolve_names(
  ctx: &JobCtx<'_>,
  ids: &[i64],
) -> Result<HashMap<i64, NameRecord>, Error>;
```

It takes an arbitrary id set and returns a map of the ids ESI **could** resolve. It is **not**
Character-Detail-coupled: it knows nothing about standings, contacts, contracts, or mail, so S7-mail
calls the same function. It does **not** re-implement the raw POST; it calls the universe sub-client's
`names(&[i64]) -> Vec<NameRecord>` method, which owns the wire format.

#### Batching and deduplication

`resolve_names` deduplicates the input (sort + dedup) and splits it into ≤1000-id chunks, issuing one
POST per chunk. This satisfies constraint 1 transparently for every caller — a job assembling 4000
contact ids never has to know the cap exists.

#### Graceful partition of unresolvable ids (404)

On a `404` for a chunk, the resolver **bisects** the chunk and retries each half, recursing until it
isolates the unresolvable id(s) at singleton granularity. A singleton that still 404s is **dropped
from the result** (logged at debug), never mapped to a placeholder name. Every resolvable id in the
same original chunk is preserved. This converts ESI's all-or-nothing batch into per-id resolution
without the render path ever seeing a fabricated name.

A 404-heavy worst case costs `O(k·log n)` requests for `k` bad ids in a chunk of `n`; in practice
standings/contacts/mail sets contain few-to-no dead ids, so the common path is one request per chunk.

#### Genuine failures still abort

Any **non-404** error from ESI — `5xx`, throttle/error-limit, network, decode — propagates out of
`resolve_names` as `Err`. The calling sync job then aborts its cycle and writes nothing (ADR-0002's
abort-without-writing contract), rather than persisting a half-resolved set. So "an id is gone" is
tolerated, but "ESI is unhealthy" is not papered over.

#### Factions are out of scope

Factions resolve from the seeded SDE faction table (Part 1). Callers join SDE for faction ids and
route only the **non-static** ids (characters/corporations/alliances/etc.) through `resolve_names`.
The resolver has no faction branch; the separation is enforced at the call site and documented here.

#### Persistence: denormalized per row, no shared name-cache table

Resolved names are written **onto each consuming job's own rows** (e.g. `character_standings.from_name`,
`character_contacts.contact_name`, contract party names, mail sender/recipient names), not into a
shared `entity_names` cache table. This follows ADR-0002 (each row complete and renderable on write)
and the canonical-data-model preference (one home per fact, on the row that needs it). A shared
cache table was rejected: it would add a second home for the same fact, a staleness/eviction policy,
and a join on every render — costs the per-row denormalization avoids. `NameRecord.category` is
available to callers that need to bucket a row by entity kind (e.g. contact `All / Character / Corp /
Alliance`).

### Consequences

#### Positive

- Batching, deduplication, and the all-or-nothing-404 hazard are solved **once** and reused by every
  consumer, including S7-mail.
- The render path never calls ESI for a name (ADR-0002); names are on the row.
- A single dead id can no longer wipe the names of an entire standings/contacts set.
- Factions keep their single SDE home; no duplicate name source.

#### Negative

- Denormalized names can drift if an entity is renamed; refresh happens only when the owning job
  re-syncs that row. Acceptable: entity renames are rare and the next sync cycle reconciles them.
- The same name may be stored on multiple rows across tables (e.g. one corp appearing in both standings
  and contacts). Accepted as the cost of the no-shared-cache decision.
- A pathological chunk full of dead ids degrades to many bisection requests; bounded by `O(k·log n)`
  and not expected in real data.

### Future Work

- If profiling ever shows name resolution dominating a sync cycle, an opt-in shared cache could be
  layered **behind** `resolve_names` without changing its signature or any caller — out of scope here.

---

## Affected Areas

- `src/features/splash/` — `run_bootstrap` replaces its hardcoded sleeps with the real version-gated
  seed flow; the splash→main transition gates on genuine completion; a `seed.rs` pipeline orchestrates
  download → extract → strict-order seed → marker write.
- `src/clients/sde.rs` — new SDE downloader (uncached HTTP + zip extract); deliberately **not** part of
  `esi.rs`, since the SDE is not an ESI endpoint.
- `src/sync/jobs/names.rs` — the shared dynamic-entity name resolver (`resolve_names`) and its tests.
- `src/sync/jobs.rs` — registers the `names` module.
- `src/clients/esi/universe.rs` — the `names()` raw POST method the resolver calls.
- `src/sync/jobs/resolve.rs` — unchanged; it stays as the lazy gap-fill path for reference ids.
- `migrations/0020_*.sql` — `certificates`, `certificate_skills`, `ship_masteries`, and the
  `app_metadata` KV table, all with real foreign keys.
- `src/store/model/` + `src/store/repo/` — models/repos for the new tables, an `app_metadata` get/set
  accessor, and `upsert_many` bulk writers (added to `item` and the new repos).
- `src/app.rs` (`app::init`) — opens the `store::Database` and threads the handle into the splash
  bootstrap task.
- Consuming jobs (S4 `CharacterStandings` / `CharacterContacts` / `CharacterContracts`, S7-mail sync)
  call `resolve_names` and persist the names onto their rows.

## References

- [ADR-0002](0002-sync-render-separation.md) — Sync/Render Separation. The SDE is **not** this path
  (not credential-scoped ESI data, neither discovered nor written by sync); and the render path never
  live-fetches a name — names are resolved during sync and stored on the row.
- [ADR-0003](0003-canonical-data-model.md) — Canonical Data Model. The SDE is **not** app-owned data
  (EVE-id-keyed and user-immutable); and this seed is the reference-data baseline anticipated by that
  ADR's Foreign Keys Future Work.
- [ADR-0007](0007-user-configurable-storage-paths.md) — User-Configurable Storage Paths. Stream-and-
  discard honors the "no separate on-disk cache" invariant; seeded rows relocate with `pod.db`.
- Spec — "SDE Static-Data Seeding in the Splash Feature" (gest artifact `lvxzxwto`).
- Spec `tzvywnon` (S4: Character Detail).
- ESI `POST /universe/names/`.
