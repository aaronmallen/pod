---
id: "0022"
title: Linux Graphics Backend Diagnostics and AppImage GPU Stack
status: active
tags: [packaging, rendering, observability, linux, appimage, wgpu]
created: 2026-06-13
---

# ADR-0022: Linux Graphics Backend Diagnostics and AppImage GPU Stack

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

On GNOME/Wayland via the AppImage, Pod silently falls back to iced's `tiny_skia` software renderer when it cannot
initialize a wgpu (Vulkan/GL) backend. That fallback has real rendering defects (ignored per-image clip bounds and
`border_radius`) and is slower, but it is **invisible**: iced 0.14 auto-selects wgpu with a `tiny_skia` fallback and
exposes no public API for the chosen backend, and the file-log filter even pinned `wgpu_core`/`iced_wgpu` to `warn`,
suppressing iced's own adapter-selection lines. This ADR records the two cross-cutting decisions that govern the fix:

1. **Probe wgpu directly for the diagnostic**, rather than relying solely on scraping upstream log lines. Pod creates
   its own `wgpu::Instance` at startup, enumerates adapters, and logs them under a stable `pod::graphics` target — a
   source of truth we own and that CI can assert against. The file-log filter is *also* relaxed so iced's actual
   selection (and any `tiny_skia` fallback) is observable, but the owned probe does not depend on upstream wording.
2. **The AppImage bundles GL/EGL/Vulkan *loaders* (libGL/libEGL/libvulkan + Mesa), never host *drivers/ICDs***. Bundling
   a build host's GPU drivers into a portable AppImage is the well-known AppImage GPU footgun: a driver compiled against
   the build machine's kernel/GPU mismatches on user machines and breaks acceleration. We ship the loaders wgpu links
   against and let each user's host provide the kernel driver / ICD (with Mesa's llvmpipe as the universal software-GL
   floor).

This ADR is the convention the diagnostics task, the AppImage packaging task, and the CI-verification task all
implement against; they consume these decisions, they do not re-decide them.

## Context

`iced::daemon(...).run()` in `src/app.rs` builds iced's renderer with no in-tree configuration: iced 0.14 selects a
wgpu backend if it can and silently falls back to `iced_tiny_skia` otherwise. There is **no public API** to ask iced
which renderer it chose. The original report — portrait overflow and square avatar corners on a GNOME/Wayland AppImage
— is consistent with the `tiny_skia` path (it ignores per-image clip bounds and `border_radius`), but until this work
the diagnosis was inference, not confirmation from real hardware, because nothing logged the backend.

Two forces shape the design:

- **The renderer choice must be confirmable from a shipped log.** A field log from a user's machine has to answer "real
  wgpu backend, or `tiny_skia`?" greppably. iced *does* log its adapter selection through the `log` crate (bridged into
  `tracing`), but the file-log filter pinned `iced_wgpu`/`wgpu_core` to `warn`, swallowing the one-shot init lines. So
  part of the answer is just *stop suppressing* those targets. But upstream wording is version-specific and fragile to
  assert against, so Pod also needs a diagnostic it **owns**.

- **The shipped AppImage must reach a wgpu backend on Wayland without a native Vulkan stack.** wgpu needs GL/EGL/Vulkan
  *loaders* present at runtime to initialize any backend; linuxdeploy only bundles libraries that are present on the
  build host when it packages. If CI installs only `libgtk-3-dev libxdo-dev`, no Mesa/EGL/GL/Vulkan loader is on the
  builder for linuxdeploy to discover, so the AppImage ships without the stack wgpu needs and falls back to
  `tiny_skia`. The naive fix — bundle everything including drivers — reintroduces the AppImage GPU footgun.

## Decision

### 1. Own the diagnostic: probe wgpu directly

At startup, immediately after `init_tracing` (and the panic hook) and before `iced::daemon(...).run()` in
`src/app.rs::run()`, Pod calls a small `app::graphics::probe()`:

- It creates its **own** `wgpu::Instance` — separate from the one iced builds — and calls
  `enumerate_adapters(Backends::all())`.
- For each adapter it logs one line under the stable target **`pod::graphics`** carrying `name`, `backend`
  (Vulkan/Gl/Metal/Dx12/…), and `device_type` (DiscreteGpu/IntegratedGpu/Cpu/…) as structured fields.
- It then logs a single summary line with `adapter_count` and a `software_only` flag (true only when *every* adapter is
  a `Cpu` device type).
- If enumeration returns **nothing**, it logs a `warn` containing the literal token `tiny_skia` ("no wgpu graphics
  adapters found; iced will fall back to the tiny_skia software renderer"). It never panics on empty/failed
  enumeration — it logs and returns, leaving behavior unchanged.

`wgpu` is added as a **direct dependency pinned to `27`**, the version already resolved transitively through iced, so
the two stay unified and the probe shares the already-compiled backends.

**The probe answers "what adapters can wgpu see," not "what did iced select."** Those are different questions: the probe
uses a distinct `Instance`, and on a GPU-less runner an adapter may report `device_type = Cpu` (Mesa llvmpipe) yet still
yield a perfectly good wgpu **Gl** backend — which is success, not the `tiny_skia` fallback. So `software_only` is a
*device-type* signal, **not** a fallback signal. The authoritative "what did iced actually pick" answer comes from
relaxing the file-log filter (§2). Owning the probe is what makes CI's assertion robust against iced's exact log
wording while the relaxed filter corroborates the real selection.

### 2. Relax the file-log filter so iced's real choice is observable

The `FILE_FILTER` in `src/app.rs` raises two targets from `warn` to `info`:

- `iced_wgpu=info` — surfaces iced's own selected-adapter line.
- `wgpu_core=info` — surfaces wgpu's adapter/device creation line.

These are **one-shot at init, not per-frame**, so the log-volume cost is negligible. `wgpu`, `wgpu_hal`, and
`iced_winit` stay at `warn` (those *can* be per-frame chatty). iced's `tiny_skia` fallback warning sits at the global
`warn` floor and is therefore already not suppressed, so the literal `tiny_skia` token reaches the file on the fallback
path. Net effect: a shipped log distinguishes "wgpu backend X selected" from "fell back to tiny_skia" — both from
iced's own lines and from the owned `pod::graphics` probe.

### 3. AppImage bundles loaders, never drivers/ICDs

The Linux build/package path installs the GL/EGL/Vulkan **loader** stack plus Mesa (the loaders wgpu links against and a
software-GL/llvmpipe path), so linuxdeploy bundles what wgpu needs. It must **not** pull host-specific GPU
**drivers/ICDs** into the AppImage: a driver built against the build host's kernel/GPU will mismatch on user machines
and break acceleration (the AppImage GPU footgun). The contract is: **Pod ships the loaders; the user's host provides
the kernel driver / ICD; Mesa llvmpipe is the universal software-GL floor** so a wgpu Gl backend initializes even with
no hardware acceleration. Any belt-and-suspenders environment lever (e.g. `WGPU_BACKEND`) only applies on Linux when the
variable is unset and never overrides an explicit user setting. The concrete package list and packaging mechanism are
settled by the AppImage packaging task; this ADR fixes the loaders-not-drivers boundary it must honor.

## Affected Areas

- `src/app.rs` — `run()` calls `graphics::probe()` after `init_tracing`/`install_panic_hook` and before
  `iced::daemon(...).run()`; `FILE_FILTER` raises `iced_wgpu` and `wgpu_core` to `info`; `mod graphics;` is declared.
- `src/app/graphics.rs` — new module: the wgpu probe, the `pod::graphics` log lines, and the `software_only`
  classification.
- `Cargo.toml` — adds `wgpu = "27"` as a direct dependency (unified with the version iced resolves transitively).
- `.github/actions/setup/action.yml` / `.github/workflows/release.yml` — the Linux package path installs the loader +
  Mesa stack (AppImage packaging task) and runs the packaged AppImage headlessly to assert a wgpu backend, not
  `tiny_skia` (CI-verification task). These are implemented by their own tasks against this ADR.

## Consequences

### Positive

- The graphics situation is confirmable from any shipped log: available adapters (owned `pod::graphics` probe) plus
  iced's actual selection (relaxed filter). Future graphics issues are diagnosable instead of inferred.
- CI can assert on tokens Pod owns and on the `tiny_skia` marker, making the fix machine-checkable and regression-proof.
- Bundling loaders but not drivers eliminates the silent `tiny_skia` fallback on Wayland while avoiding the
  AppImage-bundled-driver mismatch footgun; llvmpipe guarantees a wgpu Gl backend even with no GPU.

### Negative

- A second `wgpu::Instance` is created at startup purely for the diagnostic (cheap, one-shot enumeration), and it is
  *not* the instance iced uses — so the probe reports availability, not iced's final selection. This nuance must be
  understood when reading the logs (documented in §1).
- Raising `wgpu_core`/`iced_wgpu` to `info` adds a few init-time lines to the file log; this is bounded because they are
  one-shot, but it is more than the previous `warn` floor.
- `software_only` is a device-type heuristic, not a fallback detector: llvmpipe reports `Cpu` yet yields a good Gl
  backend, so `software_only = true` must not be read as "tiny_skia." The fallback signal is the `tiny_skia` token, not
  this flag.

## References

- ADR-0012 — Logging and Observability Conventions (`0012-logging-and-observability.md`). The `pod::graphics` target and
  the file-log `FILE_FILTER` follow that namespace/dual-sink convention.
- Sibling spec (renderer-agnostic avatar/portrait clipping) makes clip correctness independent of the renderer; this
  ADR removes the wrong renderer from the path entirely.
- iced 0.14 auto-selects wgpu with a `tiny_skia` fallback and exposes no forced-renderer configuration in-tree.
- `docs/process/writing-adrs.md` — ADR format and status lifecycle this record follows.
