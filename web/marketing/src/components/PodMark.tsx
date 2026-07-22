import { T } from '../tokens';

interface PodMarkProps {
  size?: number;
  color?: string;
  dotColor?: string;
}

export function PodMark({ size = 28, color = T.railFg, dotColor }: PodMarkProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 200 200" style={{ display: 'block' }}>
      <line x1="42" y1="100" x2="42" y2="192" stroke={color} strokeWidth="18" strokeLinecap="round"/>
      <line x1="158" y1="8" x2="158" y2="100" stroke={color} strokeWidth="18" strokeLinecap="round"/>
      <circle cx="100" cy="100" r="58" fill="none" stroke={color} strokeWidth="18"/>
      <circle cx="100" cy="100" r="10" fill={dotColor ?? T.plasma}/>
    </svg>
  );
}
