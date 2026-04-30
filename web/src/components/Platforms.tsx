import { T } from '../tokens';
import type { Platform, Release } from '../types';
import { SectionHead } from './SectionHead';
import { PlatformCard } from './PlatformCard';

interface Props {
  platforms: Platform[];
  accent: string;
  release: Release;
}

export function Platforms({ platforms, accent, release }: Props) {
  return (
    <section id="download" style={{
      borderBottom: `1px solid ${T.rule}`,
      padding: '88px 40px',
    }}>
      <div style={{ maxWidth: 1320, margin: '0 auto' }}>
        <SectionHead
          eyebrow="Downloads"
          title="Available everywhere you fly."
          line={`Signed and notarized. Release ${release.version} · ${release.date}.`}
        />

        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
          gap: 16, marginTop: 48,
        }}>
          {platforms.map(p => <PlatformCard key={p.id} platform={p} accent={accent}/>)}
        </div>

      </div>
    </section>
  );
}
