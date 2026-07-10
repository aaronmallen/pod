---
id: "0047"
title: Scope-Free Always-On Character-Detail Tabs
status: active
tags: [character-detail, dossier, roster, ui, feature-model]
created: 2026-07-10
---

# ADR-0047: Scope-Free Always-On Character-Detail Tabs

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The character-detail view historically assumed every tab was backed by an ESI feature and its scopes: the tab
strip filtered on enabled features, the body ran a forbidden-scope wall, and `Tab::feature()` panicked for any
tab that did not map to a feature. That assumption blocked tabs built on local, manually maintained data that
has no owning scope. This decision makes a tab's owning feature optional. A tab with no owning feature is
always enabled, contributes no required scopes, and never hits the forbidden wall. The Dossier tab is the first
such tab, and future manual tabs follow the same pattern.

## Context

Character detail renders a strip of tabs above a body. Each tab used to be tied to an ESI feature through the
shell registry: `registry::feature_for_tab` returns the owning `Feature`, `enabled_tabs()` kept only the tabs
whose feature was enabled, and `tab_body()` compared the character's granted scopes against the tab's read
scopes and drew a forbidden wall when a scope was missing. `Tab::feature()` reached for the owning feature with
`.expect("every gated tab maps to a feature")`, so the whole surface treated "tab" and "scope-gated feature" as
the same thing. Two invariant tests in `src/features/shell/registry.rs` enforced that every tab maps to a
feature.

The Dossier is a per-character record assembled from local data the pilot maintains by hand. It has no ESI
endpoint and no scope to grant, so it cannot be expressed as a feature. Under the old invariant, adding it would
either panic in `feature()` or force a fake feature and fake scopes purely to satisfy the gating machinery. The
tab system needed to admit a tab that is simply always present.

## Decision

A tab's owning feature is optional. The gating surface reads the owning feature through one helper and treats
absence as "always on, no scopes":

- `Tab::owning_feature(self) -> Option<Feature>` replaces the panicking `feature()` and returns
  `registry::feature_for_tab(self)` directly. `feature_for_tab` already returned `Option<Feature>`, so no
  registry signature changes.
- `enabled_tabs()` keeps a tab when its owning feature is absent, or present and enabled:
  `tab.owning_feature().is_none_or(|feature| features.contains(&feature))`. Feature-gated tabs are filtered
  exactly as before; a scope-free tab is always included.
- `required_scopes()` returns the owning feature's scopes, or an empty slice when there is no owning feature.
  `read_scopes()` filters that same slice, so a scope-free tab reports no read scopes and the forbidden wall in
  `tab_body()` never blocks it.
- `noun()` resolves the owning feature's noun, or falls back to an empty string for a scope-free tab. It is only
  read to render the forbidden wall, which a scope-free tab never reaches.

`Tab::Dossier` is the first scope-free tab. It leads the `ORDER` array and the strip, matching the prototype in
`tmp/design/character-detail.jsx` where the Dossier tab is first and is the default selection. It carries the
`roster.tabs.dossier` label across all nine locales and renders a placeholder body for now; the real body is a
later task. It is intentionally absent from `registry::feature_for_tab`, so `feature_for_tab(Tab::Dossier)`
returns `None`, which is what makes it always-on.

## Affected Areas

- `src/features/roster/character_detail/tabs.rs`: the `Tab` enum and `ORDER`, the `owning_feature` helper that
  replaces `feature()`, the optional-aware `required_scopes`/`read_scopes`/`noun`/`enabled_tabs`, the Dossier
  body branch and scroll routing, and the colocated tests.
- `src/features/shell/registry.rs`: the two invariant tests now assert that gated tabs map to a feature while
  `Tab::Dossier` maps to `None`. `feature_for_tab` itself is unchanged.
- `assets/locales/*.toml`: the `roster.tabs.dossier` key in all nine locales.

## Consequences

### Positive

- A tab no longer has to be a scope-gated feature. Local, manually maintained tabs are expressible without
  inventing a fake feature or fake scopes.
- The "scope-free" idea lives in one place: `owning_feature()` returning `None` drives always-on visibility,
  empty scopes, and skipping the forbidden wall. No method special-cases Dossier by name.
- Feature-gated tabs are untouched at runtime. They still filter on their feature and still draw the forbidden
  wall when a read scope is missing.

### Negative

- The registry invariant weakened from "every tab maps to a feature" to "every gated tab maps to a feature."
  The tests now encode which tabs are intentionally scope-free, so a tab that should be gated but is left
  unmapped will read as always-on rather than failing loudly.
- A scope-free tab shows for every character regardless of what the character has authorized, so its body must
  handle the case where no synced data exists on its own, without leaning on the forbidden wall.

## Future Work

- The Dossier body is a placeholder here. The real body and its `character_detail` state land in a later task.

## References

- ADR-0029: Two-Level Feature Model: the feature and sub-feature model the tab gating reads from.
- `src/features/shell/registry.rs`: `feature_for_tab` and the tab-to-feature descriptors.
- `tmp/design/character-detail.jsx`: the prototype placing Dossier first and default.
