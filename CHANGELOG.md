# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semver versioning](https://semver.org/).

## [Unreleased]

## [0.4.4]

### Fixed

- Asset value is now consistent between the wallet and inventory tabs — previously the wallet fetched live prices from
  EVE's servers and could show a different total than the assets view.
- Rare and infrequently-traded items (supercarriers, faction modules, and similar) now show a price in your inventory
  and portfolio instead of 0 ISK — the app falls back to CCP's adjusted price when no Jita sell order is available.

### Performance

- Character data, clones, stockpiles, skill plans, and item types all load noticeably faster — related information is
  now fetched together in a single operation instead of separate round trips.

## [0.4.3]

### Added

- Your active ship now appears in the inventory alongside your other assets — modules, rigs, and cargo are grouped
  under the ship even when you are undocked in space.
- Corporation assets are now synced in the background during character refresh, so inventory data is always current
  without manually navigating to the assets tab.
- The inventory view now restores instantly when you navigate away and return — switching tabs no longer triggers a
  fresh reload.
- Re-authorizing a character on Windows is now more reliable; repeated auth attempts no longer fail due to a
  port-conflict error between flows.

### Fixed

- Ships docked inside player-owned structures (citadels, engineering complexes, etc.) now show the correct structure
  name and solar system in the inventory view.
- Character tags now survive background sync reliably — the fix in 0.4.2 was incomplete and only worked when the
  character list was the active tab.

## [0.4.2]

### Fixed

- Assets located in space — undocked ships, jetcans, and floating containers — now appear in the inventory view
  grouped under their solar system name instead of being invisible.
- Character tags now remain visible after a background sync — they no longer vanish until you navigate away and back.

## [0.4.1]

### Added

- The inventory tab now loads items progressively — the first 100 appear immediately and more load automatically as you
  scroll, keeping the app responsive with large inventories.
- ESI character data now syncs in the background after the splash screen, so the app is immediately usable without
  waiting for a network refresh.
- Drag-and-drop character reordering now works with empty grid slots — characters can be dragged to any position,
  including gaps between existing characters and the trailing empty row.
- Newly added characters are placed in their own unique grid slot; any characters that shared the same slot due to a
  legacy issue are automatically reassigned on startup.

### Fixed

- Blueprint copies (BPCs) no longer show a price or inflate portfolio totals in the inventory tab.
- Skill training ETAs now include the full year for long-duration skills (e.g. "22 May 2026 · 14:30" instead of
  "22 May · 14:30").
- The asset screen's character selector now correctly shows "All Characters" instead of "All Wallets".
- Opening the tag modal no longer resets the character grid's scroll position.
- Character portraits now load instantly on startup from a local cache rather than waiting for a network request.
- The portfolio tracker chart now refreshes prices every 30 minutes while the app is open, and shows today's value
  immediately on first launch rather than waiting until the next day.

### Changed

- Performance improvements to asset loading and ESI data fetching.

## [0.4.0]

### Added

- Ten user-facing feature flags replace the previous prototype set: `clone_monitoring`, `contacts`, `combat_log`,
  `eve_notifications`, `standings`, `location_tracking`, `skill_monitoring`, `mail`, `wallet`, and `asset_tracking`.
  All flags default to enabled.
- ESI OAuth scopes are now split between character and corporation flows. Corp wallet (`contracts`, `orders`, `wallet`)
  and corp asset scopes are only requested when their corresponding feature flags are enabled, so characters are never
  prompted for scopes they do not need.
- `granted_scopes` column on the characters table records which OAuth scopes were actually granted during
  authentication.
- On first launch after upgrading, granted scopes are backfilled for existing characters by decoding the current access
  token JWT locally — no network round-trip or re-authentication required.
- Nav items (Assets, Skills, Wallet, Mail) and character detail tabs (Clones, Contacts, Kill Log, Notifications,
  Standings) are hidden when their corresponding feature flag is disabled.
- When a character's granted scopes do not cover what the active feature set requires, each affected tab shows a
  scope-gap indicator with a **Re-authorize** button that restarts the OAuth flow for that character requesting the
  missing scopes.
- Background ESI fetch tasks and the location-refresh task are suppressed for disabled features, reducing unnecessary
  API calls.

### Fixed

- Settings changes now take effect immediately without restarting the app. The `ConfigUpdated` message was not
  propagating to the in-process config, so toggles appeared to save but reverted on next view.
- Re-authenticating a character now updates the existing row in place instead of inserting a duplicate record.

## [0.3.1]

### Fixed

- Native "Pod" menu now appears in the macOS menu bar. In daemon mode, `init_for_nsapp()` was called before NSApp was
  fully initialized; the menu is now re-attached once the first window opens.
- In-app update banner now appears correctly when a newer version is available. The update manifest was using `darwin-*`
  platform keys instead of the `macos-*` keys that `cargo-packager-updater` actually resolves, causing the version
  check to fail silently.

## [0.3.0]

### Added

- Native application menu with **Help → About Pod** item. The About dialog shows the current version number.
- Character grid now scrolls when more characters are added than fit on screen, with drag auto-scroll at the edges.
- Background ESI cache cleaner service purges stale cached entries so the cache does not grow unbounded between
  restarts.
- Website now shows release notes parsed directly from this changelog, with a beta channel badge and corrected
  navigation links.

### Fixed

- Application now exits cleanly when a database migration fails at startup instead of continuing with a partially
  migrated schema.
- `solar_system_id` is now persisted in the structure cache, so location data survives app restarts.
- Corporation net worth calculation no longer includes character personal assets and escrow buy orders, which were
  previously counted twice.
- Duplicate characters can no longer be added to the roster when the same character is authenticated more than once.

## [0.2.0]

### Added

- In-app auto-update: background check runs on startup and every 4 hours via `cargo-packager-updater`. A dismissible
  banner appears when a newer version is available; clicking **Update** downloads and installs the new binary in the
  background, then transitions the banner to **Restart Now**.
- File-based structured tracing with daily log rotation (7-file retention) written to the platform state directory
  under `pod/logs/`.

### Changed

- Switch to Semantic Versioning for releases.

### Fixed

- Space Grotesk now renders correctly on Windows. Bundled static TTF files were HTML documents saved with a `.ttf`
  extension, causing fontdb to fail and Windows to fall back to a symbol font (Wingdings-style rendering).
- Startup no longer opens a visible console/terminal window on Windows. Previously the default CONSOLE subsystem caused
  a PowerShell window to appear alongside the app; closing it sent CTRL\_CLOSE\_EVENT to the process, terminating Pod.
- App no longer closes immediately after the splash animation on Wayland (KDE Plasma 6 / CachyOS). In-place window
  mutation (`toggle_decorations` on a transparent frameless surface) caused the compositor to silently invalidate the
  handle; the transition now closes the splash and opens a fresh main window with correct settings from the start.

## 26.5.20

Initial beta release

[Unreleased]: https://github.com/aaronmallen/pod/compare/0.4.4...HEAD
[0.4.4]: https://github.com/aaronmallen/pod/compare/0.4.3...0.4.4
[0.4.3]: https://github.com/aaronmallen/pod/compare/0.4.2...0.4.3
[0.4.2]: https://github.com/aaronmallen/pod/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/aaronmallen/pod/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/aaronmallen/pod/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/aaronmallen/pod/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/aaronmallen/pod/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/aaronmallen/pod/compare/26.5.20...0.2.0
