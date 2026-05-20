import { T } from '../tokens';
import { NOTES } from '../generated/notes';
import type { Release } from '../types';
import { SectionHead } from './SectionHead';

const GithubIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" style={{ display: 'block' }}>
    <path d="M 12 2 C 6.48 2 2 6.48 2 12 c 0 4.42 2.87 8.17 6.84 9.5 c 0.5 0.08 0.66 -0.23 0.66 -0.5 v -1.69 c -2.77 0.6 -3.36 -1.34 -3.36 -1.34 c -0.46 -1.16 -1.11 -1.47 -1.11 -1.47 c -0.91 -0.62 0.07 -0.6 0.07 -0.6 c 1 0.07 1.53 1.03 1.53 1.03 c 0.87 1.52 2.34 1.07 2.91 0.83 c 0.09 -0.65 0.35 -1.09 0.63 -1.34 c -2.22 -0.25 -4.55 -1.11 -4.55 -4.94 c 0 -1.1 0.39 -1.99 1.03 -2.69 c -0.1 -0.25 -0.45 -1.27 0.1 -2.65 c 0 0 0.84 -0.27 2.75 1.02 c 0.8 -0.22 1.65 -0.33 2.5 -0.33 c 0.85 0 1.7 0.11 2.5 0.33 c 1.91 -1.29 2.75 -1.02 2.75 -1.02 c 0.55 1.38 0.2 2.4 0.1 2.65 c 0.64 0.7 1.03 1.59 1.03 2.69 c 0 3.84 -2.34 4.68 -4.57 4.93 c 0.36 0.31 0.69 0.92 0.69 1.85 V 21 c 0 0.27 0.16 0.59 0.67 0.5 C 19.14 20.16 22 16.42 22 12 A 10 10 0 0 0 12 2 Z" fill="currentColor"/>
  </svg>
);

interface Props {
  accent: string;
  release: Release;
}

export function Notes({ accent, release }: Props) {
  return (
    <section id="notes" style={{
      borderBottom: `1px solid ${T.rule}`,
      padding: '88px 40px',
    }}>
      <div style={{
        maxWidth: 1320, margin: '0 auto',
        display: 'grid',
        gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1.6fr)',
        gap: 64,
      }}>
        <div>
          <SectionHead
            eyebrow={`Release ${release.version}`}
            title={<>What&rsquo;s new<br/>in this build.</>}
            line={`${release.date}. Read the full log on GitHub.`}
          />
          <a
            href="https://github.com/aaronmallen/pod/blob/main/CHANGELOG.md"
            target="_blank"
            rel="noreferrer"
            style={{
              display: 'inline-flex', alignItems: 'center', gap: 10,
              marginTop: 28,
              padding: '10px 14px 10px 12px',
              border: `1px solid ${T.ruleStrong}`, borderRadius: 8,
              color: T.ink, textDecoration: 'none',
              fontFamily: '"Space Grotesk", sans-serif',
              fontSize: 13,
            }}
            onMouseEnter={e => { (e.currentTarget as HTMLAnchorElement).style.borderColor = accent; }}
            onMouseLeave={e => { (e.currentTarget as HTMLAnchorElement).style.borderColor = T.ruleStrong; }}
          >
            <GithubIcon/>
            View full changelog
            <span style={{ color: T.veryMuted }}>{'↗'}</span>
          </a>
        </div>

        <ul style={{
          listStyle: 'none', padding: 0, margin: 0,
          display: 'flex', flexDirection: 'column',
          borderTop: `1px solid ${T.rule}`,
        }}>
          {NOTES.map((n, i) => {
            const color = n.tone === 'plasma' ? accent
                        : n.tone === 'success' ? T.success
                        : n.tone === 'warning' ? T.warning
                        : T.muted;
            const bg    = n.tone === 'plasma' ? T.plasmaSoft
                        : n.tone === 'success' ? 'rgba(91,185,126,0.10)'
                        : n.tone === 'warning' ? 'rgba(217,178,82,0.12)'
                        : 'rgba(244,242,236,0.05)';
            return (
              <li key={i} style={{
                display: 'grid',
                gridTemplateColumns: 'auto 1fr',
                alignItems: 'baseline', gap: 18,
                padding: '20px 4px',
                borderBottom: `1px solid ${T.rule}`,
              }}>
                <span style={{
                  display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                  padding: '3px 8px', borderRadius: 3,
                  background: bg, color,
                  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                  fontSize: 9, letterSpacing: '0.18em', textTransform: 'uppercase',
                  fontWeight: 500,
                  minWidth: 48, justifySelf: 'start',
                  border: `1px solid ${color}33`,
                } as React.CSSProperties}>{n.tag}</span>
                <span style={{
                  fontFamily: '"Space Grotesk", sans-serif',
                  fontSize: 15, lineHeight: 1.5, color: T.ink, textWrap: 'pretty',
                } as React.CSSProperties}>{n.text}</span>
              </li>
            );
          })}
        </ul>
      </div>
    </section>
  );
}
