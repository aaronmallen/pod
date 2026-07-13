import { T } from '../tokens';
import { Icon } from './Icon';
import { PodMark } from './PodMark';
import { SectionHead } from './SectionHead';

const ATHANORS: [string, string][] = [
  ['Moon I',   'Reprocessing — high-yield ore & ice'],
  ['Moon II',  'Ore & gas compression'],
  ['Moon III', 'Moon drilling — scheduled chunks'],
  ['Moon IV',  'Fitting, repair & staging'],
];

function CorpBanner({ accent }: { accent: string }) {
  return (
    <div style={{
      position: 'relative',
      borderRadius: 12,
      overflow: 'hidden',
      border: `1px solid ${T.ruleStrong}`,
      background: `
        linear-gradient(135deg, rgba(63,184,219,0.10) 0%, rgba(63,184,219,0.04) 50%, rgba(20,38,46,0.6) 100%),
        ${T.paperRaised}
      `,
      padding: '22px 24px',
      display: 'grid',
      gridTemplateColumns: 'auto 1fr auto',
      alignItems: 'center',
      gap: 22,
      minHeight: 96,
    }}>
      <div aria-hidden style={{
        position: 'absolute', inset: 0,
        backgroundImage: `radial-gradient(${T.rule} 1px, transparent 1px)`,
        backgroundSize: '14px 14px',
        opacity: 0.5,
        pointerEvents: 'none',
      }}/>
      <div aria-hidden style={{
        position: 'absolute', right: 0, bottom: 0,
        width: 0, height: 0,
        borderStyle: 'solid',
        borderWidth: '0 0 36px 60px',
        borderColor: `transparent transparent ${accent}33 transparent`,
        pointerEvents: 'none',
      }}/>

      <div style={{
        position: 'relative',
        width: 56, height: 56,
        borderRadius: '50%',
        background: T.paperSunk,
        border: `1.5px solid ${accent}`,
        boxShadow: `0 0 0 4px rgba(63,184,219,0.10), 0 0 18px ${accent}66`,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}>
        <PodMark size={30} color={T.ink}/>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 12, position: 'relative' }}>
        <span style={{
          fontFamily: '"Space Grotesk", sans-serif',
          fontSize: 24, fontWeight: 500, letterSpacing: '0.01em',
          color: T.ink,
        }}>Hror Freeport</span>
        <span style={{
          display: 'inline-flex', alignItems: 'center', gap: 8,
          padding: '3px 8px', borderRadius: 3,
          background: 'rgba(91,185,126,0.10)', color: T.success,
          border: '1px solid rgba(91,185,126,0.30)',
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 9, letterSpacing: '0.18em', textTransform: 'uppercase',
          fontWeight: 500,
        }}>Open</span>
      </div>

      <div style={{
        display: 'flex', flexDirection: 'column', alignItems: 'flex-end',
        position: 'relative',
      }}>
        <span style={{
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 30, color: T.ink, lineHeight: 1,
          letterSpacing: '-0.01em', fontVariantNumeric: 'tabular-nums',
        }}>
          <span style={{ color: accent }}>4</span> × Athanor
        </span>
        <span style={{
          marginTop: 6,
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 9, letterSpacing: '0.22em', textTransform: 'uppercase',
          color: T.muted,
        }}>Moon refineries</span>
      </div>
    </div>
  );
}

function StatRow({ k, first, children }: { k: string; first?: boolean; children: React.ReactNode }) {
  return (
    <div style={{
      display: 'grid', gridTemplateColumns: '140px 1fr',
      alignItems: 'baseline', gap: 16,
      padding: '15px 20px',
      borderTop: first ? 0 : `1px solid ${T.rule}`,
    }}>
      <span style={{
        fontFamily: '"JetBrains Mono", ui-monospace, monospace',
        fontSize: 10, letterSpacing: '0.22em', textTransform: 'uppercase',
        color: T.muted,
      }}>{k}</span>
      <span style={{
        fontFamily: '"Space Grotesk", sans-serif',
        fontSize: 15, color: T.ink, lineHeight: 1.5,
      }}>{children}</span>
    </div>
  );
}

interface Props {
  accent: string;
}

export function Freeport({ accent }: Props) {
  return (
    <section id="freeport" style={{
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
            eyebrow="Now open"
            title={<>Public moon mining,<br/>now in Hror.</>}
            line="Four Athanor refineries anchored and open to all pilots. Dock, reprocess, compress and haul out — no standings, no application, no cut of your ore."
          />
          <div style={{
            marginTop: 28,
            display: 'inline-flex', alignItems: 'center', gap: 10,
            padding: '8px 12px',
            border: `1px solid ${T.rule}`, borderRadius: 999,
            background: T.paperSunk,
            fontFamily: '"JetBrains Mono", ui-monospace, monospace',
            fontSize: 10, letterSpacing: '0.18em', textTransform: 'uppercase',
            color: T.muted,
          }}>
            <Icon name="check" size={12}/>
            <span>Freeport · public docking · open service modules</span>
          </div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <CorpBanner accent={accent}/>

          <div style={{
            background: T.paper,
            border: `1px solid ${T.rule}`,
            borderRadius: 12,
            overflow: 'hidden',
          }}>
            <StatRow k="System" first>
              Hror <span style={{ color: T.muted }}>· highsec · Metropolis</span>
            </StatRow>
            <StatRow k="Structures">
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                {ATHANORS.map(([moon, role]) => (
                  <div key={moon} style={{ display: 'flex', alignItems: 'baseline', gap: 10 }}>
                    <span style={{
                      fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                      fontSize: 11, color: T.veryMuted, minWidth: 74,
                      letterSpacing: '0.04em',
                    }}>{moon}</span>
                    <span style={{ fontSize: 14, color: T.ink }}>{role}</span>
                  </div>
                ))}
              </div>
            </StatRow>
            <StatRow k="Access">
              Freeport — public docking, no standings required
            </StatRow>
            <StatRow k="Reprocessing">
              Open to all <span style={{ color: T.muted }}>· low public tax</span>
            </StatRow>
            <StatRow k="Extraction">
              Automated moon chunks on a fixed cycle — watch fleet channels for pops
            </StatRow>
          </div>

          <div style={{
            display: 'flex', alignItems: 'center', gap: 14,
            padding: '14px 18px',
            border: `1px dashed ${T.rule}`, borderRadius: 8,
            fontFamily: '"JetBrains Mono", ui-monospace, monospace',
            fontSize: 11, color: T.muted, lineHeight: 1.5,
            flexWrap: 'wrap',
          }}>
            <span style={{ color: T.veryMuted, letterSpacing: '0.2em', textTransform: 'uppercase', fontSize: 9 }}>Getting there</span>
            <span>
              Set destination to <span style={{ color: T.ink }}>Hror</span> — all four Athanors show in your Overview on grid.
            </span>
          </div>
        </div>
      </div>
    </section>
  );
}
