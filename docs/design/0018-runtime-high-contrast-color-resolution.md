---
id: "0018"
title: Runtime High-Contrast Color Resolution
status: active
tags: [architecture, accessibility, ui]
created: 2026-06-11
---

# ADR-0018: Runtime High-Contrast Color Resolution

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod's three secondary text tiers (`SECONDARY`, `TERTIARY`, `DIM`) and its two secondary-border alphas become
**runtime-resolvable** so a single high-contrast flag can swap them live across every window. The tiers move
from `static const iced::Color` to free functions — `color::text::secondary()`, `tertiary()`, `dim()` — and
the inline border alphas (`with_alpha(text::PRIMARY, 0.10 / 0.18)`) move to `color::rule()` /
`color::rule_strong()`. Each function branches on a single process-global `AtomicBool` (`HIGH_CONTRAST`):
when off it returns the existing translucent overlay (byte-identical to today); when on it returns a tuned
opaque solid (`SECONDARY → #CFCDC7`, `TERTIARY → #AEACA6`, `DIM → #92908B`) or a firmer border alpha
(`rule 0.10 → 0.22`, `ruleStrong 0.18 → 0.34`). The flag is initialized from
`settings.accessibility().high_contrast` at boot and updated whenever the Accessibility setting changes.
`PRIMARY` text and the dark surfaces are unchanged in both states.

## Context

The three tiers were the same off-white RGB at reduced alpha, hardwired at ~474 call sites across ~110 files
(`SECONDARY` ~310, `TERTIARY` ~164, `DIM` 8), with secondary borders written inline as
`color::with_alpha(color::text::PRIMARY, ..)`. There was no runtime palette, so high contrast could not be
applied without making these values resolve against a runtime flag.

Key constraints from the existing architecture:

- The app is an `iced::daemon` with one shared `App` read by every `view(app, id)` across all windows. There
  is no per-widget theme object threaded through view signatures.
- The colors are consumed deep inside ~30+ view functions and component helpers; threading a palette
  parameter to every one of them would touch every site **and** every signature.
- Removing or renaming the consts breaks every call site, so the resolution mechanism and the call-site sweep
  must land together to keep the repo green.
- Unlike `scale_factor` (ADR-0017), which iced re-reads every frame, colors are read inside `view` closures
  that only re-run when a window redraws. A flag flip alone does not repaint open windows.

## Decision

1. **Runtime functions backed by an `AtomicBool`.** Replace the tiers' use with free functions that branch on
   a module-level `static HIGH_CONTRAST: AtomicBool`. The existing overlay RGBA stay as private `*_OFF`
   constants (the off-state source, so the off path is provably identical to today); new `*_HC` constants hold
   the tuned solids. `color::rule()` / `color::rule_strong()` resolve the two border alphas the same way.
   `color::set_high_contrast(bool)` writes the flag; `color::high_contrast()` reads it.

   - *Alternative — thread a `&Palette` through every view:* rejected. Far more invasive (every call site
     **and** every signature), with no offsetting benefit for a single binary global toggle.
   - *Alternative — a central `high_contrast_color(base) -> Color` mapping layer:* rejected. Still touches
     every call site and adds an indirection without removing the global-flag dependency.

2. **Call-site sweep in lockstep.** Mechanically migrate `color::text::SECONDARY/TERTIARY/DIM` →
   `secondary()/tertiary()/dim()` and the `0.10 / 0.18` PRIMARY-border `with_alpha` sites → `rule()` /
   `rule_strong()` across `src`, one file at a time so each keeps compiling. After the sweep, no styling site
   references the bare tier consts — they exist only as the private off-state sources inside `color.rs`.

3. **Wire the flag to config.** Set the flag in `boot()` from `settings.accessibility().high_contrast` (after
   config load, before the first frame), and update it in the `App` update path on the
   `settings::Outcome::AccessibilityChanged` signal — the same path that hoists the rest of the accessibility
   config (ADR-0017). No DB schema change.

4. **Cross-window live update via a per-window redraw nudge.** Because the resolved colors are read inside
   each window's `view` closure (which only re-runs on redraw), flipping the flag is not enough on its own.
   On `AccessibilityChanged` the app issues a benign per-window action — `window::size(id).discard()` for
   every open window id — which schedules a fresh draw of each window. Their next frame re-reads the resolved
   colors, so high contrast applies live across all open windows with no restart and no new message type. A
   no-op refresh task per window is an accepted mechanism for this; the size query is discarded.

## Consequences

- With the flag off, rendered output is byte-identical to today: the off-state constants are the original
  RGBA values, and `PRIMARY` and the dark surfaces are untouched.
- With the flag on, secondary/tertiary/dim text become the tuned opaque solids and secondary borders firm up
  app-wide, live across every open window.
- The tier values are now resolved per call rather than referenced as a const. The branch is a single relaxed
  atomic load; this is negligible relative to layout/draw cost and runs only during `view`.
- The flag is a single process-global; high contrast is a binary app-wide mode, not a per-window or
  per-widget override. This matches the feature's intent and keeps the call sites parameter-free.
- Third-party widget chrome (slider, scrollbars) that does not route through these tiers is unaffected by the
  toggle; any future high-contrast variant for those is a separate, additive change.
- The Contrast panel UI (the toggle + preview) is delivered separately; it only needs to set
  `high_contrast` in config and emit `AccessibilityChanged` — the engine and live-update plumbing here do the
  rest. Its preview should call the resolved color functions so it reflects the live state truthfully.
