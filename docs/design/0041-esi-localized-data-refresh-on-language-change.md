---
id: "0041"
title: ESI Localized-Data Refresh on Language Change
status: active
tags: [i18n, sync, sde, reference-data, esi]
created: 2026-06-27
---

# ADR-0041: ESI Localized-Data Refresh on Language Change

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod is gaining a user-selectable UI language (rust-i18n) and, with it, the obligation to request ESI data in
that language and to seed the EVE Static Data Export (SDE) in that language. ESI localized endpoints accept a
`?language=<code>` query parameter and the SDE ships per-language name and description columns. Once the pilot
switches language, every localized string already on disk - item type names and descriptions, market group names,
station and structure names, region and constellation and solar-system names, race and faction and bloodline names -
is stale, because it was fetched or seeded under the old language. This ADR defines how Pod refreshes that data. The
contract has six parts: a fixed set of language-dependent `JobKind`s whose synced text is language-bound; a single
chokepoint that appends `?language=<code>` to ESI requests; a persisted "last-synced-language"
marker compared to the configured language on boot; a forced re-sync of the language-dependent jobs (by expiring their
`sync_ledger` rows) when the marker and the configuration disagree; folding the language code into the SDE
`composite_version` so a language change re-seeds the SDE in place with an English fallback; and a restart-gated apply
flow, so a language switch is committed once, at a clean boundary, rather than half-applied mid-session.

## Context

Today Pod is monolingual. It never sends `?language=` on any ESI request (confirmed by grep across
`src/clients/esi/`), and the SDE seed extracts only the English field of every localized SDE record: the
`LocalizedString` deserializer in `src/features/splash/seed.rs` reads `{ en }` and discards the rest. Reference text
reaches the database by two paths that this ADR must keep in sync with the chosen language:

1. **The SDE bulk seed** (`src/features/splash/seed.rs`). On first run and on each SDE build bump it seeds the
   reference tables (`item_types`, `item_groups`, `item_categories`, `market_groups`, regions, constellations,
   solar systems, stations, races, bloodlines, factions, NPC corporations, agents, certificates) from the SDE YAML.
   This is the overwhelming majority of localized text. The seed runs once per `composite_version`, gated by the
   `sde_version` marker file at `state_home()/pod/sde_version` (see `composite_version`, `sde_is_current`).
2. **Lazy ESI backfill of reference rows.** When a sync job encounters a type, station, structure, region, or
   faction the SDE never seeded (a player-built structure, a freshly published type, an NPC the seed skipped), it
   resolves the missing row from ESI on demand and writes it into the same shared reference tables. These resolvers
   live in `src/sync/jobs/resolve.rs` (`resolve_item_type`, `resolve_market_group`) and
   `src/sync/structure_resolution.rs` (`resolve_asset_references`, `resolve_structure`, `resolve_station`,
   `resolve_region`, `resolve_constellation`, `resolve_solar_system`, plus race/faction/bloodline list lookups).
   They hit ESI universe endpoints (`/universe/types/{id}/`, `/universe/groups/{id}/`, `/universe/categories/{id}/`,
   `/markets/groups/{id}/`, `/universe/stations/{id}/`, `/universe/structures/{id}/`, `/universe/regions/{id}/`,
   `/universe/constellations/{id}/`, `/universe/systems/{id}/`, `/universe/races/`, `/universe/factions/`,
   `/universe/bloodlines/`) that all return localized text and accept `?language=`.

The `/universe/names/` resolver (`src/sync/jobs/names.rs`) is deliberately out of scope: it resolves character,
corporation, and alliance ids to their canonical, language-invariant names, which do not vary by language. Wallet
amounts, blueprint quantities, skill points, prices, contract terms, and mail and notification bodies are
language-neutral or user-authored, never ESI-localized.

The sync engine already has the machinery a refresh needs. The `sync_ledger` table
(`migrations/0001_create_sync_ledger.sql`, repo `src/store/repo/sync_ledger.rs`) keys per
`(subject_type, subject_id, kind)`, where `kind` is the `JobKind` Debug string, and records `last_success_at` and an
optional `next_eligible_at`. A job is eligible to re-run when `next_eligible_at` has passed, or, failing that, when
`last_success_at + interval` has passed (`ledger_eligible_at` in `src/sync/engine.rs`). Deleting a ledger row makes
its job present as never-attempted, so it re-fetches on the next pass. ADR-0040 set the precedent: migration `0110`
runs `DELETE FROM sync_ledger WHERE kind IN (...)` once on upgrade to force a re-fetch, following the one-time-repair
pattern of migrations 0102 and 0104.

The configured language will live on `AccessibilityConfig` (`src/config.rs`), persisted in the `[accessibility]`
table of `config.toml` alongside `scale` and `high_contrast`, loaded at boot via `config::load()`. Scale and
contrast apply live today, but a language change is categorically different: it invalidates data already rendered
across every open window and triggers a multi-minute background re-fetch, so it cannot be applied frame-by-frame the
way scale is.

## Decision

### 1. The language-dependent JobKind set

A `JobKind` is **language-dependent** if it fetches ESI text that varies by language and persists it. Concretely, a
job is language-dependent if it resolves and writes any localized reference row - directly or through a `resolve_*`
helper. The fixed set, which sibling task `kluwowlr` encodes (for example as a `JobKind::is_language_dependent(self)`
predicate, mirroring `is_global`), is:

| JobKind                 | How it carries localized text                                               |
| ----------------------- | --------------------------------------------------------------------------- |
| `AssetSync`             | `resolve_asset_references` resolves item type, station, and structure names |
| `CorporationStructures` | persists structure names and resolves their item types                      |
| `CharacterClones`       | resolves implant item-type names and clone structure names                  |
| `CharacterTelemetry`    | resolves current solar-system, station, structure, and ship item-type names |
| `CharacterSkills`       | `resolve_item_type` backfills skill type names                              |
| `CharacterContracts`    | `resolve_asset_references` resolves contract item types and locations       |
| `CorporationContracts`  | `resolve_asset_references` resolves contract item types and locations       |
| `CharacterKillmails`    | `resolve_asset_references` resolves ship, fitting, and location names       |
| `CorporationKillmails`  | `resolve_asset_references` resolves ship, fitting, and location names       |
| `CharacterProfile`      | resolves race, bloodline, faction, and org names via the resolve helpers    |
| `CorporationProfile`    | resolves race, bloodline, faction, and org names via the resolve helpers    |
| `CharacterStandings`    | resolves faction names from `/universe/factions/`                           |
| `CorporationStandings`  | resolves faction names from `/universe/factions/`                           |
| `CharacterContacts`     | resolves faction names from `/universe/factions/`                           |
| `CorporationContacts`   | resolves faction names from `/universe/factions/`                           |

Every other `JobKind` is language-neutral and is never forced by a language switch. The set is small and explicit on
purpose: the bulk of localized text comes from the SDE re-seed (section 5), not from these jobs. The jobs only repair
the lazy-backfilled gaps, so the forced re-sync is bounded and cheap. The set must stay co-located with the resolver
code: when a new job starts persisting localized reference rows, it joins this set, and the dependency belongs in a
test that fails if a `resolve_*` caller is added without being marked language-dependent.

### 2. Append `?language=<code>` to ESI requests

Sibling task `twxqrlun` threads the configured language into ESI requests. The contract:

- Localized ESI GET requests carry `?language=<code>` where `<code>` is the configured language code in ESI's
  accepted set (`en`, `de`, `fr`, `ja`, `ru`, `ko`, `es`, `zh`). `en` is the default and may be sent explicitly or
  omitted; either is equivalent at ESI.
- The parameter is appended where the ESI client owns the request, not smeared across every callsite. The preferred
  shape is a language held on the ESI `Client` (set once at construction from config) that the universe and market
  and dogma route methods append when they build their URL, so individual jobs and resolvers do not each remember to
  add it. The parameter is appended **only on the ESI host**, never leaked onto the SDE mirror, zKillboard, or the
  image CDN, exactly as ADR-0015 conditionally injects `X-Compatibility-Date` for ESI only.
- Appending must compose with pagination, which already appends `?page=` or `&page=` depending on whether the URL
  has a query string (`src/clients/http.rs`). Building the language parameter through the same query-aware joining
  (or via `reqwest::Url`) keeps a single well-formed query string.
- The `/universe/names/` resolver is exempt; its output is language-invariant, so adding `?language=` would be
  meaningless and is omitted.

### 3. Detect a language switch on boot

Sibling task `rousqpkl` persists a "last-synced-language" marker: the language code under which the localized data
currently on disk was fetched and seeded. It is stored as a plain-text marker file at
`state_home()/pod/synced_language`, mirroring the existing `sde_version` marker, because it describes the *state of
the data on disk*, not a user preference (the preference is the language in `config.toml`). Keeping it next to
`sde_version` keeps both data-state markers in one place and out of the user-facing config file.

On boot, after config load and before the sync engine starts dispatching, Pod compares the configured language
(`AccessibilityConfig.language`, defaulting to `en`) against the marker:

- Marker absent (first run, or upgrade from a pre-i18n build): treat the existing English data as already matching
  the default `en`. Write the marker as the configured language. A pilot who has never picked a language sees no
  re-fetch.
- Marker present and equal to the configured language: no switch, nothing to do.
- Marker present and different from the configured language: a switch is detected; run the refresh in sections 4 and
  5, then rewrite the marker to the configured language only after the SDE re-seed succeeds, so an interrupted
  refresh re-triggers on the next boot rather than silently leaving mixed-language data marked as consistent.

### 4. Force a full re-sync of the language-dependent jobs

Sibling task `nlwrsvyt` performs the forced re-sync on a detected switch. The mechanism reuses the ADR-0040 ledger
precedent rather than inventing a new path:

- Expire every `sync_ledger` row whose `kind` is in the section-1 set, across all subjects. Deleting the rows (or
  setting `next_eligible_at` to the past) makes those jobs present as never-attempted, so the engine re-dispatches
  them on the first pass after boot and they re-fetch under the now-current `?language=`.
- A repo helper, for example `sync_ledger::expire_kinds(db, kinds)` issuing
  `DELETE FROM sync_ledger WHERE kind IN (...)`, is the natural home; the kind list comes from the section-1
  predicate, never a hard-coded duplicate. Re-fetched rows overwrite their reference-table rows in place via the
  existing upserts (`resolve_item_type`, `resolve_market_group`, `sde::upsert_*`), which key on the stable numeric
  id, so the localized columns are rewritten in the new language while the id stays put and all foreign keys hold.
- This is a boot-time action, not a migration, because the trigger is a config change the user can make repeatedly,
  not a one-time schema upgrade. It is idempotent: expiring an already-expired ledger row is a no-op, and the
  upserts are safe to re-run. It runs after the SDE re-seed (section 5) is queued, so the bulk of the data lands
  first and the jobs only repair what the seed could not cover.

### 5. Re-seed the SDE in the new language

Siblings `kmmvtzxy` and `vqktzlwu` make the SDE seed language-aware. Two coupled changes:

- **Read the requested language, fall back to `en`.** `LocalizedString` currently exposes only `en`. It gains the
  ability to pick the configured language's field and fall back to `en` when that language is missing for a given
  record (the SDE does not translate every string in every language). Every `seed_*` function that today calls
  `.en()` instead selects the configured language with the English fallback, so a partially translated SDE still
  produces a complete, never-empty name column.
- **Fold the language into the seed identity.** `composite_version` (`src/features/splash/seed.rs`) currently
  formats `"{sde_build}+pod-{pkg}+seed-{revision}"`. It gains the configured language code, for example
  `"{sde_build}+pod-{pkg}+seed-{revision}+lang-{code}"`. Because the `sde_version` marker stores this composite and
  `sde_is_current` re-seeds whenever the marker differs, adding the language to the composite makes a language change
  invalidate the marker and re-run the full seed automatically, with no separate trigger. The re-seed replaces the
  reference rows in place: every `seed_*` path already upserts by primary key (`upsert_many_*`, `ON CONFLICT ... DO
  UPDATE`), so the localized columns are overwritten in the new language without dropping rows or breaking foreign
  keys. The English data already on disk needs no migration; it is simply overwritten on the next seed.

Sequencing on a detected switch: the SDE re-seed (driven by the composite-version mismatch) does the bulk
overwrite, then the section-4 ledger expiry forces the language-dependent jobs to repair the lazily-backfilled rows
the seed never covered. Both write to the same reference tables through the same upserts, so order only affects which
writer touches a given row first, never correctness.

### 6. Live versus restart, and the user-facing flow

Sibling `wzkmsqyk` decides the apply semantics; `skskrswo` provides the restart mechanism. A language change
**requires a restart to fully apply**, and the change is committed at the next boot rather than mid-session:

- The settings UI persists the new language to `config.toml` immediately, but does **not** apply it live the way
  scale and contrast do. Applying live would require swapping every rust-i18n string, invalidating every cached
  localized row, and driving a multi-minute background re-fetch while windows render half-translated, mixed-language
  data. That is the failure this ADR exists to prevent.
- Instead, choosing a new language shows a clear prompt that the language takes effect after a restart, and offers
  to restart now via the `skskrswo` restart mechanism. On the next boot the marker comparison in section 3 fires,
  the SDE re-seed and ledger expiry run during the normal splash, and the app comes up fully in the new language
  with localized data refreshing in the background under the existing freshness model.
- This makes the refresh observable through the existing freshness UI: the expired language-dependent jobs read as
  `CatchingUp` until they re-land, so the pilot sees honest progress rather than a frozen or mixed UI. Restart is
  the natural seam because the SDE seed already runs at splash, before the sync engine dispatches, so re-seeding on
  the boot after a switch reuses the existing startup path with no new mid-session orchestration.

## Affected Areas

- `src/config.rs`: `AccessibilityConfig` gains a `language` field (sibling task), default `en`, persisted in the
  `[accessibility]` table; this ADR consumes it.
- `src/clients/esi.rs` and `src/clients/esi/{universe,market,dogma,races,faction,bloodlines}.rs`: append
  `?language=<code>` to localized ESI requests, ESI-host only, query-string-aware.
- `src/features/splash/seed.rs`: `LocalizedString` selects the configured language with `en` fallback; every
  `seed_*` path uses it; `composite_version` folds in the language code so the `sde_version` marker re-seeds on a
  language change.
- `src/sync/job.rs`: a `JobKind::is_language_dependent` predicate (sibling `kluwowlr`) over the section-1 set, with
  a test pinning it to the resolver callers.
- `src/store/repo/sync_ledger.rs`: an `expire_kinds` helper that deletes ledger rows for a kind set (sibling
  `nlwrsvyt`).
- Boot path (`src/app.rs` splash/store-ready flow): read the configured language and the `synced_language` marker,
  detect a switch, queue the re-seed and the ledger expiry, rewrite the marker on success (sibling `rousqpkl`).
- `src/features/settings*`: language picker that persists to config and prompts for a restart rather than applying
  live (siblings `wzkmsqyk`, `skskrswo`).

## Consequences

### Positive

- Reuses two proven mechanisms - the `composite_version` seed gate and the `sync_ledger` expiry-forces-refetch
  pattern from ADR-0040 - so a language change rides existing, tested paths rather than a bespoke invalidation
  engine.
- The forced re-sync is bounded to a small, explicit set of jobs; the SDE re-seed carries the bulk, so a switch is
  one full seed plus a handful of cheap repair jobs, not a re-fetch of everything.
- In-place upserts keyed on stable numeric ids mean no rows are dropped and no foreign keys break; only the
  localized text columns change, so budgets, assets, and every numeric consumer are untouched.
- The English fallback guarantees a complete name column even when the SDE leaves a string untranslated in the
  chosen language, so no record renders blank.
- Committing the switch at restart keeps the UI consistent: the pilot never sees a half-translated, mixed-language
  screen, and the refresh surfaces through the existing freshness indicators.

### Negative

- A language change costs a restart and a full SDE re-seed (download already cached for the current build, but a
  full re-parse and re-upsert), so switching is a deliberate, multi-second-to-minute operation, not instant.
- During the post-restart background refresh, lazily-backfilled rows briefly remain in the old language until their
  jobs re-land; the bulk SDE text is correct immediately, but a rarely-touched structure or type name can lag a
  cycle.
- Adding a new job that persists localized reference rows requires remembering to add it to the language-dependent
  set; the pinned test mitigates this but the coupling is real.
- The language code becomes part of the SDE seed identity, so the `composite_version` string and any tooling that
  parses it must account for the new `+lang-` segment.

## Open Questions

- Should `en` be sent explicitly on ESI requests or omitted (both are equivalent at ESI)? Omitting keeps URLs
  shorter and cache keys stable for the default-language majority; sending is uniform.
- Should the post-restart refresh show a one-time toast ("Refreshing data in `<language>`...") in addition to the
  per-job freshness state, or is the freshness UI sufficient?
- Is a restart strictly required, or could a future iteration apply the rust-i18n string swap live while gating only
  the data re-seed behind a restart? This ADR chooses the simpler all-at-restart contract for v1.

## Future Work

- Per-language reference columns (storing `en` plus the active language side by side) would let a switch flip
  instantly without a re-fetch, at a storage and seed-time cost; deferred until the restart-gated model proves
  insufficient.
- Translating Pod's own UI chrome via rust-i18n is the sibling i18n work this ADR depends on but does not define;
  this ADR governs only the ESI/SDE *data* refresh, not the static-string layer.

## References

- ADR-0006 (Static and Reference Data): the SDE seed and reference-table model this refresh re-seeds.
- ADR-0014 (Persisted Sync Ledger and Honest Job Outcomes): the `sync_ledger` freshness and eligibility model the
  forced re-sync expires.
- ADR-0015 (ESI Request-Layer Policy): the ESI-host-only header/parameter injection pattern the `?language=`
  contract follows.
- ADR-0017 (Interface Scale and Accessibility Config): the `AccessibilityConfig` and `config.toml` home the
  `language` field joins, and the live-apply model a language change deliberately does not follow.
- ADR-0036 (Freshness-First Sync Status): the freshness states through which the post-switch refresh surfaces to the
  pilot.
- ADR-0040 (Per-Wallet Journal Identity): the `DELETE FROM sync_ledger` one-time-refetch precedent (migration 0110)
  the forced re-sync reuses.
- `src/features/splash/seed.rs` (`LocalizedString`, `composite_version`, `sde_is_current`), `src/sync/jobs/resolve.rs`,
  `src/sync/structure_resolution.rs`, `src/store/repo/sync_ledger.rs`, `src/sync/job.rs` (`JobKind`).
