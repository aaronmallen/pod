import { useState } from 'react';
import { T } from '../tokens';
import { SectionHead } from './SectionHead';

const ENTRIES: { question: string; answer: React.ReactNode }[] = [
  {
    question: "The app says it's damaged on macOS",
    answer: (
      <>
        macOS Gatekeeper quarantines apps downloaded outside the Mac App
        Store. To remove the quarantine attribute, open Terminal and run:
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
        Then try launching Pod again. You only need to do this once.
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
        maxHeight: open ? 480 : 0,
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
