import https from 'https';
import fs from 'fs';
import path from 'path';

type BuildId =
  | 'mac-arm'
  | 'mac-x64'
  | 'win-x64-exe'
  | 'win-x64-msi'
  | 'lin-deb'
  | 'lin-app'
  | 'lin-flatpak';

interface BuildAsset {
  id: BuildId;
  arch: string;
  ext: string;
  size: string;
  downloadUrl: string;
  filename: string;
}

interface PlatformBuilds {
  id: string;
  name: string;
  sub: string;
  builds: BuildAsset[];
}

interface Release {
  version: string;
  date: string;
  channel: 'stable' | 'nightly';
}

interface GitHubAsset {
  name: string;
  size: number;
  browser_download_url: string;
}

interface GitHubRelease {
  tag_name: string;
  published_at: string;
  prerelease: boolean;
  assets: GitHubAsset[];
}

function parseArgs(): { token: string | null } {
  const args = process.argv.slice(2);
  const idx = args.indexOf('--token');
  if (idx !== -1 && args[idx + 1]) {
    return { token: args[idx + 1] };
  }
  return { token: process.env.GITHUB_TOKEN ?? null };
}

function httpsGet(
  url: string,
  headers: Record<string, string>,
): Promise<string> {
  return new Promise((resolve, reject) => {
    const req = https.get(url, { headers }, (res) => {
      if (
        res.statusCode !== undefined &&
        res.statusCode >= 300 &&
        res.statusCode < 400 &&
        res.headers.location
      ) {
        resolve(httpsGet(res.headers.location, headers));
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode} fetching ${url}`));
        return;
      }
      const chunks: Buffer[] = [];
      res.on('data', (c: Buffer) => chunks.push(c));
      res.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
      res.on('error', reject);
    });
    req.on('error', reject);
  });
}

function formatSize(bytes: number): string {
  const mb = bytes / (1024 * 1024);
  if (mb >= 1) return `${Math.round(mb)} MB`;
  return `${Math.round(bytes / 1024)} KB`;
}

function classifyAsset(name: string): BuildId | null {
  const lower = name.toLowerCase();
  const isX64 = lower.includes('x86_64') || lower.includes('_x64');
  if (lower.includes('aarch64') && lower.endsWith('.dmg')) return 'mac-arm';
  if (isX64 && lower.endsWith('.dmg'))                     return 'mac-x64';
  if (isX64 && lower.endsWith('.exe'))                     return 'win-x64-exe';
  if (isX64 && lower.endsWith('.msi'))                     return 'win-x64-msi';
  if (lower.endsWith('.deb'))                              return 'lin-deb';
  if (lower.endsWith('.appimage'))                         return 'lin-app';
  if (lower.endsWith('.flatpak'))                          return 'lin-flatpak';
  return null;
}

const BUILD_META: Record<BuildId, { arch: string; ext: string }> = {
  'mac-arm':     { arch: 'Apple Silicon',        ext: 'dmg'      },
  'mac-x64':     { arch: 'Intel · x86_64',       ext: 'dmg'      },
  'win-x64-exe': { arch: 'Installer',             ext: 'exe'      },
  'win-x64-msi': { arch: 'MSI · enterprise',     ext: 'msi'      },
  'lin-deb':     { arch: '.deb · Debian/Ubuntu', ext: 'deb'      },
  'lin-app':     { arch: 'AppImage · universal',  ext: 'AppImage' },
  'lin-flatpak': { arch: 'Flatpak',               ext: 'flatpak'  },
};

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    year: 'numeric', month: 'long', day: 'numeric',
  });
}

function serialize(value: unknown, indent = 0): string {
  const pad = '  '.repeat(indent);
  const inner = '  '.repeat(indent + 1);
  if (value === null || value === undefined) return String(value);
  if (typeof value === 'string') return JSON.stringify(value);
  if (typeof value === 'number' || typeof value === 'boolean') return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return '[]';
    return `[\n${value.map(v => `${inner}${serialize(v, indent + 1)}`).join(',\n')},\n${pad}]`;
  }
  if (typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length === 0) return '{}';
    return `{\n${entries.map(([k, v]) => `${inner}${k}: ${serialize(v, indent + 1)}`).join(',\n')},\n${pad}}`;
  }
  return String(value);
}

async function main(): Promise<void> {
  const { token } = parseArgs();
  if (!token) {
    console.error('error: GitHub token required. Set GITHUB_TOKEN or pass --token <pat>.');
    process.exit(1);
  }

  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    Authorization: `Bearer ${token}`,
    'User-Agent': 'pod-fetch-release/1.0',
    'X-GitHub-Api-Version': '2022-11-28',
  };

  console.log('Fetching latest release from GitHub...');
  let raw: string;
  try {
    raw = await httpsGet('https://api.github.com/repos/aaronmallen/pod/releases/latest', headers);
  } catch (err) {
    console.error(`error: Failed to fetch release — ${(err as Error).message}`);
    process.exit(1);
  }

  let release: GitHubRelease;
  try {
    release = JSON.parse(raw) as GitHubRelease;
  } catch {
    console.error('error: Invalid JSON in GitHub API response.');
    process.exit(1);
  }

  if (!release.tag_name) {
    console.error('error: API response missing tag_name.');
    process.exit(1);
  }
  if (!release.assets?.length) {
    console.error(`error: Release ${release.tag_name} has no assets.`);
    process.exit(1);
  }

  const version = release.tag_name.replace(/^v/, '');
  const date = formatDate(release.published_at);
  const channel: 'stable' | 'nightly' = release.prerelease ? 'nightly' : 'stable';

  console.log(`Release: ${version} (${channel}) — ${date}`);

  const buildAssets: BuildAsset[] = [];
  for (const asset of release.assets) {
    const id = classifyAsset(asset.name);
    if (!id) { console.log(`  skip: ${asset.name}`); continue; }
    buildAssets.push({
      id,
      arch: BUILD_META[id].arch,
      ext: BUILD_META[id].ext,
      size: formatSize(asset.size),
      downloadUrl: asset.browser_download_url,
      filename: asset.name,
    });
    console.log(`  mapped: ${asset.name} → ${id}`);
  }

  if (buildAssets.length === 0) {
    console.error('error: No recognisable build assets found in the release.');
    process.exit(1);
  }

  const order: BuildId[] = ['mac-arm', 'mac-x64', 'win-x64-exe', 'win-x64-msi', 'lin-deb', 'lin-app', 'lin-flatpak'];
  buildAssets.sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));

  const platformBuilds: PlatformBuilds[] = [
    { id: 'macos',   name: 'macOS',   sub: '12 Monterey or later',        builds: buildAssets.filter(b => b.id === 'mac-arm' || b.id === 'mac-x64') },
    { id: 'windows', name: 'Windows', sub: '10 (1809) or later · 64-bit', builds: buildAssets.filter(b => b.id === 'win-x64-exe' || b.id === 'win-x64-msi') },
    { id: 'linux',   name: 'Linux',   sub: 'glibc 2.31+ · X11 / Wayland', builds: buildAssets.filter(b => b.id === 'lin-deb' || b.id === 'lin-app' || b.id === 'lin-flatpak') },
  ].filter(p => p.builds.length > 0);

  const releaseData: Release = { version, date, channel };

  const outDir = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../src/generated');
  fs.mkdirSync(outDir, { recursive: true });
  const outFile = path.join(outDir, 'release.ts');

  const content =
    `import type { Release, PlatformBuilds } from '../types';\n\n` +
    `export const RELEASE: Release = ${serialize(releaseData)};\n\n` +
    `export const PLATFORM_BUILDS: PlatformBuilds[] = ${serialize(platformBuilds)};\n`;

  fs.writeFileSync(outFile, content, 'utf8');
  console.log(`\nWrote: ${outFile}`);
}

main();
