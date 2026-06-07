---
id: "0001"
title: UI Architecture and Module Structure
status: active
tags: [ui, architecture, style]
created: 2026-06-06
---

# ADR-0001: UI Architecture and Module Structure

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The application is a single crate built on Iced. UI code is organized by feature, where each feature owns its full
Model-View-Update triple. Views are pure layout functions that arrange self-rendering components; they are not
themselves components. Shared components and design tokens live in a presentation-only `ui/` layer that contains no
application behavior. Windows are an OS-surface concern managed exclusively by an `app/` routing layer, and features
stay window-agnostic. All styling values must come from the token system.

## Context

The application is a single crate. We need a home for views, shared components, and style tokens, and a clear separation
between application behavior and presentation.

Several forces shape this decision:

- In Iced, a feature is naturally its MVU triple (`State` + `Message` + `update` + `view`). A separate "controller"
  layer is just glue between a view's MVU and app-level services; folding that glue into the feature is exactly
  feature-based organization. `update()` is the controller.
- "View" and "component" are different things. Conflating them — making each view a component with its own
  `render()` — blurs responsibilities.
- "Window" and "view" are different axes. The splash screen renders in its own OS window. In Iced multi-window, only the
  root `Application` can render per-window (`view(&self, window::Id)`) or open/close windows, so window management is
  inherently app-level no matter how code is organized.
- Hardcoded colors, padding, and size constants scattered across the UI make visual consistency hard to maintain. A
  token system is a single source of truth only if its use is mandatory.

## Decision

### Module structure

```text
src/
├── main.rs                  thin: build settings + run the app
├── app.rs + app/            root Iced Application
│   └── windows.rs           window::Id → feature view + window settings;
│                            open/close; translate feature intent → window ops
├── features.rs + features/
│   └── splash.rs + splash/  splash.rs: State, Message, update, view
│       └── status.rs        feature-private component
└── ui.rs + ui/
    ├── components.rs + components/   shared, stateless, message-generic
    │   ├── logo.rs
    │   └── footer.rs
    └── style.rs + style/    color, spacing, typography, radius, shadow tokens
```

Per project convention, modules use named files (`ui.rs` + `ui/`), never `mod.rs`.

### Features own MVU

Each feature directory owns its `State`, `Message`, `update`, and `view`. There is no separate controller layer; side
effects live in the feature's `update`.

### Views vs components

- A **component** is a self-rendering building block. It is stateless and message-generic — parameterized by the
  caller's `Message` via callbacks, never defining app messages of its own.
- A **view** is a pure layout/composition function, `fn view(state) -> Element<Message>`. It instantiates components and
  arranges them on the page. A view is not a component and has no `render()` of its own.

### Component placement

A component lives where it is used. Used by one feature → it lives in that feature's directory (e.g.
`features/splash/status.rs`, a future `features/character/character_card.rs`). Genuinely shared across features, or app
chrome → `ui/components/` (e.g. `logo`, `footer`). Default to feature-private; promote to `ui/` only on real, present
sharing — no premature promotion, and no artificial waiting once something is actually shared.

### Windows vs views

- A **window** is an OS surface with settings (size, decorations, position) and lifecycle (open/close/drag/resize),
  identified by `window::Id`.
- A **view** is content arranged into a surface and is window-agnostic; it never holds a `window::Id`.
- The window↔view mapping and lifecycle is **routing**, owned by `app/`, because that is the only place Iced permits
  it.
- Features emit **semantic intent** (e.g. `DragWindow`, `LoadingComplete`); `app/` performs the corresponding window
  operation (`window::drag(id)`, open/close). The feature never performs window operations directly.

### Style token mandate

All color and size values in the UI must come from the token system in `ui/style/`. The following are prohibited outside
the token definition files:

- Raw color constructors (`Color::from_rgb(...)`, `Color::from_rgba(...)`) outside `ui/style/color.rs`
- Numeric padding/margin literals outside `ui/style/spacing.rs`
- Raw float border radii outside `ui/style/radius.rs`
- Hardcoded shadow values outside `ui/style/shadow.rs`

When a needed value does not exist in the token system, add it there first.

Exception: values that are explicitly user-configurable at runtime are exempt, since they come from user preferences and
cannot be defined statically.

## Affected Areas

- New `app/`, `features/`, and `ui/` module trees
- `main.rs` becomes a thin entry point that builds and runs the app
- Consolidates the style token mandate (previously a separate decision)

## Dependencies

| Dependency | Version | Purpose                                      |
|------------|---------|----------------------------------------------|
| iced       | 0.14    | UI framework; multi-window support required  |

## Consequences

### Positive

- Feature code is cohesive — state, messages, update, and layout live together.
- The `ui/` layer is pure presentation; behavior cannot leak into it, giving a clean seam between features and shared
  rendering.
- Windows are handled where Iced forces them (`app/`), and features stay window-agnostic and portable across windows.
- A single token source enforces visual consistency; design changes happen in one place.

### Negative

- The token mandate is enforced by convention and code review, not the compiler.
- `app/` becomes a central routing hub that must know every window and feature — a coupling point that grows with the
  app.
- The feature-emits-intent / app-performs-operation split adds one hop of indirection for window operations.

## Open Questions

- Should per-window settings (size, decorations) live next to the feature (e.g. a `window_settings()` the feature
  exposes) or in the `app/windows` registry? Decide at implementation time for the splash window.
