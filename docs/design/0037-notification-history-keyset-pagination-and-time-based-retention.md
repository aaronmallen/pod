---
id: "0037"
title: Notification History — Keyset Pagination and Time-Based Retention
status: active
tags: [notifications, storage, ui, pagination]
created: 2026-06-25
---

# ADR-0037: Notification History — Keyset Pagination and Time-Based Retention

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The in-app notification center (table `notifications`, epic "zyrmyrlk") is gaining a "History" view that shows all prior
notifications via infinite scroll. We back that history with keyset (cursor) pagination keyed on `(created_at, id)`
rather than `LIMIT`/`OFFSET`, and we replace the existing count-based prune (keep the newest 200 surfaced rows) with a
time-based retention window (~90 days). The "New" view continues to read unread rows directly. We explicitly reject
offset pagination and in-memory windowing of a fixed 200-row cache.

## Context

Today `notifications::list(db, 200)` loads the most recent 200 surfaced rows in one shot, and `emit()`
opportunistically prunes surfaced rows beyond `SURFACED_RETENTION = 200`. There is no deeper history and no pagination.
The redesigned center splits into "New" (unread only) and "History" (everything, scrolled). Two problems must be
solved:

1. Reading deep history a page at a time. Notifications insert at the top (newest-first, `ORDER BY created_at DESC,
   id DESC`) and new ones arrive while the panel is open (a sync pulse can `emit()` mid-scroll). With `LIMIT`/`OFFSET`,
   any insert above the current window shifts every page boundary, so the next page re-shows a row already seen or skips
   one. Keyset pagination — "give me the N rows strictly older than this `(created_at, id)` cursor" — is immune to
   that shift: the cursor names a stable position in the ordering, not an ordinal count.
2. Bounding table growth without discarding recent history. A count cap of 200 is fine for a shallow feed but wrong for
   deep history: a burst of events can evict still-recent notifications purely by volume. A time window keeps everything
   within the recency horizon regardless of burst size, and notification rows are tiny, so a 90-day window is cheap.

## Decision

- Keyset pagination. Add a repo query that returns up to `limit` surfaced rows (`suppressed = 0`) strictly before a
  supplied `(created_at, id)` cursor, ordered `created_at DESC, id DESC`; a `None` cursor returns the newest page. The
  UI accumulates pages as the user scrolls (reusing the `VirtualList` / `responsive_window` infinite-scroll pattern the
  mail feature already uses), tracks the last cursor plus a `has_more` flag plus a loading guard, and resets to the
  first page when a refresh brings newer rows. Default page size 50.
- Dedicated unread read for "New". Add a repo query returning unread surfaced rows (`read_at IS NULL AND
  suppressed = 0`) so the New view is correct independently of how deep History has paged (the loaded History
  accumulator is no longer a reliable source of "all unread").
- Time-based retention. Replace the count-based prune with one that deletes surfaced rows whose `created_at` is older
  than a tunable `NOTIFICATION_RETENTION_DAYS` (~90). Suppressed watermark rows remain exempt — they are the
  notify-once dedup ledger and pruning one would let its event re-fire.
- Index. Add a composite index suited to the keyset scan over surfaced rows (covering the `suppressed` filter plus the
  `(created_at, id)` ordering) in a new migration.

## Affected Areas

- `src/store/repo/notifications.rs` — new keyset page query, new unread-list query, time-based prune replacing
  `SURFACED_RETENTION`.
- `migrations/NNNN_*.sql` — composite keyset index on `notifications`.
- `src/notifications.rs` — engine: public owner-name resolution helper for paged rows; prune call site.
- `src/app.rs` — History accumulator state, cursor, `has_more`, loading guard, load-more message plus scroll wiring,
  `VirtualList` History body.

## Consequences

### Positive

- Stable infinite scroll: no duplicated or skipped rows when notifications arrive mid-scroll.
- Genuinely deep history, bounded by recency rather than an arbitrary row count.
- Reuses the established `VirtualList` / `ListScrolled` pattern, so the UX matches the mail list.

### Negative

- The notifications table grows within the retention window (acceptable: rows are tiny).
- Keyset queries are slightly more code than offset and need the composite index to stay fast.
- Paged History rows need owner-name ("who") resolution outside the single refresh Snapshot, adding a name-resolution
  call per loaded page.

## Open Questions

- Page size — defaulting to 50; revisit if scroll feels chunky.
- Retention window — defaulting to ~90 days via a tunable const.
- Whether "Clear all" stays alongside the tabs (current plan: keep it).

## Future Work

- User-configurable retention window.
- Cross-device / cross-owner notification sync (explicitly out of scope; the table is local-only).

## References

- Spec: gest `xoqzlukp` — Notification center: New/History tabs, deep keyset history, mark-all-read, toast icon
  fidelity.
- Existing infinite-scroll precedent: `src/features/mail/message_list.rs`, `src/ui/components/virtual_list.rs`.
- Existing store: `src/store/repo/notifications.rs`, `migrations/0103_create_notifications.sql`.
