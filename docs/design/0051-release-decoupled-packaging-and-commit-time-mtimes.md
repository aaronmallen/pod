---
id: "0051"
title: Release Decoupled Packaging & Commit-Time Mtimes
status: active
tags: [ci, infrastructure, caching]
created: 2026-07-22
---

# ADR-0051: Release Decoupled Packaging & Commit-Time Mtimes

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The Release workflow stops queueing the entire package gauntlet behind the Build workflow: packaging starts the
moment the tag lands, and `wait-for-build` becomes a pure gate that only `publish-release` consumes, so nothing
publishes unless Build passed and a Build failure still fails the run. Package jobs additionally stamp every
tracked file with the tagged commit's committer date before building (`scripts/ci/commit-mtime`), which makes a
same-commit rerun (an rc tag followed by the real tag on the identical commit) fingerprint-clean against the
restored R2 cache and skips the roughly 52-minute leaf-crate release recompile on macOS. Together with the
save-skip from ADR-0050 (which already covers the `v0-pkg*` namespaces), a real-tag Release run drops from about
99 minutes of wall-clock to a projected 40, and an rc run from about 95 to a projected 60.

## Context

The 0.7.5 release cycle on 2026-07-22 provides the baseline. The rc run (29894617161) took 1h 35m and the real-tag
run (29899774263) took 1h 39m, with this critical path on the real tag:

- `wait-for-build` 36.9 min: every package job needed this job, so the whole gauntlet queued behind the full Build
  workflow even though packaging consumes nothing Build produces. Build runs on the same tag push and gates the
  candidate anyway; serializing it in front of packaging bought no additional safety on the real tag, whose commit
  the rc had already proven.
- `package-macos (aarch64)` 59.3 min, of which `cargo build --release` was 52m 42s. The job had restored the rc
  run's cache at the exact key (same commit, same Cargo.lock, same rustc) in about 33 seconds, and cargo's log
  shows exactly one `Compiling` line: every dependency was reused and only the leaf crate rebuilt. The same
  pattern held on Windows (9m 52s) and Linux (5m 12s). The only fingerprint input that differed from the run that
  saved the cache was file mtimes: a fresh checkout stamps sources newer than the restored artifacts, and cargo's
  release-profile freshness check is mtime-based.
- The remainder is packaging proper and is not workflow-addressable: NSIS compression about 9 min and WiX about
  3.5 min on Windows, dmg plus updater tarball about 2.5 min on macOS. The 2 to 3.5 min cache-save tails are
  already usually no-ops via ADR-0050's exists-check, which the `v0-pkg*` namespaces share.

## Decision

### Decouple packaging from the Build gate

`create-release` and `generate-icons` start immediately on the tag push, and the package jobs need only those
two. `wait-for-build` keeps its name and its polling step but loses its rc-detection output; `publish-release`
lists it in `needs` with default semantics, so a Build failure blocks publishing (and fails the run) without
sitting in front of the package gauntlet. RC detection moves from a job output to inline
`contains(github.ref_name, '-rc.')` expressions with the same literal-substring semantics the old bash check had.
On a real tag the draft release is now created while Build is still running; `publish-release` flips it public
only after the gate passes, so a failed run can leave behind an unpublished draft, which is invisible to users.

### Stamp sources with the commit time so same-commit reruns reuse the cache

`scripts/ci/commit-mtime` sets every tracked file's mtime to the checked-out commit's committer date. The
workflow checkout is depth-1, so the stamp is uniform and deterministic: the same commit always produces the same
mtimes. The rc run builds and saves `v0-pkg*`; the real-tag run on the identical commit restores that cache, sees
no source newer than the cached artifacts, and the release build finishes in seconds instead of re-paying the
leaf-crate compile. Any new commit moves every tracked mtime forward, so cutting a new rc always rebuilds.

An under-rebuild (cargo wrongly reusing stale artifacts) would require the same package version, the same
Cargo.lock, and a tagged commit whose committer date predates the cached outputs. Real releases always bump the
version in Cargo.toml, which changes the crate fingerprint and forces the rebuild regardless of mtimes; rc fix
commits are created after the previous candidate ran, so their committer dates are newer than its cache. The
remaining exposure is tagging a commit created before the newest cache save for an unchanged version and lock,
which the rc-first process does not produce. If a stale reuse is ever suspected, cut a new rc (new commit, newer
mtimes) or prune the `v0-pkg*` objects in R2.

### Explicitly unchanged

Job names, the tag trigger and `-rc.` semantics, the full artifact set per platform, the R2 cache namespaces and
key derivation (no forced re-warm), the item-icons flow and its known Windows download flake playbook
(rerun-with-failed, never retries in the workflow), and the steps of `create-release`, `publish-release`, and
`deploy-pages`.

## Affected Areas

- `.github/workflows/release.yml`: job graph and rc-detection wiring; new mtime-stamp step in the package jobs.
- `scripts/ci/commit-mtime`: new script, POSIX sh, GNU and BSD date compatible.
- `docs/process/release.md`: wording for what waits on Build (publishing, no longer packaging).
- Release-only jobs (`create-release`, `publish-release`, `deploy-pages`) skip rc tags, so their rewired
  conditions first fire on a real tag; the changes are wiring-only and the steps are untouched.

## Consequences

- Projected wall-clock: a real tag becomes max(Build, packaging) plus the publish tail. With packaging legs at
  roughly 7 min (macOS, clean), 16 min (Windows, NSIS/WiX bound), and 10 min (Linux), the real tag lands around
  40 minutes today and tracks Build's own wall-clock as it improves. An rc run is bounded by the unavoidable
  leaf-crate release compile on macOS at roughly 55 to 60 minutes. Measured numbers are deferred to the next
  release cycle.
- A Build failure no longer prevents packaging from running, so a genuinely broken rc burns package-runner
  minutes it previously saved; the run still fails via the gate, and rc artifacts were ephemeral anyway.
- `build.rs` embeds a build date; if an rc and its real tag straddle midnight UTC the value changes and the leaf
  crate recompiles once. Correctness is unaffected.
- Untracked files keep fresh checkout mtimes. The generated item icons are cargo-packager resources, not compile
  inputs, so they do not dirty the build.
- The stamp step depends on tar-preserved mtimes in the R2 cache and cargo's mtime-based freshness; if cargo
  moves to checksum freshness the step becomes harmless rather than wrong.
