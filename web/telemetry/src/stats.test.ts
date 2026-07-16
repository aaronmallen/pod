// stats.ts tests. The aggregation layer is unit-testable independently of the
// HTTP route: each function takes a `D1Database` and returns a typed result.
//
// We drive them with a tiny in-memory fake D1 that holds seeded `events` and
// `crashes` rows and answers the exact `prepare(SQL).bind(...).all()/.first()`
// shapes stats.ts issues, applying the WHERE/GROUP BY (and the trailing-day
// window, parsed from the bound offset) in JS. This keeps the fixtures
// consistent with the column shape db.test.ts/contract.test.ts pin
// (anon_id, stream, os, event_kind, app_version, ... ) without a real database.

import { describe, expect, it } from "vitest";

import {
  clampWindowDays,
  getCrashCount,
  getCrashGroups,
  getDashboardStats,
  getTrends,
  getLanguageBreakdown,
  getOsBuckets,
  getPerformance,
  getPlatformBreakdown,
  getScreenBreakdown,
  getSchemaMix,
  getTopFeatures,
  getVersionAdoption,
  osFamily,
} from "./stats";

interface EventRow {
  anon_id: string;
  schema: number;
  app_version: string;
  stream: string;
  event_kind: string | null;
  name: string | null;
  toggle_on: number | null;
  load_ms: number | null;
  frame_p95_ms: number | null;
  heap_mb: number | null;
  os: string | null;
  os_version: string | null;
  arch: string | null;
  window_size: string | null;
  screen_size: string | null;
  locale: string | null;
  app_language: string | null;
  received_at: string;
}

interface CrashRow {
  anon_id: string;
  schema: number;
  app_version: string;
  message: string;
  backtrace: string | null;
  context_log: string | null;
  received_at: string;
}

const todayIso = new Date().toISOString();
const day = (d: string) => `${d}T12:00:00Z`;
const daysAgo = (n: number) => new Date(Date.now() - n * 86400000).toISOString();

function seedEvents(): EventRow[] {
  const base = {
    schema: 1,
    load_ms: null,
    frame_p95_ms: null,
    heap_mb: null,
    os: null,
    os_version: null,
    arch: null,
    window_size: null,
    screen_size: null,
    locale: null,
    app_language: null,
    toggle_on: null,
    event_kind: null,
    name: null,
  };
  return [
    // environment rows (one per install)
    { ...base, anon_id: "a", app_version: "0.9.4", stream: "environment", os: "macos", os_version: "15", arch: "aarch64", window_size: "2560x1440", screen_size: "3440x1440", locale: "en", app_language: "de", received_at: todayIso },
    // legacy row: screen_size + app_language NULL (the client predates the fields) -> "unknown" bucket
    { ...base, anon_id: "b", app_version: "0.9.4", stream: "environment", os: "linux", os_version: "unknown", arch: "x86_64", window_size: "unknown", screen_size: null, locale: "unknown", app_language: null, received_at: todayIso },
    { ...base, anon_id: "c", app_version: "0.9.3", stream: "environment", os: "macos", os_version: "15", arch: "aarch64", window_size: "2560x1440", screen_size: "3440x1440", locale: "en", app_language: "de", received_at: todayIso },
    // usage rows
    { ...base, anon_id: "a", app_version: "0.9.4", stream: "usage", event_kind: "view_open", name: "wallet", received_at: todayIso },
    { ...base, anon_id: "b", app_version: "0.9.4", stream: "usage", event_kind: "view_open", name: "wallet", received_at: todayIso },
    { ...base, anon_id: "a", app_version: "0.9.4", stream: "usage", event_kind: "feature_toggle", name: "skills.plan_optimizer", toggle_on: 1, received_at: todayIso },
    { ...base, anon_id: "b", app_version: "0.9.4", stream: "usage", event_kind: "feature_toggle", name: "skills.plan_optimizer", toggle_on: 0, received_at: todayIso },
    // performance rows
    { ...base, anon_id: "a", app_version: "0.9.4", stream: "performance", name: "wallet", load_ms: 100, frame_p95_ms: 10, heap_mb: 80, received_at: todayIso },
    { ...base, anon_id: "b", app_version: "0.9.4", stream: "performance", name: "wallet", load_ms: 200, frame_p95_ms: 20, heap_mb: 90, received_at: todayIso },
  ];
}

function seedCrashes(): CrashRow[] {
  return [
    { anon_id: "a", schema: 1, app_version: "0.9.4", message: "boom", backtrace: JSON.stringify(["frame_a", "frame_b"]), context_log: JSON.stringify(["log line 1"]), received_at: daysAgo(6) },
    { anon_id: "b", schema: 1, app_version: "0.9.4", message: "boom", backtrace: JSON.stringify(["frame_c"]), context_log: null, received_at: daysAgo(4) },
    { anon_id: "c", schema: 1, app_version: "0.9.3", message: "kapow", backtrace: null, context_log: null, received_at: daysAgo(5) },
  ];
}

function cutoffIso(offset: unknown): string {
  const m = /-(\d+) days/.exec(String(offset));
  const days = m ? Number(m[1]) : 0;
  return new Date(Date.now() - days * 86400000).toISOString();
}

/**
 * Minimal fake D1 that recognizes each SQL string stats.ts issues (by stable
 * substrings) and computes the aggregate over the seeded rows in JS, honoring
 * the trailing-day window parsed from the first bound param.
 */
function fakeDb(events: EventRow[], crashes: CrashRow[]) {
  function distinct<T>(xs: T[]): number {
    return new Set(xs).size;
  }

  // Each install's latest environment row within the window — the snapshot the
  // platform/screen/language panels attribute the install to (mirrors the
  // ROW_NUMBER(... PARTITION BY anon_id ORDER BY received_at DESC) subquery).
  function latestEnvPerAnon(cutoff: string): EventRow[] {
    const latest = new Map<string, EventRow>();
    for (const e of events.filter((e) => e.stream === "environment" && e.received_at >= cutoff)) {
      const cur = latest.get(e.anon_id);
      if (!cur || e.received_at > cur.received_at) latest.set(e.anon_id, e);
    }
    return [...latest.values()];
  }

  function run(sql: string, params: unknown[]): { results: unknown[]; first: unknown } {
    const s = sql.replace(/\s+/g, " ").trim();

    // Version adoption: current version per anon within the window. Other
    // panels also use ROW_NUMBER now, so key on the app_version grouping.
    if (s.includes("ROW_NUMBER") && s.includes("GROUP BY app_version")) {
      const cutoff = cutoffIso(params[0]);
      const latest = new Map<string, { v: string; at: string }>();
      for (const e of events.filter((e) => e.received_at >= cutoff)) {
        const cur = latest.get(e.anon_id);
        if (!cur || e.received_at > cur.at) latest.set(e.anon_id, { v: e.app_version, at: e.received_at });
      }
      const counts = new Map<string, number>();
      for (const { v } of latest.values()) counts.set(v, (counts.get(v) ?? 0) + 1);
      const results = [...counts.entries()]
        .map(([app_version, installs]) => ({ app_version, installs }))
        .sort((a, b) => b.installs - a.installs);
      return { results, first: null };
    }
    // Trends, new-installs line: first-ever event day per anon_id, windowed.
    if (s.includes("substr(MIN(received_at),1,10)")) {
      const cutoff = cutoffIso(params[0]).slice(0, 10);
      const firstSeen = new Map<string, string>();
      for (const e of events) {
        const d = e.received_at.slice(0, 10);
        const cur = firstSeen.get(e.anon_id);
        if (!cur || d < cur) firstSeen.set(e.anon_id, d);
      }
      const byDay = new Map<string, number>();
      for (const d of firstSeen.values()) {
        if (d >= cutoff) byDay.set(d, (byDay.get(d) ?? 0) + 1);
      }
      const results = [...byDay.entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([d, installs]) => ({ day: d, installs }));
      return { results, first: null };
    }
    // Trends, usage line: daily distinct anon_id over the window.
    if (s.includes("substr(received_at,1,10) AS day")) {
      const cutoff = cutoffIso(params[0]);
      const byDay = new Map<string, string[]>();
      for (const e of events.filter((e) => e.received_at >= cutoff)) {
        const d = e.received_at.slice(0, 10);
        if (!byDay.has(d)) byDay.set(d, []);
        byDay.get(d)!.push(e.anon_id);
      }
      const results = [...byDay.entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([d, ids]) => ({ day: d, active: distinct(ids) }));
      return { results, first: null };
    }
    // Windowed total crash count.
    if (s.includes("AS total FROM crashes")) {
      const cutoff = cutoffIso(params[0]);
      return { results: [], first: { total: crashes.filter((c) => c.received_at >= cutoff).length } };
    }
    // Install trend total.
    if (s.includes("COUNT(DISTINCT anon_id) AS total")) {
      const cutoff = cutoffIso(params[0]);
      return { results: [], first: { total: distinct(events.filter((e) => e.received_at >= cutoff).map((e) => e.anon_id)) } };
    }
    // OS buckets (windowed, environment). Keyed on its unique projection so it
    // doesn't intercept the platform/screen/language snapshot queries, which
    // also window on `stream='environment' AND received_at`.
    if (s.includes("SELECT os, COUNT(DISTINCT anon_id)")) {
      const cutoff = cutoffIso(params[0]);
      const groups = new Map<string | null, string[]>();
      for (const e of events.filter((e) => e.stream === "environment" && e.received_at >= cutoff)) {
        if (!groups.has(e.os)) groups.set(e.os, []);
        groups.get(e.os)!.push(e.anon_id);
      }
      const results = [...groups.entries()].map(([os, ids]) => ({ os, installs: distinct(ids) }));
      return { results, first: null };
    }
    // Language breakdown: current app_language per install within the window.
    if (s.includes("GROUP BY app_language")) {
      const cutoff = cutoffIso(params[0]);
      const groups = new Map<string | null, string[]>();
      for (const e of latestEnvPerAnon(cutoff)) {
        if (!groups.has(e.app_language)) groups.set(e.app_language, []);
        groups.get(e.app_language)!.push(e.anon_id);
      }
      const results = [...groups.entries()].map(([app_language, ids]) => ({
        app_language,
        installs: distinct(ids),
      }));
      return { results, first: null };
    }
    // Platform breakdown (os/os_version/arch): current snapshot per install.
    if (s.includes("GROUP BY os, os_version, arch")) {
      const cutoff = cutoffIso(params[0]);
      const groups = new Map<string, { row: EventRow; ids: string[] }>();
      for (const e of latestEnvPerAnon(cutoff)) {
        const k = `${e.os}|${e.os_version}|${e.arch}`;
        if (!groups.has(k)) groups.set(k, { row: e, ids: [] });
        groups.get(k)!.ids.push(e.anon_id);
      }
      const results = [...groups.values()].map(({ row, ids }) => ({
        os: row.os,
        os_version: row.os_version,
        arch: row.arch,
        installs: distinct(ids),
      }));
      return { results, first: null };
    }
    // Screen breakdown (window_size/screen_size): current snapshot per install.
    if (s.includes("GROUP BY window_size, screen_size")) {
      const cutoff = cutoffIso(params[0]);
      const groups = new Map<string, { row: EventRow; ids: string[] }>();
      for (const e of latestEnvPerAnon(cutoff)) {
        const k = `${e.window_size}|${e.screen_size}`;
        if (!groups.has(k)) groups.set(k, { row: e, ids: [] });
        groups.get(k)!.ids.push(e.anon_id);
      }
      const results = [...groups.values()].map(({ row, ids }) => ({
        window_size: row.window_size,
        screen_size: row.screen_size,
        installs: distinct(ids),
      }));
      return { results, first: null };
    }
    // Top features.
    if (s.includes("WHERE stream='usage'")) {
      const groups = new Map<string, { event_kind: string; name: string; count: number; toggled_on: number }>();
      for (const e of events.filter((e) => e.stream === "usage")) {
        const k = `${e.event_kind}|${e.name}`;
        if (!groups.has(k)) groups.set(k, { event_kind: e.event_kind!, name: e.name!, count: 0, toggled_on: 0 });
        const g = groups.get(k)!;
        g.count += 1;
        if (e.toggle_on === 1) g.toggled_on += 1;
      }
      return { results: [...groups.values()].sort((a, b) => b.count - a.count), first: null };
    }
    // Performance.
    if (s.includes("WHERE stream='performance'")) {
      const groups = new Map<string, EventRow[]>();
      for (const e of events.filter((e) => e.stream === "performance")) {
        if (!groups.has(e.name!)) groups.set(e.name!, []);
        groups.get(e.name!)!.push(e);
      }
      const avg = (xs: (number | null)[]) => {
        const vals = xs.filter((x): x is number => x !== null);
        return vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : null;
      };
      const results = [...groups.entries()].map(([name, rows]) => ({
        name,
        samples: rows.length,
        avg_load_ms: avg(rows.map((r) => r.load_ms)),
        avg_frame_p95_ms: avg(rows.map((r) => r.frame_p95_ms)),
        avg_heap_mb: avg(rows.map((r) => r.heap_mb)),
      }));
      return { results, first: null };
    }
    // Crash groups.
    if (s.includes("FROM crashes GROUP BY app_version, message")) {
      const groups = new Map<string, { app_version: string; message: string; count: number; last_seen: string }>();
      for (const c of crashes) {
        const k = `${c.app_version}|${c.message}`;
        if (!groups.has(k)) groups.set(k, { app_version: c.app_version, message: c.message, count: 0, last_seen: "" });
        const g = groups.get(k)!;
        g.count += 1;
        if (c.received_at > g.last_seen) g.last_seen = c.received_at;
      }
      return { results: [...groups.values()].sort((a, b) => b.count - a.count), first: null };
    }
    // Crash detail (most recent occurrence).
    if (s.includes("SELECT backtrace, context_log FROM crashes")) {
      const [app_version, message] = params as [string, string];
      const match = crashes
        .filter((c) => c.app_version === app_version && c.message === message)
        .sort((a, b) => b.received_at.localeCompare(a.received_at))[0];
      return { results: [], first: match ? { backtrace: match.backtrace, context_log: match.context_log } : null };
    }
    // Schema mix (events).
    if (s.includes("SELECT schema, COUNT(*) AS n FROM events")) {
      const groups = new Map<number, number>();
      for (const e of events) groups.set(e.schema, (groups.get(e.schema) ?? 0) + 1);
      return { results: [...groups.entries()].map(([schema, n]) => ({ schema, n })), first: null };
    }
    // Schema mix (crashes).
    if (s.includes("SELECT schema, COUNT(*) AS n FROM crashes")) {
      const groups = new Map<number, number>();
      for (const c of crashes) groups.set(c.schema, (groups.get(c.schema) ?? 0) + 1);
      return { results: [...groups.entries()].map(([schema, n]) => ({ schema, n })), first: null };
    }

    throw new Error(`unrecognized SQL in fakeDb: ${s}`);
  }

  return {
    prepare(sql: string) {
      const exec = (params: unknown[]) => run(sql, params);
      const make = (params: unknown[]) => ({
        bind: (...p: unknown[]) => make(p),
        all: async () => ({ results: exec(params).results, success: true, meta: {} }),
        first: async () => exec(params).first,
      });
      return make([]);
    },
  } as unknown as Parameters<typeof getDashboardStats>[0];
}

describe("clampWindowDays", () => {
  it("defaults non-finite / null to 30", () => {
    expect(clampWindowDays(null)).toBe(30);
    expect(clampWindowDays(Number.NaN)).toBe(30);
  });
  it("clamps below 1 and above 365", () => {
    expect(clampWindowDays(0)).toBe(1);
    expect(clampWindowDays(-5)).toBe(1);
    expect(clampWindowDays(10000)).toBe(365);
  });
  it("truncates a valid value", () => {
    expect(clampWindowDays(45.9)).toBe(45);
  });
});

describe("osFamily", () => {
  it("maps raw os strings to coarse families", () => {
    expect(osFamily("windows")).toBe("windows");
    expect(osFamily("Windows_NT")).toBe("windows");
    expect(osFamily("macos")).toBe("mac");
    expect(osFamily("darwin")).toBe("mac");
    expect(osFamily("linux")).toBe("linux");
  });
  it("buckets unknown and null into other", () => {
    expect(osFamily("unknown")).toBe("other");
    expect(osFamily("haiku")).toBe("other");
    expect(osFamily(null)).toBe("other");
  });
});

describe("getTrends", () => {
  it("returns the window-wide distinct total and per-day points with both lines", async () => {
    const db = fakeDb(seedEvents(), seedCrashes());
    const trend = await getTrends(db, 30);
    expect(trend.windowDays).toBe(30);
    expect(trend.totalDistinct).toBe(3); // a, b, c
    expect(trend.points.length).toBeGreaterThan(0);
    const today = trend.points[trend.points.length - 1];
    expect(today.usage).toBe(3); // a, b, c all have events today
    expect(today.installs).toBe(3); // and today is each anon_id's first-seen day
  });

  it("counts an install as new only on its first-seen day, usage on every active day", async () => {
    const events = seedEvents();
    // Give "a" an older first-seen day inside the window.
    events.push({ ...events[0], stream: "usage", event_kind: "view_open", name: "roster", received_at: daysAgo(5) });
    const trend = await getTrends(fakeDb(events, []), 30);
    const oldDay = daysAgo(5).slice(0, 10);
    const today = todayIso.slice(0, 10);
    const oldPoint = trend.points.find((p) => p.day === oldDay)!;
    const todayPoint = trend.points.find((p) => p.day === today)!;
    expect(oldPoint.installs).toBe(1); // a first seen here
    expect(oldPoint.usage).toBe(1);
    expect(todayPoint.installs).toBe(2); // only b and c are new today
    expect(todayPoint.usage).toBe(3); // a still counts as active today
  });
});

describe("getCrashCount", () => {
  it("counts crash rows within the window", async () => {
    const total = await getCrashCount(fakeDb(seedEvents(), seedCrashes()), 30);
    expect(total).toBe(3);
  });
  it("excludes crashes older than the window", async () => {
    const old = seedCrashes().map((c) => ({ ...c, received_at: day("2000-01-01") }));
    const total = await getCrashCount(fakeDb(seedEvents(), old), 30);
    expect(total).toBe(0);
  });
});

describe("getOsBuckets", () => {
  it("buckets distinct installs by coarse OS family, summing to the total", async () => {
    const events = seedEvents();
    // an install on an unmapped os falls into "other"
    events.push({ ...events[0], anon_id: "d", os: "freebsd" });
    const buckets = await getOsBuckets(fakeDb(events, seedCrashes()), 30);

    const by = (f: string) => buckets.find((b) => b.family === f)!.installs;
    expect(by("mac")).toBe(2); // a, c
    expect(by("linux")).toBe(1); // b
    expect(by("windows")).toBe(0);
    expect(by("other")).toBe(1); // d (freebsd -> other)

    const total = buckets.reduce((s, b) => s + b.installs, 0);
    expect(total).toBe(4); // a, b, c, d
  });
});

describe("getPlatformBreakdown", () => {
  it("groups by os/os_version/arch and keeps the unknown bucket", async () => {
    const rows = await getPlatformBreakdown(fakeDb(seedEvents(), seedCrashes()), 30);
    expect(rows).toHaveLength(2); // macos/15/aarch64 (2 installs) + linux/unknown
    const macos = rows.find((r) => r.os === "macos");
    expect(macos!.installs).toBe(2); // a + c
    const linux = rows.find((r) => r.os === "linux");
    expect(linux!.os_version).toBe("unknown"); // unknown bucket surfaced, not hidden
    expect(linux!.arch).toBe("x86_64");
    // platform rows no longer carry screen geometry
    expect("window_size" in (rows[0] as unknown as Record<string, unknown>)).toBe(false);
  });

  it("counts an install once, at its current snapshot, when its platform changed", async () => {
    const events = seedEvents();
    // "a" reported macos 15 today (seed) but had macos 14 earlier — it must not
    // appear in both buckets. Its latest snapshot (macos 15) wins.
    events.push({
      ...events[0],
      os_version: "14",
      received_at: new Date(Date.now() - 3 * 86400000).toISOString(),
    });
    const rows = await getPlatformBreakdown(fakeDb(events, seedCrashes()), 30);
    // Still two buckets: macos/15 (a + c) and linux/unknown (b). No macos/14 row.
    expect(rows.find((r) => r.os === "macos" && r.os_version === "14")).toBeUndefined();
    const macos15 = rows.find((r) => r.os === "macos" && r.os_version === "15");
    expect(macos15!.installs).toBe(2); // a (current) + c, a counted exactly once
    const total = rows.reduce((sum, r) => sum + r.installs, 0);
    expect(total).toBe(3); // a, b, c — buckets sum to the install total, no double-count
  });
});

describe("getScreenBreakdown", () => {
  it("groups distinct installs by window_size/screen_size and keeps unknown", async () => {
    const rows = await getScreenBreakdown(fakeDb(seedEvents(), seedCrashes()), 30);
    expect(rows).toHaveLength(2);
    const hi = rows.find((r) => r.window_size === "2560x1440");
    expect(hi!.screen_size).toBe("3440x1440");
    expect(hi!.installs).toBe(2); // a + c share the geometry
    const unk = rows.find((r) => r.window_size === "unknown");
    expect(unk!.screen_size).toBe("unknown"); // NULL screen_size collapses to unknown
  });
});

describe("getLanguageBreakdown", () => {
  it("groups distinct installs by app_language and keeps the unknown bucket", async () => {
    const rows = await getLanguageBreakdown(fakeDb(seedEvents(), seedCrashes()), 30);
    expect(rows).toHaveLength(2); // de (a, c) + unknown (b, NULL app_language)
    const de = rows.find((r) => r.app_language === "de");
    expect(de!.installs).toBe(2);
    const unknown = rows.find((r) => r.app_language === "unknown");
    expect(unknown!.installs).toBe(1);
  });
});

describe("getTopFeatures", () => {
  it("counts usage events and splits feature_toggle on/off", async () => {
    const rows = await getTopFeatures(fakeDb(seedEvents(), seedCrashes()));
    const toggle = rows.find((r) => r.name === "skills.plan_optimizer");
    expect(toggle!.event_kind).toBe("feature_toggle");
    expect(toggle!.count).toBe(2);
    expect(toggle!.toggledOn).toBe(1); // one on, one off
    const view = rows.find((r) => r.name === "wallet");
    expect(view!.toggledOn).toBeNull(); // view_open carries no toggle
  });
});

describe("getVersionAdoption", () => {
  it("counts each install at its current (latest) version", async () => {
    const rows = await getVersionAdoption(fakeDb(seedEvents(), seedCrashes()), 30);
    const v094 = rows.find((r) => r.app_version === "0.9.4");
    expect(v094!.installs).toBe(2); // a, b
    const v093 = rows.find((r) => r.app_version === "0.9.3");
    expect(v093!.installs).toBe(1); // c
  });

  it("counts an upgraded install only toward its latest version", async () => {
    const recent = new Date(Date.now() - 2 * 86400000).toISOString();
    const older = new Date(Date.now() - 5 * 86400000).toISOString();
    const events: EventRow[] = [
      { ...seedEvents()[0], anon_id: "u", app_version: "0.6.9", received_at: older },
      { ...seedEvents()[0], anon_id: "u", app_version: "0.6.10", received_at: recent },
    ];
    const rows = await getVersionAdoption(fakeDb(events, []), 30);

    expect(rows.find((r) => r.app_version === "0.6.9")).toBeUndefined();
    expect(rows.find((r) => r.app_version === "0.6.10")!.installs).toBe(1);
  });
});

describe("getPerformance", () => {
  it("averages load/frame/heap per view", async () => {
    const rows = await getPerformance(fakeDb(seedEvents(), seedCrashes()));
    const wallet = rows.find((r) => r.name === "wallet");
    expect(wallet!.samples).toBe(2);
    expect(wallet!.avgLoadMs).toBe(150); // (100+200)/2
    expect(wallet!.avgFrameP95Ms).toBe(15);
    expect(wallet!.avgHeapMb).toBe(85);
  });
});

describe("getCrashGroups", () => {
  it("groups by version+message and parses backtrace/context_log JSON arrays", async () => {
    const rows = await getCrashGroups(fakeDb(seedEvents(), seedCrashes()));
    const boom = rows.find((r) => r.message === "boom");
    expect(boom!.count).toBe(2);
    expect(boom!.backtrace).toEqual(["frame_c"]); // most recent (daysAgo(4)) occurrence
    expect(boom!.context_log).toEqual([]); // null context_log -> []
    const kapow = rows.find((r) => r.message === "kapow");
    expect(kapow!.backtrace).toEqual([]); // null backtrace -> []
  });
});

describe("getSchemaMix", () => {
  it("returns a single row when only one schema is present", async () => {
    const rows = await getSchemaMix(fakeDb(seedEvents(), seedCrashes()));
    expect(rows).toHaveLength(1);
    expect(rows[0].schema).toBe(1);
  });
  it("returns multiple rows when contract versions diverge", async () => {
    const ev = seedEvents();
    ev.push({ ...ev[0], schema: 2 });
    const rows = await getSchemaMix(fakeDb(ev, seedCrashes()));
    expect(rows.map((r) => r.schema)).toEqual([1, 2]);
  });
});

describe("getDashboardStats", () => {
  it("assembles every panel with a generatedAt timestamp", async () => {
    const stats = await getDashboardStats(fakeDb(seedEvents(), seedCrashes()), 30);
    expect(stats.windowDays).toBe(30);
    expect(stats.installs.totalDistinct).toBe(3);
    expect(stats.crashTotal).toBe(3);
    expect(stats.osBuckets).toHaveLength(4); // windows/mac/linux/other
    expect(stats.osBuckets.reduce((s, b) => s + b.installs, 0)).toBe(3);
    expect(stats.platforms.length).toBe(2);
    expect(stats.screens.length).toBe(2);
    expect(stats.languages.length).toBe(2);
    expect(stats.features.length).toBeGreaterThan(0);
    expect(stats.versions.length).toBe(2);
    expect(stats.performance.length).toBe(1);
    expect(stats.crashes.length).toBe(2);
    expect(stats.schemas.length).toBe(1);
    expect(typeof stats.generatedAt).toBe("string");
  });
});
