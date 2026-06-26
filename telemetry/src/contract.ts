// Closed-world telemetry contract validators (spec mmmzstpq §6.1 / §6.4 / §9.4).
//
// This file is the Worker half of the frozen wire contract. It MUST accept
// EXACTLY the shape the Rust contract types (src/telemetry_contract.rs) emit and
// the two golden fixtures pin:
//
//   * test/fixtures/telemetry/session_all_streams.json
//   * test/fixtures/telemetry/crash_batch.json
//
// "Closed world" = every object's key set is checked; an unknown key anywhere is
// a rejection, not a silently-ignored extra. Disabled streams are OMITTED keys,
// never `null`.

/** Maximum per-token / per-string length (route tokens, names). */
export const MAX_TOKEN_LEN = 512;
/** Crash message cap (§6.1.2 / §9.4): truncate, do not reject. */
export const MAX_CRASH_MESSAGE_LEN = 2048;
/** context_log line cap (§6.1.2): truncate, do not reject. */
export const MAX_CONTEXT_LOG_LINE_LEN = 1024;

/** Lowercase sha256 hex anon-id: `^[0-9a-f]{64}$`. */
const ANON_ID_RE = /^[0-9a-f]{64}$/;
/** Per-process session tag: `^s_[0-9a-f]{8}$`. */
const SESSION_RE = /^s_[0-9a-f]{8}$/;

const USAGE_EVENT_KINDS = ["view_open", "feature_toggle", "sub_section"] as const;
const SESSION_STREAM_KEYS = ["usage", "performance", "environment"] as const;

/** Result of validation: ok, or the first violation that wins (fail-closed). */
export type ValidationResult =
  | { ok: true; envelope: Envelope }
  | { ok: false; reason: string };

export type UsageEventKind = (typeof USAGE_EVENT_KINDS)[number];

export interface App {
  version: string;
  git_sha?: string;
  build_date?: string;
}

export interface UsageEvent {
  t: string;
  kind: UsageEventKind;
  name: string;
  on?: boolean;
}

export interface UsageStream {
  events: UsageEvent[];
}

export interface PerformanceViewEntry {
  name: string;
  load_ms: number;
  frame_p95_ms: number;
}

export interface PerformanceStream {
  views: PerformanceViewEntry[];
  heap_mb: number;
}

export interface EnvironmentStream {
  os: string;
  os_version: string;
  arch: string;
  display: string;
  locale: string;
}

export interface CrashReport {
  crashed_at: string;
  message: string;
  location?: string;
  backtrace?: string[];
  context_log?: string[];
}

export interface CrashStream {
  reports: CrashReport[];
}

export interface Streams {
  usage?: UsageStream;
  performance?: PerformanceStream;
  environment?: EnvironmentStream;
  crashes?: CrashStream;
}

export interface Envelope {
  schema: number;
  kind: "session" | "crash";
  id: string;
  session: string;
  app: App;
  sent_at: string;
  streams: Streams;
}

// ---- small helpers -------------------------------------------------------

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function isInt(v: unknown): v is number {
  return typeof v === "number" && Number.isInteger(v);
}

function isNonEmptyString(v: unknown): v is string {
  return typeof v === "string" && v.length > 0;
}

/** Reject any key on `obj` not present in `allowed`. */
function onlyKeys(obj: Record<string, unknown>, allowed: readonly string[]): string | null {
  for (const k of Object.keys(obj)) {
    if (!allowed.includes(k)) return k;
  }
  return null;
}

/**
 * RFC3339 parse check. We require a `Z` or numeric offset form that
 * `Date.parse` accepts; a bare/invalid string fails. Mirrors the Rust side
 * accepting the golden `2026-06-25T14:32:08Z` shape.
 */
function isRfc3339(v: unknown): v is string {
  if (typeof v !== "string" || v.length === 0) return false;
  // Require date + 'T' + time, ending in Z or ±HH:MM.
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$/.test(v)) {
    return false;
  }
  return !Number.isNaN(Date.parse(v));
}

// ---- per-stream validators ----------------------------------------------

function validateUsage(usage: unknown): string | null {
  if (!isObject(usage)) return "usage must be an object";
  const extra = onlyKeys(usage, ["events"]);
  if (extra) return `unknown usage key: ${extra}`;
  if (!Array.isArray(usage.events)) return "usage.events must be an array";
  for (const ev of usage.events) {
    if (!isObject(ev)) return "usage.events[] must be an object";
    const extraEv = onlyKeys(ev, ["t", "kind", "name", "on"]);
    if (extraEv) return `unknown usage event key: ${extraEv}`;
    if (!isRfc3339(ev.t)) return "usage event t must be RFC3339";
    if (typeof ev.kind !== "string" || !USAGE_EVENT_KINDS.includes(ev.kind as UsageEventKind)) {
      return "usage event kind not in allow-list";
    }
    if (!isNonEmptyString(ev.name) || ev.name.length > MAX_TOKEN_LEN) {
      return "usage event name invalid";
    }
    // `on` is present iff kind === feature_toggle.
    if (ev.kind === "feature_toggle") {
      if (typeof ev.on !== "boolean") return "feature_toggle requires on:bool";
    } else if ("on" in ev) {
      return "on present on a non-feature_toggle event";
    }
  }
  return null;
}

function validatePerformance(perf: unknown): string | null {
  if (!isObject(perf)) return "performance must be an object";
  const extra = onlyKeys(perf, ["views", "heap_mb"]);
  if (extra) return `unknown performance key: ${extra}`;
  if (!Array.isArray(perf.views)) return "performance.views must be an array";
  for (const view of perf.views) {
    if (!isObject(view)) return "performance.views[] must be an object";
    const extraView = onlyKeys(view, ["name", "load_ms", "frame_p95_ms"]);
    if (extraView) return `unknown performance view key: ${extraView}`;
    if (!isNonEmptyString(view.name) || view.name.length > MAX_TOKEN_LEN) {
      return "performance view name invalid";
    }
    if (!isInt(view.load_ms) || view.load_ms < 0) return "load_ms must be a non-negative int";
    if (!isInt(view.frame_p95_ms) || view.frame_p95_ms < 0) return "frame_p95_ms must be a non-negative int";
  }
  if (!isInt(perf.heap_mb) || perf.heap_mb < 0) return "heap_mb must be a non-negative int";
  return null;
}

function validateEnvironment(env: unknown): string | null {
  if (!isObject(env)) return "environment must be an object";
  const keys = ["os", "os_version", "arch", "display", "locale"];
  const extra = onlyKeys(env, keys);
  if (extra) return `unknown environment key: ${extra}`;
  for (const k of keys) {
    if (typeof env[k] !== "string") return `environment.${k} must be a string`;
  }
  return null;
}

function validateCrashes(crashes: unknown): string | null {
  if (!isObject(crashes)) return "crashes must be an object";
  const extra = onlyKeys(crashes, ["reports"]);
  if (extra) return `unknown crashes key: ${extra}`;
  if (!Array.isArray(crashes.reports) || crashes.reports.length === 0) {
    return "crashes.reports must be a non-empty array";
  }
  for (const report of crashes.reports) {
    if (!isObject(report)) return "crashes.reports[] must be an object";
    const extraR = onlyKeys(report, ["crashed_at", "message", "location", "backtrace", "context_log"]);
    if (extraR) return `unknown crash report key: ${extraR}`;
    if (!isRfc3339(report.crashed_at)) return "crash crashed_at must be RFC3339";
    if (!isNonEmptyString(report.message)) return "crash message must be a non-empty string";
    if ("location" in report && typeof report.location !== "string") {
      return "crash location must be a string";
    }
    if ("backtrace" in report) {
      if (!Array.isArray(report.backtrace) || !report.backtrace.every((f) => typeof f === "string")) {
        return "crash backtrace must be string[]";
      }
    }
    if ("context_log" in report) {
      if (!Array.isArray(report.context_log) || !report.context_log.every((l) => typeof l === "string")) {
        return "crash context_log must be string[]";
      }
    }
  }
  return null;
}

// ---- envelope ------------------------------------------------------------

function validateApp(app: unknown): string | null {
  if (!isObject(app)) return "app must be an object";
  const extra = onlyKeys(app, ["version", "git_sha", "build_date"]);
  if (extra) return `unknown app key: ${extra}`;
  if (!isNonEmptyString(app.version)) return "app.version must be a non-empty string";
  if ("git_sha" in app && typeof app.git_sha !== "string") return "app.git_sha must be a string";
  if ("build_date" in app && typeof app.build_date !== "string") return "app.build_date must be a string";
  return null;
}

function validateStreams(kind: "session" | "crash", streams: unknown): string | null {
  if (!isObject(streams)) return "streams must be an object";
  // No null stream values anywhere.
  for (const [k, v] of Object.entries(streams)) {
    if (v === null) return `stream ${k} must not be null`;
  }
  if (kind === "session") {
    const extra = onlyKeys(streams, SESSION_STREAM_KEYS);
    if (extra) return `unknown session stream key: ${extra}`;
    if ("crashes" in streams) return "session batch must not carry crashes";
    if ("usage" in streams) {
      const r = validateUsage(streams.usage);
      if (r) return r;
    }
    if ("performance" in streams) {
      const r = validatePerformance(streams.performance);
      if (r) return r;
    }
    if ("environment" in streams) {
      const r = validateEnvironment(streams.environment);
      if (r) return r;
    }
    return null;
  }
  // kind === "crash": exactly { crashes }.
  const keys = Object.keys(streams);
  if (keys.length !== 1 || keys[0] !== "crashes") {
    return "crash batch streams must be exactly { crashes }";
  }
  return validateCrashes(streams.crashes);
}

/**
 * Validate a parsed JSON body against the frozen contract.
 *
 * @param body     the parsed request JSON (untrusted)
 * @param maxSchema highest `schema` the Worker knows (env.SCHEMA_VERSION)
 */
export function validateEnvelope(body: unknown, maxSchema: number): ValidationResult {
  if (!isObject(body)) return { ok: false, reason: "body must be an object" };

  const extra = onlyKeys(body, ["schema", "kind", "id", "session", "app", "sent_at", "streams"]);
  if (extra) return { ok: false, reason: `unknown envelope key: ${extra}` };

  if (!isInt(body.schema) || body.schema < 1 || body.schema > maxSchema) {
    return { ok: false, reason: "schema out of range" };
  }
  if (body.kind !== "session" && body.kind !== "crash") {
    return { ok: false, reason: "kind must be session|crash" };
  }
  if (typeof body.id !== "string" || !ANON_ID_RE.test(body.id)) {
    return { ok: false, reason: "id must match ^[0-9a-f]{64}$" };
  }
  if (typeof body.session !== "string" || !SESSION_RE.test(body.session)) {
    return { ok: false, reason: "session must match ^s_[0-9a-f]{8}$" };
  }
  const appErr = validateApp(body.app);
  if (appErr) return { ok: false, reason: appErr };
  if (!isRfc3339(body.sent_at)) return { ok: false, reason: "sent_at must be RFC3339" };

  const streamsErr = validateStreams(body.kind, body.streams);
  if (streamsErr) return { ok: false, reason: streamsErr };

  return { ok: true, envelope: body as unknown as Envelope };
}

// ---- §5.6 fail-closed crash free-text reject -----------------------------

const PATH_TOKEN_RE = /\/Users\/|\/home\/|C:\\Users\\/;
const URL_WITH_PATH_RE = /https?:\/\/[^\s/]+\/\S/;
const DIGIT_RUN_RE = /\d{7,}/;

/**
 * Return the offending string if any crash free-text field (message, every
 * backtrace frame, every context_log line) contains an `@`, an absolute home
 * path token, a URL with a path, or a >=7-digit run (§5.6 / §9.3 step 8). A
 * client-scrub miss then fails closed (400) instead of landing in D1.
 */
export function crashFreeTextViolation(envelope: Envelope): string | null {
  const crashes = envelope.streams.crashes;
  if (!crashes) return null;
  const offends = (s: string): boolean =>
    s.includes("@") || PATH_TOKEN_RE.test(s) || URL_WITH_PATH_RE.test(s) || DIGIT_RUN_RE.test(s);
  for (const report of crashes.reports) {
    if (offends(report.message)) return "crash message failed free-text reject";
    for (const frame of report.backtrace ?? []) {
      if (offends(frame)) return "crash backtrace frame failed free-text reject";
    }
    for (const line of report.context_log ?? []) {
      if (offends(line)) return "crash context_log line failed free-text reject";
    }
  }
  return null;
}
