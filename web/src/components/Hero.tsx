import { T } from '../tokens';
import type { BuildAsset, Platform, Release } from '../types';
import type { OsInfo } from '../utils/detectOs';
import { HeroPreview } from './HeroPreview';
import { PrimaryDownload } from './PrimaryDownload';

interface HeroProps {
  os: OsInfo;
  layout: 'split' | 'stacked';
  accent: string;
  archChoice: string;
  onChangeArch: (arch: string) => void;
  release: Release;
  platforms: Platform[];
}

export function Hero({ os, layout, accent, archChoice, onChangeArch, release, platforms }: HeroProps) {
  const macBuild = archChoice || 'mac-arm';
  const isStacked = layout === 'stacked';

  const platform = platforms.find(p => p.id === os.id) ?? platforms[0];
  const buildId = os.id === 'macos' ? macBuild : (os.buildId || platform.builds[0].id);
  const primaryBuild: BuildAsset = platform.builds.find(b => b.id === buildId) ?? platform.builds[0];

  const macPlatform = platforms.find(p => p.id === 'macos') ?? platforms[0];

  return (
    <section style={{
      position: 'relative',
      borderBottom: `1px solid ${T.rule}`,
      overflow: 'hidden',
    }}>
      <div aria-hidden style={{
        position: 'absolute', inset: 0,
        backgroundImage: `
          linear-gradient(${T.rule} 1px, transparent 1px),
          linear-gradient(90deg, ${T.rule} 1px, transparent 1px)
        `,
        backgroundSize: '64px 64px',
        maskImage: 'radial-gradient(ellipse 70% 50% at 30% 40%, #000 30%, transparent 75%)',
        WebkitMaskImage: 'radial-gradient(ellipse 70% 50% at 30% 40%, #000 30%, transparent 75%)',
        opacity: 0.6,
        pointerEvents: 'none',
      }}/>
      <div aria-hidden style={{
        position: 'absolute',
        top: '20%', right: '-10%',
        width: 720, height: 720,
        borderRadius: '50%',
        background: `radial-gradient(circle, ${accent}22 0%, transparent 65%)`,
        filter: 'blur(20px)',
        pointerEvents: 'none',
      }}/>

      <div style={{
        position: 'relative',
        maxWidth: 1320, margin: '0 auto',
        padding: isStacked ? '96px 40px 80px' : '88px 40px 96px',
        display: 'grid',
        gridTemplateColumns: isStacked ? '1fr' : 'minmax(0, 1.05fr) minmax(0, 1fr)',
        gap: 64, alignItems: 'center',
      }}>
        <div style={{ maxWidth: 600 }}>
          <div style={{
            display: 'inline-flex', alignItems: 'center', gap: 10,
            padding: '6px 10px 6px 8px',
            border: `1px solid ${T.rule}`, borderRadius: 999,
            background: T.paperSunk,
            marginBottom: 28,
          }}>
            <span style={{
              padding: '1px 7px', borderRadius: 999,
              background: release.channel !== 'stable' ? 'rgba(217,178,82,0.12)' : T.plasmaSoft,
              color: release.channel !== 'stable' ? T.warning : accent,
              fontFamily: '"JetBrains Mono", ui-monospace, monospace',
              fontSize: 9, letterSpacing: '0.22em', textTransform: 'uppercase',
              fontWeight: 500,
            }}>{release.channel}</span>
            <span style={{
              fontFamily: '"JetBrains Mono", ui-monospace, monospace',
              fontSize: 10, letterSpacing: '0.16em', color: T.muted,
              textTransform: 'uppercase',
            }}>v{release.version} · {release.date}</span>
          </div>

          <h1 style={{
            margin: 0,
            fontFamily: '"Space Grotesk", sans-serif',
            fontWeight: 500,
            fontSize: 'clamp(48px, 5.6vw, 76px)',
            lineHeight: 1.02,
            letterSpacing: '-0.025em',
            color: T.ink,
            textWrap: 'balance',
          } as React.CSSProperties}>
            Your EVE Online capsule,<br/>
            <span style={{ color: T.muted }}>on the desktop.</span>
          </h1>

          <p style={{
            marginTop: 24, marginBottom: 36,
            fontSize: 17, lineHeight: 1.5,
            color: T.muted, maxWidth: 520,
            textWrap: 'pretty',
          } as React.CSSProperties}>
            Pod is a native EVE Online companion for every pilot you fly.
            Wallets, skills, fitting, mail and assets — across every character —
            in one keyboard-driven window.
          </p>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 14, alignItems: 'flex-start' }}>
            <PrimaryDownload os={os} build={primaryBuild} accent={accent}/>

            {os.id === 'macos' && (
              <div style={{
                display: 'inline-flex',
                border: `1px solid ${T.rule}`, borderRadius: 6,
                overflow: 'hidden',
              }}>
                {macPlatform.builds.map(b => {
                  const active = b.id === macBuild;
                  return (
                    <button key={b.id} onClick={() => { onChangeArch(b.id); }} style={{
                      padding: '6px 12px',
                      background: active ? T.plasmaSoft : 'transparent',
                      color: active ? accent : T.muted,
                      fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                      fontSize: 9, letterSpacing: '0.18em',
                      textTransform: 'uppercase', fontWeight: 500,
                      border: 0, cursor: 'pointer',
                      borderRight: b.id === 'mac-x64' ? 0 : `1px solid ${T.rule}`,
                    }}>{b.arch.replace('· ', '')}</button>
                  );
                })}
              </div>
            )}

            <div style={{
              display: 'flex', alignItems: 'center', gap: 16,
              fontFamily: '"JetBrains Mono", ui-monospace, monospace',
              fontSize: 10, letterSpacing: '0.14em', color: T.veryMuted,
              textTransform: 'uppercase',
            }}>
              <span>{primaryBuild.size}</span>
              <span>·</span>
              <a href="#download" style={{ color: T.muted, textDecoration: 'none' }}
                onMouseEnter={e => { e.currentTarget.style.color = T.ink; }}
                onMouseLeave={e => { e.currentTarget.style.color = T.muted; }}
              >Other platforms →</a>
            </div>
          </div>
        </div>

        {!isStacked && <HeroPreview accent={accent}/>}
      </div>
      {isStacked && (
        <div style={{ maxWidth: 1320, margin: '0 auto', padding: '0 40px 88px' }}>
          <HeroPreview accent={accent}/>
        </div>
      )}
    </section>
  );
}
