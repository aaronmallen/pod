// pod-telemetry Cloudflare Worker (spec mmmzstpq §9).
//
// Routes: pod.aaronmallen.dev/telemetry/*
//   * /telemetry/v1/ingest -> write path (POST, write-key auth)
//   * /telemetry/admin     -> read-only maintainer dashboard (task sqsmupwn,
//                             gated by Cloudflare Access in the CF dashboard)
//
// Pipeline is fail-closed, first-failure-wins, with empty response bodies:
//   1. path     -> 404 (anything but /telemetry/v1/ingest)
//   2. method   -> 405 (non-POST; Allow: POST)
//   3. key      -> 401 (constant-time compare against the valid-key set)
//   4. size     -> 413 (Content-Length > 256 KiB; read is also capped)
//   5. type     -> 415 (Content-Type not application/json)
//   6. parse    -> 400 (invalid JSON)
//   7. contract -> 400 (closed-world validation, §9.4)
//   8. crash    -> 400 (§5.6 fail-closed crash free-text reject)
//   9. insert   -> 500 on DB error, else 204
//
// The Worker NEVER reads or stores the client IP: there is no reference to
// CF-Connecting-IP or request.cf anywhere in this file (grep-verifiable).

import type { D1Database, ExecutionContext, ScheduledController } from "@cloudflare/workers-types";

import { crashFreeTextViolation, validateEnvelope } from "./contract";
import { buildStatements } from "./db";
import { renderDashboard } from "./render";
import { clampWindowDays, getDashboardStats } from "./stats";

export interface Env {
  DB: D1Database;
  SCHEMA_VERSION: string;
  /** Active write-key (the degenerate one-element rotation set). */
  POD_TELEMETRY_KEY?: string;
  /** Optional comma-separated rotation set (§9.6). */
  POD_TELEMETRY_KEYS?: string;
}

const INGEST_PATH = "/telemetry/v1/ingest";
const ADMIN_PATH = "/telemetry/admin";
const MAX_BODY_BYTES = 256 * 1024;

const RETENTION_DELETE = [
  "DELETE FROM events  WHERE received_at < strftime('%Y-%m-%dT%H:%M:%SZ','now','-90 days')",
  "DELETE FROM crashes WHERE received_at < strftime('%Y-%m-%dT%H:%M:%SZ','now','-90 days')",
];

/** Empty-body response with the given status. */
function status(code: number, headers?: Record<string, string>): Response {
  return new Response(null, { status: code, headers });
}

/** HTML response (used by the dashboard). Never cached; never indexed. */
function html(body: string, code = 200): Response {
  return new Response(body, {
    status: code,
    headers: {
      "Content-Type": "text/html; charset=utf-8",
      "Cache-Control": "no-store",
      "X-Robots-Tag": "noindex, nofollow",
    },
  });
}

/**
 * Constant-time string compare (length check + XOR-accumulate over char codes).
 * Avoids `===` so a timing side-channel can't probe the key byte-by-byte.
 */
function constantTimeEqual(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) {
    diff |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return diff === 0;
}

/** The set of accepted write-keys (POD_TELEMETRY_KEYS ∪ POD_TELEMETRY_KEY). */
function validKeys(env: Env): string[] {
  const keys: string[] = [];
  if (env.POD_TELEMETRY_KEYS) {
    for (const k of env.POD_TELEMETRY_KEYS.split(",")) {
      const trimmed = k.trim();
      if (trimmed.length > 0) keys.push(trimmed);
    }
  }
  if (env.POD_TELEMETRY_KEY && env.POD_TELEMETRY_KEY.length > 0) {
    keys.push(env.POD_TELEMETRY_KEY);
  }
  return keys;
}

/** Constant-time check that `presented` matches at least one accepted key. */
function keyAccepted(env: Env, presented: string | null): boolean {
  if (presented === null) return false;
  let accepted = false;
  // Compare against every key (no early return) so timing is independent of
  // which/whether a key matched.
  for (const k of validKeys(env)) {
    if (constantTimeEqual(presented, k)) accepted = true;
  }
  return accepted;
}

async function handleIngest(request: Request, env: Env): Promise<Response> {
  // 2. method
  if (request.method !== "POST") {
    return status(405, { Allow: "POST" });
  }

  // 3. write-key (constant-time)
  if (!keyAccepted(env, request.headers.get("X-Pod-Telemetry-Key"))) {
    return status(401);
  }

  // 4. size (declared, then enforced on the actual read)
  const declared = request.headers.get("Content-Length");
  if (declared !== null) {
    const n = Number(declared);
    if (Number.isFinite(n) && n > MAX_BODY_BYTES) return status(413);
  }

  // 5. content-type
  const contentType = request.headers.get("Content-Type") ?? "";
  if (!contentType.toLowerCase().startsWith("application/json")) {
    return status(415);
  }

  // 6. read (capped) + parse
  const raw = await request.text();
  if (new TextEncoder().encode(raw).length > MAX_BODY_BYTES) return status(413);
  let body: unknown;
  try {
    body = JSON.parse(raw);
  } catch {
    return status(400);
  }

  // 7. closed-world contract validation
  const maxSchema = Number(env.SCHEMA_VERSION);
  const result = validateEnvelope(body, maxSchema);
  if (!result.ok) return status(400);

  // 8. fail-closed crash free-text reject (§5.6)
  if (crashFreeTextViolation(result.envelope)) return status(400);

  // 9. map + INSERT in one batched transaction
  const stmts = buildStatements(env.DB, result.envelope);
  if (stmts.length > 0) {
    await env.DB.batch(stmts);
  }
  return status(204);
}

/**
 * Read-only maintainer dashboard (task sqsmupwn).
 *
 * Access control: this route is gated by Cloudflare Access (a single-user SSO
 * policy configured in the CF dashboard, NOT in code). The ingest write-key is
 * deliberately NOT reused here -- it ships inside released binaries and is
 * anti-abuse, not auth. By the time a request reaches the Worker, Access has
 * already authenticated the maintainer and attached a Cf-Access-Jwt-Assertion
 * header. We require that header's presence as cheap defense-in-depth (so a
 * misconfigured/removed Access policy fails closed to 403 rather than serving
 * aggregates openly), but Access remains the primary gate -- we do not verify
 * the JWT signature here.
 *
 * GET only; renders aggregate-only HTML. No mutations, SELECT only.
 */
async function handleAdmin(request: Request, env: Env): Promise<Response> {
  if (request.method !== "GET") return status(405, { Allow: "GET" });

  // Defense-in-depth: Access injects this header on every authenticated request.
  // Its absence means the request did not traverse the Access policy.
  if (!request.headers.get("Cf-Access-Jwt-Assertion")) return status(403);

  const url = new URL(request.url);
  const daysParam = url.searchParams.get("days");
  const windowDays = clampWindowDays(daysParam === null ? null : Number(daysParam));

  const stats = await getDashboardStats(env.DB, windowDays);
  return html(renderDashboard(stats));
}

export default {
  async fetch(request: Request, env: Env, _ctx: ExecutionContext): Promise<Response> {
    try {
      const url = new URL(request.url);
      // Read-only maintainer dashboard (Cloudflare Access-gated).
      if (url.pathname === ADMIN_PATH) return await handleAdmin(request, env);
      // 1. path (ingest + 404-for-everything-else, unchanged)
      if (url.pathname !== INGEST_PATH) return status(404);
      return await handleIngest(request, env);
    } catch {
      // Top-level guard: never leak a stack trace.
      return status(500);
    }
  },

  async scheduled(_controller: ScheduledController, env: Env, _ctx: ExecutionContext): Promise<void> {
    // Daily retention sweep (§6.5 / §9.5): drop rows older than 90 days. Keyed
    // on received_at; same strftime form as the column DEFAULT so the string
    // comparison is correct.
    await env.DB.batch(RETENTION_DELETE.map((sql) => env.DB.prepare(sql)));
  },
};
