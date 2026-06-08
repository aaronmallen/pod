---
id: "0015"
title: ESI Request-Layer Policy — Rate Limiting, Token Bucketing, Compatibility Date
status: active
tags: [architecture, esi, http, rate-limiting]
created: 2026-06-08
---

# ADR-0015: ESI Request-Layer Policy — Rate Limiting, Token Bucketing, Compatibility Date

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

CCP developer relations contacted the pod author directly about how pod talks to ESI. This record captures three linked
decisions at the shared HTTP request layer (`src/clients/http.rs` and the ESI route layer), made in response:

1. **Proactive, header-driven, in-memory per-group rate limiting** — parse the `X-Ratelimit-*` budget headers ESI now
   returns, learn the route→group mapping dynamically, and pre-emptively space requests as a group's budget drains,
   keeping the reactive `429`/`420` handling as a safety net.
2. **Per-application rate-limit bucketing** — attach an owned character's Bearer token to the otherwise-public
   killmail-detail request so ESI buckets pod as `<sourceIP>:<applicationID>` instead of sharing the per-IP bucket with
   every app behind the same NAT.
3. **Compatibility-date versioning** — strip the deprecating `vN/` URL prefixes and send a single pinned
   `X-Compatibility-Date` header on ESI-host requests only.

All three are request-shaping changes. None of them touch the database schema: rate-limit budgets are in-memory, the
Bearer token comes from the existing credential store
([ADR-0005](0005-eve-sso-authentication-and-deeplink-transport.md)), and the compatibility date is a compiled-in
constant.

## Context

pod is one ESI consumer among many, and ESI's October–December 2025 changes tightened how it expects clients to behave.
Four issues surfaced; this record addresses the three that live at the request layer. (The fourth — replacing the 80 MiB
SDE download with a `latest.jsonl` build-number probe — is a static-data concern under
[ADR-0006](0006-static-and-reference-data.md) and is handled separately.)

The shared `http::Client` is the single outbound chokepoint for every host pod talks to — ESI, zKillboard, the SDE
static-data host, and the image CDN all flow through it. Any per-host behavior therefore has to be applied
conditionally inside that one client rather than as a global default, or it leaks onto hosts it must not reach.

## Decision

### 1. Proactive per-group rate limiting

**Context.** Every ESI response now carries `X-Ratelimit-Group` (the route group), `X-Ratelimit-Limit` (`"N/<window>"`,
e.g. `"150/15m"`), `X-Ratelimit-Remaining`, and `X-Ratelimit-Used`. pod previously ignored these and only reacted after
ESI returned `429`/`420`, so it ran *at* the limit and tripped throttles — most visibly on killmail-detail routes. CCP's
guidance is to slow down *as you approach* a limit, not to operate at it.

**Decision.** The shared client parses the four budget headers from every response inside the `send_logged` chokepoint
and maintains in-memory per-group budget state — `{ limit, remaining, window, estimated reset, next-allowed-at }` keyed
by group. The route→group mapping is learned dynamically from responses and cached, because a route's group is only
known after the first response to it; until then the client is optimistic and relies on the reactive catch. Route keys
are normalized — numeric and SHA-hash path segments collapse to a placeholder — so that, for example, every
killmail-detail URL maps to the one learned group rather than to a unique per-killmail key.

The gate runs **per request**: before sending, a request whose group is at or below a low-water fraction of its limit is
delayed, spacing the remaining budget across the rest of the window; when the budget is exhausted it waits for the
window to reset. Because the gate is per-request, the concurrent paginated fan-out (`get_json_paginated`) cannot burst
past a group's budget. Budgets are in-memory only and are **not** persisted across restarts — windows are short and the
state is relearned from headers. The reactive safety net is unchanged: `429 → Error::RateLimit`, `420 →
Error::ErrorLimited`, and the sync engine's global pause / job reschedule / `Event::BackingOff` behavior
([ADR-0014](0014-persisted-sync-ledger-and-honest-outcomes.md)) all continue to apply.

This **replaces** the dormant, never-wired prefix-based `RateLimiter` (the dead-code `ClientBuilder::rate_limit`), which
is removed; no dead rate-limit code remains.

**Consequences.** pod now slows itself before being throttled, which removes the killmail-detail `429`s and reduces
needless load on ESI. The cost is added in-memory state and a small per-request lock; the budget is only as accurate as
the most recent headers, and a route is ungated until its first response teaches the client its group — both acceptable
because the reactive net still backstops the optimistic window.

### 2. Per-application bucketing via a Bearer token on killmail-detail

**Context.** Unauthenticated ESI requests are bucketed per `<sourceIP>`, so every app behind a shared NAT/IP competes
for one budget. CCP documents that supplying an access token re-buckets the caller as `<sourceIP>:<applicationID>`,
isolating pod's budget. The killmail-detail request is public and was sent unauthenticated, so it shared the per-IP
bucket — the same routes that were tripping limits.

**Decision.** Attach an owned character's Bearer access token (obtained through the existing auth path —
[ADR-0005](0005-eve-sso-authentication-and-deeplink-transport.md)) to the killmail-detail request. The killmail sync job
already holds an owned-character grant, so a valid token is available without new credential plumbing. Degrade
gracefully: when no owned token is available, fall back to a genuinely unauthenticated request — it still works, it just
shares the IP bucket. Scope is **killmail-detail only** for now; routes that are already authenticated get
`<applicationID>:<characterID>` bucketing automatically and are unchanged. Broadening the token "trick" to other public
routes is deferred (see Future Work).

**Consequences.** Killmail enrichment gets its own application-scoped budget instead of fighting other apps on the same
IP. The trade-off is mild token-refresh pressure on a route that did not previously need a token, and a behavioral
branch (authenticated vs. anonymous) that must keep working in both modes.

### 3. Compatibility-date versioning

**Context.** ESI is migrating from `/v1/ … /v6/` URL prefixes to a single `X-Compatibility-Date` header. The versioned
prefixes are not yet formally removed, but new routes are compat-date-only, and pod hardcodes a `vN/` prefix at 46 call
sites across the route modules. CCP merged all historical versions and guarantees at least a year of backward
compatibility, so pinning a single date is safe.

**Decision.** Strip the `vN/` prefix from all ESI call sites and send one `X-Compatibility-Date` header, pinned to a
single named constant `COMPATIBILITY_DATE = "2026-06-08"` (ISO `YYYY-MM-DD`) with a doc comment describing the **manual
bump policy**: the date is bumped deliberately, in its own change, after verifying representative deserializers against
the new route behavior — never automatically and never to "today" at build time, since that could silently shift
response shapes the deserializers expect.

The header is sent on **ESI-host requests only**. Because the `http::Client` is shared across ESI, zKillboard, the SDE
static-data host ([ADR-0006](0006-static-and-reference-data.md)), and the image CDN, the header cannot be a global
reqwest default; it is injected conditionally for the ESI host (or by the ESI layer itself), and must not appear on
zKill or SDE requests.

**Consequences.** pod stops depending on the deprecating prefix scheme and converges on the header ESI is steering
clients toward, with one obvious place to pin and bump the date. The trade-off is that the pin is a manual maintenance
obligation: an un-bumped date eventually ages toward the back-compat horizon, and a careless bump can change response
shapes, so the bump is gated on deserializer verification.

## Affected Areas

- `src/clients/http.rs` — the `send_logged` chokepoint gains header parsing and the per-group budget gate; the dead
  prefix `RateLimiter` is removed.
- `src/clients/esi.rs` and the route modules under `src/clients/esi/` — `vN/` prefixes stripped; the ESI-host
  `X-Compatibility-Date` header injected.
- `src/clients/esi/killmail.rs` and `src/sync/jobs/character_killmails.rs` — the killmail-detail request threads an
  owned-character Bearer token, with anonymous fallback.
- The sync engine's `429`/`420` → pause/reschedule/`Event::BackingOff` path is **unchanged** and remains the reactive
  safety net beneath the proactive gate.
- **No database schema change.** Budgets are in-memory, the token comes from the existing credential store, and the
  compatibility date is a compiled-in constant.

## Consequences

### Positive

- pod becomes a well-behaved ESI client: it spaces itself under per-group budgets, isolates its killmail budget per
  application, and tracks ESI's versioning direction — directly resolving the issues CCP raised.
- The behavior lives at the single shared request layer, so it is consistent across every ESI route without
  per-call-site bookkeeping.
- Zero schema and zero persistence: the changes are entirely request-shaping and relearned from headers on each run.

### Negative

- The proactive gate adds in-memory state and a per-request lock, and is optimistic until a route's group is learned.
- The killmail path now carries an optional token and must work in both authenticated and anonymous modes.
- `COMPATIBILITY_DATE` is a standing manual obligation — it must be bumped deliberately, with deserializer verification,
  before it ages out of ESI's backward-compatibility window.

## Open Questions

- The exact low-water threshold and spacing formula for the proactive gate are tuning parameters, refined against
  observed ESI behavior rather than fixed by this record.
- Whether to broaden per-application Bearer bucketing beyond killmail-detail depends on how much token-refresh pressure
  the wider route set would add.

## Future Work

- Extend the token "trick" to other public ESI routes if they prove rate-limited, weighing the added token-refresh load.
- Route zKillboard onto the shared `http::Client` so it inherits the cache, the pod User-Agent, and its own rate-limit
  group (tracked separately within this same effort).

## References

- [ADR-0005](0005-eve-sso-authentication-and-deeplink-transport.md) — EVE SSO Authentication and Deeplink Transport. The
  source of the owned-character access token attached for per-application bucketing.
- [ADR-0006](0006-static-and-reference-data.md) — Static and Reference Data. The SDE static-data host shares the same
  `http::Client`; the compatibility-date header and the rate-limit groups must not reach it.
- [ADR-0014](0014-persisted-sync-ledger-and-honest-outcomes.md) — Persisted Sync Ledger and Honest Job Outcomes. The
  reactive `429`/`420` → engine pause/reschedule/`BackingOff` behavior that remains the safety net beneath the proactive
  gate.
- Rate-limiting blog — <https://developers.eveonline.com/blog/hold-your-horses-introducing-rate-limiting-to-esi>.
- Rate-limiting docs — <https://developers.eveonline.com/docs/services/esi/rate-limiting/>.
- Versioning-change blog — <https://developers.eveonline.com/blog/changing-versions-v42-was-getting-out-of-hand>.
- Spec — "ESI Client Health — Rate Limiting, Compatibility Dates, Token Bucketing & SDE Short-Circuit" (gest artifact
  `zrmklsxt`).
