import { useState } from 'react';
import { T } from '../tokens';
import { SectionHead } from './SectionHead';

// Plain-text projection of the Q&A used for the FAQPage JSON-LD emitted into
// the prerendered landing page. Kept alongside the rich JSX `ENTRIES` so the
// two stay in lockstep; the JSX renders for humans, this feeds search crawlers.
export interface FaqItem {
  question: string;
  answer: string;
}

export const FAQ_ITEMS: FaqItem[] = [
  {
    question: "The app says it's damaged on macOS",
    answer:
      'macOS Gatekeeper quarantines apps downloaded outside the Mac App Store. ' +
      'The simplest fix is through System Settings: 1. Try to open Pod once (it ' +
      'will be blocked). 2. Open System Settings → Privacy & Security. 3. Scroll ' +
      'to the Security section near the bottom and click Open Anyway next to Pod. ' +
      '4. Confirm by clicking Open. On macOS Sequoia (15) and later the older ' +
      'right-click → Open trick no longer works, so use the steps above. ' +
      'Alternatively, you can clear the quarantine attribute from Terminal: ' +
      'xattr -dr com.apple.quarantine /Applications/Pod.app. Either way, you only ' +
      'need to do this once.',
  },
  {
    question: 'Windows blocked the app / SmartScreen warning',
    answer:
      'Windows SmartScreen blocks apps from unrecognised publishers. Click More ' +
      'info then Run anyway to proceed. The prompt appears once per install.',
  },
  {
    question: 'Smart App Control blocked the app (no “Run anyway”)',
    answer:
      'Some Windows 11 PCs run Smart App Control, a stricter layer than ' +
      'SmartScreen that hard-blocks apps from unverified publishers with no ' +
      'per-app override. If you only see Okay and Get apps from the Store, ' +
      "you'll need to turn Smart App Control off: 1. Open Windows Security. " +
      '2. Go to App & browser control → Smart App Control settings. 3. Set it to ' +
      'Off. 4. Re-run the installer (you may then see the normal SmartScreen ' +
      'prompt — click More info → Run anyway). On current Windows 11 builds you ' +
      'can switch Smart App Control back on later from the same screen; older ' +
      'builds required reinstalling Windows to re-enable it. Note that there is ' +
      'no per-app exception — the file Properties “Unblock” checkbox only affects ' +
      'SmartScreen, not Smart App Control. Only turn it off if you are ' +
      'comfortable running unsigned software you trust.',
  },
  {
    question: 'Why does Pod request so many EVE Online permissions?',
    answer:
      'Every permission Pod requests is tied directly to a feature you can see ' +
      "and control. We never ask for access we don't use. If you'd like to limit " +
      'what Pod can access, open Settings before signing in and disable any ' +
      'features you don’t need. Pod will only request permissions for the ' +
      'features that are turned on — so you stay in control of what you share.',
  },
  {
    question: 'Does Pod store my data anywhere?',
    answer:
      'Pod stores all character data in a local SQLite database on your machine. ' +
      'Nothing is sent to Pod servers — there are no Pod servers. The only ' +
      'outbound connection is to esi.evetech.net (CCP’s official API) to sync ' +
      'your character data, with your explicit consent.',
  },
  {
    question: 'Is this built by AI?',
    answer:
      'Pod is built by a software engineer by trade. AI tools are used to augment ' +
      'the development workflow — things like code completion, review, and ' +
      'research — but every decision, every line of code, and every release is ' +
      'subject to human oversight.',
  },
];

const ENTRIES: { question: string; answer: React.ReactNode }[] = [
  {
    question: "The app says it's damaged on macOS",
    answer: (
      <>
        macOS Gatekeeper quarantines apps downloaded outside the Mac App
        Store. The simplest fix is through System Settings:
        <br/><br/>
        1. Try to open Pod once (it will be blocked)<br/>
        2. Open <strong>System Settings</strong> →{' '}
        <strong>Privacy &amp; Security</strong><br/>
        3. Scroll to the <strong>Security</strong> section near the bottom and
        click <strong>Open Anyway</strong> next to Pod<br/>
        4. Confirm by clicking <strong>Open</strong>
        <br/><br/>
        On macOS Sequoia (15) and later the older right-click → Open trick no
        longer works, so use the steps above. Alternatively, you can clear the
        quarantine attribute from Terminal:
        <br/><br/>
        <code style={{
          display: 'block',
          fontFamily: '"JetBrains Mono", ui-monospace, monospace',
          fontSize: 13,
          background: T.paperSunk,
          border: `1px solid ${T.rule}`,
          borderRadius: 6,
          padding: '10px 14px',
          color: T.ink,
          letterSpacing: '0.02em',
        }}>
          xattr -dr com.apple.quarantine /Applications/Pod.app
        </code>
        <br/>
        Either way, you only need to do this once.
      </>
    ),
  },
  {
    question: "Windows blocked the app / SmartScreen warning",
    answer: (
      <>
        Windows SmartScreen blocks apps from unrecognised publishers. Click{' '}
        <strong>More info</strong> then <strong>Run anyway</strong> to
        proceed. The prompt appears once per install.
      </>
    ),
  },
  {
    question: "Smart App Control blocked the app (no “Run anyway”)",
    answer: (
      <>
        Some Windows 11 PCs run <strong>Smart App Control</strong>, a stricter
        layer than SmartScreen that hard-blocks apps from unverified publishers
        with no per-app override. If you only see <strong>Okay</strong> and{' '}
        <strong>Get apps from the Store</strong>, you&apos;ll need to turn Smart
        App Control off:
        <br/><br/>
        1. Open <strong>Windows Security</strong><br/>
        2. Go to <strong>App &amp; browser control</strong> →{' '}
        <strong>Smart App Control settings</strong><br/>
        3. Set it to <strong>Off</strong><br/>
        4. Re-run the installer (you may then see the normal SmartScreen prompt
        — click <strong>More info</strong> → <strong>Run anyway</strong>)
        <br/><br/>
        On current Windows 11 builds you can switch Smart App Control back on
        later from the same screen; older builds required reinstalling Windows
        to re-enable it. Note that there&apos;s no per-app exception — the file
        Properties &ldquo;Unblock&rdquo; checkbox only affects SmartScreen, not
        Smart App Control. Only turn it off if you&apos;re comfortable running
        unsigned software you trust.
      </>
    ),
  },
  {
    question: "Why does Pod request so many EVE Online permissions?",
    answer: (
      <>
        Every permission Pod requests is tied directly to a feature you can
        see and control. We never ask for access we don&apos;t use.
        <br/><br/>
        If you&apos;d like to limit what Pod can access, open{' '}
        <strong>Settings</strong> before signing in and disable any features
        you don&apos;t need. Pod will only request permissions for the
        features that are turned on — so you stay in control of what you
        share.
      </>
    ),
  },
  {
    question: "Does Pod store my data anywhere?",
    answer: (
      <>
        Pod stores all character data in a local SQLite database on your
        machine. Nothing is sent to Pod servers — there are no Pod servers.
        The only outbound connection is to{' '}
        <strong>esi.evetech.net</strong> (CCP&apos;s official API) to sync
        your character data, with your explicit consent.
      </>
    ),
  },
  {
    question: "Is this built by AI?",
    answer: (
      <>
        Pod is built by a software engineer by trade. AI tools are used to
        augment the development workflow — things like code completion,
        review, and research — but every decision, every line of code, and
        every release is subject to human oversight.
      </>
    ),
  },
];

function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg
      width={18}
      height={18}
      viewBox="0 0 24 24"
      fill="none"
      style={{
        display: 'block',
        flexShrink: 0,
        transition: 'transform 220ms ease',
        transform: open ? 'rotate(180deg)' : 'rotate(0deg)',
        color: T.muted,
      }}
    >
      <path
        d="M 6 9 L 12 15 L 18 9"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function FaqEntry({ question, answer }: { question: string; answer: React.ReactNode }) {
  const [open, setOpen] = useState(false);

  return (
    <div style={{ borderBottom: `1px solid ${T.rule}` }}>
      <button
        onClick={() => setOpen((v: boolean) => !v)}
        style={{
          width: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 16,
          padding: '22px 0',
          background: 'transparent',
          border: 0,
          cursor: 'pointer',
          textAlign: 'left',
          color: T.ink,
          fontFamily: '"Space Grotesk", sans-serif',
          fontSize: 17,
          fontWeight: 500,
          letterSpacing: '-0.01em',
          lineHeight: 1.3,
        }}
        aria-expanded={open}
      >
        <span>{question}</span>
        <ChevronIcon open={open}/>
      </button>

      <div style={{
        overflow: 'hidden',
        maxHeight: open ? 720 : 0,
        opacity: open ? 1 : 0,
        transition: 'max-height 280ms ease, opacity 220ms ease',
      }}>
        <div style={{
          paddingBottom: 22,
          fontFamily: '"Space Grotesk", sans-serif',
          fontSize: 15,
          lineHeight: 1.65,
          color: T.muted,
          maxWidth: 680,
        }}>
          {answer}
        </div>
      </div>
    </div>
  );
}

export function Faq() {
  return (
    <section id="faq" style={{
      background: T.paperSunk,
      borderBottom: `1px solid ${T.rule}`,
      padding: '88px 40px',
    }}>
      <div style={{ maxWidth: 1320, margin: '0 auto' }}>
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1.6fr)',
          gap: 64,
        }}>
          <div>
            <SectionHead
              eyebrow="FAQ"
              title={<>Common<br/>questions.</>}
              line="Quick answers to things that come up often."
            />
          </div>

          <div style={{ borderTop: `1px solid ${T.rule}` }}>
            {ENTRIES.map((e, i) => (
              <FaqEntry key={i} question={e.question} answer={e.answer}/>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
