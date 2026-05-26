import { Fragment } from 'react';
import { T } from '../tokens';
import { SectionHead } from './SectionHead';

const REQS: [string, string][] = [
  ['macOS',   '12 Monterey or later · Apple Silicon or Intel · 240 MB disk'],
  ['Windows', '10 (build 1809) or later · 64-bit · 240 MB disk'],
  ['Linux',   'glibc 2.31 · X11 or Wayland · 260 MB disk'],
  ['Network', 'HTTPS to esi.evetech.net · 1 MB/hr typical sync'],
  ['Storage', 'Local SQLite cache · data stays on your machine'],
];

interface Props {
  accent: string;
}

export function Requirements({ accent: _accent }: Props) {
  return (
    <section style={{
      borderBottom: `1px solid ${T.rule}`,
      padding: '88px 40px',
      background: T.paperSunk,
    }}>
      <div style={{
        maxWidth: 1320, margin: '0 auto',
        display: 'grid',
        gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1.6fr)',
        gap: 64,
      }}>
        <SectionHead
          eyebrow="Requirements"
          title="What Pod needs to run."
          line="Lightweight by design — Pod runs comfortably in the background while you fly."
        />

        <div style={{
          display: 'grid', gridTemplateColumns: '160px 1fr',
          gap: 0,
          borderTop: `1px solid ${T.rule}`,
        }}>
          {REQS.map(([k, v]) => (
            <Fragment key={k}>
              <div style={{
                padding: '18px 0', borderBottom: `1px solid ${T.rule}`,
                fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                fontSize: 10, letterSpacing: '0.22em', textTransform: 'uppercase',
                color: T.muted,
              }}>{k}</div>
              <div style={{
                padding: '18px 0', borderBottom: `1px solid ${T.rule}`,
                fontFamily: '"Space Grotesk", sans-serif',
                fontSize: 14, color: T.ink, lineHeight: 1.5,
              }}>{v}</div>
            </Fragment>
          ))}
        </div>
      </div>
    </section>
  );
}
