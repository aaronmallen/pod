---
id: "0020"
title: Calendar Feature
status: active
tags: [architecture, features, sync, ui]
created: 2026-06-12
---

# ADR-0020: Calendar Feature

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod gains a view of each pilot's in-game EVE calendar — fleet ops, CTAs, structure timers, alliance/corp/faction
events, EVE-server events, and personal reminders — as a **standalone top-level rail screen that mirrors Mail**, not a
sub-view of Mail. It is an ESI-backed, character-scoped feature with an "All Pilots" combined view, a local DB cache,
five views (agenda/day/week/month/year), an event-detail modal, RSVP responding, and attendees tallies. The feature is
deliberately **read + respond only** — ESI's calendar surface exposes no create/edit/delete path Pod uses, so the
feature never authors events. It is the **first feature wired entirely through the central feature registry**: Calendar
is added as a single registry `Descriptor` (its scopes, rail destination, and sync jobs) and **reuses the shared
403/missing-scope re-auth gate** rather than building its own. Pod-derived synthetic overlays (skill-queue, market, and
contract milestones) are an opt-in extra, **off by default**.

## Context

CCP exposes a pilot's calendar through ESI as a list endpoint, a per-event detail endpoint, a per-event attendees
endpoint, and a respond (RSVP) write endpoint. Pod tracks a capsuleer's skills, wallet, assets, and mail but had no
consolidated place to see upcoming events, respond to them, or correlate them with the activity it already syncs.

Two pieces of existing architecture shape the design:

- **Mail is the proven template.** Mail (`src/features/mail/`, `src/sync/jobs/character_mail.rs`,
  `src/store/repo/mail.rs`, its migrations, and the `Route::Mail` app wiring) is already an ESI-backed,
  character-scoped, top-level rail screen with an "All Pilots" combined view, a per-feature tweaks panel, an outbox
  write path, and an attention badge. Calendar is the same shape, so it follows Mail wholesale rather than inventing a
  new structure.
- **The central feature registry now exists.** Per-feature wiring — scope set, rail destination, character-detail tab,
  and the sync `JobKind`s a feature drives — is defined once in `features::registry` as a `Descriptor` per `Feature`,
  with the auth scope set, rail gating, sync schedule, and the missing-scope re-auth state all deriving from that one
  source of truth (see ADR-0019). Calendar is the first feature added *after* that registry landed, so it can be wired
  as a single descriptor edit instead of touching several inline `match` arms.

An early design note suggested folding the calendar into a Mail tab; that is superseded. The rail design places a
badged Calendar icon directly below Mail, and the consolidated multi-character use case is identical to Mail's, so a
standalone top-level feature is the right altitude.

## Decision

### Standalone top-level feature mirroring Mail

Calendar is its own feature module (`src/features/calendar/`) with a shell (rail/header/account-switcher/date-nav/
type-legend/view control), the five views, an event-detail modal, loaders, and an RSVP submodule — structurally
paralleling `src/features/mail/`. It owns a `Route::Calendar`, a `rail::Destination::Calendar` mapped directly below
the Mail rail item, an attention badge (upcoming events needing attention), and the app-level `Message`/navigate/view/
dispatch wiring, each mirroring its Mail equivalent. The account switcher provides an "All Pilots" combined,
color-coded view plus per-character scope, and a per-feature tweaks panel (view/density/color-by/local-time/overlays/
week-start/weekends) persists like every other feature's tweaks.

### ESI read + respond only — no authoring

The feature deliberately does not create, edit, or delete events. It reads the list/detail/attendees endpoints and
writes only an RSVP response (accepted/tentative/declined) for events that invite one. Two new ESI scopes back this:
a read scope (`esi-calendar.read_calendar_events.v1`) and a respond scope
(`esi-calendar.respond_calendar_events.v1`). RSVP writes go through the existing durable outbox (ADR-0010) as a new
`KindHandler`, applying optimistically, executing the ESI `PUT`, and compensating (reverting the local response) on
failure — mirroring Mail's set-read handler.

### DB-cached, synced like Mail

A new sync job (`JobKind::CharacterCalendar`) fetches the event list, caches per-event detail (skipping refetch when
unchanged, like mail bodies), fetches attendees, resolves owner names, and upserts into new
`character_calendar` / `character_calendar_attendees` tables (cascade-deleted with the owning character, indexed on
character and timestamp, like mail). Per-event errors are non-fatal so one bad event does not abort the job. The views
render from this cache, in EVE/UTC with optional local time, color-coded by owner-type or pilot.

### Single registry descriptor — the first registry-native feature

Calendar is registered as one `features::registry` descriptor: `Feature::Calendar` → scopes
`[read, respond]`, rail `Destination::Calendar`, jobs `[CharacterCalendar]`, and **no character-detail tab**
(it is top-level only). Every consumer — the auth scope set, rail gating, the sync schedule, the route guard, and the
Settings → Features catalog entry — derives from that descriptor. Toggling Calendar in Settings applies live with no
restart: disabling hides the rail icon, makes `Route::Calendar` unreachable (redirecting to Characters), drops its sync
jobs and its scopes from new auths, and re-enabling restores instantly with retained data (non-destructive). This is
the first feature whose wiring is authored entirely as a registry descriptor rather than as scattered inline mappings.

### Reuse the shared 403 / missing-scope re-auth gate

A character authed before Calendar existed lacks the calendar scopes, so its read 403s. Rather than build a calendar-
specific re-auth flow, the feature **reuses the shared missing-scope re-auth gate** keyed off the registry's scope set:

- The per-character ("mine") view replaces the calendar with the shared gate (lock glyph, forbidden status, the
  required calendar scopes drawn from the registry as chips, and an SSO re-authenticate action).
- The combined "All Pilots" view drops unauthorized pilots' calendars and shows a slim amber banner naming them with a
  re-authenticate action.
- The account picker marks unauthorized pilots with a lock indicator and a "not authorized" subtitle.

Re-authentication requests the full enabled-feature scope set (registry behavior), resolving every gap at once.

### Pod-derived synthetic overlays, off by default

Calendar can overlay synthetic, non-respondable events derived **at load time in Rust** from already-synced data —
skill-queue completions, market-order expiries, contract expiries — with no new ESI calls and no new schema. Each
overlay source is gated on its own feature flag (skill completions only when skill monitoring is enabled; market /
contract milestones only when the wallet feature is enabled) and on the calendar's own "Pod overlays" tweak, which is
**off by default**. Overlay events are visibly distinct, tagged, and never respondable.

### Accessibility by convention

All calendar UI draws color from `src/ui/style/color.rs` (no hardcoded colors), so the runtime high-contrast switch
(ADR-0018) flips calendar colors live, and owner-type/pilot accent colors derive from the shared palette. Interface
scale (ADR-0017) applies globally via the daemon's `scale_factor`; the calendar screen needs no per-view scaling work.

## Consequences

- Capsuleers get a consolidated, multi-character view of upcoming EVE events with RSVP, correlated alongside the rest
  of Pod's data, without leaving the app.
- Because Calendar is read + respond only, Pod never owns the authoring lifecycle of calendar events — there is no
  create/edit/delete surface to keep in sync with the game.
- Calendar validates the central feature registry on a real, substantial feature: a single descriptor edit wires the
  scopes, rail, sync jobs, and live-toggle behavior, and it inherits the shared 403 re-auth gate for free. Future
  features follow the same one-descriptor pattern.
- Pod-derived overlays add value from existing data at zero ESI cost, but being off by default they never surprise a
  user who only wants their in-game calendar.
- The attendees-fetch cost, the exact attention-badge rule, and the event sync window are tuning decisions left to the
  implementation (fetch attendees for upcoming/important events, lazy elsewhere; bound the sync window to cap volume).

## References

- [ADR-0019: Central Feature Registry](0019-central-feature-registry.md) — the single source of truth Calendar is the
  first feature to register against, and the origin of the shared scope set and 403 re-auth gate it reuses.
- [ADR-0010: ESI Write Path / Durable Outbox](0010-esi-write-path-outbox.md) — the outbox the RSVP respond write runs
  through (apply / execute / compensate).
- [ADR-0011: Eager Full-Body Mail Sync](0011-eager-full-body-mail-sync.md) — the Mail feature whose architecture
  Calendar mirrors (rail screen, combined view, DB cache, outbox writes).
- [ADR-0017: Interface Scale and Accessibility Config](0017-interface-scale-and-accessibility-config.md) — the global
  interface scale the calendar screen inherits without per-view work.
- [ADR-0018: Runtime High-Contrast Color Resolution](0018-runtime-high-contrast-color-resolution.md) — the runtime
  high-contrast resolution the calendar UI gets for free by drawing color from the color module.
