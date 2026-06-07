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

> **Revised 2026-06-06:** the loose-PNG set is no longer committed to the repository. It is now gitignored and generated
> once per release by the GitHub Actions pipeline, then shipped inside each OS package via a shared workflow artifact.
> The Decision, Consequences, and Affected Areas below reflect this; the two-tier resolver and best-effort sync
> long-tail are unchanged.
>
> **History:** item/type icons were originally treated as synced data — fetched per-entity during sync with a fatal
> fetch-failure rule (in a since-retired "Image Assets as Synced Data" ADR). This ADR replaces that approach for item
> icons with the committed/CI-generated set described below, and absorbs the still-active portrait/logo decision from
> that retired record. Character portraits and corporation/alliance logos *remain* synced data, as documented under
> [Synced portraits and logos](#synced-portraits-and-logos).

## Summary

This ADR governs the whole image-assets decision area. **Item/type icons** are no longer fetched per-item during sync;
instead a set of loose 64px PNGs under `assets/images/items/{type_id}.png` is generated once per release by the CI
pipeline and shipped inside each OS package, with the working-tree set gitignored rather than committed. The runtime
resolves an icon through a **two-tier resolver**: the writable `data_dir()/images` store first, then the generated set
in the bundle. Sync is demoted to a **best-effort long-tail** that fills in only types newer than the shipped set, and
an icon fetch can never block or fail a sync job. This replaces the original "images are synced data, fetch-fail is
fatal" rule for the item-icon case.

**Character portraits and corporation/alliance logos remain synced data:** the sync engine fetches them while building
an entity's complete dataset, writes them to a durable on-disk store at derived paths, and commits the entity only once
its images are present. See [Synced portraits and logos](#synced-portraits-and-logos).

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
`mise run package`, so the CDN is hit once per release and the Windows runner never executes the POSIX generator.

**A two-tier runtime resolver.** `resolve_type_icon` (`src/store/images.rs`) consults, in order:

1. `data_dir()/images` — the writable store where the sync long-tail writes.
2. The generated items dir — `assets/images/items/` in a dev build, the cargo-packager resources dir in a packaged
   build, located by a `resource_dir()` helper that probes for the bundled `assets/` tree rather than trusting a
   compile-time path.

The data-dir tier keeps priority so a freshly-synced icon shadows the shipped one; only when both miss is the icon
`Missing` (the UI shows a silhouette). A fresh checkout with no generated icons resolves everything to silhouettes until
a contributor runs the opt-in generator.

**Sync demoted to best-effort long-tail.** The clone and abyssal jobs skip fetching when an icon is already on disk and,
on a fetch/write failure, log a warning and continue. These fetches now exist only to fill types newer than the
shipped set; they can never block or fail a sync job.

### Synced portraits and logos

Character portraits and corporation/alliance logos are **not** part of the item-icon set above. They remain *synced
data*: a presentational binary is part of an entity's dataset, not a render-time concern, and is treated under the same
completeness contract as the rest of that dataset (ADR-0002). Three reasons drive this:

- **`http_cache` is a freshness cache.** It keys entries by URL with a `Cache-Control` lifetime and prunes/revalidates
  them, so images placed there are not durable — they vanish on eviction and are unavailable offline.
- **Large binaries do not belong in SQLite.** Storing portraits and logos as BLOBs bloats the database file, its page
  cache, and every backup, and makes `VACUUM` expensive. The relational store holds relational data only.
- **Images are data.** Treating them as a render side effect would let a row exist without its image; an entity's
  dataset is not *complete* until its images are present.

The rules:

- **Synced before commit.** The sync engine fetches an entity's portraits/logos at the canonical UI sizes while
  assembling its dataset and writes them **before** committing the entity. A fetch failure fails the job — nothing is
  persisted and the cycle retries — exactly like any other step in the completeness gate (ADR-0002). So a committed
  entity always has its images.
- **Durable on-disk store at derived paths.** Images live as files under `{data_dir}/images` at deterministic paths
  derived from `(category, id, size)` — e.g. `images/characters/{id}/portrait_256.jpg`. There is **no database
  manifest**: presence on disk is the source of truth. Writes are atomic (write a temporary sibling, then rename).
- **Not the HTTP cache.** Image fetches bypass `http_cache` via `http::Client::get_bytes_uncached`; `http_cache` stays
  JSON-response-only.
- **The render layer reads locally.** The UI resolves the derived path and loads the file; it never triggers a fetch. A
  missing file is an integrity miss (a re-sync trigger), not a render-time placeholder. An initials placeholder is kept
  only as a defensive fallback (e.g. before a character's first sync completes).
- **Scope and freshness.** Character portraits are synced by the character-profile job; corporation/alliance logos are
  synced by the jobs that introduce those rows, bounded by the ids actually present in the data. With no manifest,
  refresh is driven by the owning entity's sync cadence: a job re-fetches and overwrites the image each cycle (images
  are small).

This is the one place item icons and portraits/logos diverge: item icons ship pre-generated and a missing/failing icon
is non-fatal, whereas a portrait/logo is fetched at sync time and its absence blocks the entity's commit.

## Affected Areas

- `scripts/generate/item-images` + `yq` in `.config/mise.toml` — the icon-set generator, with the coverage-floor gate.
- `.gitignore` — ignores `assets/images/items/*` (the generated PNGs and the generator's `.tmp.*`/`.hdr.*` scratch
  files) while keeping the `.no-icon` ledger tracked via a negation.
- `.github/workflows/release.yml` — the `generate-icons` job and the per-package-job artifact download.
- `assets/images/items/{type_id}.png` — the generated 64px set (built per release, gitignored).
- `src/config.rs` — the `resource_dir()` helper.
- `src/store/images.rs` — the committed-tier fallback in `resolve_type_icon`.
- `src/sync/jobs/character_clones.rs`, `src/sync/jobs/abyssals.rs` — icon fetches demoted to best-effort.
- The render sites (`inventory.rs`, `clones.rs`, `killlog.rs`) — all resolve through `resolve_type_icon`.

For synced portraits/logos:

- `src/store/images.rs` — the rooted on-disk `Store` (derived paths, atomic write).
- `src/clients/eve_image.rs` — image URL builders and the uncached `fetch`.
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
- Releases always ship a freshly-generated set with no manual maintainer step, and the CDN is hit exactly once per
  release.

### Negative

- A fresh checkout has no item icons; the app shows silhouettes until a contributor runs the opt-in
  `mise run generate:item-images` (or the sync long-tail fills them in).
- The shipped set is only as current as the release-time generation; brand-new types rely on the best-effort sync tail
  until the next release.
- A CDN outage during the `generate-icons` job fails the release (by design, via the coverage-floor gate) rather than
  shipping a sparse set.

For synced portraits/logos:

- Images are durable: they survive cache pruning and are available offline, and the database stays lean (relational
  data only).
- Completeness is enforced at the source: an entity is committed only once its images are on disk, and the render layer
  is trivial — resolve a path and load it.
- An entity sync fails if the image server is unreachable (completeness is strict): a transient image hiccup blocks that
  entity's profile until the next retry.
- A manifest-free store cannot be queried for freshness or inventory; refresh keys off sync cadence. A committed row's
  image file can be deleted out from under the app (mitigated by treating a missing file as a re-sync trigger), and
  orphan collection for untracked entities has no DB cascade.

## Future Work

- bpc/bpo blueprint variants are deferred (nothing renders them today; `inventory.rs` passes `is_blueprint_copy =
  None`).
- Icon sizes other than 64px and type renders (512px) remain out of scope — nothing consumes them.

## References

- ADR-0002 — Sync/Render Separation and Aggregation Chaining (`0002-sync-render-separation.md`). Portraits/logos obey
  its completeness contract; this ADR folds in the since-retired "Image Assets as Synced Data" record for the item-icon
  case.
- ADR-0003 — Canonical Data Model (`0003-canonical-data-model.md`)
- Spec `kolowpzv` — Pre-generate and Commit Item Icons.
- RFC `lpvoysql` — predecessor (downloaded-archive approach, superseded here).
- EVE image server: <https://docs.esi.evetech.net/docs/image_server.html>
