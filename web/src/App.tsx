import { useEffect, useState } from 'react';
import { T } from './tokens';
import { PLATFORMS } from './data';
import { RELEASE, PLATFORM_BUILDS } from './generated/release';
import { detectOS, type OsInfo } from './utils/detectOs';

// Deterministic OS used during the build-time prerender AND the first client
// render, so the server-rendered markup matches what React produces on initial
// hydration (no mismatch). Windows is the most common visitor platform; the
// real OS is resolved from `navigator` after mount via `useEffect` below.
const DEFAULT_OS: OsInfo = { id: 'windows', label: 'Windows', buildId: 'win-x64-exe' };
import { TopNav } from './components/TopNav';
import { Hero } from './components/Hero';
import { Platforms } from './components/Platforms';
import { Features } from './components/Features';
import { Notes } from './components/Notes';
import { Faq } from './components/Faq';
import { Support } from './components/Support';
import { Requirements } from './components/Requirements';
import { Footer } from './components/Footer';

const ACCENT = T.plasma;

export function App() {
  const [archChoice, setArchChoice] = useState('mac-arm');
  const [os, setOs] = useState<OsInfo>(DEFAULT_OS);

  // Resolve the real OS only after mount. The first client render uses
  // DEFAULT_OS to match the prerendered HTML; this effect then swaps in the
  // detected platform once the DOM has hydrated.
  useEffect(() => {
    setOs(detectOS());
  }, []);

  useEffect(() => {
    const id = window.location.hash.slice(1);
    if (!id) return;

    // Re-apply the hash after mount: the browser's initial anchor scroll runs
    // before React injects the sections, so defer one frame until they exist.
    requestAnimationFrame(() => {
      document.getElementById(id)?.scrollIntoView({ behavior: 'smooth' });
    });
  }, []);

  const platforms = PLATFORM_BUILDS.length > 0 ? PLATFORM_BUILDS : PLATFORMS;

  return (
    <div style={{
      background: T.paper, color: T.ink, minHeight: '100dvh',
      fontFamily: "'Space Grotesk', system-ui, sans-serif",
    }}>
      <TopNav accent={ACCENT} release={RELEASE} />
      <Hero
        os={os}
        layout="split"
        accent={ACCENT}
        archChoice={archChoice}
        onChangeArch={setArchChoice}
        release={RELEASE}
        platforms={platforms}
      />
      <Platforms platforms={platforms} accent={ACCENT} release={RELEASE} />
      <Features accent={ACCENT} />
      <Notes accent={ACCENT} release={RELEASE} />
      <Faq />
      <Support accent={ACCENT} />
      <Requirements accent={ACCENT} />
      <Footer release={RELEASE} />
    </div>
  );
}
