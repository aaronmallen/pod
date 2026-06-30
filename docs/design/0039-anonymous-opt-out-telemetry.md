---
id: "0039"
title: Anonymous Opt-Out Telemetry
status: active
tags: [telemetry, privacy, infrastructure]
created: 2026-06-25
---

# ADR-0039: Anonymous Opt-Out Telemetry

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod adds anonymous, opt-out telemetry (default ON, silent settings-only disclosure). The native Rust/Iced client
batches four default-ON streams — usage, performance, environment, crashes — and POSTs them fire-and-forget to a
Cloudflare Worker that validates a static write-key and INSERTs into D1 (SQLite). The install identifier is
`sha256(machine_id)` derived at send time (never stored, never resettable). Crashes are buffered to disk and uploaded on
the next launch. The endpoint URL and write-key are baked at build time via `option_env!`, so dev builds are silent
no-ops.

## Context

Pod has no usage or crash insight today (the marketing site uses Umami; the app has nothing). We want to prioritize
features, catch regressions, and surface crashes, without compromising the project's privacy posture. Constraints
shaping the design: Pod is a single native binary (no webview/JS, no IPC); a per-install random `machine_id` already
exists; the panic hook runs in a dying process; and the maintainer's `aaronmallen.dev` zone is already on Cloudflare
with `pod.aaronmallen.dev` proxied in front of the GitHub Pages marketing site.

## Decision

- Pipeline: in-process `OnceLock` collector with flush-time gating → dedicated fire-and-forget sender
  (`clients/telemetry.rs`, not the ESI client) → Cloudflare Worker route `pod.aaronmallen.dev/telemetry/*` → D1.
- Backend storage: D1 (SQLite), scaffolded in-repo under `web/telemetry/`, deployed via wrangler (pinned in
  `.config/mise.toml`). The Worker structurally never reads `CF-Connecting-IP`.
- Identity: `sha256(machine_id)`, derived per send, never stored, no reset.
- Crashes: buffered to disk (NDJSON in the log dir, no SQLite migration) and sent + deleted on next launch.
- Config injection: build-time `option_env!` (`POD_TELEMETRY_URL` literal + `POD_TELEMETRY_KEY` secret) from GitHub
  Actions, mirroring the updater key pattern.
- Disclosure: opt-out, default ON, silent — a Settings Telemetry category with a master switch + four stream toggles; no
  first-run prompt; the marketing site stays silent.

### Rejected alternatives

- Cloudflare Analytics Engine instead of D1 — cheaper at scale but sampled and awkward for reading individual crash
  reports; D1 is queryable and free at Pod's scale.
- Reuse `machine_id` directly / a rotating id / no id — reusing couples telemetry to an operational identifier;
  rotating/none loses active-install dedup. Hashing keeps dedup without exposing the operational id; a deterministic
  hash is intentionally non-resettable.
- In-session crash upload — impossible from a dying process; disk-buffer + next-launch is the only reliable capture.
- Dedicated `telemetry.aaronmallen.dev` subdomain / `*.workers.dev` — a route on the already-proxied host needs zero
  new DNS/cert and leaves the marketing site untouched.
- First-run opt-in prompt — contradicts the opt-out/default-on posture and adds friction.

## Affected Areas

- `src/config.rs` (`TelemetryConfig`), `src/clients/telemetry.rs`, `src/services/telemetry.rs`
- `src/app.rs` (nav/sub-section capture, panic hook, tracing layer, subscription/shutdown flush)
- `src/features/settings*` + `src/features/nav_catalog.rs` (Settings Telemetry category)
- `.github/workflows/release.yml`, `.config/mise.toml`, new `web/telemetry/` Cloudflare Worker + D1

## Dependencies

No new core Rust dependency (reuses `reqwest`, `tokio`, `serde`/`serde_json`, `sha2`, `chrono`). Backend introduces
Cloudflare Workers + D1, deployed with wrangler (pinned via mise).

## Consequences

### Positive

- Real usage/crash/performance signal with a structural privacy boundary (no IP column, no stored id, JSON allow-list
  scrubbing, fail-closed Worker content check).
- Cheap, queryable backend; dev builds emit nothing by construction.
- Opt-out and per-stream control are discoverable, with a live "what gets sent" preview that mirrors the wire
  byte-for-byte.

### Negative

- A stable (non-rotating) `anon_id` links one install's reports within the 90-day window; fingerprint surface is reduced
  (language-only locale, coarse OS version, no IP) but not zero.
- A static write-key baked into long-lived binaries is anti-abuse only, not auth; rotation needs a multi-key acceptance
  window.
- Crash capture adds a tracing ring buffer + a process-global panic path.

## Open Questions

- locale/os_version source (ship "unknown" v1 vs add sys-locale)?
- `frame_p95_ms` (redraw-delta approximation vs defer) and `heap_mb` source (allocator counter)?
- Flush interval and display bucketing (fingerprint reduction)?
- Write-key rotation policy (multi-key set vs single-key orphan-on-rotate)?

## References

- Spec: gest artifact `mmmzstpq` (Pod Anonymous Opt-Out Telemetry — Unified Spec)
- Marketing-copy task: gest task `nwvumtyq` (site stays silent — out of scope)
