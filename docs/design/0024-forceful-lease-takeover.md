---
id: "0024"
title: Forceful Lease-Takeover Safety Protocol
status: active
tags: [architecture, storage, sync]
created: 2026-06-13
---

# ADR-0024: Forceful Lease-Takeover Safety Protocol

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The networked-drive storage model coordinates writes between machines with a single-writer lease whose only guard
against clobbering a live writer is a **staleness threshold** — a holder that has not heartbeat within
`DEFAULT_STALE_THRESHOLD` (30 seconds) is treated as gone and its lease may be reclaimed. That guard fails against a
**zombie holder**: a process whose sync engine has died but whose process is still alive and still heartbeating, so the
lease stays fresh forever while no real writer exists. Such an instance cannot be recovered by the stale-aware path and,
before this change, could only be unstuck by killing the other machine's process — exactly the "a restart fixes it"
folklore. This ADR records the decision to add a **forceful take-over** that overwrites a *fresh* foreign lease,
bypassing the staleness check, and the safety protocol that contains its data-loss risk: forceful reclaim is **never
automatic**. It is reachable **only** through an explicit user action gated by a data-loss confirmation that surfaces
how long ago the current holder was last active; the **automatic** parked-instance re-acquire and the automatic
promotion to read-write remain strictly stale-aware and never force. No database schema changes: the existing
`lease.json` and `.generation` markers are unchanged.

## Context

Under the networked-drive storage-sync model (ADR-0016), the machine that owns writes holds a lease (`lease.json`) on
the share and heartbeats it. A parked (read-only) instance polls to reclaim the lease, and the stale-test is the entire
safety story: `SyncSession::take_over` routes through the stale-aware `acquire`, which **declines a still-fresh foreign
holder** — returning `HeldBy` and writing nothing — so a live writer is never displaced. The same test backs the
periodic parked re-acquire timer.

That guard is sound only while a fresh heartbeat implies a live writer. It does not, in one important failure mode:

- **Zombie holders.** The sync engine can stop running while the process stays alive — its lease-heartbeat task keeps
  ticking, so the lease never goes stale, but no actual writes are happening and the share is owned by a writer that
  will never write again. Every other instance sees a fresh foreign lease, declines forever, and stays read-only
  indefinitely. The staleness threshold cannot distinguish a healthy writer from a heartbeating corpse — both look
  fresh — so it structurally cannot release this deadlock. The user's only recourse was to kill the zombie process on
  the other machine, which is why "restarting fixes it" but waiting does not.

Recovering from a zombie holder therefore requires the ability to reclaim a lease that is *fresh by the staleness test*.
But fresh-lease reclaim is also the one operation the stale guard exists to forbid, because if the holder is genuinely
alive its in-flight edits will be discarded (last-writer-wins, ADR-0016). This is a genuine coordination decision with a
data-loss tradeoff — when, and on whose authority, Pod is allowed to displace a writer the safety check still considers
live — distinct from the storage-location and sync-direction decisions already recorded, so it warrants its own ADR.

## Decision

### Two reclaim modes, only one of which forces

Lease reclaim has two clearly separated modes with different authority:

- **Automatic reclaim is stale-aware and never forces.** The periodic parked re-acquire (a ~30-second timer) and the
  automatic promotion of a parked instance to read-write both go through the stale-aware `take_over`. A still-fresh
  foreign holder maps to a silent no-op: the instance stays read-only and writes nothing to the share. No background
  timer, and no automatic promotion, may ever override lease freshness. This preserves the ADR-0016 invariant that a
  live writer is never clobbered without human intent.
- **User-initiated reclaim is forceful and confirmation-gated.** A new `SyncSession::force_take_over` writes the lease
  **unconditionally**, skipping the staleness check, then pulls the newer canonical copy so the working copy converges
  before this machine writes again. It is reachable from exactly one place — the explicit "Take over" affordance on the
  read-only banner — and only after the user passes a data-loss confirmation. The stale-aware `take_over` is left
  unchanged and continues to back the automatic path.

### Confirmation gate as the displacement authority

The read-only banner's "Take over" button does **not** claim immediately. It opens a data-loss confirmation that states
the share will be overwritten and surfaces **how long ago the current holder was last active**, rendered from the lease
heartbeat via the shared `format_since` helper ("last active 2m ago"). The two-stage gate is the safety protocol:

- A short last-active age ("a few seconds ago") signals a probably-live writer — the warning lets the user back off
  rather than discard a colleague's in-flight edits on a single accidental click.
- A long or frozen-but-fresh age signals the zombie case — the user has the context to knowingly displace it.

Only confirming calls `force_take_over` and promotes the instance to read-write. Cancelling closes the gate, leaves the
instance read-only, and writes nothing. The human, presented with the holder's liveness evidence, is the authority that
overrides the staleness guard; Pod never makes that call on its own.

### Containing the tradeoff

Forceful take-over is the one place Pod will overwrite a lease the staleness test still considers live, so a genuinely
live writer's uncommitted edits can be lost (last-writer-wins; the losing copy is still preserved as a timestamped
backup by the no-clobber publish primitive of ADR-0016, so the bytes are recoverable). Three properties keep this from
being a footgun: it is never reached automatically; it always shows the holder's last-active age before acting; and it
requires an explicit second confirmation. The capability is narrow and the default-safe automatic behavior is unchanged.

## Affected Areas

- `src/store/sync_session.rs` — the unconditional `force_take_over` (lease claim skipping the staleness check, then
  `pull_if_newer`), alongside the unchanged stale-aware `take_over`.
- `src/app.rs` — the `confirm_force_takeover` gate state, the `ConfirmTakeOver`/`CancelTakeOver` messages, the
  user-initiated take-over handlers, and the stale-aware automatic re-acquire timer that maps a fresh holder to a no-op.
- The read-only banner rendering — the confirmation surfacing the holder's last-active age via `format_since` and
  routing the confirmed action to the forceful claim.

## Consequences

### Positive

- A zombie holder (dead engine, live heartbeat) no longer deadlocks every other instance into permanent read-only;
  there is now an in-app recovery that does not require killing the other machine's process.
- The dangerous capability is contained: forceful reclaim is reachable only by explicit, confirmed user action and is
  impossible from any automatic path.
- The user decides with evidence — the holder's last-active age — rather than blindly.

### Negative

- Forcing a genuinely live writer discards its uncommitted edits (recoverable only as a last-writer-wins backup); the
  staleness guard that normally prevents this is deliberately bypassed on the confirmed path.
- Pod cannot positively prove a holder is a zombie versus a slow-but-live writer; it surfaces last-active age and
  delegates the judgment to the user rather than automating it.

## Future Work

- A liveness signal richer than the heartbeat (e.g. an engine-health field on the lease) could let Pod distinguish a
  zombie holder from a live writer and make the confirmation's guidance sharper, or eventually drive a safe automatic
  recovery.

## References

- [ADR-0016: Networked-Drive Storage-Sync Model](0016-networked-drive-storage-sync.md) — the single-writer lease, the
  staleness threshold, the stale-aware automatic reclaim, and the last-writer-wins / no-clobber backup invariant this
  forceful path deliberately overrides and relies on.
- [ADR-0007: User-Configurable Storage Paths](0007-user-configurable-storage-paths.md) — the configurable, possibly
  networked canonical location whose lease this protocol coordinates.
