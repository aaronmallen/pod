# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semver versioning](https://semver.org/).

## [Unreleased]

## [0.5.2]

> [!IMPORTANT]
> **This update must be installed manually.** A bug in 0.5.0 and 0.5.1 left the in-app updater turned off, so those
> versions can't detect or offer this release on their own. Download and install 0.5.2 by hand — automatic updates
> work again from this version onward.

### Fixed

- The in-app update notification works again — it was unintentionally turned off in 0.5.0, so 0.5.0 and 0.5.1 never
  checked for or offered new versions. Once you're on 0.5.2 you'll be notified about future updates as usual.
- Closing the startup splash screen now fully quits Pod. Previously it could leave a hidden background process running,
  which stopped the app from reopening until that process was killed manually.

## [0.5.1]

> [!WARNING]
> **Pod 0.5.0 is a complete rewrite of the app and is not backwards compatible with earlier versions.**
> Updating clears your existing local Pod data and starts fresh, so the first time you open it you'll need to sign in
> and re-authorize all of your characters again.

### Added

- Stockpile cards now show ISK figures — an estimated total value when fully stocked and the ISK still needed to fill
  any shortfall, each labeled as an EVE-average estimate.
- You can now export a stockpile's shopping list to EVE's in-game Multibuy — right-click a stockpile, choose between the
  full target or just the remaining shortfall, and copy a paste-ready list to the clipboard.
- Stockpile cards gained a status strip showing "Ready to ship" or how many items are short, plus an expand/collapse
  control so long item lists no longer overflow the card.

### Changed

- The stockpile create, edit, import, and export screens have been redesigned as centered modals with clearer item
  previews and actions.
- The mail compose body and the stockpile multibuy import box are now multi-line editors, so you can write, paste, and
  review multi-line content in a scrollable box instead of one cramped line.
- Character portraits and corporation and alliance logos now refresh on their own — images older than about a week are
  re-fetched during sync, so they stay current when someone changes a portrait or a corp updates its logo.

### Fixed

- Stockpile cards now count items you already own even when they sit in a station, structure, or container inside the
  stockpile's location, and across your corporation — they previously showed 0 of N for stock you actually held.
- Stockpile progress bars no longer render full at 0% (or empty at 100%) — a display bug could make an empty stockpile
  look fully stocked.
- Importing a multibuy list now reads quantities written before the item name (like "25 Mobile Tractor Unit" or
  "x25 Mobile Tractor Unit"), not just quantities at the end of the line.
- Pod no longer gets stuck on the loading screen when EVE's static data fails to refresh — if your data is already
  present it continues with a small "refresh failed" notice, and a fresh install now offers a Retry button.
- Downloading EVE's static data no longer fails on slow or large connections — the download now has a much longer
  timeout and retries transient network errors instead of giving up on the first hiccup.
- Browser sign-in now reliably hands you back to Pod on Windows even after you move the app to a new folder, and on
  Linux AppImage builds where the link previously failed silently.
- Launching Pod a second time now brings the existing window to the front instead of opening a duplicate.
- Asset sync no longer fails with a database error in an edge case where a structure owner's CEO is personally enlisted
  in factional warfare.

### Performance

- Pod starts up faster when EVE's static data hasn't changed — it checks a small version marker and skips the large
  static-data download entirely when your local copy is already current.
- Initial setup is much faster when Pod's database lives on a network drive — seeding EVE's static data, which could
  take several minutes, now completes far more quickly.

## [0.5.0]

> [!WARNING]
> **Pod 0.5.0 is a complete rewrite of the app and is not backwards compatible with earlier versions.**
> Updating clears your existing local Pod data and starts fresh, so the first time you open it you'll need to sign in
> and re-authorize all of your characters again.

### Added

- A new Abyssals view in the assets tab lists your abyssal-rolled modules with their individual mutated stats, mutation
  tier, estimated value, and location — filter the grid by module type or by a range on any stat to find an exact roll.
- Squads let you group your characters into named, color-coded sets in the character manager, so a large roster stays
  organized the way you actually fly.
- Tags are now managed from a dedicated section in Settings, where you can create, rename, recolor, reorder, and delete
  the labels you apply to your characters.
- A new Storage section in Settings shows exactly where Pod keeps its database, logs, and image cache, and lets you
  move any of them to another folder or drive — handy for keeping the database off a small system disk.
- You can now build a stockpile by pasting an in-game multibuy list — Pod reads the item names and quantities, matches
  them against EVE's item catalog, and shows a preview to reconcile against the stockpile before anything is saved.

### Changed

- Character sign-in now completes in your browser and hands you straight back to Pod through a system link, instead of
  routing the EVE login through a local web server on a fixed port — no more firewall prompts or "port already in use"
  failures, and it behaves consistently across macOS, Windows, and Linux.
- Pod's background sync with EVE has been rebuilt so your character, wallet, asset, skill, and mail data stays current
  more reliably, with less redundant traffic to EVE's servers.
- The wallet's net-worth chart now breaks your total down into liquid ISK, asset value, and escrow, and lets you hover
  any day on the timeline to read the exact value and composition at that point.

## [0.4.9]

### Fixed

- Wallet and Asset performance improvements

## [0.4.8]

### Fixed

- Your character, wallet, skill, and asset data now keeps updating reliably the whole time the app is open — the app
  refreshes your EVE sign-in just before it expires instead of occasionally using it after it has already lapsed.
- Your assets no longer appear completely empty when you own items in a structure your character can't access (such
  as a citadel you've lost docking rights to) — that location now shows as "Unknown Structure" and the rest of your
  assets load normally.

## [0.4.7]

### Fixed

- Opening the wallet no longer hangs indefinitely when EVE's servers are slow or unresponsive — connections
  now time out after 30 seconds instead of blocking forever.
- The wallet now shows your cached journal, transactions, and contracts even when EVE's login service is
  unavailable or your session needs re-authentication.
- The wallet and contracts view no longer crash with a fatal error when certain data fields are missing or
  malformed in a server response.

## [0.4.6]

### Changed

- The inventory search box now spans the full width of the panel — category pills and item count
  move to a row below, giving you more space to type filter queries.
- Mail folder and inbox icons, and the pin icon on messages, are now crisp vector graphics instead
  of Unicode characters that could look different depending on your system font.

### Fixed

- Contracts received from another player now show that player's name as the counterparty — they
  were previously showing your own character's name.
- The Contracts, Journal, and Market tab counts now update when you switch between characters —
  previously the counts showed the unfiltered total instead of the rows visible for the selected
  character.
- The app update notification banner now has consistent height and visual balance across all of
  its states (checking, available, downloading, and ready to restart).

## [0.4.5]

### Added

- The inventory filter now supports structured queries — filter by name, group, category, region, constellation,
  system, location, and item type, combine multiple terms with spaces (AND), separate values with commas (OR), and
  prefix a term with `!` to exclude it.
- A help button in the inventory search bar opens a quick-reference panel showing every supported filter keyword with
  clickable example queries.
- The assets sidebar now organizes your items in a full region → constellation → system → location → container tree,
  making it easy to drill down to exactly where something is without leaving the tab.
- The assets sidebar can now be resized by dragging its edge — your preferred width is remembered between sessions.

### Fixed

- Stockpile locations now show the actual station or structure name instead of a raw numeric ID.
- Region names in the asset sidebar now display correctly instead of showing as "Region 12345678".
- Windows users who were unable to complete the add-character or add-corporation flow (authentication hanging or
  showing a "Request Too Long" error) should now authenticate successfully.
- A corrupted or out-of-bounds saved window position no longer prevents the app from opening — it falls back to a
  centered default.
- Closing the main window now exits the app cleanly instead of leaving a background process running.

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

[Unreleased]: https://github.com/aaronmallen/pod/compare/0.5.2...HEAD
[0.5.2]: https://github.com/aaronmallen/pod/compare/0.5.1...0.5.2
[0.5.1]: https://github.com/aaronmallen/pod/compare/0.5.0...0.5.1
[0.5.0]: https://github.com/aaronmallen/pod/compare/0.4.9...0.5.0
[0.4.9]: https://github.com/aaronmallen/pod/compare/0.4.8...0.4.9
[0.4.8]: https://github.com/aaronmallen/pod/compare/0.4.7...0.4.8
[0.4.7]: https://github.com/aaronmallen/pod/compare/0.4.6...0.4.7
[0.4.6]: https://github.com/aaronmallen/pod/compare/0.4.5...0.4.6
[0.4.5]: https://github.com/aaronmallen/pod/compare/0.4.4...0.4.5
[0.4.4]: https://github.com/aaronmallen/pod/compare/0.4.3...0.4.4
[0.4.3]: https://github.com/aaronmallen/pod/compare/0.4.2...0.4.3
[0.4.2]: https://github.com/aaronmallen/pod/compare/0.4.1...0.4.2
[0.4.1]: https://github.com/aaronmallen/pod/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/aaronmallen/pod/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/aaronmallen/pod/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/aaronmallen/pod/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/aaronmallen/pod/compare/26.5.20...0.2.0
