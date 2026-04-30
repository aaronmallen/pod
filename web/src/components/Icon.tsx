const ICONS = {
  fitting: <g>
    <path d="M 10.437 20.863 A 9 9 0 0 1 3.543 8.922" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 5.106 6.215 A 9 9 0 0 1 18.894 6.215" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 20.457 8.922 A 9 9 0 0 1 13.563 20.863" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 12 8.6 L 14.94 13.7 L 9.06 13.7 Z" fill="currentColor"/>
  </g>,
  mail: <g>
    <rect x="3" y="6" width="18" height="13" rx="1.5" fill="none" stroke="currentColor" strokeWidth="2"/>
    <path d="M 4 7.5 L 12 13 L 20 7.5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
  </g>,
  assets: <g>
    <path d="M 12 4 L 20 8 L 20 16 L 12 20 L 4 16 L 4 8 Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <path d="M 4 8 L 12 12 L 20 8" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <line x1="12" y1="12" x2="12" y2="20" stroke="currentColor" strokeWidth="2"/>
  </g>,
  characters: <g>
    <circle cx="12" cy="8" r="4" fill="none" stroke="currentColor" strokeWidth="2"/>
    <path d="M 4 20 C 5 15.5, 19 15.5, 20 20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
  </g>,
  wallet: <g>
    <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2"/>
    <line x1="8" y1="7.5" x2="16" y2="7.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="16" y1="7.5" x2="8" y2="16.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="8" y1="16.5" x2="16" y2="16.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="6.5" y1="12" x2="17.5" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
  </g>,
  skills: <g>
    <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2"/>
    <path d="M 7 8 C 9 12, 15 12, 17 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 7 16 C 9 12, 15 12, 17 8" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <circle cx="12" cy="12" r="1.6" fill="currentColor"/>
  </g>,
  download: <g>
    <line x1="12" y1="3" x2="12" y2="15" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 6 10 L 12 16 L 18 10" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
    <line x1="4" y1="20" x2="20" y2="20" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
  </g>,
  github: <g>
    <path d="M 12 2 C 6.48 2 2 6.48 2 12 c 0 4.42 2.87 8.17 6.84 9.5 c 0.5 0.08 0.66 -0.23 0.66 -0.5 v -1.69 c -2.77 0.6 -3.36 -1.34 -3.36 -1.34 c -0.46 -1.16 -1.11 -1.47 -1.11 -1.47 c -0.91 -0.62 0.07 -0.6 0.07 -0.6 c 1 0.07 1.53 1.03 1.53 1.03 c 0.87 1.52 2.34 1.07 2.91 0.83 c 0.09 -0.65 0.35 -1.09 0.63 -1.34 c -2.22 -0.25 -4.55 -1.11 -4.55 -4.94 c 0 -1.1 0.39 -1.99 1.03 -2.69 c -0.1 -0.25 -0.45 -1.27 0.1 -2.65 c 0 0 0.84 -0.27 2.75 1.02 c 0.8 -0.22 1.65 -0.33 2.5 -0.33 c 0.85 0 1.7 0.11 2.5 0.33 c 1.91 -1.29 2.75 -1.02 2.75 -1.02 c 0.55 1.38 0.2 2.4 0.1 2.65 c 0.64 0.7 1.03 1.59 1.03 2.69 c 0 3.84 -2.34 4.68 -4.57 4.93 c 0.36 0.31 0.69 0.92 0.69 1.85 V 21 c 0 0.27 0.16 0.59 0.67 0.5 C 19.14 20.16 22 16.42 22 12 A 10 10 0 0 0 12 2 Z" fill="currentColor"/>
  </g>,
  arrow: <g>
    <line x1="5" y1="12" x2="19" y2="12" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <path d="M 13 6 L 19 12 L 13 18" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
  </g>,
  check: <g>
    <path d="M 5 12 L 10 17 L 19 7" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round"/>
  </g>,
} as const;

export type IconName = keyof typeof ICONS;

interface IconProps {
  name: IconName;
  size?: number;
}

export function Icon({ name, size = 22 }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" style={{ display: 'block' }}>
      {ICONS[name]}
    </svg>
  );
}
