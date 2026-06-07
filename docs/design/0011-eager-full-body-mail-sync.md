---
id: "0011"
title: Eager Full-Body Mail Sync
status: active
tags: [architecture, esi, mail, sync, sync-render-separation]
created: 2026-06-06
---

# ADR-0011: Eager Full-Body Mail Sync

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The 0.5.0 prototype fetched mail bodies **lazily, on click, from the render layer**
(`services/mail.rs::fetch_mail_body` / `load_mail_body_task`), which violates
[ADR-0002](0002-sync-render-separation.md) (render reads the DB only, never calls ESI). S7 (Mail, spec `kmrurkmq`)
moves the whole completeness story behind the sync boundary as an **eager, immutable, transactional** contract. This
ADR is the binding source of truth for the downstream S7 schema and sync tasks; it does not itself ship code. It
settles:

1. **Completeness contract.** A mail row is committed only when its **header + full stripped body + all resolved
   participant names** are present together, in **one transaction** — or the row is not persisted (mirroring the
   `resolve.rs` abort-on-incomplete pattern).
2. **Immutable bodies.** A body is fetched **at most once per `(character_id, mail_id)`** and **never refetched**;
   later header syncs mutate only `is_read` and `labels`.
3. **Render issues zero ESI calls.** The reading-pane path reads a complete, body-present row from the DB; the active
   screen surfaces background-sync results via a `ClockTick` reload, **not** a render-time fetch.
4. **Inline name resolution.** Sender + recipients resolve through the shared resolver
   ([ADR-0006](0006-static-and-reference-data.md)); the resolved name is stored **on the participation row**, not as a
   duplicated entity, and known entities join to SDE-seeded static data ([ADR-0006](0006-static-and-reference-data.md)).
5. **Skip-until-parent FK ordering** ([ADR-0003](0003-canonical-data-model.md)) for mail referencing
   not-yet-synced participants.
6. **The mail mirror schema decisions** answering every spec Open Question (participants shape + `recipients_display`
   as a render composite, labels catalog + membership join with locally-computed unread, **raw-HTML canonical** body
   storage, reserved archive/trash columns, and an `mail_unified` view for All-Inboxes).

All chosen table shapes and storage formats below are **flagged for DDL sign-off before the schema tasks implement
them** — this ADR fixes the design; the migrations are separate, signed-off tasks.

## Context

Pod has no mail feature in the shipped tree: no mail model, repo, sync job, or migration. A complete,
behavior-validated mail client exists in the 0.5.0 prototype
(`tmp/scratch/0.5.0-prototype.1/src/controllers/mail.rs`, `.../services/mail.rs`) and a visual treatment in
`tmp/design/mail.jsx`. S7 ports that proven behavior forward as a first-class, sync-backed, multi-account mail screen
— but the prototype's lazy on-click body fetch is a render-layer ESI call, which
[ADR-0002](0002-sync-render-separation.md) forbids: render reads the DB only, and every persisted row must be complete
and renderable.

Moving the fetch behind the sync boundary turns "completeness" from a render-time guess into a sync-time **contract**,
but the exact rules, table shapes, and storage formats are architectural and must be settled before schema and sync
work begins. The prototype's completeness path (`build_recipient_map`, `collect_name_ids`, `resolve_mail_name_map`,
`supplement_name_map`, `strip_html`, `derive_preview`) shows *what* must be present; this ADR fixes *how* it is
persisted and *which* shape the mirror takes. It also resolves the spec's six Open Questions — the participants table
shape, labels storage, body storage format, archive/trash reserved columns, and the All-Inboxes read path — so the
downstream tasks have one authoritative source.

This ADR governs downstream S7 tasks; it does not ship code, and it introduces no DDL itself.

## Decision

### 1. Completeness contract — header + body + names, one transaction

A dedicated **`CharacterMail`** sync job (authenticated, ownership-gated per ADR-0002 and the ownership rule) fetches
mail for each owned character. Per mail, the job assembles the full picture before persisting:

1. `GET /characters/{id}/mail/` → header list (`mail_id`, `from`, `recipients[]`, `subject`, `timestamp`, `is_read`,
   `labels`).
2. For each new `mail_id` without a stored body: `GET /characters/{id}/mail/{mail_id}/` → body HTML.
3. Resolve every participant id (sender + recipients) to a name via the shared resolver
   ([ADR-0006](0006-static-and-reference-data.md): `POST /universe/names/`, batched, deduped, gracefully partitioned),
   joining to SDE-seeded static data ([ADR-0006](0006-static-and-reference-data.md)) for entities already known.

**Only when all three are present** does the job upsert header + body + participants in **one transaction**. A row with
a missing body or an unresolved name is **never persisted** — the same abort-on-incomplete discipline as
`sync/jobs/resolve.rs`'s skill-metadata path. This is the ADR-0002 completeness invariant applied to mail: render can
never see a half-assembled mail.

### 2. Immutable bodies; later syncs touch only mutable fields

A body is fetched **exactly once** per `(character_id, mail_id)` and **never refetched**. EVE mail bodies are immutable,
so re-fetching wastes a round-trip and a cache window. The body GET is skipped for any `mail_id` that already has a
stored body; later header syncs upsert only the **mutable** fields — `is_read` and `labels` — never the body or the
participants. This is "eager full-body sync," the inverse of the prototype's lazy render-triggered fetch.

### 3. Render performs no ESI call; active screen reloads on `ClockTick`

The render / reading-pane code path issues **zero** ESI calls. Opening any mail displays a body already present in the
DB. When a background `CharacterMail` sync lands new mail or flips read state, the active screen reflects it via a
`ClockTick` reload (re-querying the DB), **not** a render-time fetch. This keeps ADR-0002's "render reads the DB only"
intact and removes the prototype's `fetch_mail_body` / `load_mail_body_task` render-layer network path entirely.

### 4. Name resolution stored on the participation row (no duplicated entity)

Non-Pod participants (characters/corps/alliances/mailing lists not tracked as Pod entities) are resolved inline via the
shared resolver and their **resolved name is stored as a property of the participation row** — *not* as a duplicated
entity record. This follows [ADR-0006](0006-static-and-reference-data.md)'s ruling that resolved names are persisted
**denormalized onto each consuming job's rows** (no shared name-cache table), so the render path never refetches.
Entities Pod *does* know (via SDE, ADR-0006, or its own synced org graph) join to that static data for display. Factions
are excluded from the resolver and resolve from seeded SDE static data, per ADR-0006.

### 5. Skip-until-parent FK ordering

A mail referencing a not-yet-synced participant uses **skip-until-parent**
([ADR-0003](0003-canonical-data-model.md)): the mail waits for the parent to land rather than failing the
whole batch with `SQLITE_CONSTRAINT_FOREIGNKEY` (787).
Because resolved participant names are stored on the participation row (decision 4) rather than foreign-keyed to a
not-synced universe entity, the FK surface is the *owning character* (`characters.id`), which is always present — so
skip-until-parent applies to the narrow set of references Pod actually inserts both sides of, exactly as ADR-0003
intends.

### 6. Mail mirror schema decisions (DDL sign-off required)

The mirror is a set of new tables, all **new migrations after the current latest** (no `ALTER` migrations), and **all
DDL below is flagged for sign-off before the schema tasks implement it**. The shapes are the *decided* design, not the
final SQL.

#### 6a. Recipients / participants — one row per participant; `recipients_display` is a render composite

A `character_mail_recipients` row per `(character_id, mail_id, recipient_id, recipient_type, recipient_name)`, mirroring
the ESI `recipients[]` shape exactly — **one home per fact** — with the resolved name stored on the participation row
(decision 4). The prototype's `MailHeader.recipients_display` string is a **render/view-time composite** over that
table, **not a stored column**: deriving composites via views/render is the canonical-data-model rule, and a stored
display string would be a second home for a fact already in the rows. **Decided: participants table per ESI shape;
`recipients_display` derived, not stored.**

#### 6b. Labels — catalog + membership join; unread computed locally

ESI labels are per-character with server ids. Store as a `character_mail_labels` **catalog** (`character_id`,
`label_id`, name, color) plus a `character_mail_label_membership(character_id, mail_id, label_id)` **join**. Per-label
**unread counts are computed locally** from the mirror (`is_read = false` joined through membership), **not** read from
ESI's label endpoint — consistent with the spec's unread-only rule, which already computes unread locally everywhere
(per-folder, per-character, rail dot). **Decided: catalog + membership join; unread computed locally.**

#### 6c. Body storage — raw ESI HTML is canonical; paragraphs/preview derived at render

Store the **raw ESI HTML** as the canonical body and derive stripped paragraphs + the 250-char preview at render
(the prototype's `strip_html` / `derive_preview` become render-time derivations), **rather than** the prototype's
pre-stripped `paragraphs.join("\n")` storage. Raw HTML is the source of truth from ESI: keeping it canonical means a
later change to the strip/preview logic re-renders correctly without re-fetching immutable bodies (decision 2), and it
honors "store the ESI response shape, derive composites." **Decided: raw HTML canonical; strip + preview derived.**

#### 6d. Archive / Trash — reserve columns now for a future ESI remap

v0.5 ships Archive and Trash as **pure app-owned local overlays** ([ADR-0003](0003-canonical-data-model.md)): moving a
mail
reassigns its overlay folder locally with no ESI call. The user reserves the right to **later** remap Archive → an ESI
label and Trash → an ESI delete. To keep that door open **without a future migration**, the overlay assignment reserves
the minimal columns now: an **original/target ESI label id** (nullable, for the eventual Archive→label remap) and a
**soft-delete intent** marker (for the eventual Trash→delete remap). They sit unused in v0.5. **Decided: reserve the
two columns; ship as local overlays.**

#### 6e. All-Inboxes — read through a `mail_unified` SQL view

The "All Inboxes" unified stream is a cross-character query. It reads through a **`mail_unified` SQL view** that merges
all owned characters' mail (each row carrying its owning `character_id` so an action routes to the correct character),
**not** an ad-hoc per-call join — the canonical model derives composites via views. **Decided: `mail_unified` view is
the All-Inboxes read path.**

### Mark-read & send ride the Outbox (ADR-0010)

Mark-read-on-open and compose/reply/forward are **mutations**, so they go through the durable Outbox
([ADR-0010](0010-esi-write-path-outbox.md)) as the only sanctioned UI→ESI write path — not through this read job.
Opening an unread mail flips `is_read` optimistically (the unread count drops immediately) and enqueues a
`mail.set_read` outbox row; sending enqueues `mail.send`. The `CharacterMail` read job **reconciles** the mirror with
ESI truth on its next pass: a successful mark-read simply matches; a failed one is corrected back to whatever ESI
reports (read-sync-wins, ADR-0010). The mail mirror tables, the read-sync job, and the four per-kind outbox
encoders/appliers/compensations are S7's to build; this ADR fixes the contract they implement.

## Affected Areas

- New mail **mirror** migrations (after the current latest; no `ALTER`): `character_mail` (headers + mutable
  `is_read`/labels), `character_mail_body` (raw HTML, immutable), `character_mail_recipients` (participants + resolved
  names), `character_mail_labels` (catalog), `character_mail_label_membership` (join), and a `mail_unified` view — **all
  DDL flagged for sign-off**.
- New app-owned **overlay** migrations (ADR-0003): snooze / star / pin / archive-trash assignment, with the reserved
  Archive→label / Trash→delete columns (decision 6d) — also flagged for sign-off.
- `src/sync/jobs/` — the new `CharacterMail` job (eager header + immutable body + resolved-name completeness contract,
  ownership-gated, read-state reconciliation).
- `src/sync/job.rs` — the new `JobKind::CharacterMail` variant (interval, required scope `esi-mail.read_mail.v1`,
  ownership gating).
- `src/features/mail/`, `src/store/model/`, `src/store/repo/` — feature code, models, and repos (the prototype's
  `src/services/` layout does not exist here).
- The shared name resolver ([ADR-0006](0006-static-and-reference-data.md): `sync/jobs/names.rs::resolve_names`) — mail
  co-consumes it rather than duplicating the `POST /universe/names/` call.
- The Outbox ([ADR-0010](0010-esi-write-path-outbox.md)) — mail's mark-read and send/compose ride it; the four mail
  outbox kinds (`mail.send` / `mail.set_read` / `mail.set_labels` / `mail.delete`) are S7's per-kind handlers.

## Consequences

### Positive

- ADR-0002 is restored: render issues no ESI call, every persisted mail is complete and renderable, and the reading
  pane always reads a body-present row from the DB.
- Immutable, fetch-once bodies bound ESI cost: a re-running sync issues no body GET for an already-bodied mail.
- Raw-HTML canonical storage keeps the body honest to the ESI response, so the strip/preview logic can evolve at render
  without re-fetching immutable bodies.
- Storing resolved names on the participation row (per ADR-0006) means no shared name-cache table and no render-time
  refetch; the participants table is the single home for the recipients fact, and `recipients_display` is a free
  derivation.
- Locally-computed per-label unread keeps every count consistent with the unread-only rule and avoids depending on a
  second ESI count source.
- Reserving the Archive→label / Trash→delete columns now keeps the future ESI remap migration-free.
- A `mail_unified` view makes All-Inboxes a single canonical read path that scales with character count.

### Negative

- Eager full-body sync front-loads a body GET per new mail at sync time (vs. the prototype's lazy fetch), increasing
  first-sync request volume — accepted, because it is the cost of the completeness guarantee and immutable bodies cap
  the total at one GET per mail ever.
- The completeness contract means a mail with an unresolvable participant is **withheld** until the name resolves (or
  is gracefully partitioned by the resolver), so a mail can be briefly absent rather than shown incomplete — the
  intended ADR-0002 tradeoff, surfaced via sync lifecycle, not a UI loading state.
- Raw-HTML storage costs slightly more bytes than pre-stripped text and pushes strip/preview to every render — a
  deliberate trade for canonical fidelity and forward-compatible rendering.
- The mirror is several tables plus a view and overlay tables; the schema is broader than a single denormalized mail
  table, and every shape needs DDL sign-off before implementation.
- Archive/Trash ship as local-only overlays in v0.5, so they do not yet reflect to ESI; the reserved columns are dead
  weight until the remap is built.

## Open Questions

- **DDL sign-off.** Every table shape and the `mail_unified` view above are *decided* but require explicit DDL sign-off
  before the schema tasks implement them (no `ALTER` migrations; new migrations after the current latest).
- **EVE/UTC snooze preset semantics.** The exact resolution of each snooze preset (e.g. "This weekend" = upcoming
  Saturday 00:00 UTC) is a triage/overlay concern owned by the S7 Mail spec, not settled here; the prototype's preset
  hints differ from the locked list and must be reconciled to it.
- **Mailing-list recipients.** Lists are usable as recipients (resolved at send time); list *management* is out of
  scope, and exactly how a list id is stored on a sent mail's participation row is a send-path detail for the Outbox
  per-kind `mail.send` handler.

## References

- [ADR-0002](0002-sync-render-separation.md) — Sync/Render Separation. The completeness invariant and "render reads the
  DB only, never the network" this ADR applies to mail (moving the prototype's render-layer body fetch behind sync).
- [ADR-0003](0003-canonical-data-model.md) — Canonical Data Model. The snooze/star/pin/archive/trash overlays
  (app-owned: app-allocated ids, direct write path, cascade FK, excluded from sync discovery) and the reserved
  Archive→label / Trash→delete columns; plus skip-until-parent FK ordering for mail referencing not-yet-synced
  participants (FK only where Pod inserts both sides).
- [ADR-0006](0006-static-and-reference-data.md) — Static and Reference Data. Known corp/alliance/type names join to
  seeded SDE static data (factions resolve from SDE, not the resolver); and the shared `resolve_names` resolver mail
  co-consumes, with resolved names persisted denormalized onto consuming rows (no shared name-cache table).
- [ADR-0010](0010-esi-write-path-outbox.md) — ESI Write Path / Durable Outbox. The write transport mark-read-on-open
  and compose/send ride; the four mail outbox kinds and read-sync-wins reconciliation.
- Spec — "S7: Mail" (gest artifact `kmrurkmq`): the three-pane client, the completeness contract, immutable bodies,
  name resolution, overlays, and the six Open Questions this ADR resolves.
- Prototype (behavior truth, replaced) — `tmp/scratch/0.5.0-prototype.1/src/services/mail.rs` (`strip_html`,
  `derive_preview`, `build_recipient_map`, `collect_name_ids`, `resolve_mail_name_map`, `supplement_name_map`,
  `fetch_mail_body`) and `.../controllers/mail.rs` (the lazy `load_mail_body_task` render-layer fetch this ADR removes).
- Visual treatment (reference only) — `tmp/design/mail.jsx`.
