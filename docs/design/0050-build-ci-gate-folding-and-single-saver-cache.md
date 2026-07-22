---
id: "0050"
title: Build CI Gate Folding & Single-Saver R2 Cache
status: active
tags: [ci, infrastructure, caching]
created: 2026-07-22
---

# ADR-0050: Build CI Gate Folding & Single-Saver R2 Cache

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

The Build workflow drops its standalone `check` matrix and the redundant Rust half of `conformance`, folding both
gates into jobs that already compile the same code, and restructures the R2 Rust cache around three rules: exactly
one designated saver per cache namespace per platform, test jobs (whose artifacts are a superset of build artifacts)
save `v0-rust` on Windows and macOS, and the Linux coverage run gets its own `v0-rust-coverage` namespace so the
cargo-llvm-cov instrumented target is finally cached at all. `scripts/ci/rust-cache save` now skips the upload when
the exact key already exists in R2, because the key (OS + rustc + Cargo.lock) embeds everything the contents depend
on, and pipes tar through `gzip -1` when it does save. Together these cut a typical warm main-branch run from about
35 minutes of wall-clock to a projected 18 to 20 minutes and roughly halve total runner-minutes.

## Context

After the apps/ relocation, a warm main-branch Build run (for example run 29894615892 on 2026-07-22) spent about
34.7 minutes of wall-clock across eleven parallel jobs, and the step timings showed the time was not going where
intuition said it was:

- The compiles themselves were fast: warm `cargo build` took 2.0 minutes on Linux and 5.4 on Windows; `mise run
  lint` (including a full `cargo clippy --all-targets --all-features`) took 4.4 minutes.
- The R2 cache save step dominated the critical path: 18.3 minutes on build (ubuntu), 23.1 on build (windows), and
  22.1 on lint. Every main push re-tarred and re-uploaded a multi-GB target directory whose dependency graph had not
  changed.
- test (ubuntu) spent 27.2 minutes because the cargo-llvm-cov instrumented build (in `target/llvm-cov-target`) was
  never cached: test jobs had no save step, and the `v0-rust` cache they restored only ever contained `cargo build`
  artifacts saved by the build jobs, which also means the Windows and macOS test jobs recompiled every test binary
  from scratch on every run.
- The `check` matrix was pure redundancy: `cargo check --all-targets --all-features` is a compile subset of lint's
  clippy invocation on Linux, and of the test + build compiles (with codegen) on Windows and macOS. The desktop
  crate has no benches or examples, so `--all-targets` covers nothing those jobs do not build.
- The `conformance` job's Rust half (`cargo nextest run --all-features conformance::`) is a strict subset of the
  full `--all-features` suite that all three test jobs already run; only its Worker vitest half is unique.

## Decision

### Fold the check and Rust-conformance gates into jobs that already compile the code

The `check` matrix is deleted. The compile-check gate survives by construction: on Linux, lint's
`cargo clippy --all-targets --all-features -- -D warnings` performs the same full-workspace check compile; on
Windows and macOS, the test job compiles every test target plus the binary and the build job compiles the binary,
so any platform-specific compile error still fails the run. The `conformance` job keeps its name and its Worker
vitest half but no longer restores a Rust cache or compiles anything; the Rust conformance suite runs inside every
test job's full-suite invocation on all three platforms, which is strictly more coverage than the old Linux-only
dedicated run.

### One designated saver per namespace per platform

Cache namespaces get exactly one writer each so the contents are deterministic:

| Namespace          | Saved by                        | Restored by                  |
|--------------------|---------------------------------|------------------------------|
| default            | lint                            | lint                         |
| v0-rust (linux)    | build (ubuntu)                  | build (ubuntu)               |
| v0-rust (win/mac)  | test (windows/macos)            | test + build (windows/macos) |
| v0-rust-coverage   | test (ubuntu)                   | test (ubuntu)                |

Test jobs become the `v0-rust` savers on Windows and macOS because a nextest build is a superset of a
`cargo build`: it compiles the same dependency graph plus every test binary. The build jobs there go restore-only.
Linux keeps build as its saver because its test job runs under cargo-llvm-cov, whose separately-instrumented target
is useless to (and cannot reuse) plain artifacts; it caches under the new `v0-rust-coverage` namespace instead,
making the coverage build incremental for the first time.

### Skip saves whose exact key already exists

`scripts/ci/rust-cache save` now checks `ci:r2 exists` for the exact object key and skips the tar + upload on a
hit. The key derivation is untouched: OS + rustc version + Cargo.lock hash. Those three inputs are the only things
the cached dependency artifacts are derived from, so an existing object for the key is still valid; source-only
drift on top of it rebuilds incrementally after restore, which is exactly the warm-compile path that already takes
single-digit minutes. This turns the 15 to 20 minute save step into a no-op on every run that does not change
Cargo.lock or the toolchain. `POD_RUST_CACHE_FORCE_SAVE=1` bypasses the check if a cache ever needs a manual
refresh. When a save does happen, tar pipes through `gzip -1` (same on-disk format the restore path already reads,
several times faster on multi-GB targets), with a marker file so a mid-pipe tar failure never uploads a truncated
archive over a good one.

## Affected Areas

- `.github/workflows/build.yml`: job graph, cache-saver assignments, deleted check matrix, node-only conformance.
- `scripts/ci/rust-cache`: save-path behavior only; restore path and key derivation unchanged.
- `release.yml` (indirectly): its `v0-pkg*` saves go through the same script and now also skip when unchanged,
  which shortens release wall-clock; the same staleness reasoning applies.
- The `main` ruleset's required status checks: the `check` context no longer exists and must be removed from the
  ruleset; `lint`, `test`, and `build` keep their names.

## Consequences

- Typical warm main-branch wall-clock drops from about 35 minutes to a projected 18 to 20 (critical path becomes
  the test jobs), and total runner-minutes fall from roughly 240 to under 110.
- Cache contents freeze between Cargo.lock/rustc changes, so warm compile times creep up as source drifts from the
  last save. Dependabot bumps the lock near-daily, which re-saves fresh artifacts; `POD_RUST_CACHE_FORCE_SAVE=1`
  is the manual escape hatch.
- On Windows and macOS the existing `v0-rust` objects (build-only artifacts) remain valid for the current key, so
  the first save of richer test artifacts is deferred until the next lock or toolchain change; test jobs stay at
  their old duration until then.
- The first run after this change is cold for `v0-rust-coverage` (one full instrumented build plus save); every
  existing namespace stays warm because no key or namespace it uses changed.
- Runs that do change Cargo.lock still pay one full save per namespace, now cheaper via `gzip -1`.
