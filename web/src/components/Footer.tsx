import { T } from '../tokens';
import type { Release } from '../types';

const PodMark = ({ size = 22, color = T.railFg }: { size?: number; color?: string }) => (
  <svg width={size} height={size} viewBox="0 0 200 200" style={{ display: 'block' }}>
    <line x1="42" y1="100" x2="42" y2="192" stroke={color} strokeWidth="18" strokeLinecap="round"/>
    <line x1="158" y1="8" x2="158" y2="100" stroke={color} strokeWidth="18" strokeLinecap="round"/>
    <circle cx="100" cy="100" r="58" fill="none" stroke={color} strokeWidth="18"/>
    <circle cx="100" cy="100" r="10" fill={T.plasma}/>
  </svg>
);

type NavCol = [string, [string, string][]];

const NAV_COLS: NavCol[] = [
  ['Pod', [
    ['Download', '#download'],
    ["What's new", '#notes'],
    ['Docs', '/docs/'],
    ['FAQ', '#faq'],
    ['Support', '#support'],
  ]],
  ['Resource', [
    ['Source on GitHub', 'https://github.com/aaronmallen/pod'],
    ['Discord', 'https://discord.gg/VZpQ56pcHw'],
    ['Report an issue', 'https://github.com/aaronmallen/pod/issues'],
  ]],
  ['Legal', [
    ['License (MIT)', 'https://github.com/aaronmallen/pod/blob/main/LICENSE'],
  ]],
];

interface Props {
  basePath?: string;
  release: Release;
}

function resolveHref(href: string, basePath: string): string {
  return href.startsWith('#') ? `${basePath}${href}` : href;
}

export function Footer({ basePath = '', release }: Props) {
  return (
    <footer>
      <div style={{
        maxWidth: 1320, margin: '0 auto',
        padding: '64px 40px 40px',
        display: 'grid',
        gridTemplateColumns: 'minmax(0, 1.4fr) 1fr 1fr 1fr',
        gap: 40,
      }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 14 }}>
            <PodMark size={22} color={T.ink}/>
            <span style={{ fontSize: 17, fontWeight: 500, letterSpacing: '-0.01em' }}>Pod</span>
          </div>
          <div style={{ fontSize: 12, color: T.muted, lineHeight: 1.6, maxWidth: 360 }}>
            An independent, fan-made companion. Pod uses the public ESI (EVE Swagger
            Interface) to read your character data {'—'} with your explicit consent, on
            your machine, at your pace.
          </div>
        </div>

        {NAV_COLS.map(([title, links]) => (
          <div key={title}>
            <div style={{
              fontFamily: '"JetBrains Mono", ui-monospace, monospace',
              fontSize: 9, letterSpacing: '0.28em', textTransform: 'uppercase',
              color: T.muted, marginBottom: 14,
            }}>{title}</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {links.map(([label, href]) => (
                <a
                  key={label}
                  href={resolveHref(href, basePath)}
                  style={{ color: T.ink, textDecoration: 'none', fontSize: 13 }}
                  onMouseEnter={e => { (e.currentTarget as HTMLAnchorElement).style.color = T.plasma; }}
                  onMouseLeave={e => { (e.currentTarget as HTMLAnchorElement).style.color = T.ink; }}
                >{label}</a>
              ))}
            </div>
          </div>
        ))}
      </div>

      <div style={{
        maxWidth: 1320, margin: '0 auto',
        padding: '0 40px 40px',
      }}>
        <div style={{
          borderTop: `1px solid ${T.rule}`,
          paddingTop: 28,
          fontSize: 11, color: T.veryMuted, lineHeight: 1.7, maxWidth: 920,
        }}>
          EVE Online and the EVE logo are the registered trademarks of Fenris Creations
          (formerly CCP hf.). All rights reserved worldwide. All other trademarks are the
          property of their respective owners. EVE Online, the EVE logo, EVE and all
          associated logos and designs are the intellectual property of Fenris Creations.
          All artwork, screenshots, characters, vehicles, storylines, world facts or other
          recognizable features of the intellectual property relating to these trademarks
          are likewise the intellectual property of Fenris Creations. Fenris Creations has
          granted permission to Pod to use EVE Online and all associated logos and designs
          for promotional and information purposes but does not endorse, and is not in any
          way affiliated with, Pod. Fenris Creations is in no way responsible for the
          content on or functioning of this website, nor can it be liable for any damage
          arising from the use of this website.
        </div>
      </div>

      <div style={{
        borderTop: `1px solid ${T.rule}`,
        background: T.rail,
        padding: '0 28px',
        height: 36,
        display: 'flex', alignItems: 'center', gap: 16,
        fontFamily: '"JetBrains Mono", ui-monospace, monospace',
        fontSize: 9, letterSpacing: '0.18em', textTransform: 'uppercase',
        color: T.muted,
      }}>
        <span style={{ color: T.veryMuted }}>build</span>
        <span style={{ color: T.ink }}>{release.version}</span>
        <span style={{ flex: 1 }}/>
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
          <span style={{ width: 6, height: 6, borderRadius: '50%', background: T.success }}/>
          All systems nominal
        </span>
        <span style={{ color: T.veryMuted }}>{'·'}</span>
        <span>{'© 2026 Pod · MIT licensed'}</span>
      </div>
    </footer>
  );
}
