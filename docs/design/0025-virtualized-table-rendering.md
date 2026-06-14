---
id: "0025"
title: Virtualized Table Rendering
status: active
tags: [architecture, performance, ui]
created: 2026-06-13
---

# ADR-0025: Virtualized Table Rendering

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod's infinite-scroll surfaces — Assets inventory, abyssals, wallet ledgers, character-detail tabs (Contacts, Kill Log,
Standings), and Mail — get progressively laggier the more rows are loaded because nothing is virtualized. Every
accumulated row, with its icon handle, multi-cell `Row`, and word-wrapping `TableCell`, is built into the iced element
tree and re-laid-out and redrawn on every frame. We introduce **a single shared `VirtualList` windowing helper** that
materializes only the rows in (and just around) the visible viewport, with leading and trailing spacer widgets
preserving scrollbar geometry, so per-frame layout/draw cost is bounded regardless of how many rows are loaded. Because
the just-shipped wrap/no-truncation fidelity makes row heights variable, the helper uses an **estimated-height-plus-overscan**
model (viewport size from `iced::widget::responsive`, a per-surface nominal row height for offset→index math, and a
generous overscan buffer) rather than fixed row heights. Alongside windowing, the in-memory `take(n)` surfaces
(abyssals, wallet, contacts) move to **real cursor-based DB pagination** to match the existing Assets-inventory pattern,
and Mail — which today fetches the entire mailbox unbounded — gains a bounded `LIMIT`/cursor fetch. The hoisted
sticky-header structure stays untouched; the helper windows only the scrollable body.

## Context

iced 0.14 ships no windowed-list widget, and the codebase uses neither `iced::widget::lazy` nor
`iced::widget::responsive`. Every infinite-scroll surface is a hand-built `scrollable(Column::with_children(rows))`, and
there is no shared scrollbar, header, or pagination infrastructure — four-plus independent implementations:

- **Assets inventory** (`src/features/assets/inventory.rs`) — cursor-based async fetch, appends pages of
  `INVENTORY_PAGE_SIZE = 200` to `state.inventory` at a `0.85` relative-scroll threshold; expandable containers insert
  child rows inline into the same flat `Column`.
- **Abyssals** (`src/features/assets/abyssals.rs`) — a card grid, full set held in memory, revealed by growing
  `abyssal_visible_count` via `take(n)` on scroll.
- **Wallet ledgers** (`src/features/wallet/shell.rs`) — journal/market/contracts, full set in memory, soft-limited by
  growing `visible_rows`.
- **Character-detail tabs** (`src/features/character_detail/tabs.rs`) — per-tab scroll routing; Contacts/Kill
  Log/Standings hold their sets in memory.
- **Mail** (`src/features/mail/message_list.rs`, `src/store/repo/mail.rs`) — no pagination at all: an unbounded
  `fetch_all`, with every message rendered into a `Column` under Today/Yesterday/Earlier day-bucket headers.

Two facts constrain any windowing design:

1. **Row heights are variable and must stay so.** The wrap/no-truncation fidelity work means a row's height is
   content-driven (a one- or two-line name cell, multi-line wallet entries). There is no fixed `ROW_HEIGHT` to revert to
   except the 30px headers. So an exact offset→row-index mapping is not available from data alone.
2. **Only relative scroll offset is tracked.** Every surface reads `viewport.relative_offset().y` (0.0–1.0). Pixel
   offsets, content height, and viewport height are not currently observed; `responsive` is required to obtain viewport
   geometry.

The sync-contention fixes — the sync-disruption axis of the lag — ship first and independently; this ADR addresses the
orthogonal steady-state scroll-lag axis and depends on nothing in that work.

## Decision

### A single shared `VirtualList` windowing helper

A new `src/ui/components/` widget, not per-feature windowing. It accepts a total item count, a per-surface estimated row
height, and a per-index row-renderer closure, wraps the body in `iced::widget::responsive` to obtain viewport height,
tracks the scroll offset in app/feature state, and renders a leading `Space` spacer (the summed height of off-screen
rows above the window), the materialized window `[first_visible − overscan .. first_visible + window + overscan]`, and a
trailing `Space` spacer for the rows below. The helper lives **outside** any surface's hoisted header so the existing
sticky-header structure is preserved; surfaces keep their own `TableCell`-based row renderers. Surfaces with non-flat
structure flatten into the helper's flat index space themselves: Assets expands open containers into the index stream,
Mail interleaves day-bucket section headers as indexable items, and the abyssals card grid windows by row-of-cards.
Designing the helper against Assets inventory (the richest consumer: expandable rows + hoisted sortable header +
existing cursor pagination) first fixes the API before the other surfaces adopt it.

### Estimated-height-plus-overscan windowing

Not fixed row height, and not lazy-memoization-only. Fixed heights conflict with wrap fidelity (multi-line cells would
clip); lazy memoization alone cuts redraw but not the off-screen layout cost that is the actual lag. The helper computes
the window from a per-surface nominal row height and renders a generous overscan margin so the small one-vs-two-line
height variance never opens a visible gap. Minor scrollbar-thumb drift from height estimation is accepted; it is
invisible at the overscan margins used.

### Real cursor-based DB pagination for every surface

The in-memory `take(n)` surfaces (abyssals, wallet ledgers, character-detail Contacts/Kill Log/Standings) move to
cursor-based DB pagination mirroring Assets inventory, rather than holding the full set in memory once rendering is
windowed. Mail gains a bounded `LIMIT`/cursor fetch in `src/store/repo/mail.rs` in place of `fetch_all`. This keeps
memory bounded as well as render cost, and gives every surface one consistent pagination model.

### Out of scope / unchanged

The hoisted sticky-header structure; table styling; and the wrap/no-truncation fidelity behavior (the helper works with
variable row height — it does not revert it).

## Consequences

- **Bounded per-frame cost.** Scrolling Assets/abyssals/wallet/Mail with thousands of rows loaded materializes only
  viewport-plus-overscan rows, so layout and draw cost no longer grows with the loaded set.
- **One windowing surface to reason about.** A single helper concentrates the spacer/overscan/offset-math complexity;
  surfaces contribute only a row renderer and a flattening of their structure into the flat index space. The helper must
  be designed up front to support the union of consumer needs (expandable rows, section headers, multi-column card rows)
  so later adopters do not have to reopen it.
- **Estimation trade-off.** Scrollbar geometry is approximate because heights are estimated; this is a deliberate,
  bounded inaccuracy masked by overscan, accepted in exchange for keeping wrap fidelity.
- **Pagination breadth.** Moving abyssals/wallet/contacts to cursor pagination is more work than windowing alone and
  touches their repos, but removes full-set-in-memory holds and unifies the pagination model. Mail's new `LIMIT` closes
  a genuine unbounded-fetch hazard (a large mailbox).
- **New iced dependencies in practice.** `responsive` (and optionally `lazy`) enter the codebase for the first time;
  their interaction with scroll-offset tracking and the hoisted header is established by the foundation work and reused
  thereafter.

## References

- [ADR-0001: UI Architecture and Module Structure](0001-ui-architecture-and-module-structure.md) — the UI module
  structure the shared `VirtualList` helper slots into under `src/ui/components/`.
- The wrap/no-truncation table-fidelity work that constrains the row-height model and makes row heights variable.
- The sync-contention fixes for UI scroll lag — the prerequisite that ships first and independently, addressing the
  orthogonal sync-disruption axis of the lag.
- iced 0.14: `iced::widget::responsive` (viewport geometry) and `iced::widget::lazy` (memoized rendering).
- Surfaces: `src/features/assets/shell.rs`, `src/features/assets/inventory.rs`, `src/features/assets/abyssals.rs`,
  `src/features/wallet/shell.rs`, `src/features/character_detail/tabs.rs`, `src/features/mail/message_list.rs`,
  `src/store/repo/mail.rs`, `src/ui/components/table_cell.rs`.
