import { useMemo } from 'react';

interface SparkChartProps {
  accent: string;
}

export function SparkChart({ accent }: SparkChartProps) {
  const pts = useMemo(() => {
    const N = 40;
    const out: number[] = [];
    let y = 0.55;
    let seed = 0.123;
    for (let i = 0; i < N; i++) {
      seed = (seed * 9301 + 49297) % 233280 / 233280;
      const drift = (i / (N - 1)) * 0.18;
      const noise = (seed - 0.5) * 0.08;
      y = Math.max(0.15, Math.min(0.95, y + noise + drift / N));
      out.push(y);
    }
    return out;
  }, []);

  const W = 100, H = 100;
  const path = pts.map((v, i) => {
    const x = (i / (pts.length - 1)) * W;
    const yy = H - v * H * 0.85 - H * 0.05;
    return `${i === 0 ? 'M' : 'L'} ${x.toFixed(2)} ${yy.toFixed(2)}`;
  }).join(' ');
  const area = path + ` L ${W} ${H} L 0 ${H} Z`;

  return (
    <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none"
      style={{ display: 'block', width: '100%', height: 96 }}>
      <defs>
        <linearGradient id="sparkGrad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={accent} stopOpacity="0.35"/>
          <stop offset="100%" stopColor={accent} stopOpacity="0"/>
        </linearGradient>
      </defs>
      <path d={area} fill="url(#sparkGrad)"/>
      <path d={path} fill="none" stroke={accent} strokeWidth="1.2" vectorEffect="non-scaling-stroke"/>
    </svg>
  );
}
