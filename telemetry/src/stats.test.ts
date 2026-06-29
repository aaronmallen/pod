// stats.ts tests. The aggregation layer is unit-testable independently of the
// HTTP route: each function takes a `D1Database` and returns a typed result.
//
// We drive them with a tiny in-memory fake D1 that holds seeded `events` and
// `crashes` rows and answers the exact `prepare(SQL).bind(...).all()/.first()`
// shapes stats.ts issues, applying the WHERE/GROUP BY in JS. This keeps the
// fixtures consistent with the column shape db.test.ts/contract.test.ts pin
// (anon_id, stream, os, event_kind, app_version, ... ) without a real database.

import { describe, expect, it } from "vitest";

import {
  clampWindowDays,
  getCrashGroups,
  getDashboardStats,
  getInstallTrend,
  getLanguageBreakdown,
  getPerformance,
  getPlatformBreakdown,
  getSchemaMix,
  getTopFeatures,
  getVersionAdoption,
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
    { anon_id: "a", schema: 1, app_version: "0.9.4", message: "boom", backtrace: JSON.stringify(["frame_a", "frame_b"]), context_log: JSON.stringify(["log line 1"]), received_at: day("2026-06-20") },
    { anon_id: "b", schema: 1, app_version: "0.9.4", message: "boom", backtrace: JSON.stringify(["frame_c"]), context_log: null, received_at: day("2026-06-22") },
    { anon_id: "c", schema: 1, app_version: "0.9.3", message: "kapow", backtrace: null, context_log: null, received_at: day("2026-06-21") },
  ];
}

/**
 * Minimal fake D1 that recognizes each SQL string stats.ts issues (by stable
 * substrings) and computes the aggregate over the seeded rows in JS.
 */
function fakeDb(events: EventRow[], crashes: CrashRow[]) {
  function distinct<T>(xs: T[]): number {
    return new Set(xs).size;
  }

  function run(sql: string, params: unknown[]): { results: unknown[]; first: unknown } {
    const s = sql.replace(/\s+/g, " ").trim();

    // Install trend: daily distinct anon_id
    if (s.includes("substr(received_at,1,10) AS day")) {
      const byDay = new Map<string, string[]>();
      for (const e of events) {
        const d = e.received_at.slice(0, 10);
        if (!byDay.has(d)) byDay.set(d, []);
        byDay.get(d)!.push(e.anon_id);
      }
      const results = [...byDay.entries()]
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([d, ids]) => ({ day: d, installs: distinct(ids) }));
      return { results, first: null };
    }
    // Install trend total
    if (s.includes("COUNT(DISTINCT anon_id) AS total")) {
      return { results: [], first: { total: distinct(events.map((e) => e.anon_id)) } };
    }
    // Language breakdown (more specific than the platform branch below, which
    // also matches WHERE stream='environment'; keep this first).
    if (s.includes("GROUP BY app_language")) {
      const groups = new Map<string | null, string[]>();
      for (const e of events.filter((e) => e.stream === "environment")) {
        if (!groups.has(e.app_language)) groups.set(e.app_language, []);
        groups.get(e.app_language)!.push(e.anon_id);
      }
      const results = [...groups.entries()].map(([app_language, ids]) => ({
        app_language,
        installs: distinct(ids),
      }));
      return { results, first: null };
    }
    // Platform breakdown
    if (s.includes("WHERE stream='environment'")) {
      const groups = new Map<string, { row: EventRow; ids: string[] }>();
      for (const e of events.filter((e) => e.stream === "environment")) {
        const k = `${e.os}|${e.os_version}|${e.arch}|${e.window_size}|${e.screen_size}`;
        if (!groups.has(k)) groups.set(k, { row: e, ids: [] });
        groups.get(k)!.ids.push(e.anon_id);
      }
      const results = [...groups.values()].map(({ row, ids }) => ({
        os: row.os,
        os_version: row.os_version,
        arch: row.arch,
        window_size: row.window_size,
        screen_size: row.screen_size,
        installs: distinct(ids),
      }));
      return { results, first: null };
    }
    // Top features
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
    // Performance
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
    // Version adoption
    if (s.includes("GROUP BY app_version ORDER BY installs")) {
      const groups = new Map<string, string[]>();
      for (const e of events) {
        if (!groups.has(e.app_version)) groups.set(e.app_version, []);
        groups.get(e.app_version)!.push(e.anon_id);
      }
      const results = [...groups.entries()]
        .map(([app_version, ids]) => ({ app_version, installs: distinct(ids) }))
        .sort((a, b) => b.installs - a.installs);
      return { results, first: null };
    }
    // Crash groups
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
    // Crash detail (most recent occurrence)
    if (s.includes("SELECT backtrace, context_log FROM crashes")) {
      const [app_version, message] = params as [string, string];
      const match = crashes
        .filter((c) => c.app_version === app_version && c.message === message)
        .sort((a, b) => b.received_at.localeCompare(a.received_at))[0];
      return { results: [], first: match ? { backtrace: match.backtrace, context_log: match.context_log } : null };
    }
    // Schema mix (events)
    if (s.includes("SELECT schema, COUNT(*) AS n FROM events")) {
      const groups = new Map<number, number>();
      for (const e of events) groups.set(e.schema, (groups.get(e.schema) ?? 0) + 1);
      return { results: [...groups.entries()].map(([schema, n]) => ({ schema, n })), first: null };
    }
    // Schema mix (crashes)
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

describe("getInstallTrend", () => {
  it("returns the window-wide distinct total and per-day points", async () => {
    const db = fakeDb(seedEvents(), seedCrashes());
    const trend = await getInstallTrend(db, 30);
    expect(trend.windowDays).toBe(30);
    expect(trend.totalDistinct).toBe(3); // a, b, c
    expect(trend.points.length).toBeGreaterThan(0);
  });
});

describe("getPlatformBreakdown", () => {
  it("groups by os/version/arch/window_size/screen_size and keeps the unknown bucket", async () => {
    const rows = await getPlatformBreakdown(fakeDb(seedEvents(), seedCrashes()));
    expect(rows).toHaveLength(2); // macos/15/aarch64 (2 installs) + linux/unknown
    const macos = rows.find((r) => r.os === "macos");
    expect(macos!.window_size).toBe("2560x1440");
    expect(macos!.screen_size).toBe("3440x1440");
    const linux = rows.find((r) => r.os === "linux");
    expect(linux).toBeDefined();
    expect(linux!.os_version).toBe("unknown"); // unknown bucket surfaced, not hidden
    expect(linux!.window_size).toBe("unknown");
    expect(linux!.screen_size).toBe("unknown"); // NULL screen_size collapses to the unknown bucket
  });
});

describe("getLanguageBreakdown", () => {
  it("groups distinct installs by app_language and keeps the unknown bucket", async () => {
    const rows = await getLanguageBreakdown(fakeDb(seedEvents(), seedCrashes()));
    expect(rows).toHaveLength(2); // de (a, c) + unknown (b, NULL app_language)
    const de = rows.find((r) => r.app_language === "de");
    expect(de!.installs).toBe(2);
    const unknown = rows.find((r) => r.app_language === "unknown");
    expect(unknown).toBeDefined(); // NULL app_language surfaced, not hidden
    expect(unknown!.installs).toBe(1);
  });
});

describe("getTopFeatures", () => {
  it("counts usage events and splits feature_toggle on/off", async () => {
    const rows = await getTopFeatures(fakeDb(seedEvents(), seedCrashes()));
    const toggle = rows.find((r) => r.name === "skills.plan_optimizer");
    expect(toggle).toBeDefined();
    expect(toggle!.event_kind).toBe("feature_toggle");
    expect(toggle!.count).toBe(2);
    expect(toggle!.toggledOn).toBe(1); // one on, one off
    const view = rows.find((r) => r.name === "wallet");
    expect(view!.toggledOn).toBeNull(); // view_open carries no toggle
  });
});

describe("getVersionAdoption", () => {
  it("counts distinct installs per app_version", async () => {
    const rows = await getVersionAdoption(fakeDb(seedEvents(), seedCrashes()));
    const v094 = rows.find((r) => r.app_version === "0.9.4");
    expect(v094!.installs).toBe(2); // a, b
    const v093 = rows.find((r) => r.app_version === "0.9.3");
    expect(v093!.installs).toBe(1); // c
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
    // most recent (2026-06-22) occurrence's backtrace
    expect(boom!.backtrace).toEqual(["frame_c"]);
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
    expect(stats.platforms.length).toBe(2);
    expect(stats.languages.length).toBe(2);
    expect(stats.features.length).toBeGreaterThan(0);
    expect(stats.versions.length).toBe(2);
    expect(stats.performance.length).toBe(1);
    expect(stats.crashes.length).toBe(2);
    expect(stats.schemas.length).toBe(1);
    expect(typeof stats.generatedAt).toBe("string");
  });
});
