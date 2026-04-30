import { T } from '../tokens';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { PodMark } from './PodMark';
import { SparkChart } from './SparkChart';

interface HeroPreviewProps {
  accent: string;
}

export function HeroPreview({ accent }: HeroPreviewProps) {
  return (
    <div style={{
      position: 'relative',
      borderRadius: 14,
      overflow: 'hidden',
      background: T.paper,
      border: `1px solid ${T.ruleStrong}`,
      boxShadow: `
        0 40px 80px -30px rgba(0,0,0,0.7),
        0 16px 32px -20px rgba(0,0,0,0.6),
        0 1px 0 rgba(255,255,255,0.04) inset
      `,
      fontFamily: '"Space Grotesk", sans-serif',
    }}>
      <div style={{
        height: 36, padding: '0 14px',
        display: 'flex', alignItems: 'center', gap: 8,
        background: T.rail,
        borderBottom: '1px solid rgba(0,0,0,0.6)',
      }}>
        {(['#ff5f57', '#febc2e', '#28c840'] as const).map(c => (
          <span key={c} style={{
            width: 12, height: 12, borderRadius: '50%',
            background: c, opacity: 0.85,
          }}/>
        ))}
        <span style={{
          marginLeft: 16,
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 10, letterSpacing: '0.18em', textTransform: 'uppercase',
          color: T.muted,
        }}>Pod · Wallet</span>
      </div>

      <div style={{ display: 'flex', height: 460 }}>
        <div style={{
          width: 56, background: T.rail,
          borderRight: '1px solid rgba(0,0,0,0.4)',
          display: 'flex', flexDirection: 'column', alignItems: 'center',
          padding: '14px 0', gap: 14,
        }}>
          <PodMark size={22} color={T.railFg} dotColor={accent}/>
          <div style={{ height: 1, width: 28, background: T.rule, margin: '6px 0' }}/>
          {(['characters', 'skills', 'mail', 'wallet', 'assets'] as IconName[]).map(n => {
            const active = n === 'wallet';
            return (
              <div key={n} style={{
                width: 36, height: 36, borderRadius: 8,
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                background: active ? T.plasmaSoft : 'transparent',
                color: active ? accent : T.muted,
                border: active ? `1px solid ${accent}55` : '1px solid transparent',
              }}>
                <Icon name={n} size={18}/>
              </div>
            );
          })}
        </div>

        <div style={{ flex: 1, padding: '20px 22px', display: 'flex', flexDirection: 'column', gap: 18 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' }}>
            <div>
              <div style={{
                fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                fontSize: 9, letterSpacing: '0.28em', textTransform: 'uppercase',
                color: T.muted, marginBottom: 6,
              }}>NET WORTH · ALL WALLETS</div>
              <div style={{
                fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                fontSize: 34, color: T.ink, fontVariantNumeric: 'tabular-nums',
                letterSpacing: '-0.01em',
              }}>
                42.8B<span style={{ color: T.muted, fontSize: 18, marginLeft: 4 }}>ISK</span>
              </div>
              <div style={{
                marginTop: 6,
                fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                fontSize: 11, color: T.success, fontVariantNumeric: 'tabular-nums',
              }}>+ 2.41% · 24h</div>
            </div>
            <div style={{
              display: 'inline-flex', gap: 4,
              padding: 3,
              background: T.paperSunk, border: `1px solid ${T.rule}`,
              borderRadius: 6,
            }}>
              {(['1d', '7d', '30d', '90d', 'YTD'] as const).map(p => (
                <span key={p} style={{
                  padding: '4px 9px', borderRadius: 4,
                  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                  fontSize: 9, letterSpacing: '0.14em', textTransform: 'uppercase',
                  background: p === '30d' ? T.plasmaSoft : 'transparent',
                  color: p === '30d' ? accent : T.muted,
                }}>{p}</span>
              ))}
            </div>
          </div>

          <SparkChart accent={accent}/>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <div style={{
              display: 'grid', gridTemplateColumns: '1.4fr 1fr auto auto',
              padding: '8px 0',
              borderBottom: `1px solid ${T.rule}`,
              fontFamily: '"JetBrains Mono", ui-monospace, monospace',
              fontSize: 9, letterSpacing: '0.22em', textTransform: 'uppercase',
              color: T.muted,
            }}>
              <div>Reference</div><div>Party</div>
              <div style={{ textAlign: 'right' }}>When</div>
              <div style={{ textAlign: 'right', paddingLeft: 24 }}>Amount</div>
            </div>
            {(
              [
                ['Bounty prize',    'Concord',           '4m',  '+12,480,000', T.success],
                ['Market sell',     'Jita 4-4 · MoonCo', '17m', '+86,200,000', T.success],
                ['Broker fee',      'Jita 4-4',          '17m', '-1,724,000',  T.danger],
                ['Mission reward',  'Sister Alitura',    '38m', '+3,180,000',  T.success],
                ['Contract payout', 'Hauler-7',          '1h',  '-21,000,000', T.danger],
              ] as [string, string, string, string, string][]
            ).map((row, i) => (
              <div key={i} style={{
                display: 'grid', gridTemplateColumns: '1.4fr 1fr auto auto',
                padding: '10px 0',
                borderBottom: `1px solid ${T.rule}`,
                fontSize: 12, color: T.ink,
                alignItems: 'baseline',
              }}>
                <div>{row[0]}</div>
                <div style={{ color: T.muted }}>{row[1]}</div>
                <div style={{
                  textAlign: 'right',
                  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                  fontSize: 11, color: T.muted,
                }}>{row[2]}</div>
                <div style={{
                  textAlign: 'right', paddingLeft: 24,
                  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                  fontSize: 12, color: row[4], fontVariantNumeric: 'tabular-nums',
                }}>{row[3]}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
