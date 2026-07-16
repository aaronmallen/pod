// Read-only dashboard HTML renderer (task sqsmupwn).
//
// This is the PRESENTATION half: a pure function from the typed `DashboardStats`
// (see stats.ts) to a single HTML document. Charts are inline `<svg>`, styling
// is one inline `<style>`, and the two-level tabs are pure CSS (hidden radio
// inputs + `:checked` sibling selectors) -- no JavaScript runs in the page.
//
// SELF-CONTAINED INVARIANT, RELAXED: this page is no longer fully offline. It
// pulls Space Grotesk + JetBrains Mono from Google Fonts via a `<link>` so the
// maintainer dashboard matches the marketing site's typography. This is a
// deliberate, scoped exception for this Cloudflare Access-gated maintainer-only
// route; the ingest path and everything else stay request-free. No `<script>`
// is ever emitted.
//
// Every DB-derived string (feature names, os strings, crash messages, backtrace
// frames) is HTML-escaped before interpolation, so a hostile field value can't
// inject markup into the maintainer's rendered page.

import { T } from "../../marketing/src/tokens";
import type {
  CrashGroup,
  DashboardStats,
  FeatureRow,
  LanguageRow,
  OsBucket,
  OsFamily,
  PerformanceRow,
  PlatformRow,
  ScreenRow,
  SchemaRow,
  Trends,
  VersionRow,
} from "./stats";

const FONTS_HREF =
  "https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;700&family=JetBrains+Mono:wght@400;500&display=swap";

const OS_LABEL: Record<OsFamily, string> = {
  windows: "Windows",
  mac: "macOS",
  linux: "Linux",
  other: "Other",
};

const OS_COLOR: Record<OsFamily, string> = {
  windows: T.plasma,
  mac: T.success,
  linux: T.warning,
  other: T.muted,
};

export function escapeHtml(value: unknown): string {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function num(v: number | null): string {
  return v === null ? "n/a" : escapeHtml(v);
}

function pct(part: number, total: number): string {
  if (total <= 0) return "0%";
  return `${Math.round((part / total) * 100)}%`;
}

function sparkline(trend: Trends): string {
  const pts = trend.points;
  if (pts.length === 0) {
    return `<p class="empty">No events received in the last ${escapeHtml(trend.windowDays)} days.</p>`;
  }

  const width = 720;
  const height = 140;
  const pad = 8;
  const padLeft = 34;
  const padBottom = 22;
  const max = Math.max(1, ...pts.map((p) => Math.max(p.installs, p.usage)));
  const innerW = width - padLeft - pad;
  const innerH = height - pad - padBottom;
  const baseY = pad + innerH;
  const stepX = pts.length > 1 ? innerW / (pts.length - 1) : 0;

  const coordsFor = (value: (p: (typeof pts)[number]) => number) =>
    pts.map((p, i) => {
      const x = padLeft + (pts.length > 1 ? i * stepX : innerW / 2);
      const y = pad + innerH - (value(p) / max) * innerH;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });

  const dotsFor = (coords: string[], value: (p: (typeof pts)[number]) => number, label: string, cls: string) =>
    pts
      .map((p, i) => {
        const [x, y] = coords[i].split(",");
        return `<circle class="${cls}" cx="${x}" cy="${y}" r="2.5"><title>${escapeHtml(p.day)}: ${escapeHtml(
          value(p),
        )} ${label}</title></circle>`;
      })
      .join("");

  const installCoords = coordsFor((p) => p.installs);
  const usageCoords = coordsFor((p) => p.usage);

  const yTicks = [...new Set([0, Math.round(max / 2), max])].map((value) => {
    const y = pad + innerH - (value / max) * innerH;
    return `<line x1="${padLeft - 4}" y1="${y.toFixed(1)}" x2="${padLeft}" y2="${y.toFixed(1)}" class="axis"/>
  ${value > 0 ? `<line x1="${padLeft}" y1="${y.toFixed(1)}" x2="${width - pad}" y2="${y.toFixed(1)}" class="grid"/>` : ""}
  <text x="${padLeft - 7}" y="${(y + 3).toFixed(1)}" class="tick" text-anchor="end">${escapeHtml(value)}</text>`;
  });

  const xLabelIndices = [...new Set(pts.length > 1 ? [0, Math.floor((pts.length - 1) / 2), pts.length - 1] : [0])];
  const xTicks = xLabelIndices.map((i) => {
    const x = padLeft + (pts.length > 1 ? i * stepX : innerW / 2);
    const anchor = i === 0 && pts.length > 1 ? "start" : i === pts.length - 1 ? "end" : "middle";
    return `<line x1="${x.toFixed(1)}" y1="${baseY}" x2="${x.toFixed(1)}" y2="${baseY + 4}" class="axis"/>
  <text x="${x.toFixed(1)}" y="${baseY + 15}" class="tick" text-anchor="${anchor}">${escapeHtml(
      pts[i].day.slice(5),
    )}</text>`;
  });

  return `<ul class="legend spark-legend">
  <li><span class="swatch" style="background:${T.plasma}"></span>
    <span class="lg-label">New installs / day</span></li>
  <li><span class="swatch" style="background:${T.success}"></span>
    <span class="lg-label">Usage / day</span></li>
</ul>
<svg class="spark" viewBox="0 0 ${width} ${height}" role="img"
  aria-label="New installs and distinct active installs per day over the last ${escapeHtml(trend.windowDays)} days">
  <line x1="${padLeft}" y1="${pad}" x2="${padLeft}" y2="${baseY}" class="axis"/>
  <line x1="${padLeft}" y1="${baseY}" x2="${width - pad}" y2="${baseY}" class="axis"/>
  ${yTicks.join("\n  ")}
  ${xTicks.join("\n  ")}
  <polyline points="${usageCoords.join(" ")}" class="line usage" fill="none"/>
  <polyline points="${installCoords.join(" ")}" class="line" fill="none"/>
  ${dotsFor(usageCoords, (p) => p.usage, "active", "usage")}
  ${dotsFor(installCoords, (p) => p.installs, "new", "installs")}
</svg>
<p class="caption">peak ${escapeHtml(max)} / day &middot; ${escapeHtml(
    pts.length,
  )} active day(s) in window</p>`;
}

function bar(value: number, max: number): string {
  const width = max > 0 ? Math.max(2, Math.round((value / max) * 100)) : 0;
  return `<span class="bar" style="width:${width}%"></span>`;
}

function pie(buckets: OsBucket[]): string {
  const slices = buckets.filter((b) => b.installs > 0);
  const total = slices.reduce((s, b) => s + b.installs, 0);
  if (total === 0) return `<p class="empty">No platform data in window.</p>`;

  const r = 56;
  const cx = 80;
  const cy = 80;
  const stroke = 24;
  const circumference = 2 * Math.PI * r;

  let acc = 0;
  const ring = slices
    .map((b) => {
      const frac = b.installs / total;
      const len = frac * circumference;
      const dash = `${len.toFixed(2)} ${(circumference - len).toFixed(2)}`;
      const offset = (-acc * circumference).toFixed(2);
      acc += frac;
      return `<circle class="pie-slice" cx="${cx}" cy="${cy}" r="${r}" fill="none"
    stroke="${OS_COLOR[b.family]}" stroke-width="${stroke}"
    stroke-dasharray="${dash}" stroke-dashoffset="${offset}"
    transform="rotate(-90 ${cx} ${cy})"><title>${escapeHtml(OS_LABEL[b.family])}: ${escapeHtml(
        b.installs,
      )}</title></circle>`;
    })
    .join("");

  const legend = slices
    .map(
      (b) => `<li><span class="swatch" style="background:${OS_COLOR[b.family]}"></span>
    <span class="lg-label">${escapeHtml(OS_LABEL[b.family])}</span>
    <span class="lg-n">${escapeHtml(b.installs)} &middot; ${pct(b.installs, total)}</span></li>`,
    )
    .join("");

  return `<div class="pie-wrap">
  <svg class="pie" viewBox="0 0 160 160" role="img" aria-label="Installs by OS family">
    <circle cx="${cx}" cy="${cy}" r="${r}" fill="none" stroke="${T.rule}" stroke-width="${stroke}"/>
    ${ring}
  </svg>
  <ul class="legend">${legend}</ul>
</div>`;
}

function metric(value: number, label: string): string {
  return `<div class="tile"><div class="tile-n">${escapeHtml(value)}</div>
  <div class="tile-l">${escapeHtml(label)}</div></div>`;
}

function windowControl(windowDays: number): string {
  return `<form class="window-control" method="get" action="">
  <label for="days">Window</label>
  <input id="days" name="days" type="number" min="1" max="365" value="${escapeHtml(windowDays)}">
  <span class="unit">days</span>
  <button type="submit">Apply</button>
</form>`;
}

function renderVersions(rows: VersionRow[]): string {
  if (rows.length === 0) return emptySection("Version adoption", "No events in window.");
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
  <p class="note">Distinct installs by their <em>current</em> <code>app_version</code> (latest event in window).</p>
  <table>
    <thead><tr><th>Version</th><th class="n">Installs</th><th></th></tr></thead>
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

function renderOverview(stats: DashboardStats): string {
  const tiles = `<div class="tiles">
  ${metric(stats.installs.totalDistinct, `distinct installs (last ${stats.windowDays} days)`)}
  ${metric(stats.crashTotal, `crashes (last ${stats.windowDays} days)`)}
</div>`;

  return `<section class="panel" id="panel-overview">
  ${renderSchemas(stats.schemas)}
  ${windowControl(stats.windowDays)}
  ${tiles}
  <p class="note">Distinct <code>anon_id</code> = sha256(machine_id), one per install. This is install count, not user count.</p>
  <section>
    <h2>Installs by OS</h2>
    ${pie(stats.osBuckets)}
  </section>
  <section>
    <h2>Trends</h2>
    ${sparkline(stats.installs)}
  </section>
  ${renderVersions(stats.versions)}
  ${renderCrashes(stats.crashes)}
</section>`;
}

function renderPlatforms(rows: PlatformRow[]): string {
  const inner =
    rows.length === 0
      ? emptySection("Platform breakdown", "No environment rows yet.")
      : platformTable(rows);
  return `<section class="panel" id="panel-platforms">${inner}</section>`;
}

function platformTable(rows: PlatformRow[]): string {
  const max = Math.max(1, ...rows.map((r) => r.installs));
  const body = rows
    .map(
      (r) => `<tr>
    <td>${escapeHtml(r.os)}</td>
    <td>${escapeHtml(r.os_version)}</td>
    <td>${escapeHtml(r.arch)}</td>
    <td class="n">${escapeHtml(r.installs)}</td>
    <td class="barcell">${bar(r.installs, max)}</td>
  </tr>`,
    )
    .join("");
  return `<section>
  <h2>Platform breakdown</h2>
  <p class="note">Distinct installs by their <em>current</em> OS / version / arch (latest environment report in window), so each install is counted once. The literal <code>"unknown"</code> bucket is shown, never hidden.</p>
  <table>
    <thead><tr><th>OS</th><th>OS version</th><th>Arch</th><th class="n">Installs</th><th></th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function renderScreens(rows: ScreenRow[]): string {
  const inner =
    rows.length === 0
      ? emptySection("Screen breakdown", "No environment rows yet.")
      : screenTable(rows);
  return `<section class="panel" id="panel-screens">${inner}</section>`;
}

function screenTable(rows: ScreenRow[]): string {
  const max = Math.max(1, ...rows.map((r) => r.installs));
  const body = rows
    .map(
      (r) => `<tr>
    <td>${escapeHtml(r.window_size)}</td>
    <td>${escapeHtml(r.screen_size)}</td>
    <td class="n">${escapeHtml(r.installs)}</td>
    <td class="barcell">${bar(r.installs, max)}</td>
  </tr>`,
    )
    .join("");
  return `<section>
  <h2>Screen breakdown</h2>
  <p class="note">Distinct installs by their <em>current</em> window size / screen size (latest environment report in window), so each install is counted once. The literal <code>"unknown"</code> bucket is shown, never hidden.</p>
  <table>
    <thead><tr><th>Window size</th><th>Screen size</th><th class="n">Count</th><th></th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function renderFeatures(features: FeatureRow[], performance: PerformanceRow[]): string {
  return `<section class="panel" id="panel-features">
  <input class="subtab" type="radio" name="ftab" id="ftab-top" checked>
  <input class="subtab" type="radio" name="ftab" id="ftab-perf">
  <nav class="subnav">
    <label for="ftab-top">Top features</label>
    <label for="ftab-perf">Performance</label>
  </nav>
  <div class="subpanel" id="subpanel-top">${featuresTable(features)}</div>
  <div class="subpanel" id="subpanel-perf">${performanceTable(performance)}</div>
</section>`;
}

function featuresTable(rows: FeatureRow[]): string {
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

function performanceTable(rows: PerformanceRow[]): string {
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

function renderLanguages(rows: LanguageRow[]): string {
  const inner =
    rows.length === 0
      ? emptySection("Language breakdown", "No environment rows yet.")
      : languageTable(rows);
  return `<section class="panel" id="panel-languages">${inner}</section>`;
}

function languageTable(rows: LanguageRow[]): string {
  const max = Math.max(1, ...rows.map((r) => r.installs));
  const body = rows
    .map(
      (r) => `<tr>
    <td>${escapeHtml(r.app_language)}</td>
    <td class="n">${escapeHtml(r.installs)}</td>
    <td class="barcell">${bar(r.installs, max)}</td>
  </tr>`,
    )
    .join("");
  return `<section>
  <h2>Language breakdown</h2>
  <p class="note">Distinct installs by their <em>current</em> chosen UI <code>app_language</code> (the in-app language pick, not the OS locale; latest environment report in window), so each install is counted once. The literal <code>"unknown"</code> bucket is shown, never hidden.</p>
  <table>
    <thead><tr><th>Language</th><th class="n">Installs</th><th></th></tr></thead>
    <tbody>${body}</tbody>
  </table>
</section>`;
}

function renderSchemas(rows: SchemaRow[]): string {
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
  <p class="note">More than one contract <code>schema</code> is present. Panels aggregate across versions; interpret with care.</p>
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
  :root { color-scheme: dark; }
  * { box-sizing: border-box; }
  body {
    font: 15px/1.5 'Space Grotesk', system-ui, -apple-system, sans-serif;
    margin: 0; padding: 2rem 1.25rem 4rem; max-width: 960px; margin-inline: auto;
    color: ${T.ink}; background: ${T.paperSunk};
  }
  h1 { font-size: 1.5rem; margin: 0 0 .25rem; letter-spacing: -.01em; }
  h2 { font-size: 1.05rem; margin: 0 0 .5rem; }
  .sub-hd { color: ${T.muted}; margin: 0 0 1.5rem; font-size: .9rem; }
  section { background: ${T.paper}; border: 1px solid ${T.rule}; border-radius: 12px; padding: 1.25rem; margin-bottom: 1.25rem; }
  section section { background: ${T.paperRaised}; }
  section.warn { border-color: ${T.warning}; background: ${T.paperRaised}; }
  code { background: ${T.paperSunk}; padding: .05rem .3rem; border-radius: 4px; font-family: 'JetBrains Mono', ui-monospace, monospace; font-size: .85em; color: ${T.plasma}; }
  .note { color: ${T.muted}; font-size: .85rem; margin: .25rem 0 .75rem; }
  .empty { color: ${T.veryMuted}; font-style: italic; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: .35rem .5rem; border-bottom: 1px solid ${T.rule}; vertical-align: middle; }
  th { font-size: .75rem; text-transform: uppercase; letter-spacing: .04em; color: ${T.veryMuted}; }
  td.n, th.n { text-align: right; font-variant-numeric: tabular-nums; font-family: 'JetBrains Mono', ui-monospace, monospace; }
  td.barcell { width: 30%; }
  .bar { display: inline-block; height: 10px; border-radius: 5px; background: ${T.plasma}; min-width: 2px; }
  .sub { color: ${T.veryMuted}; font-size: .8em; }

  .tiles { display: flex; gap: 1rem; flex-wrap: wrap; margin: .5rem 0 1rem; }
  .tile { flex: 1 1 180px; background: ${T.paperRaised}; border: 1px solid ${T.rule}; border-radius: 10px; padding: 1rem 1.1rem; }
  .tile-n { font-size: 2.1rem; font-weight: 700; font-variant-numeric: tabular-nums; color: ${T.ink}; }
  .tile-l { color: ${T.muted}; font-size: .8rem; }

  .window-control { display: flex; align-items: center; gap: .5rem; font-size: .85rem; color: ${T.muted}; margin-bottom: .5rem; }
  .window-control input { width: 5rem; background: ${T.paperSunk}; color: ${T.ink}; border: 1px solid ${T.ruleStrong}; border-radius: 6px; padding: .25rem .4rem; font: inherit; }
  .window-control button { background: ${T.plasmaSoft}; color: ${T.plasma}; border: 1px solid ${T.plasma}; border-radius: 6px; padding: .25rem .7rem; font: inherit; cursor: pointer; }

  .pie-wrap { display: flex; gap: 1.5rem; align-items: center; flex-wrap: wrap; }
  svg.pie { width: 160px; height: 160px; flex: 0 0 auto; }
  .legend { list-style: none; margin: 0; padding: 0; display: grid; gap: .35rem; }
  .legend li { display: flex; align-items: center; gap: .5rem; }
  .swatch { width: .8rem; height: .8rem; border-radius: 3px; display: inline-block; }
  .lg-label { min-width: 5rem; }
  .lg-n { color: ${T.muted}; font-variant-numeric: tabular-nums; font-size: .85rem; }

  svg.spark { width: 100%; height: auto; display: block; margin: .5rem 0 .25rem; }
  svg.spark .line { stroke: ${T.plasma}; stroke-width: 2; }
  svg.spark .line.usage { stroke: ${T.success}; }
  svg.spark .axis { stroke: ${T.ruleStrong}; stroke-width: 1; }
  svg.spark .grid { stroke: ${T.rule}; stroke-width: 1; stroke-dasharray: 2 4; }
  svg.spark .tick { fill: ${T.veryMuted}; font: 9px 'JetBrains Mono', ui-monospace, monospace; }
  svg.spark circle { fill: ${T.plasma}; }
  svg.spark circle.usage { fill: ${T.success}; }
  .spark-legend { display: flex; gap: 1.25rem; margin-top: .25rem; }
  .caption { color: ${T.veryMuted}; font-size: .8rem; margin: 0; }

  details { border-bottom: 1px solid ${T.rule}; padding: .4rem 0; }
  summary { cursor: pointer; display: flex; gap: .6rem; align-items: baseline; flex-wrap: wrap; }
  .cr-count { font-weight: 700; color: ${T.danger}; font-variant-numeric: tabular-nums; }
  .cr-ver { font-family: 'JetBrains Mono', ui-monospace, monospace; font-size: .85em; color: ${T.muted}; }
  .cr-msg { flex: 1; }
  .cr-seen { color: ${T.veryMuted}; font-size: .8em; }
  .cr-block { margin: .5rem 0 .25rem 1rem; }
  .cr-block ol { margin: .25rem 0; font-family: 'JetBrains Mono', ui-monospace, monospace; font-size: .8rem; color: ${T.muted}; }

  .tab { position: absolute; left: -9999px; opacity: 0; }
  .tabnav { display: flex; gap: .25rem; border-bottom: 1px solid ${T.ruleStrong}; margin-bottom: 1.25rem; flex-wrap: wrap; }
  .tabnav label { cursor: pointer; padding: .5rem .9rem; color: ${T.muted}; border-bottom: 2px solid transparent; margin-bottom: -1px; font-weight: 500; }
  .tabnav label:hover { color: ${T.ink}; }
  .panel { display: none; }
  #tab-overview:checked ~ .tabnav label[for="tab-overview"],
  #tab-platforms:checked ~ .tabnav label[for="tab-platforms"],
  #tab-screens:checked ~ .tabnav label[for="tab-screens"],
  #tab-features:checked ~ .tabnav label[for="tab-features"],
  #tab-languages:checked ~ .tabnav label[for="tab-languages"] {
    color: ${T.ink}; border-bottom-color: ${T.plasma};
  }
  #tab-overview:checked ~ #panel-overview,
  #tab-platforms:checked ~ #panel-platforms,
  #tab-screens:checked ~ #panel-screens,
  #tab-features:checked ~ #panel-features,
  #tab-languages:checked ~ #panel-languages { display: block; }

  .subtab { position: absolute; left: -9999px; opacity: 0; }
  .subnav { display: flex; gap: .25rem; margin-bottom: 1rem; }
  .subnav label { cursor: pointer; padding: .3rem .8rem; color: ${T.muted}; border: 1px solid ${T.rule}; border-radius: 6px; font-size: .85rem; }
  .subnav label:hover { color: ${T.ink}; }
  .subpanel { display: none; }
  #ftab-top:checked ~ .subnav label[for="ftab-top"],
  #ftab-perf:checked ~ .subnav label[for="ftab-perf"] { color: ${T.ink}; border-color: ${T.plasma}; }
  #ftab-top:checked ~ #subpanel-top,
  #ftab-perf:checked ~ #subpanel-perf { display: block; }

  footer { color: ${T.veryMuted}; font-size: .8rem; text-align: center; margin-top: 2rem; }
`;

export function renderDashboard(stats: DashboardStats): string {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex, nofollow">
<title>pod telemetry &middot; maintainer dashboard</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="${FONTS_HREF}" rel="stylesheet">
<style>${STYLE}</style>
</head>
<body>
<h1>pod telemetry dashboard</h1>
<p class="sub-hd">Read-only aggregates over opt-in telemetry. Generated ${escapeHtml(stats.generatedAt)}.</p>
<div class="tabs">
<input class="tab" type="radio" name="tab" id="tab-overview" checked>
<input class="tab" type="radio" name="tab" id="tab-platforms">
<input class="tab" type="radio" name="tab" id="tab-screens">
<input class="tab" type="radio" name="tab" id="tab-features">
<input class="tab" type="radio" name="tab" id="tab-languages">
<nav class="tabnav">
  <label for="tab-overview">Overview</label>
  <label for="tab-platforms">Platforms</label>
  <label for="tab-screens">Screens</label>
  <label for="tab-features">Features</label>
  <label for="tab-languages">Languages</label>
</nav>
${renderOverview(stats)}
${renderPlatforms(stats.platforms)}
${renderScreens(stats.screens)}
${renderFeatures(stats.features, stats.performance)}
${renderLanguages(stats.languages)}
</div>
<footer>Aggregate counts only. No raw events, anon_ids, or PII are shown.</footer>
</body>
</html>`;
}
