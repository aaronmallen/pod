---
id: "0030"
title: Cross-Platform Keyboard Architecture
status: active
tags: [keyboard, ui]
created: 2026-06-21
---

# ADR-0030: Cross-Platform Keyboard Architecture

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod gains a single app-level keyboard architecture: one central, cross-platform shortcut subscription that maps chords
to messages, plus text-input focus tracking that answers "is the user typing right now?" at dispatch time. These two
pieces replace the macOS-only quit subscription and become the substrate the command palette (`/` trigger) and
focus-search (Ctrl/Cmd+K) build on. This ADR **extends ADR-0001** (UI architecture and module structure) by adding the
keyboard layer to the app shell.

## Context

Keyboard support before this work was thin and mac-biased:

- The quit shortcut subscription was `#[cfg(target_os = "macos")]`, so Ctrl+Q did nothing on Linux/Windows.
- There was no app-wide shortcut dispatch and no "open Settings" chord.
- The app never observed which text input was focused — it relied entirely on iced's internal focus. Without that
  signal, `/`-opens-palette is unsafe (a `/` typed into a search box must not open the palette), and Ctrl/Cmd+K cannot
  know whether to steal focus.

## Decision

1. **Central shortcut dispatch.** One app-level keyboard subscription maps chords to messages, replacing the mac-only
   quit subscription. It lives in `src/app/shortcuts.rs` as a `Chord` enum (`FocusSearch`, `OpenSettings`, `Quit`)
   keyed off iced's `modifiers.command()` (already Cmd on macOS / Ctrl elsewhere), so every chord works on every
   platform from one definition. The app installs a single `shortcuts::subscription` and routes each `Chord` through
   one `handle_shortcut` dispatcher.
2. **Text-input focus tracking.** A `FocusTracker` records whether any text input currently holds focus, and
   `probe_focus` queries iced's focused-widget state at dispatch time. Together they answer "is the user typing?", the
   signal that gates typing-vs-chord decisions app-wide.
3. **Route → primary-search registry.** `src/features/focus_search.rs` maps a route (rail `Destination`) to the stable
   `Id` of that view's primary search input. Ctrl/Cmd+K focuses the current route's search via this registry, and is a
   no-op on routes that register no search.
4. **The chord-vs-typing rule.** Modifier chords (Ctrl/Cmd+\*) always dispatch, even while a text input is focused.
   Bare-key triggers (`/`) dispatch only when no text input is focused; otherwise the keystroke falls through to the
   focused input. The `FocusTracker` signal is what enforces this distinction.

## v1 shortcut table

| Chord       | Action                          | Notes                                                                  |
|-------------|---------------------------------|------------------------------------------------------------------------|
| Ctrl/Cmd+Q  | Quit                            | All platforms (drops the former macOS cfg-gate).                       |
| Ctrl/Cmd+,  | Open Settings                   | Cross-platform "preferences" idiom; routes to the Settings rail dest.  |
| Ctrl/Cmd+K  | Focus the current view's search | No-op on routes without a registered search (`focus_search` registry). |
| /           | Open the command palette        | Only when no text input is focused. **Planned** — see below.           |

Shortcuts are a fixed set in v1 — not user-rebindable.

The `Chord` enum that shipped covers the three modifier chords (`FocusSearch`, `OpenSettings`, `Quit`). The bare-key
`/` command-palette trigger is **planned/in-progress**: the focus-tracking substrate it depends on is in place, but the
palette surface and its `/` gating land in a later task of the keyboard epic.

## Affected Areas

- `src/app/shortcuts.rs` — `Chord` enum, `Chord::for_event`/`for_key`, `FocusTracker`, `probe_focus`, and the
  app-level `subscription`.
- `src/features/focus_search.rs` — route → primary-search `Id` registry for Ctrl/Cmd+K.
- `src/app.rs` — installs the subscription, threads a `FocusTracker`, and dispatches each `Chord` in `handle_shortcut`.

## Consequences

### Positive

- A single subscription owns global chords; the platform-specific quit gate is removed/generalized, so Ctrl/Cmd+Q works
  everywhere.
- The focus signal is a shared dependency: the palette and focus-search specs consume it rather than re-deriving focus
  state.
- Future shortcuts extend one table/dispatch rather than scattering cfg-gated subscriptions.

### Negative

- Chords are a fixed set in v1; user rebinding is out of scope and would need a config-backed table later.
- The command palette is not yet wired; only its substrate (central dispatch + focus tracking) shipped here.

## Alternatives considered

- **Per-feature key handling.** Rejected — duplicates the chord-vs-typing rule and re-introduces platform drift.
- **Relying on iced internal focus only.** Rejected — the app cannot observe it at dispatch time, which is exactly what
  the `/` gate needs; `FocusTracker` + `probe_focus` make the signal available where chords are decided.

## References

- ADR-0001 (UI architecture and module structure) — extended by this ADR.
- Epic `tlzlumms` (Rail cascade + command palette + keyboard navigation).
