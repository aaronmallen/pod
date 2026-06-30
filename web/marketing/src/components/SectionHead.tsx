import { T } from '../tokens';

interface Props {
  eyebrow: string;
  title: React.ReactNode;
  line?: string;
}

export function SectionHead({ eyebrow, title, line }: Props) {
  return (
    <div>
      <div style={{
        fontFamily: '"JetBrains Mono", ui-monospace, monospace',
        fontSize: 10, letterSpacing: '0.28em', textTransform: 'uppercase',
        color: T.plasma, marginBottom: 18,
      }}>{eyebrow}</div>
      <h2 style={{
        margin: 0,
        fontFamily: '"Space Grotesk", sans-serif',
        fontSize: 'clamp(32px, 3.4vw, 44px)',
        fontWeight: 500, letterSpacing: '-0.02em', lineHeight: 1.05,
        color: T.ink, textWrap: 'balance',
      } as React.CSSProperties}>{title}</h2>
      {line && (
        <div style={{
          marginTop: 14, fontSize: 14, color: T.muted, lineHeight: 1.5,
          maxWidth: 520, textWrap: 'pretty',
        } as React.CSSProperties}>{line}</div>
      )}
    </div>
  );
}
