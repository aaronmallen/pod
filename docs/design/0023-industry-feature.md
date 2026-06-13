---
id: "0023"
title: Industry Feature
status: active
tags: [architecture, features, sync, ui]
created: 2026-06-13
---

# ADR-0023: Industry Feature

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod gains a view of each pilot's and corporation's industry activity — the manufacturing, research, copying,
invention, and reaction jobs currently running — as a **standalone top-level rail screen modeled wholesale on the
Calendar feature**, not a sub-view of any existing screen. It is an ESI-backed, character- and corporation-scoped
feature with an "All Industry" combined view, a local DB cache, a scope picker, a per-feature tweaks panel, and a
segmented tab control. This first cut deliberately ships only **the shell plus the Jobs tab** — a live, multi-owner
monitor of running jobs with 1-second countdowns, slot-usage meters, and estimated output value. The remaining tabs
(Blueprints, Planner, Mining, Extractions) are deferred to later specs. Industry is **read + sync only — there is no
authoring/outbox path** (ESI exposes no job create/deliver surface Pod uses), it is wired through the **central feature
registry** as a single descriptor (the second registry-native feature after Calendar), and it **reuses the shared
403/missing-scope re-auth gate** rather than building its own. Two new ESI scopes back it, and the corporation sync is
gated on the in-game `Factory_Manager` role.

## Context

CCP exposes a pilot's and corporation's running industry jobs through ESI as paginated, authed list endpoints. Pod
tracks a capsuleer's skills, wallet, assets, mail, and calendar but had no consolidated place to see what is cooking,
what is ready to deliver, and how job slots are utilized — an industrialist had to leave Pod to check. The Jobs tab
fills that gap; the surrounding shell establishes the spine the deferred tabs will share.

Two pieces of existing architecture shape the design:

- **Calendar is the proven template.** Calendar (`src/features/calendar/`, its sync jobs, repo, migrations, and the
  `Route::Calendar` app wiring) is already an ESI-backed, character-scoped, top-level rail screen with an "All Pilots"
  combined view, a scope/account picker, a per-feature tweaks panel, an attention badge, and a registry-derived
  missing-scope re-auth gate (see ADR-0020). Industry is the same shape, so it follows Calendar wholesale rather than
  inventing a new structure — extended only to cover corporation scopes alongside character scopes.
- **The central feature registry now exists.** Per-feature wiring — scope set, rail destination, character-detail tab,
  and the sync `JobKind`s a feature drives — is defined once in `features::registry` as a `Descriptor` per `Feature`,
  with the auth scope set, rail gating, sync schedule, route guard, and the missing-scope re-auth state all deriving
  from that one source of truth (see ADR-0019). Industry is wired as a single descriptor edit, the same way Calendar
  was, rather than touching several scattered inline `match` arms.

The original design proposed a Jita-sell/buy regional price-source tweak and intended to distinguish BPO from BPC
blueprint renders per job; both are superseded below — Pod has no regional pricing, and the ESI job payload does not
state the blueprint variant — in favor of one honest valuation source and a single render variant.

## Decision

### Standalone top-level feature mirroring Calendar

Industry is its own feature module (`src/features/industry/`) with a shell (rail/header with scope picker and stat
strip, a segmented tab control, and the tab content), loaders, and a live-clock subscription — structurally
paralleling `src/features/calendar/`. It owns a `Route::Industry`, a `rail::Destination::Industry` placed between the
Skills and Mail rail items, an attention badge, and the app-level `Message`/navigate/view/dispatch wiring, each
mirroring its Calendar equivalent. The scope picker (reusing `ui::components::picker`) provides an "All Industry"
combined, color-coded view plus per-character and per-corporation scopes. A per-feature tweaks panel (density,
side-rail toggle, group-by, bar color) persists like every other feature's tweaks.

### Shell plus Jobs tab only — other tabs deferred

The tab control is a segmented control rendering a single Jobs tab in this cut, but built to grow: Blueprints,
Planner, Mining, and Extractions are intentionally out of scope and land in later specs. Out of this cut, by
extension, are a persisted blueprints table and the blueprints ESI scope, the true BPO-vs-BPC image distinction, the
industry cost-index/facility-list endpoints, and regional market pricing. The shell, the scope picker, the
sync + storage spine, and the registry wiring are designed to be shared by those future tabs without rework.

The Jobs tab lists running jobs as horizontal progress bars with a live 1-second countdown (a warning state under one
hour, a success state when ready). Each row shows the job's blueprint render, the product name, an activity chip
(including invention success where applicable), runs, facility + security + installer, a progress bar with percentage
and ETA, and an estimated ISK output value. Ready-to-deliver jobs sort first with a success accent. A filter bar
(All / In progress / Ready, with live counts) and a group-by control (None / Owner / Activity / Facility, with sticky
group headers) organize the list, and a toggleable side rail shows per-owner job-slot meters, the next jobs to
complete, and an activity mix.

### ESI read + sync only — no authoring

The feature deliberately does not create, deliver, or cancel jobs. It reads the character and corporation industry-job
list endpoints and writes nothing back to ESI, so there is **no durable-outbox/write path** (contrast Calendar's RSVP
respond write and Mail's set-read). Two new ESI scopes back the reads: a character scope and a corporation scope. Two
new periodic read-sync jobs (`JobKind::CharacterIndustryJobs` and `CorporationIndustryJobs`) fetch the paginated lists
hourly and **full-replace** the local cache tables (upsert-before-prune, batched), and the Jobs tab renders from that
cache.

### DB-cached, synced like the rest of Pod

Two new tables (`character_industry_jobs` / `corporation_industry_jobs`) cache the job lists, keyed on `job_id`, with
the owning character/corporation cascade-deleting their rows and **only `character_id`/`corporation_id` carrying
foreign keys** — every other id (blueprint, product, facility, station/structure, installer) is denormalized per the
canonical-data-model rule (ADR-0003). The character sync mirrors the asset sync's character flow (a not-ready guard
until the parent profile exists); the corporation sync mirrors the corporation-wallet flow — it resolves the
corporation, **gates on the in-game `Factory_Manager` role**, classifies 403/transport errors, and routes a 401 into
re-auth. Outcomes are honest per ADR-0014: `Skipped` on a missing role or scope, `NotReady` before the parent row
exists, and `Failed` only on a genuine transport error or 403. ESI dates are carried as strings, not parsed to a
calendar type, and the views render from the cache.

### Single registry descriptor — the second registry-native feature

Industry is registered as one `features::registry` descriptor: `Feature::Industry` → scopes
`[character-industry, corporation-industry]`, rail `Destination::Industry`, jobs
`[CharacterIndustryJobs, CorporationIndustryJobs]`, and **no character-detail tab** (it is top-level only). Every
consumer — the auth scope set, rail gating, the sync schedule, the route guard, and the Settings → Features catalog
entry (under the World section) — derives from that descriptor. Toggling Industry in Settings applies live with no
restart: disabling hides the rail icon, makes `Route::Industry` unreachable (redirecting to Characters), drops its
sync jobs and its scopes from new auths, and re-enabling restores instantly with retained data (non-destructive).
Following Calendar, Industry's wiring is authored entirely as a registry descriptor rather than as scattered inline
mappings.

### Reuse the shared 403 / missing-scope re-auth gate

Because Pod did not previously request the industry scopes, **every existing character and corporation hits the shared
missing-scope re-auth gate** until re-authenticated. Rather than build an industry-specific re-auth flow, the feature
reuses the shared gate keyed off the registry's scope set, exactly as Calendar does:

- The per-owner ("Mine") view replaces the Jobs content with the shared gate (lock glyph, forbidden status, the
  required industry scopes drawn from the registry as chips, and an SSO re-authenticate action).
- The combined "All Industry" view drops unauthorized pilots/corps and shows a slim amber banner naming them with a
  re-authenticate action.
- The scope picker marks unauthorized pilots/corps with a lock indicator and a "not authorized" subtitle.

Re-authentication requests the full enabled-feature scope set (registry behavior), resolving every gap at once.

### Slot capacity derived from synced skills

Per-character slot maximums are derived in Rust from already-synced skills rather than fetched: manufacturing slots
from the mass-production skills, science/research slots from the laboratory-operation skills, and reaction slots from
the mass-reactions skills (the precise base and formula are an implementation detail confirmed against current EVE
mechanics during planning). Used slots are the running jobs bucketed by activity (manufacturing; science for
research/copying/invention; reactions). Corporation slot maximums aggregate the member characters' caps. The header
stat strip (Active jobs + ready, Job slots used/max, In production, Job fees) and the side-rail meters read from these
derived values.

### Honest ESI-estimate valuation

Per-row "ISK out" and the header "In production" stat are computed from Pod's existing global market-price cache,
using the ESI **average price** and labeled explicitly as an ESI estimate. The original design's
Jita-sell/buy/regional price-source tweak is **dropped** — Pod has no regional pricing — in favor of a single honest
source. There are no new market ESI calls; valuation reads the cache Pod already maintains.

### The blueprint render — one intentional design deviation

Each job row shows the actual blueprint render of the job's blueprint type, drawn from the committed icon set's
blueprint renders (ADR-0013) in a rounded, clipped tile, with an avatar-style tonal-initials tile as the fallback when
the render is missing. This is **the feature's one intentional deviation** from Pod's usual item-initial tiles, chosen
because a recognizable blueprint image is the most useful identifier for an industry job. This cut always renders the
BPO variant: the ESI job payload does not state whether a job's blueprint is original or a copy, so the true variant is
deferred alongside the future Blueprints tab, where it becomes a one-argument change to the existing icon resolver.

### Facility and security resolution by reuse

Facility, system, and security data reuse Assets' existing station/structure/reference-data resolution (ADR-0006),
with a graceful fallback (system name plus raw id) for player-owned structures Pod cannot resolve. No new
resolution path or endpoint is introduced for this cut.

### Accessibility by convention

All Industry UI draws color from `src/ui/style/color.rs` (no hardcoded colors), so the runtime high-contrast switch
(ADR-0018) flips Industry colors live, and owner/activity accent colors derive from the shared palette. Interface
scale (ADR-0017) applies globally via the daemon's `scale_factor`; the Industry screen needs no per-view scaling work.

## Consequences

- Industrialists get a consolidated, multi-character and multi-corporation view of running jobs — countdowns, slot
  usage, and estimated output value — without leaving Pod.
- Because Industry is read + sync only, Pod never owns the authoring lifecycle of industry jobs; there is no
  create/deliver/cancel surface to keep in sync with the game, and no outbox to reason about.
- Industry is the second feature wired entirely as a registry descriptor, reinforcing the one-descriptor pattern: a
  single edit wires the scopes, rail, sync jobs, route guard, and live-toggle behavior, and it inherits the shared 403
  re-auth gate for free.
- Shipping the shell plus the Jobs tab first establishes the sync, storage, scope-picker, and registry spine that the
  deferred Blueprints / Planner / Mining / Extractions tabs reuse, keeping each future tab a smaller, additive change.
- Deriving slot capacity from skills and valuing output from the existing average-price cache adds value from
  already-synced data at zero additional ESI cost, at the price of an estimate rather than a live regional quote — an
  honesty trade made explicit in the UI.
- The corporation `Factory_Manager` role gate, the exact slot-capacity formula, structure-name fidelity for
  player-owned facilities, and corp slot-meter aggregation are tuning decisions left to the implementation.

## References

- [ADR-0020: Calendar Feature](0020-calendar-feature.md) — the standalone-rail-feature template Industry mirrors
  wholesale (rail screen, combined view, scope picker, DB cache, registry wiring, shared re-auth gate).
- [ADR-0019: Central Feature Registry](0019-central-feature-registry.md) — the single source of truth Industry
  registers against, and the origin of the shared scope set and 403 re-auth gate it reuses.
- [ADR-0015: ESI Request-Layer Policy](0015-esi-request-layer-policy.md) — the endpoint, scope, and pagination
  conventions the industry-job reads follow.
- [ADR-0003: Canonical Data Model](0003-canonical-data-model.md) — the rule that only `character_id`/`corporation_id`
  carry foreign keys while every other id is denormalized.
- [ADR-0014: Persisted Sync Ledger and Honest Job Outcomes](0014-persisted-sync-ledger-and-honest-outcomes.md) — the
  honest `Skipped`/`NotReady`/`Failed` outcome semantics the industry sync jobs report.
- [ADR-0002: Sync/Render Separation and Aggregation Chaining](0002-sync-render-separation.md) — the separation the
  feature follows between the sync jobs that cache job lists and the views that render them.
- [ADR-0006: Static and Reference Data](0006-static-and-reference-data.md) — the reference-data resolution Industry
  reuses for facility, system, and security data.
- [ADR-0013: Committed Item Icons and Synced Portraits/Logos](0013-committed-item-icon-set.md) — the committed
  blueprint renders the Jobs tab draws for its one intentional design deviation.
- [ADR-0017: Interface Scale and Accessibility Config](0017-interface-scale-and-accessibility-config.md) — the global
  interface scale the Industry screen inherits without per-view work.
- [ADR-0018: Runtime High-Contrast Color Resolution](0018-runtime-high-contrast-color-resolution.md) — the runtime
  high-contrast resolution the Industry UI gets for free by drawing color from the color module.
