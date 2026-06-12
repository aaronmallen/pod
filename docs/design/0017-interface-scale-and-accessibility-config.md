---
id: "0017"
title: Interface Scale and Accessibility Config
status: active
tags: [architecture, accessibility, ui, config]
created: 2026-06-11
---

# ADR-0017: Interface Scale and Accessibility Config

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod gains a user-controllable **interface scale** and the scaffolding for a dedicated **Accessibility**
settings category. Scale is implemented as a single interface-zoom axis (layout, glyphs, and images move
together) by wiring iced's `daemon.scale_factor(fn(&App, window::Id) -> f64)` callback to return
`accessibility.scale / 100.0`. The value is persisted in a new `[accessibility]` table in `config.toml`
(`scale`, default `100`; `high_contrast`, default `false`) following the existing overrides-only figment
pattern — defaults are never written to disk. The accessibility configuration is held on the shared `App`
struct so every per-window `view(&App, id)` call observes the same value on its next frame and re-scales
live with no restart and no cross-window messaging. This ADR also establishes the `Category::Accessibility`
settings tab and the `settings::Outcome` signal that hoists changed config back onto `App`, both of which a
follow-on high-contrast feature builds upon.

## Context

Pod's UI uses fixed sizes with no in-app control over scale. The only recourse was OS-level display zoom,
which scales the entire desktop bluntly. Issue #32 asks for an interface-scale control plus high contrast.

Key constraints from the existing architecture:

- The app is an `iced::daemon` with a single shared `App` struct read by every `view(app, id)` call across
  all windows (settings, character manager, wallet, skills, etc.).
- iced 0.14 exposes an unwired `daemon.scale_factor(fn(&App, window::Id) -> f64)` callback. Its return
  value multiplies on top of OS DPI and scales layout, glyphs, and images uniformly.
- Configuration persistence already follows an overrides-only figment pattern (`FeatureFlags`,
  `StorageConfig`): `#[serde(default)]` fields with `skip_serializing_if` guards so unchanged values never
  reach `config.toml`.
- The settings window already dispatches per-category tabs and signals the app via `settings::Outcome`
  (e.g. `Persist`, `SyncNow`); the features path hoists changed settings back onto the runtime.

Issue #32 originally proposed a separate independent font-size axis in addition to layout scale. The design
deliberately collapses these into one combined interface-zoom axis: a single slider/preset moves text and
layout together, which is simpler to reason about and matches what `scale_factor` does natively.

## Decision

1. **One interface-zoom axis via `scale_factor`.** Wire the daemon's `scale_factor` callback to return
   `app.accessibility.scale as f64 / 100.0` (1.0 at 100%). Layout, glyphs, and fixed-size images
   (portraits, corp logos) all scale together — this is accepted as correct interface-zoom behavior, with
   no special exemption for images.

2. **`[accessibility]` config table.** Add an `AccessibilityConfig { scale: u8, high_contrast: bool }`
   sub-struct to `config.rs` with `#[serde(default)]` + `skip_serializing_if` guards, mirroring
   `FeatureFlags`. Defaults are `scale = 100`, `high_contrast = false`, and are omitted from disk. No DB
   schema change.

3. **Config on the shared `App`.** Hold the accessibility config on `App` (loaded in `boot()` from the same
   `config::load()` already called there, defaulting on error). Every window's next frame sees the current
   value; no per-window messaging is needed. Changes from the settings window are hoisted onto `App` via a
   `settings::Outcome` signal, parallel to the existing features-changed path.

4. **`Category::Accessibility` scaffold.** Add the enum variant, its `ALL`/dispatch/`reset_active` arms, and
   an `accessibility_tab` submodule mirroring the existing tabs. The sidebar badge shows the current scale:
   `"100%"`, a preset's percentage, or `"112% · custom"` (literal percentage + a `· custom` suffix) when the value
   sits between presets, mirroring the fine-scale readout. (High-contrast's `· HC` badge suffix is consumed
   by the follow-on contrast feature.)

5. **Scale controls.** Five presets (XS 85%, S 92%, M 100% default, L 125%, XL 150%) as a segmented row, plus
   a fine slider over 85–150% in 1% steps with a live readout and preset tick marks. Presets and slider write
   the same `scale` value and stay in sync. iced's built-in `slider` widget is used here for the first time,
   so first-use styling (plasma thumb, dark border) is expected.

6. **Reset scope.** "Reset to defaults" with the Accessibility category active returns `scale` to 100% (and
   `high_contrast` to false), matching the per-category reset behavior of the other tabs.

## Consequences

- Users get a live, restart-free interface zoom that applies uniformly across every open window. At 100% the
  UI is visually identical to today.
- Fixed-size images scale with everything else; this is the intended true-zoom behavior, not a regression.
- Window min-size enforcement on scale-up is **out of scope** (tracked separately): content may scroll or
  clip in very small windows at 150%.
- The `high_contrast` field is persisted but inert in this change; the contrast behavior, colors, and UI are
  delivered by a follow-on accessibility feature that reuses this scaffold (the tab, the config table, and
  the `Outcome` hoist path).
- The `[accessibility]` table joins `[features]` and `[storage]` as an overrides-only config section; a
  fresh install writes nothing new to `config.toml`.
