---
id: "0005"
title: EVE SSO Authentication and Deeplink Transport
status: active
tags: [architecture, auth, clients, security, cross-platform, deeplink]
created: 2026-06-06
---

# ADR-0005: EVE SSO Authentication and Deeplink Transport

## Status

![Active](https://img.shields.io/badge/Active-green?style=for-the-badge)

## Summary

Pod authenticates with EVE SSO using OAuth2 with PKCE as a public client (no secret). The
authorization code is returned to the app through a **custom-scheme deep link**,
`eveauth-pod://callback`, registered as the application's single redirect URI — avoiding a loopback
HTTP port, which EVE SSO does not accept and which fails on some Windows machines. Identity and
credentials are persisted in separate tables so a credential's refresh lifecycle is independent of
character sync state, and development and production use **separate ESI applications** (EVE allows one
redirect URI per app). The cross-platform **transport** for the callback URL is a single
[`interprocess`](https://crates.io/crates/interprocess) local socket (named pipe on Windows, abstract
Unix socket on Linux) serving as **both** the single-instance lock **and** the URL channel: a second
launch forwards its `argv` URL over that socket and exits; a cold start parses `argv` in `main.rs`
before `app::run()` and drains a `PENDING` buffer in `stream()`. Linux scheme registration is passive
on packaged channels and self-registering only where there is no install step; macOS keeps its
Apple-Event handler with reduced `unsafe` via `objc2-foundation`.

---

## Part 1 — EVE SSO Authentication

### Context

EVE SSO native authentication is OAuth2 with PKCE. The redirect URIs EVE accepts are constrained:
its developer portal states it will redirect only to a single URL, and allows `https` plus custom
schemes beginning with `eveauth`. Notably it does **not** accept `http://localhost` loopback
redirects.

The common native-app pattern (RFC 8252) is a loopback HTTP listener on an ephemeral port. That is
unavailable here for two reasons: EVE disallows the loopback redirect, and binding a local port
fails in practice on some Windows setups (host firewall, endpoint-protection policy, or the port
being unavailable) — we have a confirmed user who cannot bind it.

We also must not break existing 0.4.x users when introducing the new flow, and EVE's one-redirect-
URI-per-application rule means we cannot add a new redirect to the production app without replacing
the old one.

A character's identity and its OAuth credential are distinct concerns with different lifecycles: a
character record describes who the character is; a credential record describes the right to act as
them, with its own expiry and refresh-token rotation. Mixing them would couple token refresh to
character display state and complicate multi-account scenarios.

### Decision

**Protocol.** OAuth2 with PKCE, public client. The `client_id` is embedded in the binary as the
default of the `eve_client_id` setting and can be overridden in `config.toml` (bring-your-own);
there is no client secret and no build-time environment variable.

**Callback transport: custom-scheme deep link.** The registered redirect URI is
`eveauth-pod://callback` (valid per EVE's rule: begins with `eveauth`, ends in a letter). After the
user authorizes in their system browser, EVE redirects to `eveauth-pod://callback?code=…&state=…`;
the OS hands that URL to Pod via a registered protocol handler, the running instance receives it
(single-instance forwarding over a non-TCP IPC channel — see Part 2 — so no port is bound), and the
app parses and completes the exchange. No loopback listener is used.

**Layering.** The transport-independent core lives in `src/features/auth/session.rs`: `parse_callback`
extracts `code`/`state` from the callback URL, and `complete_sign_in` validates `state`, calls
`eve_sso::exchange_code`, and persists the credential. `src/features/auth/deep_link.rs` owns the OS
scheme handler and single-instance URL forwarding; the OS registration itself is a packaging concern.
Persisting the credential is the source of truth — the sync engine discovers it on its next pass
(ADR-0002); the feature may also `Handle::enroll` to sync sooner.

**Identity and credentials in separate tables.** Character identity and OAuth credentials live in
distinct tables. A credential row references a character by id but carries its own lifecycle (expiry,
refresh-token rotation), so it can exist before the character record is fully synced, and refreshing
a token never rewrites identity rows. This isolates token refresh, multi-account support, and future
credential rotation from character display state (the separation ADR-0002 relies on).

**Separate dev and production applications.** Because an EVE application has one redirect URI, a
dedicated Pod-Dev application (redirect `eveauth-pod://callback`) is used during development; its
`client_id` is the bundled default. A production release ships its own application's `client_id` as
that default. This isolates testing from released users and their error/scope budget.

**`https` paste fallback (future).** For environments where the OS scheme handler is not registered,
a hosted `https://…/auth/callback` page (also EVE-allowed) can relay to the deep link and display
the code for manual paste. It is not part of the initial implementation; the auth core already
accepts a pasted callback, so adding it later requires no core change.

### Consequences

#### Positive

- No loopback port is bound, so the Windows binding failure (and firewall/AV cases) cannot occur.
- It is EVE's sanctioned native path (the `eveauth-` scheme namespace exists for this).
- Dev and production are cleanly isolated by separate applications.
- PKCE public client means there is no secret to embed or leak.
- Credential rotation and token refresh are isolated to the auth layer with no coupling to character
  display state.

#### Negative

- Per-OS protocol-handler registration plus single-instance URL forwarding is real, platform-specific
  work (resolved in Part 2).
- Where the handler is not registered, sign-in needs the (deferred) `https` paste fallback.
- The one-redirect-URI rule means shipping 0.5.x will use its own production application (new
  `client_id`/redirect) so that 0.4.x clients keep working; the redirect cannot simply be changed in
  place.
- The embedded `client_id` is visible in the binary — acceptable for a PKCE public client, but it
  must not be mistaken for a secret.

### Open Questions

- How a production release selects its own application's `client_id` as the bundled default (a
  release-build concern; a runtime environment variable is ruled out).
- The exact scope set requested at sign-in (all read scopes up front vs a minimal core with
  incremental re-auth).
- Whether to host the `https` fallback page for 0.5.x or rely solely on the deep link initially.

---

## Part 2 — Cross-Platform Deeplink Transport

### Context

Pod receives the EVE SSO callback through the `eveauth-pod://` custom scheme (Part 1). The OS launches
a fresh `pod eveauth-pod://…` process with the URL in `argv`; that URL must reach the already-running
instance (or, on a cold start, the instance it becomes) and feed the existing `deliver()` /
`subscription()` abstraction.

Part 1 chose the custom-scheme deep link over a loopback HTTP listener, and originally left the
mechanism open: the per-OS single-instance IPC mechanism (unix socket vs named pipe vs platform
URL-open event). At the time, only macOS routed the callback — `deep_link::install()` was a no-op
everywhere else, so on Windows and Linux the OS spawned a new process with the URL in `argv` but
nothing detected the running instance, forwarded the URL, or parsed `argv`. Sign-in (and therefore
all authenticated functionality) could not complete off macOS. Part 2 resolves that mechanism so it is
not re-litigated.

Two forces shape the choice:

- **One transport for two jobs.** The app needs a single-instance guard (a second launch must not open
  a second window) *and* a way to hand the second launch's URL to the primary. These are the same
  problem if the lock is a bound socket: binding is the lock, and the bound socket is the channel.
- **No bound TCP port.** Part 1 chose the custom scheme precisely to avoid binding a loopback port
  (EVE disallows the loopback redirect, and binding fails on some Windows hosts). The transport must
  therefore be a non-TCP IPC primitive — a named pipe or a Unix socket.

The macOS handler already works but hand-rolls Cocoa FFI with raw `msg_send!`, carrying ~7 `unsafe`
blocks, ~5 of which are avoidable.

### Decision

**`interprocess` local socket as both lock and transport.** Use the `interprocess` crate's local
socket — a **named pipe on Windows**, an **abstract Unix socket on Linux** — as both the
single-instance lock and the URL transport. The primary binds a per-user-named socket at startup and
runs a blocking accept loop on a dedicated `std::thread`; each received message is prefix-validated
against the `eveauth-pod://` scheme and passed to the existing `deliver(url)`. One socket does both
jobs: a successful bind *is* the single-instance claim, and the bound socket *is* the channel — there
is no separate lock file or port.

*Rejected alternatives:*

- **The `single-instance` crate.** It provides only a lock, no channel, so a second IPC mechanism would
  still be needed to forward the URL; it also pulls in unmaintained transitive dependencies. Using
  `interprocess` for both jobs avoids a second primitive and the dependency liability.
- **Hand-rolled per-OS IPC** (raw named-pipe / Unix-socket FFI). This duplicates, in `unsafe` and
  platform `cfg`, exactly what `interprocess` already abstracts safely and cross-platform. New Win/Linux
  code is required to contain zero `unsafe`; a hand-rolled path defeats that.

**Warm-forward = forward-then-exit (v1).** When the app is already running, a second launch fails to
bind the socket (`AddrInUse`), connects to the primary, writes its `argv` URL, and exits. The existing
window is **not** raised in v1; focus-existing-window is deferred (see Future Work). Forward-then-exit
keeps the second process's job to a single write with no UI coordination, no window-server calls, and
no extra platform surface.

**Cold-start = `argv` parse in `main.rs`, drained in `stream()`.** When no instance is running, the
launched process *is* the callback carrier. `main.rs` parses `argv` for an `eveauth-pod://` URL **before
`app::run()`**, becomes the primary, and stashes the URL in a new static `PENDING` slot. `stream()`
drains `PENDING` immediately after it registers the mpsc sender, because `deliver()` silently drops while
the channel is unwired — parsing early but delivering late, once the runtime can receive, is what makes
the cold-start callback survive.

**Linux scheme registration: passive where packaged, self-register only where there is no install step.**
Packaged channels (deb, pacman, flatpak) register the scheme through their own desktop-database mechanisms
(triggers / hooks / Flatpak export); the app does nothing at runtime there. Because `eveauth-pod` is a
**unique** scheme, no explicit `xdg-mime default` is needed — a unique scheme routes via the
`mimeinfo.cache` fallback. For channels with **no install step** — AppImage and dev — the app
**self-registers idempotently at startup**: it writes `~/.local/share/applications/<id>.desktop`
(`MimeType=x-scheme-handler/eveauth-pod;` and `Exec=… %u`) and runs `update-desktop-database`. The
self-registration is confined to the no-install-step case so packaged installs are never overwritten by
a runtime side effect.

**macOS keeps the Apple-Event handler, with reduced `unsafe`.** There is no safe winit/iced hook for the
`kAEGetURL` Apple Event, so macOS keeps its Apple-Event handler. `extract_url` and the
`sharedAppleEventManager` call are refactored onto `objc2-foundation` typed bindings (already in the
dependency tree), removing ~5 of the 7 `unsafe` blocks. The one remaining `setEventHandler…` FFI call is
documented with a `// SAFETY:` comment, and `#![deny(unsafe_op_in_unsafe_fn)]` is added. New Win/Linux
code contains zero `unsafe`.

**Security: forged callbacks are inert.** The accept loop prefix-validates every message against
`eveauth-pod://` and ignores anything else. More fundamentally, a forged or replayed callback delivered
over the socket cannot complete a token exchange: the PKCE verifier and the CSRF `state` are held
**in-process** and never travel over the socket, so an attacker who can write to the socket still lacks
the secrets the exchange validates against. The socket is also per-user-named to prevent cross-user
collisions on shared hosts.

### Dependencies

| Dependency         | Version | Purpose                                                                                    |
|--------------------|---------|--------------------------------------------------------------------------------------------|
| `interprocess`     | latest  | Local socket (named pipe / abstract Unix socket): single-instance lock and URL transport.  |
| `objc2-foundation` | latest  | Typed bindings replacing raw Cocoa FFI in the macOS Apple-Event handler.                   |

### Consequences

#### Positive

- One primitive (`interprocess` local socket) is both the lock and the channel — no separate lock file,
  no second IPC mechanism, no bound TCP port (preserving the no-loopback property from Part 1).
- Forward-then-exit keeps the second process trivial: one write, no UI/window-server coordination.
- New Windows/Linux code is `unsafe`-free; macOS `unsafe` drops from ~7 blocks to a single documented
  FFI call.
- Forged callbacks over the socket are inert: PKCE verifier and CSRF `state` stay in-process.
- A unique scheme means no `xdg-mime default` is required, and packaged Linux channels need no runtime
  registration at all.

#### Negative

- An abstract Unix socket is per-network-namespace; under Flatpak it is per-sandbox, which would matter
  only if focus-existing-window is later pursued (it would then need a D-Bus/portal `--talk-name`).
- The cold-start path depends on the `PENDING` URL surviving until `stream()` has wired the mpsc sender
  and the iced runtime is `Ready`; the ordering must be preserved by any future boot refactor.
- macOS still carries one `unsafe` FFI call (the Apple-Event handler registration), since no safe
  winit/iced hook exists.

### Future Work

- **Focus / raise the existing window** on a warm-forward callback (deferred from v1). On Linux/Flatpak
  this may require a D-Bus/portal activation path rather than a plain socket write.
- **`https` paste fallback** for environments where the OS scheme handler is unregistered (already
  deferred in Part 1; the auth core accepts a pasted callback).

---

## Affected Areas

- `src/features/auth.rs` + `src/features/auth/` — the auth feature.
- `src/features/auth/session.rs` — `REDIRECT_URI`, `parse_callback`, `complete_sign_in`; unchanged by
  the transport, which feeds the existing `deliver()`.
- `src/features/auth/deep_link.rs` — OS scheme handler and single-instance URL forwarding, restructured
  into platform submodules (`deep_link/windows.rs`, `deep_link/linux.rs`, shared single-instance helper;
  no `mod.rs` per project convention).
- `src/clients/eve_sso.rs` — PKCE primitives (`sign_in`, `exchange_code`).
- `src/main.rs` — `argv` parse for the `eveauth-pod://` URL before `app::run()`, and the `PENDING` slot.
- `src/app.rs` / `stream()` — drains `PENDING` after registering the mpsc sender; constructs the
  `eve_sso` client from the `eve_client_id` setting.
- `src/store/` — separate `characters` and `credentials` tables and their repos.
- `src/config.rs` — the `eve_client_id` setting and its bundled default; the per-user `dir_spec` base
  used to namespace the socket name.
- A single source-of-truth `SCHEME` (`eveauth-pod`) constant shared by `argv` parsing and Linux
  registration.
- OS packaging — protocol-handler registration (macOS `CFBundleURLTypes`, Windows installer
  registration via cargo-packager NSIS/WiX, Linux distro desktop-DB `x-scheme-handler/`).
- External: the EVE application registration (its single redirect URI and `client_id`).

## References

- [ADR-0002](0002-sync-render-separation.md) — Sync/Render Separation. Persisting the credential is the
  source of truth; the sync engine discovers it; render never calls ESI.
- RFC 8252 — OAuth 2.0 for Native Apps.
- EVE SSO documentation: <https://docs.esi.evetech.net/docs/sso/>.
- `src/features/auth/deep_link.rs`, `src/main.rs`, `src/app.rs`, `src/features/auth/session.rs`,
  `src/config.rs`.
