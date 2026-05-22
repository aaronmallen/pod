import { useState } from 'react';
import { T } from './tokens';
import { PLATFORMS } from './data';
import { RELEASE, PLATFORM_BUILDS } from './generated/release';
import { detectOS } from './utils/detectOs';
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
  const os = detectOS();

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
