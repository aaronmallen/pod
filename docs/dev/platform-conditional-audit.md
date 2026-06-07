# Platform-conditional code audit

A static, code-level audit of every platform gate (`#[cfg(target_os = ...)]`,
`#[cfg(not(target_os = ...))]`, `#[cfg(windows)]`, `#[cfg(unix)]`,
`#[cfg(any(...))]`) across the crate, plus `Cargo.toml` per-target dependencies
and the `windows_subsystem` attribute. The app targets macOS, Windows, and Linux
but was audited on macOS only — this is a code-symmetry review, not a runtime
test on each OS.

**Verdict legend:** *correct* (the gate does the right per-OS work) ·
*intentional documented no-op* (a deliberate platform-only treatment with no
counterpart needed elsewhere) · *gap-fixed* (an unintended asymmetry that was
corrected during this audit).

Date: 2026-06-06.

## Summary

- **Platform gates catalogued: 17** (excludes 3 `#[cfg(not(test))]` sites that
  are test gates, not platform gates — listed at the end for completeness).
- **Correct: 16**
- **Intentional documented no-op: 1** (`disable_shadow` non-macOS arm)
- **Gap-fixed: 0** (no behavioral fixes; one clarifying comment added)

No asymmetry bugs were found. Deep-link install/forwarding and the native menu
are complete and consistent across all three OSes. `disable_shadow` is the only
genuinely macOS-only path and its no-op elsewhere is correct.

## Gate catalogue

### `src/main.rs`

| line | gate | macOS | Windows | Linux | verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | `#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]` | n/a (no console subsystem concept) | builds as a GUI subsystem binary, so launching from Explorer / a `.url` scheme handler shows no flash of a console window | n/a | correct |

`windows_subsystem = "windows"` is exclusively a Windows linker concern; macOS
and Linux have no console-subsystem attribute and need none. Correctly gated.

### `src/app.rs` — `disable_shadow`

The splash window is opened (line ~416-424) with `decorations: false,
transparent: true`. On macOS a transparent, undecorated `NSWindow` still draws a
system drop shadow around its bounds, which is visually wrong for the
rounded/transparent splash. `disable_shadow` sends `setHasShadow: false` to the
backing `NSWindow` via `objc2`.

| line | gate | behavior | verdict |
| --- | --- | --- | --- |
| 2134 | `#[cfg(target_os = "macos")]` `fn disable_shadow` | resolves the AppKit `RawWindowHandle`, walks `ns_view -> window`, and calls `setHasShadow: false` to remove the macOS system drop shadow on the transparent splash | correct |
| 2156 | `#[cfg(not(target_os = "macos"))]` `fn disable_shadow` | returns `Task::none()` — Windows and Linux do not draw a system shadow around a transparent undecorated window, so there is nothing to suppress | intentional documented no-op |

The two arms form a complete `cfg`/`cfg(not)` pair, so the function exists on
every target and the single call site (`on_window_opened` →
`Some(Window::Splash) => disable_shadow(id)`, line ~2161-2166) compiles and
behaves uniformly everywhere. A clarifying `//` comment was added above the
non-macOS arm during this audit explaining why it is a no-op (see *Code changes*
below). No behavioral change.

### `src/services/menu.rs` — native menu

The menu is built once with `muda` in `build()` (line 110), producing a single
"Pod" submenu containing About Pod, Check for Updates…, Clear Cache, and Quit
(Cmd/Ctrl+Q via `Modifiers::META`). `init()` (line 62) leaks the menu for the
process lifetime and starts the event pump.

| line | gate | macOS | Windows | Linux | verdict |
| --- | --- | --- | --- | --- | --- |
| 73 | `#[cfg(target_os = "macos")]` | `menu.init_for_nsapp()` attaches the built menu to the running `NSApp` (the platform-specific app-wide install step muda requires on macOS) | n/a | n/a | correct |
| 75 | `#[cfg(not(target_os = "macos"))]` | n/a | `let _ = menu;` — muda owns the platform wiring (the menu bar is attached per-window by the windowing layer); no app-wide init call exists to make here | same as Windows | correct |

This is the documented muda contract: on macOS the menu must be explicitly
installed onto the `NSApp`, whereas on Windows/Linux muda attaches the menu
through its own window integration and there is no equivalent app-wide init.
Both arms run `spawn_event_pump()` afterward (line 78), so menu **action
dispatch is identical on all three OSes**: `muda::MenuEvent::receiver()` is
drained on the `pod-menu-events` thread, each event id is mapped by the
platform-independent `action_for_id` (line 45) to a `MenuAction`, and `deliver`
forwards it into the iced subscription stream. No `MenuAction` mapping, build
step, or accelerator is platform-gated. Correct and consistent.

### `src/features/auth/deep_link.rs` — module wiring & forwarding

| line | gate | macOS | Windows | Linux | verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | `#[cfg(target_os = "linux")] mod linux;` | not compiled | not compiled | compiled | correct |
| 3 | `#[cfg(target_os = "macos")] mod macos;` | compiled | not compiled | not compiled | correct |
| 5 | `#[cfg_attr(target_os = "macos", allow(dead_code))] mod single_instance;` | compiled but unused (mac uses Apple Events, not single-instance forwarding), hence `allow(dead_code)` | compiled & used | compiled & used | correct |
| 7 | `#[cfg(target_os = "windows")] mod windows;` | not compiled | compiled | not compiled | correct |
| 34 | `#[cfg_attr(target_os = "macos", allow(dead_code))] pub fn set_pending` | defined; unused on mac (no `forward_or_claim` path stashes a URL there) so dead-code-allowed | used | used | correct |
| 42 | `#[cfg(target_os = "macos")] macos::install();` | registers the Apple Event GetURL handler | — | — | correct |
| 44 | `#[cfg(target_os = "windows")] windows::install();` | — | becomes single-instance primary + spawns the IPC listener | — | correct |
| 46 | `#[cfg(target_os = "linux")] linux::install();` | — | — | becomes single-instance primary + spawns listener, then self-registers the `.desktop` x-scheme-handler | correct |
| 50 | `#[cfg(any(target_os = "linux", target_os = "windows"))] fn forward_or_claim` | n/a | reads the `eveauth-pod://` URL from argv; forwards it to the primary over the local socket (returns `true` → caller exits), else stashes it as pending and continues as primary | same as Windows | correct |
| 62 | `#[cfg(not(any(target_os = "linux", target_os = "windows")))] fn forward_or_claim` | returns `false` — macOS delivers deep links via Apple Events to the already-running app, so there is no argv-forward-then-exit path | n/a | n/a | correct |
| 67 | `#[cfg(any(target_os = "linux", target_os = "windows"))] fn url_from_args` | n/a | argv scan for the scheme prefix (only used by the forward path) | same | correct |

Platform-independent pieces (no gate, shared by all OSes): `SCHEME`, `deliver`,
`subscription`/`stream`, and the `PENDING`/`SENDER` statics. `install()` (line
41) is itself ungated and dispatches into exactly one platform `install`, so it
exists and is called identically from `main.rs` → `auth::install()` →
`deep_link::install()` on every OS.

### Deep-link submodules

- **`deep_link/macos.rs`** — registers a `PodDeepLinkHandler` via
  `NSAppleEventManager::setEventHandler...forEventClass(kInternetEventClass,
  kAEGetURL)`. When macOS routes an `eveauth-pod://` open to the running app, the
  handler extracts the URL (`keyDirectObject`) and calls `super::deliver`. This
  is the macOS equivalent of the single-instance forward: the OS itself routes
  the URL to the live process, so no argv handoff is needed. `install()` forgets
  the handler to keep it alive for the process lifetime.
- **`deep_link/windows.rs`** — `install()` tries `try_become_primary()`; if it
  wins the lock it `spawn_listener`s to receive forwarded URLs. (Scheme
  registration on Windows is handled by the packager via the
  `deep-link-protocols` metadata in `Cargo.toml`, not by self-registration at
  runtime.)
- **`deep_link/linux.rs`** — `install()` does the same single-instance
  primary/listener dance as Windows **and** self-registers the scheme by writing
  a `dev.aaronmallen.pod.desktop` entry (with `MimeType=x-scheme-handler/<scheme>`
  and `Exec=... %u`) into the user applications dir, then runs
  `update-desktop-database` (best-effort, warn-on-failure). This is the narrow
  Linux self-register noted in the project decisions.
- **`deep_link/single_instance.rs`** — ungated, shared by Linux and Windows
  (`allow(dead_code)` on mac). Uses `interprocess` `GenericNamespaced` local
  sockets. `socket_name()` derives a per-data-dir name via a SHA-256 hash so the
  socket is stable per install. `forward_to_primary` connects and writes the
  URL; `accept_loop` reads, `validate`s the scheme prefix (rejects non-`eveauth-pod`
  payloads), and `deliver`s. This is the **forward-then-exit** mechanism: a
  second instance forwards its URL to the primary and `main.rs` exits.

**Deep-link conclusion:** install + forwarding is complete and consistent across
all three OSes. macOS uses Apple Events (OS-routed delivery to the live app);
Windows and Linux use single-instance IPC forwarding (second instance forwards
the argv URL to the primary, then `main` exits); Linux additionally self-registers
the scheme; Windows/macOS rely on the packager for registration. All three paths
funnel into the same shared `deliver` → `subscription` stream, so downstream
handling is uniform.

### `Cargo.toml`

| line | gate | effect | verdict |
| --- | --- | --- | --- |
| 36 | `[target.'cfg(target_os = "macos")'.dependencies]` | pulls in `objc2`, `objc2-core-services` (AE feature), and `objc2-foundation` (Apple Event descriptor/manager + NSString) only on macOS | correct |

These crates back `deep_link/macos.rs` (Apple Events) and `app.rs::disable_shadow`
(`setHasShadow:`). They are macOS-only by nature and must not be compiled on
Windows/Linux. The `deep-link-protocols` packager metadata (line 33-34,
ungated) handles scheme registration for the packaged macOS/Windows builds. No
per-target deps are needed for Windows or Linux because their platform behavior
relies on already-present cross-platform crates (`interprocess`, `dir_spec`,
`muda`). Correct.

## Non-platform `cfg` sites (out of scope, listed for completeness)

These matched the broad grep but are **test gates, not platform gates**, and are
left unchanged:

- `src/features/about.rs:42` — `#[cfg(not(test))]` guards `open::that_detached`
  so unit tests never spawn a real browser.
- `src/features/skill_plan_editor.rs:856` and `:872` — `#[cfg(not(test))]`
  guards on side-effecting paths (clipboard / file-dialog) for the same reason.

These are consistent with the project rule that tests must not perform real side
effects.

## Code changes made by this audit

Audit-only with one clarifying comment; **no behavioral change**:

- `src/app.rs` (above the `#[cfg(not(target_os = "macos"))] fn disable_shadow`
  arm, ~line 2156): added a `//` comment explaining that only macOS draws a
  system drop shadow around a transparent undecorated window, so the no-op
  elsewhere is intentional.

No other code was modified. `mise run check` compiles cleanly on macOS.
