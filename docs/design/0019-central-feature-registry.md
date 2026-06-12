---
id: "0019"
title: Central Feature Registry as Single Source of Truth
status: active
tags: [architecture, features, sync, ui]
created: 2026-06-12
---

# ADR-0019: Central Feature Registry as Single Source of Truth

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Every per-`Feature` wiring — its ESI scope set, its nav-rail destination, its character-detail tab, and the
sync `JobKind`s it drives — is defined once in a single registry (`features::registry`). One `Descriptor`
per `Feature` holds all four concerns, and the four consumers derive from it instead of carrying their own
`match` arms: `feature_scopes` (auth), the rail gating (`Feature → Destination`), `Tab::feature` /
`Tab::required_scopes` (character-detail), and `JobKind::feature` (sync). The reverse lookups
(`feature_for_job`, `feature_for_tab`, `feature_for_destination`) are derived by scanning the registry, so a
forward edit keeps every reverse mapping consistent automatically. Adding or changing a feature's wiring is a
single-site edit.

## Context

The `Feature → X` mappings lived in three independent inline `match` arms with no shared origin:

- `features::auth::feature_scopes` — `Feature → &[scope]`.
- `features::character_detail::tabs` — `Tab → Feature` and `Tab → &[scope]`.
- `sync::job::JobKind::feature` — `JobKind → Feature`.

The nav-rail had **no** feature mapping at all; it rendered every icon unconditionally. Because the four
concerns were authored separately, they drifted: a change to one feature's wiring meant remembering to edit
several files, and there was no compiler or test pressure to keep them aligned. The feature-toggle work
(live rail gating, live sync reconcile, missing-scope re-auth) needed all four concerns to agree on one
truth — the rail and the 403 re-auth state both have to read the *same* scope set the auth flow requests.

## Decision

Introduce `features::registry` with a `Descriptor` returned by `descriptor(feature)`:

```rust
pub struct Descriptor {
  pub jobs: &'static [JobKind],
  pub rail: Option<Destination>,
  pub scopes: &'static [&'static str],
  pub tab: Option<Tab>,
}
```

`Option` marks concerns a feature does not participate in: features with no rail icon (e.g.
`CloneMonitoring`) carry `rail: None`; features surfaced as a top-level screen rather than a tab (e.g.
`Wallet`) carry `tab: None`. `jobs` and `scopes` are non-empty for every feature.

The four consumers derive from the registry:

- `feature_scopes(feature)` returns `descriptor(feature).scopes`.
- `Tab::feature` / `Tab::required_scopes` resolve through `feature_for_tab` and the owning feature's scopes.
- `JobKind::feature` returns `feature_for_job(self)`.
- Rail gating reads `descriptor(feature).rail`, with `feature_for_destination` for the route guard.

Reverse lookups scan `Feature::ALL` for the descriptor whose forward field matches. A `JobKind` owned by no
feature (`CharacterProfile`, `CorporationProfile`) maps to `None` and therefore always runs, regardless of
the toggle state — preserving the prior behavior. The public signatures consumed elsewhere (`scopes_for`,
`JobKind::feature`, `Tab::feature`, `enabled_tabs`) are unchanged, so this is a pure refactor with no
behavior change.

The registry lives under `features` because it is the natural owner of the cross-cutting `Feature` wiring and
can name all four target types (`Destination`, `Tab`, `JobKind`, scopes) without inverting the existing
dependency direction.

## Consequences

- Adding or changing a feature's scopes, rail icon, tab, or jobs is a one-site edit in the registry; the
  reverse lookups and all four consumers follow automatically.
- The registry is the shared origin the live-toggle and missing-scope re-auth work reads from, so the rail,
  the sync schedule, and the 403 state cannot disagree about a feature's scopes or jobs.
- Reverse lookups are linear scans of the ten-feature table — negligible, and never on a hot path.
- The invariants that previously lived implicitly across three files (every tab maps to a feature, no job is
  owned by two features) are now enforced by registry unit tests.
