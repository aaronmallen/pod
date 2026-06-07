---
id: "0003"
title: Canonical Data Model
status: active
tags: [architecture, database, sync, db]
created: 2026-06-06
---

# ADR-0003: Canonical Data Model

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod's data model rests on three rules that together define how facts are stored, related, and derived.
**(1) App-owned data** — data the user authors inside Pod (squads, tags, skill plans, saved fittings) —
lives in its own tables, is written **directly by the feature that owns it** (a third write path
alongside the sync engine), uses application-allocated ids rather than ESI ids, and is never read or
written by the sync engine. **(2) Foreign keys within sync scope** — the database enforces a foreign
key only where Pod's sync inserts **both** rows of the relationship in the same transaction (a pilot's
own org stack); references to entities Pod does not sync are kept as plain reference ids without a
foreign key. **(3) Derived character state** — a character's live state (wallet balance, total skill
points) is derived from fundamental tables at read time via a SQL **VIEW**, never stored as a cached
scalar. The unifying principle is **one home per fact**: enforce what we sync, denormalize what we
don't, and derive composites rather than caching them.

---

## Part 1 — App-Owned Data

### Context

[ADR-0002](0002-sync-render-separation.md) describes two roles: the sync engine writes complete ESI
records to the database, and the render layer reads them. It does not address data the user creates
locally. Pod has a growing amount of such data — the first instance is squads (user-defined groupings
of characters), with tags, skill plans, and saved fittings to follow.

This data has different properties from ESI data: it has no canonical EVE id, it is mutated in
response to direct user actions (not a background cycle), and it must never be overwritten or
discovered by sync. Forcing it through the sync engine, or keying it on ESI ids it does not have,
would be wrong on both counts.

### Decision

App-owned data is a distinct category with its own rules:

- **Storage.** Its own tables, created by their own migrations, with `repo/` functions like any
  other table. The render layer reads it through those repos, consistent with ADR-0002 (display
  data comes from the database).
- **Write path.** The owning feature writes it **directly** (its `update` calls the repo via a
  task), not through the sync engine. This is a deliberate third write path: sync writes ESI
  records; features write user-authored records; render reads both.
- **Identity.** Rows use application-allocated ids (SQLite-assigned `INTEGER PRIMARY KEY`), not ESI
  ids. This intentionally differs from the ESI-sourced tables, whose `id` is the EVE entity id.
- **Sync exclusion.** The sync engine neither reads, writes, nor discovers app-owned tables. Subject
  discovery (ADR-0002) draws only from `credential::all`/`character::all`; app-owned tables have no
  credentials and so are excluded by construction.
- **References.** App-owned tables may foreign-key into sync-owned tables (e.g.
  `character_squads.character_id` → `characters.id`) with `ON DELETE CASCADE`, so removing a
  character cleans up the user data that referenced it.

### Consequences

#### Positive

- User data and ESI data stay cleanly separated; sync can never clobber user-authored records.
- The render layer keeps a single read model (everything comes from the database) while gaining a
  well-defined place to persist user actions.
- Cascading foreign keys keep app-owned data consistent when the sync-owned records it references go
  away.

#### Negative

- App-owned ids break the otherwise-uniform "`id` is the ESI id" convention; a reader must know
  which tables are app-owned.
- Two write paths touch the database (sync and features); they must stay on separate tables so they
  never contend for the same rows.

---

## Part 2 — Foreign Keys Within Sync Scope

### Context

The org schema (migration `0005_create_orgs.sql`) modelled a deeply connected reference graph with
foreign keys throughout. Real ESI data cannot satisfy most of them, and a freshly signed-in
character fails to persist with `SQLITE_CONSTRAINT_FOREIGNKEY` (error 787) — observed, not just
predicted, once the engine logged the failure reason:

- `races.alliance_id` is an NPC **faction** id (Caldari = 500001), not a player alliance.
- `corporations.ceo_id` / `creator_id` and `alliances.creator_id` reference **arbitrary characters**.
- `bloodlines.corporation_id` is a **founding NPC corporation**; `bloodlines.ship_type_id` an item
  type; `corporations.{faction_id, home_station_id}`, `alliances.{creator_corporation_id,
  executor_corporation_id, faction_id}`, and `factions.{corporation_id, militia_corporation_id,
  solar_system_id}` all point at NPC/universe data the sync never fetches.

Two facts make "just enforce the foreign keys" untenable:

1. **The transaction cannot help.** `character::upsert_with_org` already inserts the whole stack in
   one transaction with deferred foreign-key checks. Deferral changes *when* the check runs
   (commit, not per-statement); it does not create missing rows. The referenced rows are never
   inserted, so the commit still fails.
2. **Some references can never be foreign keys.** A `characters` row has `NOT NULL` foreign keys to
   its own corp/race/bloodline. Enforcing `corporations.ceo_id -> characters` would mean inserting
   the CEO character, which needs *its* corp, whose CEO needs *their* stack — a non-terminating
   recursion through arbitrary pilots. The same applies to every character-referencing org key.

So the question is not whether to drop foreign keys but **where to draw the line.**

### Decision

Enforce a foreign key only when the character-profile sync inserts both ends in the same
transaction. Concretely:

**Enforced (kept):**

- `characters` → `corporations`, `races`, `bloodlines`, `alliances`, `factions`
- `corporations` → `alliances` (a pilot's alliance is their corporation's alliance, always inserted)
- `bloodlines` → `races`

**Stored without a foreign key (plain id columns):**

- `races.alliance_id` (an NPC faction)
- `bloodlines.corporation_id`, `bloodlines.ship_type_id`
- `corporations.ceo_id`, `corporations.creator_id`, `corporations.faction_id`,
  `corporations.home_station_id`
- `alliances.creator_corporation_id`, `alliances.creator_id`,
  `alliances.executor_corporation_id`, `alliances.faction_id`
- `factions.corporation_id`, `factions.militia_corporation_id`, `factions.solar_system_id`

The principle: **enforce what we sync; denormalize what we don't.** The integrity that matters for a
character manager — a stored pilot always resolves to real corp/race/bloodline/alliance/faction rows
— is preserved; the dropped keys were links into data outside Pod's world.

`races.alliance_id` is additionally **mismodelled**: it is a faction, not an alliance. Renaming it to
`faction_id` is a follow-up cleanup.

### Consequences

#### Positive

- Real characters persist; the org stack commits.
- The meaningful invariant holds: a stored character always has real org rows.
- No recursive or universe-wide syncing is required to store one pilot.

#### Negative

- Denormalized ids (CEO, founders, ship type, home station, …) may reference rows absent locally;
  callers must handle that.
- Less database-enforced integrity for reference data; correctness there rests on the sync logic.

### Future Work

A dedicated universe/reference sync (factions, item types, NPC corporations, stations, solar
systems) would let several of these become real foreign keys again: `races` → `factions` (after the
rename), `bloodlines` → `item_types`, `corporations` → `factions`/`stations`, `factions` →
`corporations`/`solar_systems`. The character-referencing keys (`ceo_id`, `creator_id`) can never be
enforced and stay denormalized. The reference-data half of this Future Work is realized by
[ADR-0006](0006-static-and-reference-data.md) (SDE seeding).

---

## Part 3 — Derived Character State

### Context

The roster card needs a pilot's current ISK balance and total SP. The obvious schema adds
`wallet_balance` and `total_sp` columns to a `character_state` table that sync upserts each cycle.
Two problems make a cached scalar the wrong model:

1. **A cached balance drifts against the ledger.** The wallet feature stores a journal/transaction
   ledger regardless (the design calls for browsing it later). A separate `wallet_balance` column is
   a second copy of a value the journal already determines — and the two diverge the moment one
   updates and the other does not. That is how you end up with several different displays of the same
   balance.

2. **`SUM(amount)` from zero is structurally wrong.** ESI's wallet journal is a rolling ~30-day
   window; the full history is never available, so summing deltas from zero can never reconstruct the
   true balance. Every journal entry instead carries ESI's *post-entry running balance*. The current
   balance is therefore the **newest entry's balance**, not a sum.

`total_sp` has the same shape: ESI's `total_sp` equals `SUM(skillpoints_in_skill)` over the full
skill sheet, which Pod stores anyway (`character_skills`). Caching the scalar duplicates a value the
sheet already defines.

### Decision

Model live state as a read-only VIEW over fundamental tables; store no derived scalars.

- **`character_telemetry`** — a real table, one mutable snapshot row per character
  (`online`/`solar_system_id`/`station_id`/`structure_id`/`ship_*`), upserted by the fast telemetry
  job. Pure live telemetry, no aggregates.
- **`character_skills`** — the full skill sheet (whole-sheet replace each cycle). The single source
  of `total_sp`.
- **`character_wallet_journal`** — the fundamental wallet ledger, append-only by ESI entry id
  (`ON CONFLICT(id) DO NOTHING`), accumulating history beyond ESI's 30-day window. The single source
  of `wallet_balance`.
- **`character_state`** — a `CREATE VIEW` based on `characters` (so a row always exists) that
  LEFT JOINs `character_telemetry` and derives:
  - `total_sp` = `SUM(character_skills.skillpoints_in_skill)` for the character — `NULL` until skills
    sync.
  - `wallet_balance` = the **latest running balance carried forward**: take the newest journal entry
    whose `balance IS NOT NULL` (ESI omits it on some ref-types), then add the `amount`s of any later
    entries — `NULL` until the journal syncs.

Every non-key column is nullable; `NULL` cleanly means "that resource has not synced yet" and the
card renders `—`. The model derives `FromRow` with `Option<T>` fields. A SQLite view is read-only, so
its repo exposes only `get`/`all` (no upsert, no `INSTEAD OF` triggers).

ISK is stored and derived as `REAL`/`f64`, matching the ESI DTO and CCP's own per-entry running
balance, so there is no compounding-rounding concern; the carry-forward tail sums at most a handful
of `amount`s.

### Consequences

#### Positive

- One definition of balance and total SP, in one place; no cached scalar to drift.
- Correct balance despite ESI's rolling window: the running balance is authoritative, not a sum.
- The journal accumulates durable history beyond ESI's 30 days, while the displayed balance stays
  cheap (one indexed lookup plus a short carry-forward tail).
- The view reads exactly like a table through `sqlx::FromRow`, so consumers are unaware it is derived.

#### Negative

- A read-time aggregate is recomputed per query rather than read from a column; acceptable at roster
  scale (a handful of characters, indexed by `character_id`).
- The zero-activity edge case shows `—` until the journal has at least one entry; there is no
  authoritative `/wallet/` scalar fallback (deliberately out of scope).
- View columns being all-nullable pushes `Option` handling onto every consumer.

### Future Work

- Surface `unallocated_sp` when a skills view needs it (deferred; no clean single-job owner today).
- Wallet journal/transaction browsing UI over the now-stored ledger.
- If exact-integer ISK arithmetic is ever required, revisit `REAL` vs scaled integers.

---

## Affected Areas

- `migrations/` — app-owned tables get their own migrations (e.g. `0006_create_squads.sql`).
- `migrations/0005_create_orgs.sql` — the foreign key declarations.
- `migrations/0007_create_character_telemetry.sql`, `0009_create_character_skills.sql`,
  `0010_create_character_wallet_journal.sql` — the fundamental tables.
- `migrations/0012_create_character_state.sql` — the VIEW; must be numbered after the tables it reads.
- `src/store/model/` + `src/store/repo/` — models and repos for app-owned data (e.g. `squad`), the
  read-only `character_state` model/repo (`state`/`all_states`), and the org repos.
- `src/store/repo/character.rs` (`upsert_with_org`) — relies on the enforced keys being satisfiable.
- Features that own app-owned data write it directly via their repos.
- The telemetry and wallet sync jobs — write the base tables only; never the view.
- `src/sync/` — must continue to exclude app-owned tables from discovery and jobs.
- Roster card rendering — reads `character_state` for ISK/SP, treating `NULL` as unsynced (`—`).

## References

- [ADR-0002](0002-sync-render-separation.md) — Sync/Render Separation. The two write/read roles this
  model extends with a third (feature) write path.
- [ADR-0006](0006-static-and-reference-data.md) — Static and Reference Data. Realizes the
  reference-data baseline anticipated by the Foreign Keys Future Work.
- Spec: Full-Design Character Cards (`ttpzyopk`)
