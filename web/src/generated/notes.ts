import type { Note } from '../types';

export const NOTES: Note[] = [
  { tag: 'NEW', tone: 'plasma', text: "In-app auto-update: background check runs on startup and every 4 hours via `cargo-packager-updater`. A dismissible banner appears when a newer version is available; clicking **Update** downloads and installs the new binary in the background, then transitions the banner to **Restart Now**." },
  { tag: 'NEW', tone: 'plasma', text: "File-based structured tracing with daily log rotation (7-file retention) written to the platform state directory under `pod/logs/`." },
  { tag: 'CHANGE', tone: 'warning', text: "Switch to Semantic Versioning for releases." },
  { tag: 'FIX', tone: 'success', text: "Space Grotesk now renders correctly on Windows. Bundled static TTF files were HTML documents saved with a `.ttf` extension, causing fontdb to fail and Windows to fall back to a symbol font (Wingdings-style rendering)." },
  { tag: 'FIX', tone: 'success', text: "Startup no longer opens a visible console/terminal window on Windows. Previously the default CONSOLE subsystem caused a PowerShell window to appear alongside the app; closing it sent CTRL\\_CLOSE\\_EVENT to the process, terminating Pod." },
  { tag: 'FIX', tone: 'success', text: "App no longer closes immediately after the splash animation on Wayland (KDE Plasma 6 / CachyOS). In-place window mutation (`toggle_decorations` on a transparent frameless surface) caused the compositor to silently invalidate the handle; the transition now closes the splash and opens a fresh main window with correct settings from the start." },
];
