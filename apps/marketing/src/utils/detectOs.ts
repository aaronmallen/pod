export interface OsInfo {
  id: 'macos' | 'windows' | 'linux';
  label: string;
  buildId: string;
}

function pickMac(_ua: string): OsInfo {
  return { id: 'macos', label: 'macOS', buildId: 'mac-arm' };
}

export function detectOS(): OsInfo {
  if (typeof navigator === 'undefined') {
    return { id: 'macos', label: 'macOS', buildId: 'mac-arm' };
  }
  const ua = (navigator.userAgent || '').toLowerCase();
  const platform = (navigator.platform || '').toLowerCase();
  const uaData = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData;
  if (uaData && uaData.platform) {
    const p = uaData.platform.toLowerCase();
    if (p.includes('mac'))     return pickMac(ua);
    if (p.includes('windows')) return { id: 'windows', label: 'Windows', buildId: 'win-x64-exe' };
    if (p.includes('linux'))   return { id: 'linux',   label: 'Linux',   buildId: 'lin-app' };
  }
  if (ua.includes('mac') || platform.includes('mac'))     return pickMac(ua);
  if (ua.includes('windows') || platform.includes('win')) return { id: 'windows', label: 'Windows', buildId: 'win-x64-exe' };
  if (ua.includes('linux') || platform.includes('linux')) return { id: 'linux',   label: 'Linux',   buildId: 'lin-app' };
  return { id: 'macos', label: 'macOS', buildId: 'mac-arm' };
}
