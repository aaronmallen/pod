import { T } from '../tokens';
import type { Platform } from '../types';
import { PlatformGlyph } from './PlatformGlyph';

const ArrowIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" style={{ display: 'block' }}>
    <line x1="5" y1="12" x2="19" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 13 6 L 19 12 L 13 18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
  </svg>
);

interface Props {
  platform: Platform;
  accent: string;
}

export function PlatformCard({ platform, accent }: Props) {
  return (
    <div
      style={{
        background: T.paper,
        border: `1px solid ${T.rule}`,
        borderRadius: 12,
        padding: '22px 22px 6px',
        display: 'flex', flexDirection: 'column',
        transition: 'border-color 140ms ease, transform 140ms ease',
      }}
      onMouseEnter={e => {
        (e.currentTarget as HTMLDivElement).style.borderColor = T.ruleStrong;
        (e.currentTarget as HTMLDivElement).style.transform = 'translateY(-1px)';
      }}
      onMouseLeave={e => {
        (e.currentTarget as HTMLDivElement).style.borderColor = T.rule;
        (e.currentTarget as HTMLDivElement).style.transform = 'none';
      }}
    >
      <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between' }}>
        <div>
          <div style={{
            fontFamily: '"Space Grotesk", sans-serif',
            fontSize: 22, fontWeight: 500, color: T.ink,
            letterSpacing: '-0.01em',
          }}>{platform.name}</div>
          <div style={{
            marginTop: 4,
            fontFamily: '"JetBrains Mono", ui-monospace, monospace',
            fontSize: 10, letterSpacing: '0.14em', textTransform: 'uppercase',
            color: T.muted,
          }}>{platform.sub}</div>
        </div>
        <PlatformGlyph id={platform.id} accent={accent}/>
      </div>

      <div style={{
        marginTop: 22, marginBottom: 6,
        display: 'flex', flexDirection: 'column',
      }}>
        {platform.builds.map(b => (
          <a
            key={b.id}
            href={b.downloadUrl || '#'}
            style={{
              display: 'grid',
              gridTemplateColumns: '1fr auto auto',
              alignItems: 'center', gap: 14,
              padding: '14px 0',
              borderTop: `1px solid ${T.rule}`,
              color: T.ink, textDecoration: 'none',
            }}
            onMouseEnter={e => { (e.currentTarget as HTMLAnchorElement).style.background = 'rgba(244,242,236,0.025)'; }}
            onMouseLeave={e => { (e.currentTarget as HTMLAnchorElement).style.background = 'transparent'; }}
          >
            <span style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              <span style={{ fontSize: 14, color: T.ink }}>{b.arch}</span>
              <span style={{
                fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                fontSize: 10, letterSpacing: '0.14em', textTransform: 'uppercase',
                color: T.veryMuted,
              }}>.{b.ext} · {b.size}</span>
            </span>
            <span style={{
              fontFamily: '"JetBrains Mono", ui-monospace, monospace',
              fontSize: 10, letterSpacing: '0.18em', textTransform: 'uppercase',
              color: T.muted,
            }}>Get</span>
            <span style={{
              width: 28, height: 28, borderRadius: 6,
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              background: T.paperSunk, border: `1px solid ${T.rule}`,
              color: accent,
            }}>
              <ArrowIcon/>
            </span>
          </a>
        ))}
      </div>
    </div>
  );
}
