---
id: "0042"
title: Overlay Z-Layering Convention
status: active
tags: [overlays, ui]
created: 2026-06-27
---

# ADR-0042: Overlay Z-Layering Convention

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod gains a single shared overlay-layer scale that is the sole authority for how overlays stack. One named ordering,
low to high, is `Dropdown(10) < RailCascade(20) < Palette(30) < Notifications(40) < Modal(50)`. Every overlay defers to
it: a widget that implements iced's `index()` returns its layer's `z()`, and the `Stack`-composed overlay layers (the
root `Stack` in `app.rs`, and the `modal_overlay` / `stable_overlay` layer vectors) order their layer vectors against
the same named constants. This replaces the previous practice of each overlay hardcoding its own `index()`, which left
the planner-search popover and the rail cascade tied at `1.0` and stacking nondeterministically.

## Context

Pod is an iced 0.14 desktop app, and iced has no shared notion of how overlays stack across subtrees. Each overlay
decided its own stacking order locally: `AnchoredDropdown`, the rail hover flyout (the rail cascade), the command
palette, the notifications panel, and modals. Two of them hardcoded the same value, `index() == 1.0`. iced breaks a tie
between overlays from different subtrees by incidental tree order, so whenever two of these were visible at once their
draw order and hit-testing were nondeterministic.

This surfaced concretely in Industry > Planner. Typing into the product search opens an `AnchoredDropdown` popover. That
popover and the rail's hover flyout both reported `index() == 1.0`, so they rendered and hit-tested on top of each other
unpredictably, and which one a click reached depended on tree order rather than intent. There was no written record of a
layering order, so the next overlay author had no convention to follow and would reintroduce a bare hardcoded `index()`
and recreate the same tie.

The values in the scale are already locked by the implementation of the parent spec. This record documents the
convention so it is discoverable and enforceable.

## Decision

Define one shared overlay-layer scale and make it the only place overlay stacking order is expressed.

1. **A single shared scale.** A shared `OverlayLayer` module holds the named layers and their numeric values. It is the
   single authority for overlay stacking order. No overlay hardcodes a bare `index()` value anymore.

2. **The ordering.** Low to high, the scale is:

   `Dropdown(10) < RailCascade(20) < Palette(30) < Notifications(40) < Modal(50)`

   The gaps between values leave room to insert future layers without renumbering. The rail cascade sits above
   content-level dropdowns and always wins that collision; a blocking modal sits above everything else.

3. **Two ways overlays consume the scale.**
   - A widget that implements iced's `index()` returns its layer's `z()` value rather than a literal. This covers true
     `overlay()`-bearing widgets such as `AnchoredDropdown` and the rail cascade's `SideFlyout`.
   - Layers composed by stacking, rather than by `index()`, order their layer vectors against the same named constants.
     This covers the root `Stack` in `app.rs` and the `modal_overlay` / `stable_overlay` helpers, which push elements
     into a `Vec` in the order the scale prescribes.

   Both paths read from the one scale, so the cross-subtree ordering is consistent whether iced resolves it through
   `index()` or through stack child order.

## Affected Areas

- The shared `OverlayLayer` module that defines the scale.
- `src/ui/components/anchored_dropdown.rs` — the dropdown overlay returns `OverlayLayer::Dropdown`.
- `src/ui/components/rail.rs` — the `SideFlyout` rail cascade returns `OverlayLayer::RailCascade`.
- `src/ui/components/modal_overlay.rs` — the `modal_overlay` / `stable_overlay` layer vectors order against the scale.
- `src/app.rs` — the root `Stack` orders the palette, notifications, and other overlay layers against the scale.

## Consequences

### Positive

- Overlay stacking is deterministic and independent of tree order. The rail cascade renders and hit-tests above content
  dropdowns every time, which resolves the planner-search versus rail-cascade collision.
- There is one place to read and change the ordering. A new overlay picks a layer from the scale instead of inventing a
  number, so the tie at `1.0` cannot reappear.
- Overlays may coexist. No overlay is forced closed to make stacking predictable; the scale composites them in a fixed
  order instead.

### Negative

- Author discipline is required: a new overlay must use the scale, and a reviewer has to catch any return to a hardcoded
  `index()`. The unit tests that assert each overlay's layer value guard against silent drift.
- Adding a layer between two existing ones relies on the numeric gaps; a future ordering that needs more than the gaps
  allow would require renumbering the scale.

## Alternatives considered

- **A central overlay host or portal.** Route every overlay through one host that owns stacking, rather than letting
  each widget report its own layer. Rejected as too risky against iced 0.14's per-widget `overlay()` model: it would
  mean fighting the framework's composition model for the whole overlay surface. The shared scale gets deterministic
  ordering without that rewrite.
- **Forced single-open / mutual exclusion.** Allow at most one overlay open at a time so stacking never has to be
  resolved. Rejected because legitimate combinations coexist (a dropdown open while the rail cascade is hovered, a
  palette over a panel), and forcing them closed would harm the interaction rather than fix the layering.

The resulting behavior is that overlays may coexist but composite deterministically, with the rail cascade winning over
content dropdowns.

## References

- Spec: Overlay layering convention and planner-search / rail-cascade collision fix.
- `docs/process/writing-adrs.md` — ADR format and draft conventions.
