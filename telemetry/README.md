# pod-telemetry

The Cloudflare Worker + D1 ingest backend for pod's anonymous, opt-out telemetry
(spec `mmmzstpq` §6 / §9). It lives in-repo under `telemetry/` and is deployed
manually with `wrangler`; CI only bakes the URL + write-key into the released
binary.

The Worker claims exactly the route `pod.aaronmallen.dev/telemetry/*` (only
`/telemetry/v1/ingest` is live). The marketing site at the apex stays untouched.

## What it does

`POST /telemetry/v1/ingest` validates a static write-key and the frozen JSON
contract, INSERTs the envelope into D1, and returns `204 No Content`. It
**never reads or stores the client IP** — there is no reference to
`CF-Connecting-IP` or `request.cf` anywhere (grep-verifiable):

```sh
grep -rIn -e CF-Connecting-IP -e 'request\.cf' src/   # → no matches
```

### Request pipeline (fail-closed, first failure wins, empty bodies)

| step | condition | status |
| ---- | --------- | ------ |
| path | not `/telemetry/v1/ingest` | `404` |
| method | non-`POST` | `405` (`Allow: POST`) |
| key | `X-Pod-Telemetry-Key` missing / not in the valid set (constant-time compare) | `401` |
| size | `Content-Length` > 256 KiB, or the read exceeds 256 KiB | `413` |
| type | `Content-Type` not `application/json` | `415` |
| parse | invalid JSON | `400` |
| contract | closed-world validation fails (§9.4) | `400` |
| crash text | a crash free-text field trips the §5.6 reject | `400` |
| insert | D1 error | `500` |
| success | all checks pass | `204` |

The closed-world validator (`src/contract.ts`) accepts **exactly** the frozen
golden contract pinned by the shared fixtures
`../tests/fixtures/telemetry/session_all_streams.json` and `crash_batch.json`
(the same files the Rust contract crate `src/telemetry_contract.rs` asserts).
Any unknown key anywhere is a rejection.

### Retention

A daily cron (`0 4 * * *`) runs the `scheduled` handler, which
`DELETE`s rows older than 90 days from both `events` and `crashes`, keyed on
`received_at`. The `DELETE` and the column `DEFAULT` use the identical
`strftime('%Y-%m-%dT%H:%M:%SZ', …)` form so the string comparison is correct.

> **Cron currently disabled.** The Cloudflare account rejects the `/schedules`
> deploy, so the cron is commented out in `wrangler.toml` and the worker runs
> without auto-prune (the `scheduled` handler is still in the code). Re-enable
> the cron once Triggers work on the account. Until then, prune manually:
>
> ```sh
> npx wrangler d1 execute pod-telemetry --remote --command \
>   "DELETE FROM events WHERE received_at < strftime('%Y-%m-%dT%H:%M:%SZ','now','-90 days'); \
>    DELETE FROM crashes WHERE received_at < strftime('%Y-%m-%dT%H:%M:%SZ','now','-90 days');"
> ```

## Dashboard

`GET /telemetry/admin` serves a **read-only maintainer dashboard**: a single,
self-contained HTML page (inline `<style>`, inline `<svg>`, no external JS / CSS
/ CDN, no client-side JavaScript) rendering aggregate-only views over the live
D1 data via the existing `DB` binding. It shows six panels:

1. **Active installs** — `COUNT(DISTINCT anon_id)` over a `received_at` window
   (default 30 days, override with `?days=N`, clamped to 1..365), drawn as an
   inline-SVG sparkline. Labeled honestly as distinct *installs* that opted in
   (`anon_id` = `sha256(machine_id)`, one per install, not "users").
2. **Platform breakdown** — distinct installs grouped by `os`, `os_version`,
   `arch`, `display` over `WHERE stream='environment'`. The literal `"unknown"`
   bucket is shown, never filtered out.
3. **Top features** — usage events grouped by `event_kind` + `name` over
   `WHERE stream='usage'`, with the on/off split for `feature_toggle` rows.
4. **Version adoption** — distinct installs per `app_version`.
5. **Performance** — average `load_ms`, `frame_p95_ms`, `heap_mb` per view over
   `WHERE stream='performance'`.
6. **Crash groups** — grouped by `app_version` + `message` (idx_crashes_group),
   each expandable (`<details>`) to its most recent backtrace + context log.

If more than one contract `schema` is present across the data, a schema-mix
panel is shown so different wire-contract versions are not silently aggregated.

The query layer (`src/stats.ts`) is pure: each function takes a `D1Database`
and returns typed, aggregate-only objects (`SELECT`-only, parameter-bound). It
is unit-tested in `src/stats.test.ts` against seeded fixtures, independent of
the HTTP route. The renderer (`src/render.ts`) HTML-escapes every DB-derived
string. Only aggregate counts leave the Worker; no raw events, anon_ids, or
PII are rendered.

### Access control

The route is gated by **Cloudflare Access** (configured in the CF dashboard,
NOT in code). It does **not** reuse the ingest write-key
(`POD_TELEMETRY_KEY` / `POD_TELEMETRY_KEYS`): that key ships inside released
binaries and is anti-abuse, not auth. As cheap defense-in-depth the Worker also
requires the `Cf-Access-Jwt-Assertion` header that Access injects (a request
that did not traverse the Access policy gets `403`), but Access is the primary
gate; the Worker does not verify the JWT signature itself.

Set up the Access policy once in the CF dashboard:

1. Cloudflare dashboard → **Zero Trust → Access → Applications → Add an
   application → Self-hosted**.
2. Application domain: `pod.aaronmallen.dev`, path `/telemetry/admin`.
3. Policy: **Allow**, single-user SSO — include rule **Emails** =
   the maintainer's address (or your IdP group). Everything else is blocked.
4. Save. Now `/telemetry/admin` requires SSO; `/telemetry/v1/ingest` is
   unaffected (Access only covers the admin path).

### Post-deploy verification

After `wrangler deploy`:

```sh
# Admin route is reachable behind Access (interactive SSO in a browser):
open https://pod.aaronmallen.dev/telemetry/admin            # → SSO, then the page
# A direct hit without an Access session is challenged/blocked by Access (302/403):
curl -sI https://pod.aaronmallen.dev/telemetry/admin | head -1

# Ingest auth is unchanged: a missing/bad key is still 401, NOT 404:
curl -s -o /dev/null -w '%{http_code}\n' -X POST \
  https://pod.aaronmallen.dev/telemetry/v1/ingest \
  -H 'Content-Type: application/json' --data '{}'           # → 401
```

## Layout

```text
telemetry/
  wrangler.toml             route + D1 binding + daily cron
  package.json  tsconfig.json
  src/index.ts              fetch + scheduled handlers; admin route wiring
  src/contract.ts           closed-world validators (§6.1 / §9.4) + §5.6 reject
  src/db.ts                 envelope → D1 rows (§6.2), parameter-bound
  src/stats.ts              read-only D1 aggregation queries (dashboard data)
  src/render.ts             dashboard data → self-contained HTML (presentation)
  src/contract.test.ts      loads the golden fixtures; asserts accept + tamper-reject
  src/db.test.ts            asserts the row mapping
  src/stats.test.ts         asserts the aggregations over seeded fixtures
  src/render.test.ts        asserts HTML escaping + self-contained output
  migrations/0001_init.sql  events + crashes tables (NO IP column)
  migrations/0002_rename_display_add_screen_size.sql  display -> window_size, add screen_size
  migrations/0003_add_app_language.sql  add app_language (chosen UI language)
  README.md                 this file
```

`telemetry/` is committed; `.wrangler/` is gitignored.

## Toolchain (mise + aube)

`node` and `aube` come from `.config/mise.toml`. The TypeScript dev dependencies
— `wrangler` (v4), `typescript`, `vitest`, `@cloudflare/workers-types` — are
declared in `package.json` and installed by `aube` (locked in `aube-lock.yaml`),
so deploys use the same wrangler everywhere. No global installs.

```sh
mise install            # node + aube
aube install            # wrangler, typescript, vitest, workers-types → aube-lock.yaml
aube run typecheck      # tsc --noEmit
aube test               # vitest run
```

## Deploy runbook (maintainer, once)

1. `mise use "npm:wrangler@latest"` — one-time; writes the pin into
   `.config/mise.toml`; commit it. Thereafter `mise install` suffices.
2. `wrangler login` — the account owning zone `aaronmallen.dev`.
3. `wrangler whoami` — confirm the zone is visible.
4. `wrangler d1 create pod-telemetry` → paste the returned `database_id` into
   `wrangler.toml`, commit.
5. `wrangler d1 migrations apply pod-telemetry --remote`.
6. `wrangler secret put POD_TELEMETRY_KEY` (e.g. `openssl rand -hex 32`); store
   the **same** value as the GitHub Actions secret `POD_TELEMETRY_KEY`.
7. `wrangler deploy`.
8. Smoke-test against the §6.3 fixture body:

   ```sh
   curl -i -X POST https://pod.aaronmallen.dev/telemetry/v1/ingest \
     -H 'Content-Type: application/json' \
     -H "X-Pod-Telemetry-Key: $KEY" \
     --data @../tests/fixtures/telemetry/session_all_streams.json   # → 204
   curl -i -X POST https://pod.aaronmallen.dev/telemetry/v1/ingest \
     -H 'Content-Type: application/json' \
     --data @../tests/fixtures/telemetry/session_all_streams.json   # → 401 (no key)
   curl -i https://pod.aaronmallen.dev/telemetry/v1/ingest          # → 405 (GET)
   ```

9. `curl -I https://pod.aaronmallen.dev/` — still the marketing site (route
   isolation).
10. `wrangler d1 execute pod-telemetry --remote` with
    `SELECT stream, os, arch FROM events ORDER BY id DESC LIMIT 1;`.

## Write-key rotation (§9.6)

The write-key is baked into long-lived released binaries; it is anti-abuse, not
auth. To rotate without 401-orphaning already-released binaries, the Worker
accepts a **set** of valid keys via `POD_TELEMETRY_KEYS` (comma-separated),
each constant-time-compared. `POD_TELEMETRY_KEY` is the degenerate
one-element case.

Deprecation window:

1. `wrangler secret put POD_TELEMETRY_KEYS` set to `old_key,new_key`.
2. Ship the next release baking `new_key`.
3. After the prior release fleet ages out, set `POD_TELEMETRY_KEYS` to just
   `new_key` (or move it back to `POD_TELEMETRY_KEY`).
