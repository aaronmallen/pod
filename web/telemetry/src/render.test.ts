// render.ts tests. The renderer is a pure function from typed stats to a single
// HTML document. We assert: it is a complete doc with inline CSS and no
// JavaScript (the one permitted external request is the Google Fonts <link>);
// DB-derived text is HTML-escaped (injection guard); the schema panel only
// appears when more than one contract version is present; the OS pie renders;
// and the CSS-only two-level tab structure is present.

import { describe, expect, it } from "vitest";

import { escapeHtml, renderDashboard } from "./render";
import type { DashboardStats } from "./stats";

function baseStats(over: Partial<DashboardStats> = {}): DashboardStats {
  return {
    generatedAt: "2026-06-27T00:00:00.000Z",
    windowDays: 30,
    installs: { windowDays: 30, totalDistinct: 3, points: [
      { day: "2026-06-25", installs: 1, usage: 1 },
      { day: "2026-06-26", installs: 2, usage: 3 },
    ] },
    crashTotal: 4,
    osBuckets: [
      { family: "mac", installs: 2 },
      { family: "linux", installs: 1 },
      { family: "windows", installs: 0 },
      { family: "other", installs: 1 },
    ],
    platforms: [
      { os: "macos", os_version: "15", arch: "aarch64", installs: 2 },
      { os: "linux", os_version: "unknown", arch: "x86_64", installs: 1 },
    ],
    screens: [
      { window_size: "2560x1440", screen_size: "3440x1440", installs: 2 },
      { window_size: "unknown", screen_size: "unknown", installs: 1 },
    ],
    languages: [
      { app_language: "de", installs: 2 },
      { app_language: "unknown", installs: 1 },
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
  it("is a complete HTML document with inline style and no JavaScript", () => {
    const out = renderDashboard(baseStats());
    expect(out.startsWith("<!doctype html>")).toBe(true);
    expect(out).toContain("<style>");
    expect(out).toContain("</html>");
    // No script ever; the only permitted external request is Google Fonts.
    expect(out).not.toMatch(/<script/i);
    expect(out).toContain("fonts.googleapis.com");
    // No external request other than the Google Fonts CSS / static host.
    for (const url of out.match(/https?:\/\/[^"'\s]+/g) ?? []) {
      expect(url).toMatch(/fonts\.(googleapis|gstatic)\.com/);
    }
  });

  it("loads Space Grotesk and JetBrains Mono via a font link", () => {
    const out = renderDashboard(baseStats());
    expect(out).toMatch(/<link\b[^>]*fonts\.googleapis\.com/);
    expect(out).toContain("Space+Grotesk");
    expect(out).toContain("JetBrains+Mono");
    expect(out).toContain("Space Grotesk");
  });

  it("sources the palette from the marketing tokens", () => {
    const out = renderDashboard(baseStats());
    // plasma accent + sunk-paper background from tokens.ts.
    expect(out).toContain("#3FB8DB");
    expect(out).toContain("#0E0F12");
  });

  it("renders a two-level CSS-only tab structure", () => {
    const out = renderDashboard(baseStats());
    // Top-level radio tabs, one per section, in order.
    for (const id of ["tab-overview", "tab-platforms", "tab-screens", "tab-features", "tab-languages"]) {
      expect(out).toContain(`id="${id}"`);
    }
    expect(out).toContain('name="tab"');
    // Nested second-level sub-tabs inside Features.
    expect(out).toContain('name="ftab"');
    expect(out).toContain('id="ftab-top"');
    expect(out).toContain('id="ftab-perf"');
    // Panels switch on :checked siblings, with zero script.
    expect(out).toContain(":checked");
    expect(out).not.toMatch(/<script/i);
  });

  it("renders the overview headline tiles and window control", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("distinct installs");
    expect(out).toContain("crashes (last 30 days)");
    expect(out).toMatch(/name="days"/);
  });

  it("renders an inline svg pie for the OS split with a legend", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain('class="pie"');
    expect(out).toContain("pie-slice");
    expect(out).toContain("stroke-dasharray");
    // Legend labels for the non-zero families.
    expect(out).toContain("macOS");
    expect(out).toContain("Linux");
    expect(out).toContain("Other");
  });

  it("renders the Trends card with both sparkline series and a legend", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("<h2>Trends</h2>");
    expect(out).not.toContain("Install trend");
    expect(out).toContain("<svg");
    expect(out).toContain('<polyline points');
    expect(out).toContain('class="line usage"');
    expect(out).toContain("New installs / day");
    expect(out).toContain("Usage / day");
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

  it("renders the platforms tab with os/version/arch (no screen columns)", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("Platform breakdown");
    expect(out).toContain("linux");
    expect(out).toContain("aarch64");
    expect(out).not.toContain("<th>Window size</th><th>Screen size</th><th class=\"n\">Installs</th>");
  });

  it("renders the screens tab with window/screen sizes", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("Screen breakdown");
    expect(out).toContain("3440x1440");
    expect(out).toContain(">Window size<");
    expect(out).toContain(">Screen size<");
  });

  it("surfaces the literal 'unknown' bucket", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("unknown");
  });

  it("renders the language breakdown panel", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("Language breakdown");
    expect(out).toContain(">de<");
  });

  it("renders version adoption by current version", () => {
    const out = renderDashboard(baseStats());
    expect(out).toContain("Version adoption");
    expect(out).toContain("current");
    expect(out).toContain("0.9.4");
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
