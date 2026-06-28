// Envelope -> D1 rows mapping (spec mmmzstpq §6.2).
//
// All values are parameter-bound (`?`), never string-concatenated, so no field
// can break out of its column. No IP is read, derived, or stored anywhere.

import type { D1Database, D1PreparedStatement } from "@cloudflare/workers-types";

import type { EnvironmentStream, Envelope } from "./contract";

const INSERT_EVENT = `INSERT INTO events
  (anon_id, session, schema, app_version, git_sha,
    stream, event_kind, name, toggle_on,
    load_ms, frame_p95_ms, heap_mb,
    os, os_version, arch, window_size, screen_size, locale, event_at)
  VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`;

const INSERT_CRASH = `INSERT INTO crashes
  (anon_id, session, schema, app_version, git_sha,
    message, location, backtrace, context_log, crashed_at)
  VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`;

/**
 * Build the batched, parameter-bound INSERT statements for one envelope.
 *
 * kind:"session" -> one row per usage event, one row per performance view
 *   (heap_mb copied onto every perf row), and exactly one environment row.
 * kind:"crash"   -> one crashes row per report; crashed_at is taken from the
 *   report (never wall-clock-now) so a late arrival is preserved.
 */
export function buildStatements(db: D1Database, env: Envelope): D1PreparedStatement[] {
  const stmts: D1PreparedStatement[] = [];
  const gitSha = env.app.git_sha ?? null;

  if (env.kind === "session") {
    const usage = env.streams.usage;
    if (usage) {
      for (const ev of usage.events) {
        const toggleOn = ev.kind === "feature_toggle" ? (ev.on ? 1 : 0) : null;
        stmts.push(
          db.prepare(INSERT_EVENT).bind(
            env.id, env.session, env.schema, env.app.version, gitSha,
            "usage", ev.kind, ev.name, toggleOn,
            null, null, null,
            null, null, null, null, null, null, ev.t,
          ),
        );
      }
    }

    const perf = env.streams.performance;
    if (perf) {
      for (const view of perf.views) {
        stmts.push(
          db.prepare(INSERT_EVENT).bind(
            env.id, env.session, env.schema, env.app.version, gitSha,
            "performance", null, view.name, null,
            view.load_ms, view.frame_p95_ms, perf.heap_mb,
            null, null, null, null, null, null, env.sent_at,
          ),
        );
      }
    }

    const e = env.streams.environment;
    if (e) {
      // Legacy clients send `display`; new clients send `window_size`. Coalesce
      // so both record into the window_size column. screen_size is null when an
      // (old) client omits it.
      const windowSize = (e as EnvironmentStream & { display?: string }).display ?? e.window_size;
      const screenSize = e.screen_size ?? null;
      stmts.push(
        db.prepare(INSERT_EVENT).bind(
          env.id, env.session, env.schema, env.app.version, gitSha,
          "environment", null, null, null,
          null, null, null,
          e.os, e.os_version, e.arch, windowSize, screenSize, e.locale, env.sent_at,
        ),
      );
    }
    return stmts;
  }

  // kind === "crash"
  const crashes = env.streams.crashes;
  if (crashes) {
    for (const report of crashes.reports) {
      stmts.push(
        db.prepare(INSERT_CRASH).bind(
          env.id, env.session, env.schema, env.app.version, gitSha,
          report.message,
          report.location ?? null,
          report.backtrace ? JSON.stringify(report.backtrace) : null,
          report.context_log ? JSON.stringify(report.context_log) : null,
          report.crashed_at,
        ),
      );
    }
  }
  return stmts;
}
