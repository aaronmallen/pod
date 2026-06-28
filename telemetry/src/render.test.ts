// render.ts tests. The renderer is a pure function from typed stats to a single
// self-contained HTML document. We assert: it is a complete doc with inline CSS
// and no external assets; DB-derived text is HTML-escaped (injection guard);
// the schema panel only appears when more than one contract version is present;
// and the "unknown" platform bucket is rendered, not hidden.

import { describe, expect, it } from "vitest";

import { escapeHtml, renderDashboard } from "./render";
import type { DashboardStats } from "./stats";

function baseStats(over: Partial<DashboardStats> = {}): DashboardStats {
  return {
    generatedAt: "2026-06-27T00:00:00.000Z",
    windowDays: 30,
    installs: { windowDays: 30, totalDistinct: 3, points: [
      { day: "2026-06-25", installs: 1 },
      { day: "2026-06-26", installs: 2 },
    ] },
    platforms: [
      { os: "macos", os_version: "15", arch: "aarch64", window_size: "2560x1440", screen_size: "3440x1440", installs: 2 },
      { os: "linux", os_version: "unknown", arch: "x86_64", window_size: "unknown", screen_size: "unknown", installs: 1 },
    ],
    features: [
      { event_kind: "view_open", name: "wallet", count: 5, toggledOn: null },
      { event_kind: "feature_toggle", name: "skills.x", count: 4, toggledOn: 3 },
    ],
    versions: [{ app_version: "0.9.4", installs: 2 }],
    performance: [{ name: "wallet", samples: 2, avgLoadMs: 150, avgFrameP95Ms: 15, avgHeapMb: 85 }],
    crashes: [
      { app_version: "0.9.4", message: "boom", count: 2, lastSeen: "2026-06-26T00:00:00Z", backtrace: ["frame_a"], context_log: [] },
    ],
    schemas: [{ schema: 1, events: 9, crashes: 2 }],
    ...over,
  };
}

describe("escapeHtml", () => {
  it("escapes the dangerous characters", () => {
    expect(escapeHtml(`<script>"&'`)).toBe("&lt;script&gt;&quot;&amp;&#39;");
  });
});

describe("renderDashboard", () => {
  it("is a complete self-contained HTML document with inline style", () => {
    const out = renderDashboard(baseStats());
    expect(out.startsWith("<!doctype html>")).toBe(true);
    expect(out).toContain("<style>");
    expect(out).toContain("</html>");
    // No external resources: no <script>, no link rel=stylesheet, no http(s) src/href.
    expect(out).not.toMatch(/<script/i);
    expect(out).not.toMatch(/<link\b/i);
    expect(out).not.toMatch(/https?:\/\//);
  });

  it("renders an inline svg sparkline for the install trend", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("<svg");
    expect(out).toContain("<polyline");
  });

  it("labels installs honestly (not 'active users')", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("distinct installs that opted in");
    expect(out).not.toMatch(/active users/i);
  });

  it("escapes hostile DB-derived text instead of injecting markup", () => {
    const out = renderDashboard(
      baseStats({
        crashes: [
          {
            app_version: "0.9.4",
            message: "<img src=x onerror=alert(1)>",
            count: 1,
            lastSeen: "2026-06-26T00:00:00Z",
            backtrace: ["<b>frame</b>"],
            context_log: [],
          },
        ],
      }),
    );
    expect(out).not.toContain("<img src=x");
    expect(out).toContain("&lt;img src=x");
    expect(out).toContain("&lt;b&gt;frame&lt;/b&gt;");
  });

  it("surfaces the literal 'unknown' platform bucket", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("unknown");
    expect(out).toContain("linux");
  });

  it("renders the window size and screen size columns", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("Window size");
    expect(out).toContain("Screen size");
    expect(out).toContain("3440x1440");
    expect(out).not.toContain(">Display<");
  });

  it("omits the schema panel when only one schema is present", () => {
    const out = renderDashboard(baseStats());
    expect(out).not.toContain("Schema mix");
  });

  it("shows the schema panel when contract versions diverge", () => {
    const out = renderDashboard(
      baseStats({ schemas: [
        { schema: 1, events: 9, crashes: 2 },
        { schema: 2, events: 4, crashes: 0 },
      ] }),
    );
    expect(out).toContain("Schema mix");
  });

  it("renders an empty-state note when the trend window has no data", () => {
    const out = renderDashboard(baseStats({ installs: { windowDays: 30, totalDistinct: 0, points: [] } }));
    expect(out).toContain("No events received");
  });
});
