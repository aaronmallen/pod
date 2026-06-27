// Read-only dashboard HTML renderer (task sqsmupwn).
//
// This is the PRESENTATION half: a pure function from the typed `DashboardStats`
// (see stats.ts) to a single self-contained HTML document. No external JS, CSS,
// or CDN assets -- styling is one inline `<style>`, charts are inline `<svg>`,
// expand/collapse is native `<details>`. No JavaScript runs in the page.
//
// Every DB-derived string (feature names, os strings, crash messages, backtrace
// frames) is HTML-escaped before interpolation, so a hostile field value can't
// inject markup into the maintainer's rendered page.

import type {
  CrashGroup,
  DashboardStats,
  FeatureRow,
  InstallTrend,
  PerformanceRow,
  PlatformRow,
  SchemaRow,
  VersionRow,
} from "./stats";

/** HTML-escape a value for safe interpolation into element text or attributes. */
export function escapeHtml(value: unknown): string {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** Format a possibly-null average for display. */
function num(v: number | null): string {
  return v === null ? "n/a" : escapeHtml(v);
}

/**
 * Inline-SVG sparkline of daily distinct installs. No JS; a single polyline
 * plus a baseline. Empty windows render an honest "no data" note instead.
 */
function sparkline(trend: InstallTrend): string {
  const pts = trend.points;
  if (pts.length === 0) {
    return `<p class="empty">No events received in the last ${escapeHtml(trend.windowDays)} days.</p>`;
  }

  const width = 720;
  const height = 120;
  const pad = 8;
  const max = Math.max(1, ...pts.map((p) => p.installs));
  const innerW = width - pad * 2;
  const innerH = height - pad * 2;
  const stepX = pts.length > 1 ? innerW / (pts.length - 1) : 0;

  const coords = pts.map((p, i) => {
    const x = pad + (pts.length > 1 ? i * stepX : innerW / 2);
    const y = pad + innerH - (p.installs / max) * innerH;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });

  const dots = pts
    .map((p, i) => {
      const [x, y] = coords[i].split(",");
      return `<circle cx="${x}" cy="${y}" r="2.5"><title>${escapeHtml(p.day)}: ${escapeHtml(
        p.installs,
      )}</title></circle>`;
    })
    .join("");

  return `<svg class="spark" viewBox="0 0 ${width} ${height}" role="img"
  aria-label="Distinct installs per day over the last ${escapeHtml(trend.windowDays)} days">
  <line x1="${pad}" y1="${height - pad}" x2="${width - pad}" y2="${height - pad}" class="axis"/>
  <polyline points="${coords.join(" ")}" class="line" fill="none"/>
  ${dots}
</svg>
<p class="caption">peak ${escapeHtml(max)} / day &middot; ${escapeHtml(
    pts.length,
  )} active day(s) in window</p>`;
}

/** A simple horizontal bar (inline-SVG-free: a styled div) scaled to `max`. */
function bar(value: number, max: number): string {
  const pct = max > 0 ? Math.max(2, Math.round((value / max) * 100)) : 0;
  return `<span class="bar" style="width:${pct}%"></span>`;
}

function renderInstalls(trend: InstallTrend): string {
  return `<section>
  <h2>Active installs</h2>
  <p class="metric">${escapeHtml(trend.totalDistinct)}
    <span class="unit">distinct installs that opted in (last ${escapeHtml(
      trend.windowDays,
    )} days)</span></p>
  <p class="note">Counts distinct <code>anon_id</code> = sha256(machine_id), one per install. This is install count, not user count.</p>
  ${sparkline(trend)}
</section>`;
}

function renderPlatforms(rows: PlatformRow[]): string {
  if (rows.length === 0) return emptySection("Platform breakdown", "No environment rows yet.");
  const max = Math.max(1, ...rows.map((r) => r.installs));
  const body = rows
    .map(
      (r) => `<tr>
    <td>${escapeHtml(r.os)}</td>
    <td>${escapeHtml(r.os_version)}</td>
    <td>${escapeHtml(r.arch)}</td>
    <td>${escapeHtml(r.display)}</td>
    <td class="n">${escapeHtml(r.installs)}</td>
    <td class="barcell">${bar(r.installs, max)}</td>
  </tr>`,
    )
    .join("");
  return `<section>
  <h2>Platform breakdown</h2>
  <p class="note">By distinct installs per environment. The literal <code>"unknown"</code> bucket is shown, never hidden.</p>
  <table>
    <thead><tr><th>OS</th><th>OS version</th><th>Arch</th><th>Display</th><th class="n">Installs</th><th></th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function renderFeatures(rows: FeatureRow[]): string {
  if (rows.length === 0) return emptySection("Top features", "No usage events yet.");
  const max = Math.max(1, ...rows.map((r) => r.count));
  const body = rows
    .map((r) => {
      const toggle =
        r.toggledOn === null
          ? ""
          : ` <span class="sub">(${escapeHtml(r.toggledOn)} on / ${escapeHtml(
              r.count - r.toggledOn,
            )} off)</span>`;
      return `<tr>
    <td>${escapeHtml(r.event_kind)}</td>
    <td>${escapeHtml(r.name)}${toggle}</td>
    <td class="n">${escapeHtml(r.count)}</td>
    <td class="barcell">${bar(r.count, max)}</td>
  </tr>`;
    })
    .join("");
  return `<section>
  <h2>Top features</h2>
  <p class="note">Usage events by kind and name. <code>feature_toggle</code> rows show the on/off split.</p>
  <table>
    <thead><tr><th>Kind</th><th>Name</th><th class="n">Events</th><th></th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function renderVersions(rows: VersionRow[]): string {
  if (rows.length === 0) return emptySection("Version adoption", "No events yet.");
  const max = Math.max(1, ...rows.map((r) => r.installs));
  const body = rows
    .map(
      (r) => `<tr>
    <td>${escapeHtml(r.app_version)}</td>
    <td class="n">${escapeHtml(r.installs)}</td>
    <td class="barcell">${bar(r.installs, max)}</td>
  </tr>`,
    )
    .join("");
  return `<section>
  <h2>Version adoption</h2>
  <p class="note">Distinct installs per <code>app_version</code>.</p>
  <table>
    <thead><tr><th>Version</th><th class="n">Installs</th><th></th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function renderPerformance(rows: PerformanceRow[]): string {
  if (rows.length === 0) return emptySection("Performance", "No performance rows yet.");
  const body = rows
    .map(
      (r) => `<tr>
    <td>${escapeHtml(r.name)}</td>
    <td class="n">${escapeHtml(r.samples)}</td>
    <td class="n">${num(r.avgLoadMs)}</td>
    <td class="n">${num(r.avgFrameP95Ms)}</td>
    <td class="n">${num(r.avgHeapMb)}</td>
  </tr>`,
    )
    .join("");
  return `<section>
  <h2>Performance</h2>
  <p class="note">Averages per view over the performance stream.</p>
  <table>
    <thead><tr><th>View</th><th class="n">Samples</th><th class="n">Avg load (ms)</th><th class="n">Avg frame p95 (ms)</th><th class="n">Avg heap (MB)</th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function renderCrashList(items: string[], label: string): string {
  if (items.length === 0) return `<p class="empty">no ${escapeHtml(label)}</p>`;
  const lines = items.map((l) => `<li>${escapeHtml(l)}</li>`).join("");
  return `<div class="cr-block"><strong>${escapeHtml(label)}</strong><ol>${lines}</ol></div>`;
}

function renderCrashes(rows: CrashGroup[]): string {
  if (rows.length === 0) return emptySection("Crash groups", "No crashes reported.");
  const body = rows
    .map(
      (g) => `<details>
    <summary>
      <span class="cr-count">${escapeHtml(g.count)}&times;</span>
      <span class="cr-ver">${escapeHtml(g.app_version)}</span>
      <span class="cr-msg">${escapeHtml(g.message)}</span>
      <span class="cr-seen">last ${escapeHtml(g.lastSeen)}</span>
    </summary>
    ${renderCrashList(g.backtrace, "backtrace")}
    ${renderCrashList(g.context_log, "context log")}
  </details>`,
    )
    .join("");
  return `<section>
  <h2>Crash groups</h2>
  <p class="note">Grouped by <code>app_version</code> + <code>message</code>. Expand a row for its most recent backtrace and context log.</p>
  ${body}
</section>`;
}

function renderSchemas(rows: SchemaRow[]): string {
  // Only surface the panel when more than one contract version is present, so
  // schema=1-only deployments stay clean (criterion: may be absent/no-op).
  if (rows.length <= 1) return "";
  const body = rows
    .map(
      (r) => `<tr>
    <td class="n">${escapeHtml(r.schema)}</td>
    <td class="n">${escapeHtml(r.events)}</td>
    <td class="n">${escapeHtml(r.crashes)}</td>
  </tr>`,
    )
    .join("");
  return `<section class="warn">
  <h2>Schema mix</h2>
  <p class="note">More than one contract <code>schema</code> is present. Panels above aggregate across versions; interpret with care.</p>
  <table>
    <thead><tr><th class="n">Schema</th><th class="n">Event rows</th><th class="n">Crash rows</th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function emptySection(title: string, msg: string): string {
  return `<section><h2>${escapeHtml(title)}</h2><p class="empty">${escapeHtml(msg)}</p></section>`;
}

const STYLE = `
  :root { color-scheme: light dark; }
  * { box-sizing: border-box; }
  body {
    font: 15px/1.5 system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
    margin: 0; padding: 2rem 1.25rem 4rem; max-width: 880px; margin-inline: auto;
    color: #1a1a1a; background: #fafafa;
  }
  h1 { font-size: 1.5rem; margin: 0 0 .25rem; }
  h2 { font-size: 1.1rem; margin: 0 0 .5rem; }
  .sub-hd { color: #666; margin: 0 0 2rem; font-size: .9rem; }
  section { background: #fff; border: 1px solid #e3e3e3; border-radius: 10px; padding: 1.25rem; margin-bottom: 1.5rem; }
  section.warn { border-color: #e0a800; background: #fffbf0; }
  .metric { font-size: 2rem; font-weight: 600; margin: .25rem 0; }
  .metric .unit { font-size: .85rem; font-weight: 400; color: #666; display: block; }
  .note { color: #666; font-size: .85rem; margin: .25rem 0 .75rem; }
  .empty { color: #999; font-style: italic; }
  code { background: #f0f0f0; padding: .05rem .3rem; border-radius: 4px; font-size: .85em; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: .35rem .5rem; border-bottom: 1px solid #eee; vertical-align: middle; }
  th { font-size: .8rem; text-transform: uppercase; letter-spacing: .03em; color: #888; }
  td.n, th.n { text-align: right; font-variant-numeric: tabular-nums; }
  td.barcell { width: 30%; }
  .bar { display: inline-block; height: 10px; border-radius: 5px; background: #3b82f6; min-width: 2px; }
  .sub { color: #888; font-size: .8em; }
  svg.spark { width: 100%; height: auto; display: block; margin: .5rem 0 .25rem; }
  svg.spark .line { stroke: #3b82f6; stroke-width: 2; }
  svg.spark .axis { stroke: #ddd; stroke-width: 1; }
  svg.spark circle { fill: #3b82f6; }
  .caption { color: #888; font-size: .8rem; margin: 0; }
  details { border-bottom: 1px solid #eee; padding: .4rem 0; }
  summary { cursor: pointer; display: flex; gap: .6rem; align-items: baseline; flex-wrap: wrap; }
  .cr-count { font-weight: 600; color: #b91c1c; font-variant-numeric: tabular-nums; }
  .cr-ver { font-family: ui-monospace, monospace; font-size: .85em; color: #555; }
  .cr-msg { flex: 1; }
  .cr-seen { color: #999; font-size: .8em; }
  .cr-block { margin: .5rem 0 .25rem 1rem; }
  .cr-block ol { margin: .25rem 0; font-family: ui-monospace, monospace; font-size: .8rem; color: #444; }
  footer { color: #999; font-size: .8rem; text-align: center; margin-top: 2rem; }
`;

/**
 * Render the full self-contained dashboard document. No external requests; all
 * DB-derived text is HTML-escaped at interpolation.
 */
export function renderDashboard(stats: DashboardStats): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>pod telemetry &middot; maintainer dashboard</title>
<style>${STYLE}</style>
</head>
<body>
<h1>pod telemetry dashboard</h1>
<p class="sub-hd">Read-only aggregates over opt-in telemetry. Generated ${escapeHtml(
    stats.generatedAt,
  )}.</p>
${renderSchemas(stats.schemas)}
${renderInstalls(stats.installs)}
${renderPlatforms(stats.platforms)}
${renderFeatures(stats.features)}
${renderVersions(stats.versions)}
${renderPerformance(stats.performance)}
${renderCrashes(stats.crashes)}
<footer>Aggregate counts only. No raw events, anon_ids, or PII are shown.</footer>
</body>
</html>`;
}
