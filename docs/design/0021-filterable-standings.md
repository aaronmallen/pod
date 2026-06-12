---
id: "0021"
title: Filterable Standings Data Model and Effective-Standing Resolution
status: active
tags: [architecture, features, static-data, ui, standings]
created: 2026-06-12
---

# ADR-0021: Filterable Standings Data Model and Effective-Standing Resolution

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The character-detail Standings tab becomes a full-catalog, relationship-aware finder over the entire EVE NPC standings
universe — factions, NPC corporations, and agents — instead of a sparse list of the few entities a character holds an
explicit ESI standing row with. The default view lists every faction and every NPC corporation with logo, raw standing,
and effective standing; agents are catalog data with no ESI endpoint, so they are sourced from the SDE and surface only
once a facet narrows the set. A faceted query language mirroring the character-manager search drives a SQL predicate
builder over the catalog, and effective standing is computed in Rust from an agent → corp → faction cascade plus the
character's social-skill modifiers (Connections / Diplomacy / Criminal Connections). The feature is read-only and
resolves all entity images on-demand through the existing evictable image cache. The geo hierarchy and NPC station names
are eagerly seeded from the SDE so agent locations (System · Region) and security class resolve fully offline.

## Context

The Standings tab today renders only `character_standings` rows (raw ESI values), which are sparse, so the tab is often
near-empty and offers no drill-in to a faction, corp, or agent. Players cannot answer the questions that matter: which
agents they can run missions for, of what mission kind, which research agents farm a given datacore, which agents are
near them, and which they can access right now given their standing.

Three pieces of existing architecture shape the design:

- NPC agents have no ESI endpoint. The SDE (`npcCharacters.yaml` entries carrying an `agent:` key) is the only viable
  source for the ~10,966 agents, their corp/type/division/level/location, and their skill lists. The seed pipeline
  (`src/features/splash/seed.rs`, composite version marker gated on `SEED_FORMAT_REVISION`) already parses SDE YAML into
  reference tables and re-seeds existing installs when the revision bumps (ADR-0006). Standings adds new seed steps the
  same way.
- A faceted search language already exists. `src/store/search.rs::parse()` tokenizes a query into key:value facets with
  comma-OR, repeat-key-AND, leading-`-` negation, quoted phrases, and free-text fallback. The tokenizer is reusable
  as-is; only the recognized key set and the SQL predicate builder are roster-specific.
- On-demand images and lazy geo resolution exist. `src/store/images.rs` resolves portraits/logos through the evictable
  cache (ADR-0013); the map/station tables (migrations 0003/0004) exist but populate lazily today rather than from the
  seed, leaving offline gaps the standings agent rows would expose.

## Decision

### Agents from the SDE; corps folded into the shared corporations table

A new migration (0067) adds `npc_agents` (id, name, corporation_id, agent_type_id, division_id, level, is_locator,
location_id), `npc_agent_skills` (agent_id → skill type_id), `agent_types` (~13, from `agentTypes.yaml`), and
`npc_corporation_divisions` (~30, from `npcCorporationDivisions.yaml`). The cross-catalog foreign keys on `npc_agents`
and `npc_agent_skills` are `DEFERRABLE INITIALLY DEFERRED` so the non-destructive seed can land rows before their parent
corporations/stations/types within one transaction, mirroring the corporations/characters schema. NPC corporations
(~283, from `npcCorporations.yaml`) are upserted non-destructively into the existing `corporations` table — only
SDE-known columns (id, name, faction_id, ticker, home/station id) are written, leaving ESI-owned columns untouched —
rather than a parallel `npc_corporations` table, so corp resolution, logos, and faction joins reuse one code path.
`SEED_FORMAT_REVISION` bumps so existing installs re-seed and pick up the new tables on next launch.

### Eagerly seed the geo hierarchy and derive NPC station names at seed time

The map hierarchy and NPC stations are seeded eagerly from the SDE, not deferred to lazy resolution, so agent rows
resolve System · Region and security class offline:

- `mapRegions.yaml` → `regions`, `mapConstellations.yaml` → `constellations`, `mapSolarSystems.yaml` → `solar_systems`
  (name, security_status, security_class, constellation/region links).
- `npcStations.yaml` → `stations`, with a derived display name. EVE station names are not stored in the SDE but are
  constructible per CCP's static-data celestial-naming rules:

  - `useOperationName = true`: `"<orbitName> - <corporationName> <operationName>"`
  - otherwise: `"<orbitName> - <corporationName>"`
  - `orbitName` is the celestial the station orbits, resolved from `npcStations.orbitID`:

    - planet (in `mapPlanets.yaml`): `"<systemName> <Roman(celestialIndex)>"` (e.g. Tanoo IV)
    - moon (in `mapMoons.yaml`): `"<parentPlanetName> - Moon <orbitIndex>"` (Arabic, e.g. Tanoo IV - Moon 1); the parent
      planet is the moon's own `orbitID`, whose `celestialIndex` gives the Roman numeral.
  - `operationName` from `stationOperations.yaml` (operationID → operationName.en); `corporationName` from the corps
    seeded above (ownerID).
  - `mapPlanets.yaml` and `mapMoons.yaml` are parsed transiently at seed time only to build the name string — no
    planet/moon tables are added.
- The `stations` table carries ESI-only NOT-NULL columns the SDE does not provide (max_dockable_ship_volume,
  office_rental_cost, services). The seed supplies placeholder defaults (`0.0` / `'[]'`) for these and keeps the upsert
  non-destructive so the existing lazy ESI station path overwrites them with canonical values on first visit.
- Scope note: "everything" here means the geo + station data the standings feature needs to resolve offline (regions,
  constellations, solar systems, NPC stations with names). Stars, stargates, and asteroid belts are out of scope (no
  tables exist for them and the feature does not need them).

This also fixes the pre-existing lazy-geo gaps where System · Region was blank until a system was visited.

### Effective standing computed in Rust from a cascade plus social skills

Every entity shows its raw standing (the explicit ESI value, possibly 0) and an effective standing computed at load
time in Rust — there is no new persisted standing column. Effective standing cascades agent → corp → faction (an agent
with no explicit standing inherits its corp's, then its faction's), then applies the character's trained social skills:
Connections raises positive empire-faction standings, Diplomacy raises negative standings toward 0, and Criminal
Connections raises positive pirate-faction standings. The exact per-level coefficients and the empire-vs-pirate faction
classification are confirmed against live EVE values during implementation and encoded as constants. The existing
`character_standings` table is unchanged; the cascade reads it alongside `character_skills`.

### Accessibility from a level → required-standing constant table

Agent access (`accessible:` / `locked` / `reachable`) compares the character's effective standing — taken as the best
of (corp, faction) — against the standing required for the agent's level, from a level → required-standing constant
table encoded in Rust and confirmed against live EVE. This is computed in the same Rust pass as effective standing.

### A standings-specific facet set and predicate builder

The standings query reuses the `search.rs::parse()` tokenizer but defines its own recognized-key set and its own SQL
predicate builder (a sibling of the roster builder, not an overload of it) targeting factions / corporations /
`npc_agents` and their geo joins. Facets: `faction:`/`fac:`, `corp:`/`corporation:`, `agent:`, `name:`, `level:`,
`type:` (agent type), `division:` (mission kind), `accessible:`/`locked`/`reachable`, `system:`, `region:`,
`sec:high|low|null`, `near:me` (current location, active-clone fallback), `field:`/`datacore:` (ResearchAgent research
skills resolved to names via `item_types`), and a `standing:>=N` / `standing:<0` threshold. Default (no facet) returns
all factions + all NPC corps; agents are gated behind a narrowing facet and bounded by `LIMIT`. The query runs
live-debounced exactly like the roster search (generation-tracked, ~200ms debounce).

### Read-only; on-demand images; factionless corps grouped under "Other"

The feature never writes standings (the Contacts tab is a separate feature). Faction, corp, and agent images all
resolve on-demand through the evictable cache — agents as CharacterPortrait, corps as CorporationLogo, factions as the
CorporationLogo of the faction's corporationID — with no precompiled or committed assets. Factionless NPC corps (e.g.
Doomheim, CONCORD) are grouped under an "Other" section rather than excluded.

## Consequences

- The Standings tab is useful immediately even with zero standings, and players can drill into factions, corps, and
  agents with relationship-aware facets to answer real mission/tax/datacore/Epic-Arc questions.
- Eagerly seeding the geo hierarchy + ~10,966 agents (with skills) + ~283 corps + agent types + divisions + NPC stations
  grows seed time and DB size, and the station-name derivation adds a transient parse of mapPlanets/mapMoons/
  stationOperations. Seed time is measured and verified acceptable during implementation; if a single step proves too
  costly it can be chunked. The payoff is fully-offline System · Region / security-class resolution and correct station
  names, which also repairs the existing lazy-geo gaps.
- Folding NPC corps into the shared `corporations` table, and seeding placeholder columns for stations, means one code
  path each — but both require the seed upserts to be strictly non-destructive so they never clobber ESI-synced data.
- Effective standing and accessibility are computed, not stored, so they always reflect the character's current skills
  with no migration or backfill — at the cost of a per-load Rust pass over the catalog (bounded by the same filter +
  LIMIT that bounds the query).
- The exact social-skill coefficients, faction empire/pirate classification, level → required-standing constants, and
  the research-skill → datacore/field mapping are tuning values confirmed against live EVE during implementation; they
  are encoded as constants and can be corrected without schema change.

## References

- ADR-0006: Static and Reference Data — the SDE seed pipeline and re-seed-on-revision mechanism the new seed steps
  extend.
- ADR-0003: Canonical Data Model — the shared corporations/factions/maps/stations tables the seed upserts build on.
- ADR-0013: Committed Item Icons and Synced Portraits/Logos — the on-demand evictable image cache logos resolve through.
- CCP static-data guide (celestial names): <https://developers.eveonline.com/docs/guides/staticdata/#celestial-names>
