---
id: "0013"
title: Image Assets — Committed Item Icons and Synced Portraits/Logos
status: active
tags: [architecture, images, sync, storage]
created: 2026-06-06
---

# ADR-0013: Image Assets — Committed Item Icons and Synced Portraits/Logos

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

> **Revised 2026-06-08 (cross-release icon caching):** the generated item-icon set is now **cached across releases,
> keyed on the SDE build number**, so an unchanged SDE no longer re-downloads the SDE zip or re-fetches ~30k icons from
> the CDN. The `generate-icons` job resolves the build number with a cheap `curl` of the SDE
> [`latest.jsonl`](https://developers.eveonline.com/static-data/tranquility/latest.jsonl) endpoint (the same
> `buildNumber` the runtime keys on in `src/clients/sde.rs`) piped through `jq` — **without** downloading the
> multi-hundred-MB zip — and uses it as the `actions/cache` key for `assets/images/items`. The earlier "**fetch once
> per release**" rule is relaxed to "**fetch when the SDE build number changes or the cache misses**": on an exact cache
> hit the previously gated set is reused and **satisfies the coverage-floor gate by construction** (it passed the gate
> when it was generated for that exact build number); on a miss the generator runs a full cold fetch and its coverage
> gate re-validates the set, so the **CDN-outage canary still fires whenever a fetch is actually required**. `jq` is
> pinned in `.config/mise.toml [tools]`. The shared `item-images` workflow artifact still flows to the three package
> jobs unchanged.
>
> **Revised 2026-06-08 (spec `powlkvns`):** synced **character portraits and corporation/alliance logos are reclassified
> from durable data to an evictable cache.** They now live under the **cache root** (`resolved_cache_dir()/images`,
> ADR-0007), not the data root, in a **flattened single-file, single-(biggest)-size** layout — `characters/{id}.jpg`,
> `corporations/{id}.png`, `types/{id}.png` (the per-id directory and the `_{size}` suffix are dropped). The image sync
> folds in a **weekly (~7-day) sync-time staleness refetch** so a stale portrait/logo is replaced. The **committed
> item-icon bundle is UNCHANGED** by this revision, and **type icons are immutable — they are never evicted on
> staleness.** See [Synced portraits and logos](#synced-portraits-and-logos) for the revised rules; the 2026-06-06
> revision below is otherwise unaffected.
>
> **Revised 2026-06-06:** the loose-PNG set is no longer committed to the repository. It is now gitignored and generated
> once per release by the GitHub Actions pipeline, then shipped inside each OS package via a shared workflow artifact.
> The Decision, Consequences, and Affected Areas below reflect this; the two-tier resolver and best-effort sync
> long-tail are unchanged.
>
> **History:** item/type icons were originally treated as synced data — fetched per-entity during sync with a fatal
> fetch-failure rule (in a since-retired "Image Assets as Synced Data" ADR). This ADR replaces that approach for item
> icons with the committed/CI-generated set described below, and absorbs the still-active portrait/logo decision from
> that retired record. Character portraits and corporation/alliance logos are now treated as an **evictable image
> cache** rather than durable synced data, as documented under
> [Synced portraits and logos](#synced-portraits-and-logos).

## Summary

This ADR governs the whole image-assets decision area. **Item/type icons** are no longer fetched per-item during sync;
instead a set of loose 64px PNGs under `assets/images/items/{type_id}.png` is generated once per release by the CI
pipeline and shipped inside each OS package, with the working-tree set gitignored rather than committed. The runtime
resolves an icon through a **two-tier resolver**: the writable `data_dir()/images` store first, then the generated set
in the bundle. Sync is demoted to a **best-effort long-tail** that fills in only types newer than the shipped set, and
an icon fetch can never block or fail a sync job. This replaces the original "images are synced data, fetch-fail is
fatal" rule for the item-icon case.

**Character portraits and corporation/alliance logos are an evictable image cache** (revised 2026-06-08): the sync
engine fetches them while building an entity's dataset and writes them, at the single biggest size, to an **evictable
on-disk cache** under the cache root (`resolved_cache_dir()/images`, ADR-0007) — not to the durable data root. The
files use a flat `characters/{id}.jpg` / `corporations/{id}.png` layout, are refetched on a weekly sync-time staleness
check, and a missing file is repopulated lazily on the next sync rather than treated as an integrity failure. See
[Synced portraits and logos](#synced-portraits-and-logos). **Type icons are immutable and are never evicted.**

## Context

Originally every image — including item/type icons — was fetched per-entity during sync and the entity was committed
only once its images were on disk, so an image-fetch failure failed the job. For item icons this proved wrong on three
counts:

- **It blocked privileged sync.** A single implant icon fetch failure aborted the entire clone sync
  (`character_clones.rs`), and the abyssal job propagated icon fetch/write errors too.
- **Icons were still blank.** The assets, killlog, and clone-implant views render read-only from the on-disk store, but
  nothing ever downloaded icons for the long tail of types, so most icons never appeared.
- **Per-item fetching is slow.** Resolving ~30k type icons one ESI request at a time is far slower than shipping them.

This refines RFC `lpvoysql`, which proposed a GitHub-Release `tar.zst` downloaded on splash. That is rejected here
because a fresh checkout has no published archive to pull from (it breaks dev builds), and a downloaded archive lands
the same ~150 MB on disk as committing does — with none of git's delta benefits.

## Decision

**A CI-generated, per-release set of loose 64px PNGs.** Published-type icons live at
`assets/images/items/{type_id}.png` (plain 64px, no variant/size nesting). The PNGs are **gitignored**, not committed —
only the `.no-icon` ledger is tracked. They are generated each release by a shell mise file-task
(`generate:item-images`) that downloads the SDE, derives the published type ids, and fetches each icon with bounded
concurrency. The task is idempotent/resumable (skip-if-exists; a 404 is recorded resolved-empty and never retried).

Rather than hard-exit on the first failed icon, the generator finishes the full run and then applies a **coverage-floor
gate**: it exits non-zero only when the share of resolved types falls below a configurable threshold
(`--min-coverage`, default 95%), which distinguishes a genuine CDN outage from the usual long tail of types that have no
published icon. Verification runs (`--ids`) keep the strict all-must-resolve behavior.

The release workflow runs the generator **once** in a dedicated `generate-icons` job and uploads
`assets/images/items` as a shared workflow artifact. Each OS package job (`package-macos`, `package-windows`,
`package-linux`) declares `needs: generate-icons` and downloads that artifact into `assets/images/items` before
`mise run package`, so the CDN is hit at most once per release and the Windows runner never executes the POSIX
generator. The `generate-icons` job wraps the generator in an `actions/cache` step keyed on the SDE build number (see
the 2026-06-08 cross-release icon caching revision above): an unchanged SDE restores the previously gated set and skips
the generator entirely, so the SDE download and CDN fetch happen only when the build number changes or the cache
misses.

**A two-tier runtime resolver.** `resolve_type_icon` (`src/store/images.rs`) consults, in order:

1. The writable image cache (`resolved_cache_dir()/images`, ADR-0007) — flat `types/{id}.png`, where the sync long-tail
   writes. (Revised 2026-06-08: this tier moved from `data_dir()/images` to the cache root; type icons written here are
   immutable and never evicted on staleness.)
2. The generated items dir — `assets/images/items/` in a dev build, the cargo-packager resources dir in a packaged
   build, located by a `resource_dir()` helper that probes for the bundled `assets/` tree rather than trusting a
   compile-time path.

The cache tier keeps priority so a freshly-synced icon shadows the shipped one; only when both miss is the icon
`Missing` (the UI shows a silhouette). A fresh checkout with no generated icons resolves everything to silhouettes until
a contributor runs the opt-in generator.

**Sync demoted to best-effort long-tail.** The clone and abyssal jobs skip fetching when an icon is already on disk and,
on a fetch/write failure, log a warning and continue. These fetches now exist only to fill types newer than the
shipped set; they can never block or fail a sync job.

### Synced portraits and logos

Character portraits and corporation/alliance logos are **not** part of the item-icon set above. As of the 2026-06-08
revision (spec `powlkvns`) they are an **evictable image cache**, not durable synced data. They are still *fetched
during sync* — a presentational binary is part of building an entity — but they are written to a disposable cache that
may be cleared at any time and repopulated lazily, so their absence no longer blocks an entity's commit. Two reasons
still drive keeping them out of SQLite and the JSON HTTP cache:

- **`http_cache` is a freshness cache.** It keys entries by URL with a `Cache-Control` lifetime and prunes/revalidates
  them, so images placed there are not durable in a useful way for rendering, and it stays JSON-response-only.
- **Large binaries do not belong in SQLite.** Storing portraits and logos as BLOBs bloats the database file, its page
  cache, and every backup, and makes `VACUUM` expensive. The relational store holds relational data only.

The rules (revised 2026-06-08):

- **Fetched during sync, but non-blocking.** The sync engine fetches an entity's portrait/logo while assembling its
  dataset and writes it to the image cache. Because the cache is evictable, a fetch failure is **logged and skipped**
  rather than failing the entity's commit — the render layer falls back to a placeholder and the next sync repopulates
  the file. (This is a change from the prior "synced before commit / fetch failure fails the job" rule.)
- **Evictable on-disk cache, flat single-biggest-size layout.** Images live as files under the **cache root**
  (`resolved_cache_dir()/images`, ADR-0007), **not** the data root. The layout is **flattened to a single file at a
  single size** — `characters/{id}.jpg`, `corporations/{id}.png`, `types/{id}.png` — dropping the old per-id directory
  and the `_{size}` suffix. Only the **biggest** size is downloaded; iced scales it down at render time, so there is no
  on-disk resize and no per-size variants. There is **no database manifest**: presence on disk is the source of truth.
  Writes are atomic (write a temporary sibling, then rename).
- **Not the HTTP cache.** Image fetches bypass `http_cache` via `http::Client::get_bytes_uncached`; `http_cache` stays
  JSON-response-only.
- **The render layer reads locally.** The UI resolves the flat path and loads the file; it never triggers a fetch. A
  missing file is **not** an integrity failure — it is a disposable-cache miss that is repopulated on the next sync. An
  initials placeholder is shown whenever the cached file is absent (e.g. before a character's first sync completes or
  after the cache has been cleared).
- **Scope, freshness, and weekly staleness.** Character portraits are fetched by the character-profile job;
  corporation/alliance logos by the jobs that introduce those rows, bounded by the ids actually present in the data.
  Refresh keys off sync cadence, with an added **weekly (~7-day) sync-time staleness check**: a portrait/logo whose
  cached file is older than ~7 days is refetched and overwritten (EVE users do change their portraits). **Type icons
  are exempt — they are immutable and are never refetched on staleness or evicted.**

Both item icons and portraits/logos are now non-fatal: item icons ship pre-generated and a missing/failing icon shows a
silhouette, while a portrait/logo is a disposable cache entry whose absence shows a placeholder and is repopulated on
the next sync.

## Affected Areas

- `scripts/generate/item-images` + `yq`/`jq` in `.config/mise.toml` — the icon-set generator (with the coverage-floor
  gate); `jq` parses the SDE `latest.jsonl` build number for the cross-release cache key.
- `.gitignore` — ignores `assets/images/items/*` (the generated PNGs and the generator's `.tmp.*`/`.hdr.*` scratch
  files) while keeping the `.no-icon` ledger tracked via a negation.
- `.github/workflows/release.yml` — the `generate-icons` job (SDE-build-number `actions/cache` keying the generated
  set) and the per-package-job artifact download.
- `assets/images/items/{type_id}.png` — the generated 64px set (built per release, gitignored).
- `src/config.rs` — the `resource_dir()` helper.
- `src/store/images.rs` — the committed-tier fallback in `resolve_type_icon`.
- `src/sync/jobs/character_clones.rs`, `src/sync/jobs/abyssals.rs` — icon fetches demoted to best-effort.
- The render sites (`inventory.rs`, `clones.rs`, `killlog.rs`) — all resolve through `resolve_type_icon`.

For synced portraits/logos (evictable cache, revised 2026-06-08):

- `src/store/images.rs` — the cache-rooted on-disk `Store` (flat single-size paths under `resolved_cache_dir()/images`,
  atomic write, weekly staleness refetch; no longer rooted at `{data_dir}/images`).
- `src/clients/eve_image.rs` — image URL builders and the uncached `fetch`; the size enum collapses to the single
  biggest size.
- `src/clients/http.rs` — `get_bytes_uncached` (no cache read/write).
- `src/sync/{engine,job}.rs`, `src/sync/jobs/character_profile.rs` — the engine carries the image client and store; the
  character-profile job fetches and writes the portrait before the commit.
- Boot (`src/app.rs`) — constructs the production `Store` and passes it to the engine.
- The render layer (`src/features/character_manager.rs`) — loads portraits from the store.

## Consequences

### Positive

- Shipped packages render item/ship/implant icons immediately, offline, with no per-item sync fetching and no startup
  download.
- A missing or failing icon can no longer block or fail a privileged sync job.
- The repository no longer carries the ~150 MB icon set — clones stay lean and PRs no longer churn binary blobs.
- Releases always ship a current set with no manual maintainer step, and the CDN is hit at most once per release — and
  not at all when the SDE build number is unchanged, since the cross-release cache reuses the previously gated set.

### Negative

- A fresh checkout has no item icons; the app shows silhouettes until a contributor runs the opt-in
  `mise run generate:item-images` (or the sync long-tail fills them in).
- The shipped set is only as current as the release-time generation; brand-new types rely on the best-effort sync tail
  until the next release.
- A CDN outage during the `generate-icons` job fails the release (by design, via the coverage-floor gate) rather than
  shipping a sparse set.

For synced portraits/logos (evictable cache, revised 2026-06-08):

- The database stays lean (relational data only), and the cache root is disposable: clearing it (or a cache-dir
  override change, ADR-0007) is safe — files are repopulated lazily on the next sync.
- The flat single-biggest-size layout and the no-on-disk-resize rule keep the cache simple; iced scales at render time.
- A transient image-server hiccup no longer blocks an entity's commit — the fetch is skipped and the render layer shows
  a placeholder until the next sync (a deliberate trade of strict completeness for resilience, suitable for a cache).
- A manifest-free cache cannot be queried for freshness or inventory; refresh keys off sync cadence plus the ~7-day
  staleness check. A cached file can be deleted out from under the app, which is fine — it is repopulated lazily — and
  orphan collection for untracked entities relies on cache eviction rather than a DB cascade.

## Future Work

- bpc/bpo blueprint variants are deferred (nothing renders them today; `inventory.rs` passes `is_blueprint_copy =
  None`).
- Icon sizes other than 64px and type renders (512px) remain out of scope — nothing consumes them.

## References

- ADR-0002 — Sync/Render Separation and Aggregation Chaining (`0002-sync-render-separation.md`). Portraits/logos obey
  its completeness contract; this ADR folds in the since-retired "Image Assets as Synced Data" record for the item-icon
  case.
- ADR-0003 — Canonical Data Model (`0003-canonical-data-model.md`)
- ADR-0007 — User-Configurable Storage Paths (`0007-user-configurable-storage-paths.md`). The image cache follows its
  cache root and is repopulated lazily; a cache-dir override change does not move existing image files.
- Spec `kolowpzv` — Pre-generate and Commit Item Icons.
- Spec `powlkvns` — Storage path-resolution authority + image cache overhaul (the 2026-06-08 revision).
- RFC `lpvoysql` — predecessor (downloaded-archive approach, superseded here).
- EVE image server: <https://docs.esi.evetech.net/docs/image_server.html>
