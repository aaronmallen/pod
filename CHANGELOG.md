# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semver versioning](https://semver.org/).

## [Unreleased]

## [0.3.0]

### Added

- Native application menu with **Help → About Pod** item. The About
  dialog shows the current version number.
- Character grid now scrolls when more characters are added than fit
  on screen, with drag auto-scroll at the edges.
- Background ESI cache cleaner service purges stale cached entries so
  the cache does not grow unbounded between restarts.
- Website now shows release notes parsed directly from this changelog,
  with a beta channel badge and corrected navigation links.

### Fixed

- Application now exits cleanly when a database migration fails at
  startup instead of continuing with a partially migrated schema.
- `solar_system_id` is now persisted in the structure cache, so
  location data survives app restarts.
- Corporation net worth calculation no longer includes character
  personal assets and escrow buy orders, which were previously
  counted twice.
- Duplicate characters can no longer be added to the roster when the
  same character is authenticated more than once.

## [0.2.0]

### Added

- In-app auto-update: background check runs on startup and every 4
  hours via `cargo-packager-updater`. A dismissible banner appears when
  a newer version is available; clicking **Update** downloads and
  installs the new binary in the background, then transitions the
  banner to **Restart Now**.
- File-based structured tracing with daily log rotation (7-file
  retention) written to the platform state directory under
  `pod/logs/`.

### Changed

- Switch to Semantic Versioning for releases.

### Fixed

- Space Grotesk now renders correctly on Windows. Bundled static TTF
  files were HTML documents saved with a `.ttf` extension, causing
  fontdb to fail and Windows to fall back to a symbol font
  (Wingdings-style rendering).
- Startup no longer opens a visible console/terminal window on Windows.
  Previously the default CONSOLE subsystem caused a PowerShell window
  to appear alongside the app; closing it sent CTRL\_CLOSE\_EVENT to
  the process, terminating Pod.
- App no longer closes immediately after the splash animation on
  Wayland (KDE Plasma 6 / CachyOS). In-place window mutation
  (`toggle_decorations` on a transparent frameless surface) caused the
  compositor to silently invalidate the handle; the transition now
  closes the splash and opens a fresh main window with correct settings
  from the start.

## 26.5.20

Initial beta release

[Unreleased]: https://github.com/aaronmallen/pod/compare/0.3.0...HEAD
[0.3.0]: https://github.com/aaronmallen/pod/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/aaronmallen/pod/compare/26.5.20...0.2.0
