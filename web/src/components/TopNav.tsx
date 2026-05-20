import { T } from '../tokens';
import type { Release } from '../types';
import { PodMark } from './PodMark';

interface TopNavProps {
  accent: string;
  release: Release;
}

const NAV_LINKS: [string, string][] = [
  ['Download', '#download'],
  ["What's new", '#notes'],
  ['Support', '#support'],
  ['Discord', 'https://discord.gg/VZpQ56pcHw'],
  ['Source', 'https://github.com/aaronmallen/pod'],
];

export function TopNav({ accent, release }: TopNavProps) {
  return (
    <nav style={{
      position: 'sticky', top: 0, zIndex: 10,
      display: 'flex', alignItems: 'center', justifyContent: 'space-between',
      padding: '18px 40px',
      background: 'rgba(14,15,18,0.78)',
      backdropFilter: 'blur(14px) saturate(140%)',
      WebkitBackdropFilter: 'blur(14px) saturate(140%)',
      borderBottom: `1px solid ${T.rule}`,
    }}>
      <a href="#" style={{
        display: 'flex', alignItems: 'center', gap: 10,
        textDecoration: 'none', color: T.ink,
      }}>
        <PodMark size={22} color={T.ink} dotColor={accent}/>
        <span style={{
          fontFamily: '"Space Grotesk", sans-serif',
          fontSize: 17, fontWeight: 500, letterSpacing: '-0.01em',
        }}>Pod</span>
      </a>

      <div style={{ display: 'flex', alignItems: 'center', gap: 28 }}>
        {NAV_LINKS.map(([label, href]) => (
          <a key={label} href={href} style={{
            color: T.muted,
            textDecoration: 'none',
            fontSize: 13,
            transition: 'color 120ms ease',
          }}
          onMouseEnter={e => { e.currentTarget.style.color = T.ink; }}
          onMouseLeave={e => { e.currentTarget.style.color = T.muted; }}
          >{label}</a>
        ))}

        <div style={{
          display: 'inline-flex', alignItems: 'center', gap: 8,
          padding: '4px 8px 4px 10px',
          border: `1px solid ${T.rule}`, borderRadius: 999,
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 10, letterSpacing: '0.14em', color: T.muted,
        }}>
          <span style={{
            width: 6, height: 6, borderRadius: '50%',
            background: release.channel !== 'stable' ? T.warning : T.success,
            boxShadow: release.channel !== 'stable' ? `0 0 6px ${T.warning}` : `0 0 6px ${T.success}`,
          }}/>
          <span>v{release.version}</span>
          <span style={{ color: T.veryMuted }}>·</span>
          <span style={{ color: T.veryMuted }}>{release.channel}</span>
        </div>
      </div>
    </nav>
  );
}
