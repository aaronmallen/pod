import { T } from '../tokens';
import { FEATURES } from '../data';
import { SectionHead } from './SectionHead';

const ICONS: Record<string, React.ReactNode> = {
  calendar: <g>
    <rect x="4" y="5" width="16" height="16" rx="1.5" fill="none" stroke="currentColor" strokeWidth="2"/>
    <line x1="4" y1="9.5" x2="20" y2="9.5" stroke="currentColor" strokeWidth="2"/>
    <line x1="8" y1="3" x2="8" y2="6.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="16" y1="3" x2="16" y2="6.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <circle cx="12" cy="15" r="1.5" fill="currentColor"/>
  </g>,
  industry: <g>
    <path d="M 12 3 L 19.5 7.25 L 19.5 16.75 L 12 21 L 4.5 16.75 L 4.5 7.25 Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <circle cx="12" cy="12" r="3.5" fill="none" stroke="currentColor" strokeWidth="2"/>
  </g>,
  fitting: <g>
    <path d="M 10.437 20.863 A 9 9 0 0 1 3.543 8.922" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 5.106 6.215 A 9 9 0 0 1 18.894 6.215" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 20.457 8.922 A 9 9 0 0 1 13.563 20.863" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 12 8.6 L 14.94 13.7 L 9.06 13.7 Z" fill="currentColor"/>
  </g>,
  mail: <g>
    <rect x="3" y="6" width="18" height="13" rx="1.5" fill="none" stroke="currentColor" strokeWidth="2"/>
    <path d="M 4 7.5 L 12 13 L 20 7.5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
  </g>,
  assets: <g>
    <path d="M 12 4 L 20 8 L 20 16 L 12 20 L 4 16 L 4 8 Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <path d="M 4 8 L 12 12 L 20 8" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <line x1="12" y1="12" x2="12" y2="20" stroke="currentColor" strokeWidth="2"/>
  </g>,
  market: <g>
    <polyline points="4 16 9 11 13 14 20 7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    <polyline points="15 7 20 7 20 12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
  </g>,
  roster: <g>
    <circle cx="12" cy="8" r="4" fill="none" stroke="currentColor" strokeWidth="2"/>
    <path d="M 4 20 C 5 15.5, 19 15.5, 20 20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
  </g>,
  wallet: <g>
    <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2"/>
    <line x1="8" y1="7.5" x2="16" y2="7.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="16" y1="7.5" x2="8" y2="16.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="8" y1="16.5" x2="16" y2="16.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="6.5" y1="12" x2="17.5" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
  </g>,
  skills: <g>
    <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2"/>
    <path d="M 7 8 C 9 12, 15 12, 17 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 7 16 C 9 12, 15 12, 17 8" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <circle cx="12" cy="12" r="1.6" fill="currentColor"/>
  </g>,
};

function FeatureIcon({ name, size = 22 }: { name: string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" style={{ display: 'block' }}>
      {ICONS[name]}
    </svg>
  );
}

interface Props {
  accent: string;
}

export function Features({ accent }: Props) {
  return (
    <section style={{
      borderBottom: `1px solid ${T.rule}`,
      padding: '88px 40px',
      background: T.paperSunk,
    }}>
      <div style={{ maxWidth: 1320, margin: '0 auto' }}>
        <SectionHead
          eyebrow="In the box"
          title="One app, packed with features."
          line="Every Pod feature for EVE Online ships in every build. No add-ons, no plugins, no per-seat tier."
        />

        <div style={{
          marginTop: 48,
          display: 'grid',
          gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
          gap: 1,
          background: T.rule,
          border: `1px solid ${T.rule}`,
          borderRadius: 12,
          overflow: 'hidden',
        }}>
          {FEATURES.map(f => {
            const muted = !!f.soon;
            return (
              <div key={f.id} style={{
                background: T.paper,
                padding: '28px 26px 32px',
                display: 'flex', flexDirection: 'column', gap: 14,
                position: 'relative',
              }}>
                <div style={{
                  display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                }}>
                  <div style={{
                    width: 40, height: 40, borderRadius: 8,
                    background: muted ? 'rgba(244,242,236,0.04)' : T.plasmaSoft,
                    color: muted ? T.muted : accent,
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    border: `1px solid ${muted ? T.rule : accent + '33'}`,
                  }}>
                    <FeatureIcon name={f.icon} size={22}/>
                  </div>
                  {muted && (
                    <span style={{
                      padding: '3px 8px', borderRadius: 3,
                      background: 'rgba(217,178,82,0.10)',
                      color: T.warning,
                      border: `1px solid rgba(217,178,82,0.30)`,
                      fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                      fontSize: 9, letterSpacing: '0.18em', textTransform: 'uppercase',
                      fontWeight: 500,
                    }}>Coming soon</span>
                  )}
                </div>
                <div style={{
                  fontFamily: '"Space Grotesk", sans-serif',
                  fontSize: 18, fontWeight: 500,
                  color: muted ? T.muted : T.ink,
                  letterSpacing: '-0.01em',
                }}>{f.title}</div>
                <div style={{
                  fontSize: 13, lineHeight: 1.5,
                  color: muted ? T.veryMuted : T.muted,
                  textWrap: 'pretty',
                } as React.CSSProperties}>{f.line}</div>
                {f.subs && f.subs.length > 0 && (
                  <div style={{
                    display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 2,
                  }}>
                    {f.subs.map(s => (
                      <span key={s} style={{
                        padding: '3px 8px', borderRadius: 4,
                        background: 'rgba(244,242,236,0.04)',
                        border: `1px solid ${T.rule}`,
                        fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                        fontSize: 10, letterSpacing: '0.02em',
                        color: T.muted,
                      }}>{s}</span>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
