const ICONS = {
  'alert-note': <g>
    <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2"/>
    <circle cx="12" cy="8" r="0.9" fill="currentColor"/>
    <line x1="12" y1="11" x2="12" y2="16.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
  </g>,
  'alert-tip': <g>
    <path d="M 12 3 A 6 6 0 0 1 16 13.5 C 15.2 14.3 15 15 15 16 L 9 16 C 9 15 8.8 14.3 8 13.5 A 6 6 0 0 1 12 3 Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <line x1="9.5" y1="19" x2="14.5" y2="19" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <line x1="10.5" y1="21.5" x2="13.5" y2="21.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
  </g>,
  'alert-important': <g>
    <path d="M 5.5 3.5 L 18.5 3.5 A 1.5 1.5 0 0 1 20 5 L 20 14 A 1.5 1.5 0 0 1 18.5 15.5 L 11 15.5 L 7 19.5 L 7 15.5 L 5.5 15.5 A 1.5 1.5 0 0 1 4 14 L 4 5 A 1.5 1.5 0 0 1 5.5 3.5 Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <line x1="12" y1="6.5" x2="12" y2="10" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <circle cx="12" cy="12.4" r="0.9" fill="currentColor"/>
  </g>,
  'alert-warning': <g>
    <path d="M 12 3 L 22 20 L 2 20 Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <line x1="12" y1="10" x2="12" y2="14" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <circle cx="12" cy="17" r="0.9" fill="currentColor"/>
  </g>,
  'alert-caution': <g>
    <path d="M 8.7 3 L 15.3 3 L 21 8.7 L 21 15.3 L 15.3 21 L 8.7 21 L 3 15.3 L 3 8.7 Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round"/>
    <line x1="12" y1="8" x2="12" y2="13" stroke="currentColor" strokeWidth="2" strokeLinecap="round"/>
    <circle cx="12" cy="16" r="0.9" fill="currentColor"/>
  </g>,
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
  roster: <g>
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
  github: <g transform="scale(0.0375)">
    <path d="M280.5 426.5C214.5 418.5 168 371 168 309.5C168 284.5 177 257.5 192 239.5C185.5 223 186.5 188 194 173.5C214 171 241 181.5 257 196C276 190 296 187 320.5 187C345 187 365 190 383 195.5C398.5 181.5 426 171 446 173.5C453 187 454 222 447.5 239C463.5 258 472 283.5 472 309.5C472 371 425.5 417.5 358.5 426C375.5 437 387 461 387 488.5L387 540.5C387 555.5 399.5 564 414.5 558C505 523.5 576 433 576 321C576 179.5 461 64 319.5 64C178 64 64 179.5 64 321C64 432 134.5 524 229.5 558.5C243 563.5 256 554.5 256 541L256 501C249 504 240 506 232 506C199 506 179.5 488 165.5 454.5C160 441 154 433 142.5 431.5C136.5 431 134.5 428.5 134.5 425.5C134.5 419.5 144.5 415 154.5 415C169 415 181.5 424 194.5 442.5C204.5 457 215 463.5 227.5 463.5C240 463.5 248 459 259.5 447.5C268 439 274.5 431.5 280.5 426.5z" fill="currentColor"/>
  </g>,
  discord: <g transform="scale(0.0375)">
    <path d="M524.5 133.8C524.3 133.5 524.1 133.2 523.7 133.1C485.6 115.6 445.3 103.1 404 96C403.6 95.9 403.2 96 402.9 96.1C402.6 96.2 402.3 96.5 402.1 96.9C396.6 106.8 391.6 117.1 387.2 127.5C342.6 120.7 297.3 120.7 252.8 127.5C248.3 117 243.3 106.8 237.7 96.9C237.5 96.6 237.2 96.3 236.9 96.1C236.6 95.9 236.2 95.9 235.8 95.9C194.5 103 154.2 115.5 116.1 133C115.8 133.1 115.5 133.4 115.3 133.7C39.1 247.5 18.2 358.6 28.4 468.2C28.4 468.5 28.5 468.7 28.6 469C28.7 469.3 28.9 469.4 29.1 469.6C73.5 502.5 123.1 527.6 175.9 543.8C176.3 543.9 176.7 543.9 177 543.8C177.3 543.7 177.7 543.4 177.9 543.1C189.2 527.7 199.3 511.3 207.9 494.3C208 494.1 208.1 493.8 208.1 493.5C208.1 493.2 208.1 493 208 492.7C207.9 492.4 207.8 492.2 207.6 492.1C207.4 492 207.2 491.8 206.9 491.7C191.1 485.6 175.7 478.3 161 469.8C160.7 469.6 160.5 469.4 160.3 469.2C160.1 469 160 468.6 160 468.3C160 468 160 467.7 160.2 467.4C160.4 467.1 160.5 466.9 160.8 466.7C163.9 464.4 167 462 169.9 459.6C170.2 459.4 170.5 459.2 170.8 459.2C171.1 459.2 171.5 459.2 171.8 459.3C268 503.2 372.2 503.2 467.3 459.3C467.6 459.2 468 459.1 468.3 459.1C468.6 459.1 469 459.3 469.2 459.5C472.1 461.9 475.2 464.4 478.3 466.7C478.5 466.9 478.7 467.1 478.9 467.4C479.1 467.7 479.1 468 479.1 468.3C479.1 468.6 479 468.9 478.8 469.2C478.6 469.5 478.4 469.7 478.2 469.8C463.5 478.4 448.2 485.7 432.3 491.6C432.1 491.7 431.8 491.8 431.6 492C431.4 492.2 431.3 492.4 431.2 492.7C431.1 493 431.1 493.2 431.1 493.5C431.1 493.8 431.2 494 431.3 494.3C440.1 511.3 450.1 527.6 461.3 543.1C461.5 543.4 461.9 543.7 462.2 543.8C462.5 543.9 463 543.9 463.3 543.8C516.2 527.6 565.9 502.5 610.4 469.6C610.6 469.4 610.8 469.2 610.9 469C611 468.8 611.1 468.5 611.1 468.2C623.4 341.4 590.6 231.3 524.2 133.7zM222.5 401.5C193.5 401.5 169.7 374.9 169.7 342.3C169.7 309.7 193.1 283.1 222.5 283.1C252.2 283.1 275.8 309.9 275.3 342.3C275.3 375 251.9 401.5 222.5 401.5zM417.9 401.5C388.9 401.5 365.1 374.9 365.1 342.3C365.1 309.7 388.5 283.1 417.9 283.1C447.6 283.1 471.2 309.9 470.7 342.3C470.7 375 447.5 401.5 417.9 401.5z" fill="currentColor"/>
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
