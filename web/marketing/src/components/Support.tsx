import { useState } from 'react';
import { T } from '../tokens';
import { Icon } from './Icon';
import { PodMark } from './PodMark';
import { SectionHead } from './SectionHead';

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
          fontSize: 26, fontWeight: 500, letterSpacing: '0.01em',
          color: T.ink,
        }}>Pod Developers</span>
        <span style={{
          display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
          width: 16, height: 16, borderRadius: '50%',
          background: 'rgba(244,242,236,0.08)',
          color: T.muted,
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 10, fontStyle: 'italic',
        }}>i</span>
        <span style={{
          padding: '3px 8px', borderRadius: 3,
          background: T.plasmaSoft, color: accent,
          border: `1px solid ${accent}33`,
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 9, letterSpacing: '0.22em', textTransform: 'uppercase',
          fontWeight: 500,
          marginLeft: 4,
        }}>PODEV</span>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: 14, position: 'relative' }}>
        <div style={{ textAlign: 'right' }}>
          <div style={{
            fontFamily: '"JetBrains Mono", ui-monospace, monospace',
            fontSize: 9, letterSpacing: '0.28em', textTransform: 'uppercase',
            color: T.muted,
          }}>CEO</div>
          <div style={{
            marginTop: 4,
            fontFamily: '"Space Grotesk", sans-serif',
            fontSize: 16, fontWeight: 500, color: T.ink,
          }}>Pod Dev</div>
        </div>
        <div style={{
          width: 52, height: 52, borderRadius: '50%',
          overflow: 'hidden',
          border: `1px solid ${T.ruleStrong}`,
          background: `linear-gradient(135deg, oklch(34% 0.06 50) 0%, oklch(18% 0.03 50) 100%)`,
          display: 'flex', alignItems: 'flex-end', justifyContent: 'center',
          flexShrink: 0,
        }}>
          <svg width="52" height="52" viewBox="0 0 52 52">
            <defs>
              <linearGradient id="ceoSkin" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="oklch(72% 0.07 50)"/>
                <stop offset="100%" stopColor="oklch(48% 0.06 50)"/>
              </linearGradient>
              <linearGradient id="ceoHair" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="oklch(70% 0.12 70)"/>
                <stop offset="100%" stopColor="oklch(46% 0.10 60)"/>
              </linearGradient>
            </defs>
            <path d="M 4 52 C 8 38, 18 33, 26 33 C 34 33, 44 38, 48 52 Z" fill="#0E0F12"/>
            <rect x="22" y="28" width="8" height="8" fill="url(#ceoSkin)"/>
            <ellipse cx="26" cy="22" rx="9" ry="11" fill="url(#ceoSkin)"/>
            <path d="M 17 18 C 17 11, 21 8, 26 8 C 31 8, 35 11, 35 18 C 35 16, 33 14, 30 14 C 27 14, 26 16, 23 15 C 20 14, 18 16, 17 18 Z" fill="url(#ceoHair)"/>
          </svg>
        </div>
      </div>
    </div>
  );
}

function CopyRow({ label, value, accent, last }: { label: string; value: string; accent: string; last?: boolean }) {
  const [copied, setCopied] = useState(false);
  const onCopy = () => {
    if (navigator.clipboard) navigator.clipboard.writeText(value).catch(() => {});
    setCopied(true);
    setTimeout(() => setCopied(false), 1400);
  };
  return (
    <button onClick={onCopy} style={{
      width: '100%',
      display: 'grid',
      gridTemplateColumns: '120px 1fr auto',
      alignItems: 'center', gap: 16,
      padding: '16px 20px',
      background: 'transparent',
      border: 0,
      borderBottom: last ? 0 : `1px solid ${T.rule}`,
      cursor: 'pointer', textAlign: 'left',
      color: T.ink,
      fontFamily: 'inherit',
      transition: 'background 120ms ease',
    }}
    onMouseEnter={e => { e.currentTarget.style.background = 'rgba(244,242,236,0.025)'; }}
    onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
    >
      <span style={{
        fontFamily: '"JetBrains Mono", ui-monospace, monospace',
        fontSize: 10, letterSpacing: '0.22em', textTransform: 'uppercase',
        color: T.muted,
      }}>{label}</span>
      <span style={{
        fontFamily: '"Space Grotesk", sans-serif',
        fontSize: 16, fontWeight: 500, color: T.ink, letterSpacing: '-0.005em',
      }}>{value}</span>
      <span style={{
        display: 'inline-flex', alignItems: 'center', gap: 8,
        padding: '6px 10px', borderRadius: 6,
        border: `1px solid ${T.rule}`,
        background: copied ? T.plasmaSoft : T.paperSunk,
        color: copied ? accent : T.muted,
        fontFamily: '"JetBrains Mono", ui-monospace, monospace',
        fontSize: 9, letterSpacing: '0.18em', textTransform: 'uppercase',
        transition: 'all 140ms ease',
      }}>
        {copied ? <><Icon name="check" size={12}/> Copied</> : 'Copy'}
      </span>
    </button>
  );
}

interface SupportProps {
  accent: string;
}

export function Support({ accent }: SupportProps) {
  return (
    <section id="support" style={{
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
            eyebrow="Support"
            title={<>Fund the<br/>next build.</>}
            line="Pod is free and MIT-licensed. If you want to back development, the easiest way is in-game — send ISK to the Pod Developers corporation. Every donation goes straight back into time on this binary."
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
            <span>No subscriptions · no ads · anonymous opt-out telemetry</span>
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
            {([
              ['Name',   'Pod Developers'],
              ['Ticker', 'PODEV'],
              ['CEO',    'Pod Dev'],
            ] as const).map(([k, v], i, arr) => (
              <CopyRow key={k} label={k} value={v} accent={accent} last={i === arr.length - 1}/>
            ))}
          </div>

          <div style={{
            display: 'flex', alignItems: 'center', gap: 14,
            padding: '14px 18px',
            border: `1px dashed ${T.rule}`, borderRadius: 8,
            fontFamily: '"JetBrains Mono", ui-monospace, monospace',
            fontSize: 11, color: T.muted, lineHeight: 1.5,
            flexWrap: 'wrap',
          }}>
            <span style={{ color: T.veryMuted, letterSpacing: '0.2em', textTransform: 'uppercase', fontSize: 9 }}>How to send</span>
            <span>
              In-game wallet → Give ISK → paste{' '}
              <span style={{ color: T.ink }}>Pod Developers</span>{' '}
              as the recipient.
            </span>
          </div>
        </div>
      </div>
    </section>
  );
}
