---
id: "0049"
title: Planetary Interaction Colonies Feature
status: active
tags: [architecture, features, sync, data-model, industry, pi]
created: 2026-07-13
---

# ADR-0049: Planetary Interaction Colonies Feature

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod gains a first-class Colonies view of each pilot's Planetary Interaction (PI) footprint, added as a new tab in the
existing Industry feature (Jobs | Blueprints | Extractions | Colonies | Planner) rather than a standalone screen. It
is character-scoped only (ESI exposes no corporation PI endpoint), read + sync only (no authoring), and renders
entirely from the local database so PI can be monitored offline. Colony layouts are persisted in a normalized
five-table set (character_planets, character_planet_pins, character_planet_pin_contents, character_planet_routes,
character_planet_links) refreshed by a wholesale delete-stale + upsert reconciliation, fed by a new CharacterPlanets
sync job that performs an N+1 fetch (one colony-list call plus one layout call per colony). Factory recipes and the P0
to P4 production chain resolve offline from a newly ingested SDE table (planet_schematics, from
planetSchematics.yaml); PI commodity and planet-type names and per-item volumes already live in item_types and need no
new reference data. The feature is wired through the central feature registry as a new SubFeature::Colonies under
Feature::Industry, gated on the existing esi-planets.manage_planets.v1 scope, and reuses the shared missing-scope
re-auth gate.

## Context

CCP exposes a capsuleer's PI colonies through two authed ESI endpoints: GET /characters/{id}/planets/ returns a
lightweight list (planet_id, planet_type, upgrade_level, num_pins, last_update), and GET
/characters/{id}/planets/{planet_id}/ returns the full colony layout (pins with contents / extractor_details /
factory_details, routes, and links). There is no corporation PI endpoint and no PI write surface Pod would use, so the
feature is character-scoped and read-only. Pod already surfaces jobs, blueprints, and moon extractions under the
Industry feature but had no PI view, forcing PI players to leave Pod to answer basic questions (which extractors are
expiring, which colonies have gone idle, what each colony is worth per day).

Three pieces of existing architecture shape the design:

- The Industry feature and its tab shell already exist. Industry (ADR-0023) is a registry-native, scoped feature with
  a segmented tab control, per-tab loaders, and a shared re-auth gate. Colonies is another tab in that strip, not a
  new top-level screen, so it reuses the shell, the scope picker, the loader orchestration, and the missing-scope gate
  rather than inventing a parallel structure.
- The central feature registry is the single wiring source. Per-feature scope sets, rail gating, and the sync JobKinds
  a feature drives are defined once in features::registry (ADR-0019, ADR-0029). Colonies is a new SubFeature::Colonies
  sub-descriptor plus a CHARACTER_PLANETS entry in the Feature::Industry roll-up, kept in sync so the registry roll-up
  invariants hold.
- The canonical/static data split is established. Per-character ESI-backed rows live in normalized SQLite tables with
  ON DELETE CASCADE to characters(id) and wholesale-replace freshness (ADR-0003, ADR-0031); static SDE reference data
  is ingested at seed time into id-keyed tables (ADR-0006). PI colony state is the former; PI schematics are the
  latter.

## Decision

### Colonies is a tab in Industry, not a new feature

Colonies is added as SubFeature::Colonies under Feature::Industry, surfacing as a Tab::Colonies between Extractions
and Planner with a new planet() icon. It is gated on scopes::CHARACTER_PLANETS (esi-planets.manage_planets.v1, already
declared) and greys out for characters that have not granted it, driving the standard reauth banner. Corp scope is
greyed for this tab because no corporation PI endpoint exists.

### Normalized five-table colony storage

Colony layouts are persisted normalized rather than as opaque JSON blobs, for query-ability (fill %, chain resolution,
per-head decay) and consistency with the rest of the canonical data model:

- character_planets: one row per colony (planet_id, planet_type, upgrade_level, num_pins, plus resolved
  system/name/sec metadata).
- character_planet_pins: one row per pin (extractor / factory / storage / launchpad / command center), with
  extractor_details and factory_details fields inlined.
- character_planet_pin_contents: one row per (pin, type_id) content stack. A dedicated table rather than a
  contents_json column on pins, chosen for normalized consistency and so the launchpad-fill computation (Σ
  contents.amount × item volume ÷ structure capacity) is a straight join to item_types.volume with no in-app JSON
  parsing.
- character_planet_routes: one row per route (source/destination pin, commodity, quantity).
- character_planet_links: one row per link (source/destination pin).

Every table carries a per-character natural primary key, character_id ... REFERENCES characters(id) ON DELETE CASCADE,
an owner index, and RFC3339 TEXT timestamps where applicable. There is no updated_at / synced_at; freshness is
whole-owner delete-stale + upsert via replace_for_character_batched (batches of 500), matching the existing blueprints
/ assets / industry repos. Structure capacities used by the fill computation (Launchpad 10,000 m3, Storage 12,000 m3,
Command Center 500 m3) are a small hardcoded map by structure type_id, not stored data.

### N+1 per-colony detail-fetch sync

A new CharacterPlanets sync job fetches the colony list, then issues one layout call per colony (N+1), maps each
layout into the five-table row set, and wholesale-replaces per character. This is accepted because colony counts per
character are small (tens at most) and PI state changes slowly; a per-colony call is the only way ESI exposes layout.
The job is wired through every exhaustive JobKind match; the sync dispatcher's passthrough sub-handler split is
extended (or a new sub-handler extracted) to keep per-function cognitive complexity within the CRAP budget.

### Offline schematic resolution from a new SDE table

planetSchematics.yaml (schematic id to name, cycle time, input/output type_ids + quantities) is ingested by a new seed
step into a planet_schematics table (id-keyed, natural PK, no character_id), so factory recipes and the P0 to P4
production chain resolve entirely offline. PI commodity and planet-type names/icons are ordinary item_types rows
already seeded, and per-item volume (already in item_types.volume) feeds the launchpad-fill calc, so no other
reference data is added.

## Affected Areas

- migrations/: new migrations for the five colony tables and the planet_schematics table.
- src/store/model/, src/store/repo/colonies.rs, src/store/repo/sde.rs: colony models + repo with
  replace_for_character_batched, and a seed_many_planet_schematics upsert.
- src/clients/esi/character.rs, src/clients/esi/models/character.rs: two new authed endpoints + DTOs.
- src/sync/job.rs, src/sync/jobs/character_planets.rs: the CharacterPlanets JobKind + job.
- src/features/shell/registry.rs, src/config.rs: SubFeature::Colonies sub-descriptor + CHARACTER_PLANETS in the
  Industry roll-up.
- src/features/industry/: Tab::Colonies, the colonies card grid + detail drawer, and the colonies loader.
- src/features/splash/seed.rs: the planetSchematics.yaml seed step.
- assets/images/icons/planet.svg, assets/locales/*.toml: icon + [industry.colonies] locale strings.

## Consequences

- Positive: PI is monitored offline alongside the rest of Industry; normalized storage makes fill %, chain, and decay
  queries first-class rather than JSON-parsing; the feature adds no new scope (the PI scope was already declared) and
  reuses the Industry shell and reauth gate wholesale.
- Negative: The N+1 sync issues one request per colony, so sync cost scales with colony count (bounded, but not a
  single call); five new tables plus a schematics table enlarge the schema and the migration set; the
  structure-capacity map is hardcoded and must be updated if CCP changes PI structure volumes.
- Neutral: Corp PI is permanently out of scope until CCP ships an endpoint; writing/editing colonies and PI route
  optimization are explicitly deferred.
