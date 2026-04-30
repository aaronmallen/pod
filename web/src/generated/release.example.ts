// Local development fixture. Run `pnpm --dir web run fetch-release` to regenerate,
// or copy manually: cp web/src/generated/release.example.ts web/src/generated/release.ts

import type { Release, PlatformBuilds } from '../types';

export const RELEASE: Release = {
  version: '26.5.19',
  channel: 'stable',
  date:    'May 19, 2026',
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
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/26.5.19/Pod-26.5.19-arm64.dmg',
        filename:    'Pod-26.5.19-arm64.dmg',
      },
      {
        id:          'mac-x64',
        arch:        'Intel · x86_64',
        ext:         'dmg',
        size:        '136 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/26.5.19/Pod-26.5.19-x64.dmg',
        filename:    'Pod-26.5.19-x64.dmg',
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
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/26.5.19/Pod-26.5.19-x64-setup.exe',
        filename:    'Pod-26.5.19-x64-setup.exe',
      },
      {
        id:          'win-x64-msi',
        arch:        'MSI · enterprise',
        ext:         'msi',
        size:        '114 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/26.5.19/Pod-26.5.19-x64.msi',
        filename:    'Pod-26.5.19-x64.msi',
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
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/26.5.19/Pod-26.5.19.AppImage',
        filename:    'Pod-26.5.19.AppImage',
      },
      {
        id:          'lin-deb',
        arch:        '.deb · Debian / Ubuntu',
        ext:         'deb',
        size:        '118 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/26.5.19/pod_26.5.19_amd64.deb',
        filename:    'pod_26.5.19_amd64.deb',
      },
      {
        id:          'lin-flatpak',
        arch:        'Flatpak · universal',
        ext:         'flatpak',
        size:        '119 MB',
        downloadUrl: 'https://github.com/aaronmallen/pod/releases/download/26.5.19/pod-26.5.19.flatpak',
        filename:    'pod-26.5.19.flatpak',
      },
    ],
  },
];
