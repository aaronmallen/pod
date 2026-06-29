// Read-only D1 aggregation queries for the maintainer dashboard (task sqsmupwn).
//
// This is the DATA half of the dashboard: pure async functions that take a
// `D1Database` and return typed, aggregate-only result objects. No HTTP, no
// HTML, no mutation -- every statement is a parameter-bound `SELECT`, mirroring
// the ingest path's `db.prepare(SQL).bind(...).all()/.first()` style.
//
// All counts are aggregates; no per-row PII (anon_id, session) is returned to
// the caller. `anon_id` is sha256(machine_id) -- per-install, NOT a "user" --
// so install-distinctness is labeled honestly upstream in render.ts.
//
// The literal string "unknown" (never NULL) is what the client sends for an
// unresolved os_version/window_size/screen_size/locale; these rows are surfaced
// explicitly, never filtered out. (screen_size is also NULL for legacy rows
// from clients that predate the field; those collapse to "unknown" too.)

import type { D1Database } from "@cloudflare/workers-types";

/** Default trailing window (days) for the active-installs sparkline. */
export const DEFAULT_WINDOW_DAYS = 30;

/** One day's distinct-install count for the sparkline. */
export interface InstallTrendPoint {
  /** `YYYY-MM-DD` (UTC), from `substr(received_at,1,10)`. */
  day: string;
  /** `COUNT(DISTINCT anon_id)` received that day. */
  installs: number;
}

export interface InstallTrend {
  windowDays: number;
  /** Distinct installs over the whole window (not the sum of daily points). */
  totalDistinct: number;
  /** One point per day that had at least one event, ascending by day. */
  points: InstallTrendPoint[];
}

/** A platform bucket: a distinct (os, os_version, arch, window_size, screen_size) combination. */
export interface PlatformRow {
  os: string;
  os_version: string;
  arch: string;
  window_size: string;
  screen_size: string;
  /** Distinct installs reporting this exact environment combination. */
  installs: number;
}

/** A usage-feature bucket. */
export interface FeatureRow {
  event_kind: string;
  name: string;
  /** Number of usage events (NOT distinct installs). */
  count: number;
  /**
   * For `feature_toggle` rows: how many of those events were `on` (toggle_on=1).
   * `null` for non-toggle kinds where the column is unused.
   */
  toggledOn: number | null;
}

/** An app-version adoption bucket. */
export interface VersionRow {
  app_version: string;
  /** Distinct installs seen on this version. */
  installs: number;
}

/** A chosen-UI-language bucket: distinct installs per `app_language`. */
export interface LanguageRow {
  app_language: string;
  /** Distinct installs reporting this chosen UI language. */
  installs: number;
}

/** Per-view performance aggregate. */
export interface PerformanceRow {
  name: string;
  samples: number;
  avgLoadMs: number | null;
  avgFrameP95Ms: number | null;
  avgHeapMb: number | null;
}

/** One crash group (app_version + message), expandable to its samples. */
export interface CrashGroup {
  app_version: string;
  message: string;
  count: number;
  lastSeen: string;
  /** Parsed backtrace of the most recent occurrence (JSON-array TEXT in D1). */
  backtrace: string[];
  /** Parsed context_log of the most recent occurrence (JSON-array TEXT). */
  context_log: string[];
}

/** A distinct wire-contract `schema` value and its row volume. */
export interface SchemaRow {
  schema: number;
  events: number;
  crashes: number;
}

/** The full dashboard payload: every panel's typed, aggregate-only data. */
export interface DashboardStats {
  generatedAt: string;
  windowDays: number;
  installs: InstallTrend;
  platforms: PlatformRow[];
  languages: LanguageRow[];
  features: FeatureRow[];
  versions: VersionRow[];
  performance: PerformanceRow[];
  crashes: CrashGroup[];
  /** Distinct contract schemas present; surfaced only when more than one. */
  schemas: SchemaRow[];
}

/**
 * Clamp the requested window to a sane range so a hostile/garbled `?days=`
 * can't ask for an unbounded scan. Falls back to the default for non-finite
 * or out-of-range input.
 */
export function clampWindowDays(raw: number | null): number {
  if (raw === null || !Number.isFinite(raw)) return DEFAULT_WINDOW_DAYS;
  const n = Math.trunc(raw);
  if (n < 1) return 1;
  if (n > 365) return 365;
  return n;
}

/** SQLite `received_at` cutoff for the trailing `days`-day window. */
function windowCutoffSql(): string {
  // Same strftime form as the column DEFAULT, so the string comparison holds.
  return "strftime('%Y-%m-%dT%H:%M:%SZ','now',?)";
}

/**
 * Panel 1: distinct opted-in installs, bucketed by UTC day over the window,
 * plus the window-wide distinct total. NOT "active users": `anon_id` is
 * per-install (sha256(machine_id)).
 */
export async function getInstallTrend(db: D1Database, windowDays: number): Promise<InstallTrend> {
  const offset = `-${windowDays} days`;
  const dailySql = `SELECT substr(received_at,1,10) AS day, COUNT(DISTINCT anon_id) AS installs
    FROM events
    WHERE received_at >= ${windowCutoffSql()}
    GROUP BY day
    ORDER BY day ASC`;
  const totalSql = `SELECT COUNT(DISTINCT anon_id) AS total
    FROM events
    WHERE received_at >= ${windowCutoffSql()}`;

  const daily = await db.prepare(dailySql).bind(offset).all<{ day: string; installs: number }>();
  const total = await db.prepare(totalSql).bind(offset).first<{ total: number }>();

  return {
    windowDays,
    totalDistinct: total?.total ?? 0,
    points: (daily.results ?? []).map((r) => ({ day: r.day, installs: r.installs })),
  };
}

/**
 * Panel 2: platform mix from environment rows, grouped by the full
 * (os, os_version, arch, window_size, screen_size) tuple. The literal "unknown"
 * bucket is kept (and a NULL screen_size from a legacy row collapses to it).
 */
export async function getPlatformBreakdown(db: D1Database): Promise<PlatformRow[]> {
  const sql = `SELECT os, os_version, arch, window_size, screen_size, COUNT(DISTINCT anon_id) AS installs
    FROM events
    WHERE stream='environment'
    GROUP BY os, os_version, arch, window_size, screen_size
    ORDER BY installs DESC, os ASC`;
  const r = await db.prepare(sql).all<{
    os: string;
    os_version: string;
    arch: string;
    window_size: string;
    screen_size: string | null;
    installs: number;
  }>();
  return (r.results ?? []).map((row) => ({
    os: row.os ?? "unknown",
    os_version: row.os_version ?? "unknown",
    arch: row.arch ?? "unknown",
    window_size: row.window_size ?? "unknown",
    screen_size: row.screen_size ?? "unknown",
    installs: row.installs,
  }));
}

/**
 * Panel 2b: chosen UI language mix from environment rows, grouped by
 * `app_language`. A NULL value (legacy clients that predate the field)
 * collapses to the literal "unknown" bucket, surfaced rather than hidden.
 */
export async function getLanguageBreakdown(db: D1Database): Promise<LanguageRow[]> {
  const sql = `SELECT app_language, COUNT(DISTINCT anon_id) AS installs
    FROM events
    WHERE stream='environment'
    GROUP BY app_language
    ORDER BY installs DESC, app_language ASC`;
  const r = await db.prepare(sql).all<{ app_language: string | null; installs: number }>();
  return (r.results ?? []).map((row) => ({
    app_language: row.app_language ?? "unknown",
    installs: row.installs,
  }));
}

/**
 * Panel 3: top features, grouped by (event_kind, name) over the usage stream.
 * For feature_toggle rows, also report how many events were toggled on.
 */
export async function getTopFeatures(db: D1Database, limit = 50): Promise<FeatureRow[]> {
  const sql = `SELECT event_kind, name,
      COUNT(*) AS count,
      SUM(CASE WHEN toggle_on=1 THEN 1 ELSE 0 END) AS toggled_on
    FROM events
    WHERE stream='usage'
    GROUP BY event_kind, name
    ORDER BY count DESC, name ASC
    LIMIT ?`;
  const r = await db.prepare(sql).bind(limit).all<{
    event_kind: string;
    name: string;
    count: number;
    toggled_on: number;
  }>();
  return (r.results ?? []).map((row) => ({
    event_kind: row.event_kind ?? "unknown",
    name: row.name ?? "unknown",
    count: row.count,
    toggledOn: row.event_kind === "feature_toggle" ? (row.toggled_on ?? 0) : null,
  }));
}

/** Panel 4: app-version adoption -- distinct installs per app_version. */
export async function getVersionAdoption(db: D1Database): Promise<VersionRow[]> {
  const sql = `SELECT app_version, COUNT(DISTINCT anon_id) AS installs
    FROM events
    GROUP BY app_version
    ORDER BY installs DESC, app_version DESC`;
  const r = await db.prepare(sql).all<{ app_version: string; installs: number }>();
  return (r.results ?? []).map((row) => ({
    app_version: row.app_version ?? "unknown",
    installs: row.installs,
  }));
}

/** Panel 5: per-view performance averages over the performance stream. */
export async function getPerformance(db: D1Database): Promise<PerformanceRow[]> {
  const sql = `SELECT name,
      COUNT(*) AS samples,
      AVG(load_ms) AS avg_load_ms,
      AVG(frame_p95_ms) AS avg_frame_p95_ms,
      AVG(heap_mb) AS avg_heap_mb
    FROM events
    WHERE stream='performance'
    GROUP BY name
    ORDER BY samples DESC, name ASC`;
  const r = await db.prepare(sql).all<{
    name: string;
    samples: number;
    avg_load_ms: number | null;
    avg_frame_p95_ms: number | null;
    avg_heap_mb: number | null;
  }>();
  return (r.results ?? []).map((row) => ({
    name: row.name ?? "unknown",
    samples: row.samples,
    avgLoadMs: roundOrNull(row.avg_load_ms),
    avgFrameP95Ms: roundOrNull(row.avg_frame_p95_ms),
    avgHeapMb: roundOrNull(row.avg_heap_mb),
  }));
}

/** Round a possibly-null AVG to one decimal place, preserving null. */
function roundOrNull(v: number | null): number | null {
  if (v === null || !Number.isFinite(v)) return null;
  return Math.round(v * 10) / 10;
}

/**
 * Parse a JSON-array TEXT column into a string[]; tolerate null/garbage by
 * returning []. Crash backtrace / context_log are stored as JSON-array TEXT.
 */
function parseJsonStringArray(raw: string | null): string[] {
  if (typeof raw !== "string" || raw.length === 0) return [];
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((x): x is string => typeof x === "string");
  } catch {
    return [];
  }
}

/**
 * Panel 6: crash groups by (app_version, message) -- covered by
 * idx_crashes_group. Each group carries the most recent occurrence's parsed
 * backtrace + context_log for the expandable detail view.
 */
export async function getCrashGroups(db: D1Database, limit = 100): Promise<CrashGroup[]> {
  const sql = `SELECT app_version, message,
      COUNT(*) AS count,
      MAX(received_at) AS last_seen
    FROM crashes
    GROUP BY app_version, message
    ORDER BY count DESC, last_seen DESC
    LIMIT ?`;
  const groups = await db.prepare(sql).bind(limit).all<{
    app_version: string;
    message: string;
    count: number;
    last_seen: string;
  }>();

  const detailSql = `SELECT backtrace, context_log
    FROM crashes
    WHERE app_version=? AND message=?
    ORDER BY received_at DESC
    LIMIT 1`;

  const out: CrashGroup[] = [];
  for (const g of groups.results ?? []) {
    const detail = await db
      .prepare(detailSql)
      .bind(g.app_version, g.message)
      .first<{ backtrace: string | null; context_log: string | null }>();
    out.push({
      app_version: g.app_version ?? "unknown",
      message: g.message,
      count: g.count,
      lastSeen: g.last_seen,
      backtrace: parseJsonStringArray(detail?.backtrace ?? null),
      context_log: parseJsonStringArray(detail?.context_log ?? null),
    });
  }
  return out;
}

/**
 * Schema mix: distinct contract `schema` values across both fact tables. With
 * only schema=1 present this returns a single row (render.ts then omits the
 * panel), so different contract versions are never silently co-aggregated.
 */
export async function getSchemaMix(db: D1Database): Promise<SchemaRow[]> {
  const eventsSql = `SELECT schema, COUNT(*) AS n FROM events GROUP BY schema`;
  const crashesSql = `SELECT schema, COUNT(*) AS n FROM crashes GROUP BY schema`;
  const ev = await db.prepare(eventsSql).all<{ schema: number; n: number }>();
  const cr = await db.prepare(crashesSql).all<{ schema: number; n: number }>();

  const bySchema = new Map<number, SchemaRow>();
  for (const row of ev.results ?? []) {
    bySchema.set(row.schema, { schema: row.schema, events: row.n, crashes: 0 });
  }
  for (const row of cr.results ?? []) {
    const existing = bySchema.get(row.schema);
    if (existing) existing.crashes = row.n;
    else bySchema.set(row.schema, { schema: row.schema, events: 0, crashes: row.n });
  }
  return [...bySchema.values()].sort((a, b) => a.schema - b.schema);
}

/**
 * Run every panel query and assemble the full dashboard payload. Pure data:
 * the HTTP handler calls this, then hands the result to render.ts.
 */
export async function getDashboardStats(db: D1Database, windowDays: number): Promise<DashboardStats> {
  const [installs, platforms, languages, features, versions, performance, crashes, schemas] = await Promise.all([
    getInstallTrend(db, windowDays),
    getPlatformBreakdown(db),
    getLanguageBreakdown(db),
    getTopFeatures(db),
    getVersionAdoption(db),
    getPerformance(db),
    getCrashGroups(db),
    getSchemaMix(db),
  ]);

  return {
    generatedAt: new Date().toISOString(),
    windowDays,
    installs,
    platforms,
    languages,
    features,
    versions,
    performance,
    crashes,
    schemas,
  };
}
