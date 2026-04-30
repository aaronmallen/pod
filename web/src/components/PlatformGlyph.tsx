import { T } from '../tokens';

interface Props {
  id: string;
  accent: string;
}

export function PlatformGlyph({ id, accent }: Props) {
  const stroke = T.muted;
  if (id === 'macos') return (
    <svg width="36" height="36" viewBox="0 0 36 36" fill="none">
      <rect x="3" y="6" width="30" height="20" rx="3" stroke={stroke} strokeWidth="1.6"/>
      <line x1="3" y1="22" x2="33" y2="22" stroke={stroke} strokeWidth="1.6"/>
      <rect x="12" y="28" width="12" height="2" rx="1" fill={stroke}/>
      <circle cx="18" cy="15" r="2" fill={accent}/>
    </svg>
  );
  if (id === 'windows') return (
    <svg width="36" height="36" viewBox="0 0 36 36" fill="none">
      <rect x="6"  y="6"  width="11" height="11" stroke={stroke} strokeWidth="1.6"/>
      <rect x="19" y="6"  width="11" height="11" stroke={stroke} strokeWidth="1.6"/>
      <rect x="6"  y="19" width="11" height="11" stroke={stroke} strokeWidth="1.6"/>
      <rect x="19" y="19" width="11" height="11" stroke={accent} strokeWidth="1.6"/>
    </svg>
  );
  return (
    <svg width="36" height="36" viewBox="0 0 36 36" fill="none">
      <rect x="4" y="6" width="28" height="24" rx="3" stroke={stroke} strokeWidth="1.6"/>
      <path d="M 10 14 L 14 18 L 10 22" stroke={accent} strokeWidth="1.8" fill="none" strokeLinecap="round" strokeLinejoin="round"/>
      <line x1="17" y1="22" x2="24" y2="22" stroke={stroke} strokeWidth="1.6" strokeLinecap="round"/>
    </svg>
  );
}
