import { T } from '../tokens';
import type { BuildAsset } from '../types';
import type { OsInfo } from '../utils/detectOs';
import { Icon } from './Icon';

interface PrimaryDownloadProps {
  os: OsInfo;
  build: BuildAsset;
  accent: string;
}

export function PrimaryDownload({ os, build, accent }: PrimaryDownloadProps) {
  return (
    <a href={build.downloadUrl || '#'} style={{
      display: 'inline-flex', alignItems: 'center', gap: 14,
      padding: '0 22px 0 18px', height: 56,
      background: T.ink,
      color: T.paperSunk,
      borderRadius: 8,
      textDecoration: 'none',
      fontFamily: '"Space Grotesk", sans-serif',
      transition: 'transform 120ms ease, box-shadow 160ms ease',
      boxShadow: `0 1px 0 rgba(255,255,255,0.6) inset, 0 12px 30px -10px ${accent}55`,
    }}
    onMouseEnter={e => {
      e.currentTarget.style.transform = 'translateY(-1px)';
      e.currentTarget.style.boxShadow = `0 1px 0 rgba(255,255,255,0.6) inset, 0 18px 40px -12px ${accent}88`;
    }}
    onMouseLeave={e => {
      e.currentTarget.style.transform = 'none';
      e.currentTarget.style.boxShadow = `0 1px 0 rgba(255,255,255,0.6) inset, 0 12px 30px -10px ${accent}55`;
    }}
    >
      <span style={{
        display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
        width: 32, height: 32, borderRadius: 6,
        background: T.paperSunk, color: accent,
      }}>
        <Icon name="download" size={18}/>
      </span>
      <span style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>
        <span style={{ fontSize: 16, fontWeight: 500, letterSpacing: '-0.005em', lineHeight: 1.1 }}>
          Download for {os.label}
        </span>
        <span style={{
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 9, letterSpacing: '0.18em', textTransform: 'uppercase',
          color: 'rgba(14,15,18,0.55)',
        }}>{build.arch} · .{build.ext} · {build.size}</span>
      </span>
    </a>
  );
}
