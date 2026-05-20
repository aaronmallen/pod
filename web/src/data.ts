import type { Feature, Note, Platform, ReleaseData } from './types';

export const FEATURES: Feature[] = [
  { id: 'characters', icon: 'characters', title: 'Characters', line: 'Roster overview with sheets, standings and clone state.' },
  { id: 'skills',     icon: 'skills',     title: 'Skills',     line: 'Plan queues months out; compare paths side-by-side.' },
  { id: 'mail',       icon: 'mail',       title: 'Mail',       line: 'One inbox for every pilot. Labels, drafts and mailing lists.' },
  { id: 'wallet',     icon: 'wallet',     title: 'Wallet',     line: 'Journal, market, contracts and recurring across every character.' },
  { id: 'assets',     icon: 'assets',     title: 'Assets',     line: 'Search every hangar and container by name or category.' },
  { id: 'fitting',    icon: 'fitting',    title: 'Fitting',    line: 'Build, simulate and share fits offline. EFT-compatible.', soon: true },
];

export const NOTES: Note[] = [
  { tag: 'NEW', tone: 'plasma',  text: 'In-app auto-update: Pod checks for new versions on startup and every 4 hours. A dismissible banner lets you download, install, and restart in one click — no manual download required.' },
  { tag: 'FIX', tone: 'success', text: 'Space Grotesk now renders correctly on Windows. Corrupted font files were causing all UI text to fall back to a symbol font.' },
  { tag: 'FIX', tone: 'success', text: 'App no longer opens a console window on Windows at startup. Previously closing that window would silently terminate Pod.' },
  { tag: 'FIX', tone: 'success', text: 'Splash transition no longer closes the app on Wayland / KDE Plasma 6. The main window now opens fresh with the correct settings instead of mutating the splash surface.' },
];

export const PLATFORMS: Platform[] = [
  {
    id: 'macos',
    name: 'macOS',
    sub: '12 Monterey or later',
    builds: [
      { id: 'mac-arm', arch: 'Apple Silicon',  ext: 'dmg', size: '128 MB', downloadUrl: '', filename: '' },
      { id: 'mac-x64', arch: 'Intel · x86_64', ext: 'dmg', size: '136 MB', downloadUrl: '', filename: '' },
    ],
  },
  {
    id: 'windows',
    name: 'Windows',
    sub: '10 (1809) or later · 64-bit',
    builds: [
      { id: 'win-x64-exe', arch: 'Installer',        ext: 'exe', size: '112 MB', downloadUrl: '', filename: '' },
      { id: 'win-x64-msi', arch: 'MSI · enterprise', ext: 'msi', size: '114 MB', downloadUrl: '', filename: '' },
    ],
  },
  {
    id: 'linux',
    name: 'Linux',
    sub: 'glibc 2.31+ · X11 / Wayland',
    builds: [
      { id: 'lin-app', arch: 'AppImage · universal',   ext: 'AppImage', size: '124 MB', downloadUrl: '', filename: '' },
      { id: 'lin-deb', arch: '.deb · Debian / Ubuntu', ext: 'deb',      size: '118 MB', downloadUrl: '', filename: '' },
      { id: 'lin-flatpak', arch: 'Flatpak · universal', ext: 'flatpak', size: '119 MB', downloadUrl: '', filename: '' },
    ],
  },
];

export const RELEASE: ReleaseData = {
  version:     '0.2.0',
  channel:     'beta',
  build:       '0520',
  date:        'May 20, 2026',
  notesAnchor: '#notes',
};
