// Local development fixture. Run `pnpm --dir web run fetch-release` to regenerate,
// or copy manually: cp web/src/generated/release.example.ts web/src/generated/release.ts

import type { Release, PlatformBuilds } from '../types';

export const RELEASE: Release = {
  version: '0.2.0',
  channel: 'beta',
  date:    'May 20, 2026',
};

export const PLATFORM_BUILDS: PlatformBuilds[] = [
  {
    id:   'macos',
    name: 'macOS',
    sub:  '12 Monterey or later',
    builds: [
      {
        id:          'mac-arm',
        arch:        'Apple Silicon',
        ext:         'dmg',
        size:        '128 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/0.2.0/Pod-0.2.0-arm64.dmg',
        filename:    'Pod-0.2.0-arm64.dmg',
      },
      {
        id:          'mac-x64',
        arch:        'Intel · x86_64',
        ext:         'dmg',
        size:        '136 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/0.2.0/Pod-0.2.0-x64.dmg',
        filename:    'Pod-0.2.0-x64.dmg',
      },
    ],
  },
  {
    id:   'windows',
    name: 'Windows',
    sub:  '10 (1809) or later · 64-bit',
    builds: [
      {
        id:          'win-x64-exe',
        arch:        'Installer',
        ext:         'exe',
        size:        '112 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/0.2.0/Pod-0.2.0-x64-setup.exe',
        filename:    'Pod-0.2.0-x64-setup.exe',
      },
      {
        id:          'win-x64-msi',
        arch:        'MSI · enterprise',
        ext:         'msi',
        size:        '114 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/0.2.0/Pod-0.2.0-x64.msi',
        filename:    'Pod-0.2.0-x64.msi',
      },
    ],
  },
  {
    id:   'linux',
    name: 'Linux',
    sub:  'glibc 2.31+ · X11 / Wayland',
    builds: [
      {
        id:          'lin-app',
        arch:        'AppImage · universal',
        ext:         'AppImage',
        size:        '124 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/0.2.0/Pod-0.2.0.AppImage',
        filename:    'Pod-0.2.0.AppImage',
      },
      {
        id:          'lin-deb',
        arch:        '.deb · Debian / Ubuntu',
        ext:         'deb',
        size:        '118 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/0.2.0/pod_0.2.0_amd64.deb',
        filename:    'pod_0.2.0_amd64.deb',
      },
      {
        id:          'lin-flatpak',
        arch:        'Flatpak · universal',
        ext:         'flatpak',
        size:        '119 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/0.2.0/pod-0.2.0.flatpak',
        filename:    'pod-0.2.0.flatpak',
      },
    ],
  },
];
