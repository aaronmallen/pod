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
`../test/fixtures/telemetry/session_all_streams.json` and `crash_batch.json`
(the same files the Rust contract crate `src/telemetry_contract.rs` asserts).
Any unknown key anywhere is a rejection.

### Retention

A daily cron (`0 4 * * *`) runs the `scheduled` handler, which
`DELETE`s rows older than 90 days from both `events` and `crashes`, keyed on
`received_at`. The `DELETE` and the column `DEFAULT` use the identical
`strftime('%Y-%m-%dT%H:%M:%SZ', …)` form so the string comparison is correct.

## Layout

```text
telemetry/
  wrangler.toml             route + D1 binding + daily cron
  package.json  tsconfig.json
  src/index.ts              fetch + scheduled handlers
  src/contract.ts           closed-world validators (§6.1 / §9.4) + §5.6 reject
  src/db.ts                 envelope → D1 rows (§6.2), parameter-bound
  src/contract.test.ts      loads the golden fixtures; asserts accept + tamper-reject
  src/db.test.ts            asserts the row mapping
  migrations/0001_init.sql  events + crashes tables (NO IP column)
  README.md                 this file
```

`telemetry/` is committed; `.wrangler/` is gitignored.

## Toolchain (mise)

`wrangler` is pinned in `.config/mise.toml` under `[tools]` as
`"npm:wrangler"`, alongside the rest of the toolchain. `mise install`
provisions it for every contributor and CI — no `npx`, no global install.

```sh
mise install            # provisions wrangler, aube (and node)
aube install            # installs typescript + vitest (dev deps) → aube-lock.yaml
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
     --data @../test/fixtures/telemetry/session_all_streams.json   # → 204
   curl -i -X POST https://pod.aaronmallen.dev/telemetry/v1/ingest \
     -H 'Content-Type: application/json' \
     --data @../test/fixtures/telemetry/session_all_streams.json   # → 401 (no key)
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
