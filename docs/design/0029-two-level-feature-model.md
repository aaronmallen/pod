---
id: 0029
title: Two-Level Feature Model with Tolerant Config Migration
status: active
tags: [config, features]
created: 2026-06-21
---

# ADR-0029: Two-Level Feature Model with Tolerant Config Migration

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod's feature toggles become a two-level model: the existing top-level `Feature` groups now own a set of
individually-toggleable `SubFeature`s. Group enablement rolls up as "any child enabled," and a group toggle cascades to
all of its children. The persisted `FeatureFlags` now stores per-sub-feature enablement and serializes in a nested TOML
shape, behind a tolerant deserializer that also accepts the historical flat shape so existing configs upgrade with
byte-for-byte identical effective behavior.

## Context

The feature model was flat: a 12-variant `Feature` enum, a 12-bool `FeatureFlags` struct, and a flat registry
`Descriptor` mapping each group to its scopes/jobs/rail/tab. This could not express sub-features — a user could not, for
example, disable just Budget inside Wallet or Abyssals inside Assets, and Pod requested ESI scopes for capabilities the
user never touched. The granular-toggles epic (`loxpuxsr`) needs a model where toggles, scope derivation, shell gating,
and the settings UI all operate at sub-feature granularity, while every existing TOML config keeps working untouched.

## Decision

- Introduce a `SubFeature` enum (the granular level). `SubFeature::group()` rolls a child up to its `Feature`;
  `Feature::sub_features()` lists a group's children. The groups partition the sub-feature set exactly once.
- `FeatureFlags` stores a per-`SubFeature` enablement array. Group-level helpers (`is_enabled(Feature)`,
  `set_enabled(Feature, bool)`, `enabled() -> Vec<Feature>`) are preserved for existing consumers: `is_enabled` is now
  "any child on" and `set_enabled` cascades to every child. New sub-grain helpers — `is_sub_enabled`, `set_sub_enabled`,
  `enabled_sub_features`, `enabled_sub_features_of` — expose the granular level.
- Persistence stays in TOML with no DB migration. The on-disk shape is nested
  (`[features.wallet] budget = false`). A custom `Deserialize` reads each group key as either a legacy flat bool
  (cascaded onto every child) or a new nested table (read per child); any absent sub-feature defaults to enabled, so a
  brand-new sub-feature is on for legacy configs that never mentioned it. `Serialize` always emits the nested form, so
  the next save re-serializes in the new shape.
- The registry gains a `SubDescriptor` and `sub_descriptor(SubFeature)` at the sub-feature grain (scopes/jobs/rail/tab).
  The group `descriptor(Feature)` is kept as the source of truth for the static arrays, and roll-up invariant tests
  prove the union over a group's children equals the group descriptor exactly (no scope or job added or dropped).

## Affected Areas

- `src/config.rs` — `Feature`, new `SubFeature`, `FeatureFlags` (custom `Serialize`/`Deserialize`).
- `src/features/registry.rs` — new `SubDescriptor` and `sub_descriptor`, roll-up invariant tests.
- Settings consumers that read flag state at the group grain (`features_tab`, `settings`, `ui_tab`).

## Consequences

### Positive

- B/C/D/E build on a clean group/sub-feature API with proven roll-up fidelity.
- Existing flat, nested, mixed, and client-id-only configs all load with identical effective behavior.
- No DB migration; toggles remain TOML-only.

### Negative

- `FeatureFlags` no longer derives `Serialize`/`Deserialize`; the hand-written impls must stay in sync with the enums.
- This task is data-model-only: scope derivation, shell gating, the settings UI, and feature couplings are unchanged
  here and ship in sibling tasks.

## Future Work

- B: derive ESI scopes as the union over enabled sub-features for both character and corp paths; rework job ownership.
- C: gate per-sub-tab shells and character-card sections.
- D: settings Features tab nested groups, master cascade toggles, expanded catalog.
- E: couplings (Budget needs Journal-or-Transactions, Abyssals gates MutaMarket, Skill dual-surface toggle).

## References

- Epic `loxpuxsr` (Granular sub-feature toggles + scope derivation)
- ADR-0028 (owner-aware budget assignment identity)
