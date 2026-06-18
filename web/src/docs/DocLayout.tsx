import { T } from '../tokens';
import type { DocPage } from './content';

export interface DocLayoutProps {
  current: DocPage;
  pages: DocPage[];
}

export function DocLayout({ current, pages }: DocLayoutProps) {
  return (
    <div
      style={{
        background: T.paper,
        color: T.ink,
        minHeight: '100dvh',
        fontFamily: "'Space Grotesk', system-ui, sans-serif",
      }}
    >
      <header style={{ padding: '18px 40px', borderBottom: `1px solid ${T.rule}` }}>
        <a href="/" style={{ color: T.ink, textDecoration: 'none', fontSize: 17, fontWeight: 500 }}>
          Pod
        </a>
        <span style={{ color: T.veryMuted, margin: '0 10px' }}>/</span>
        <a href="/docs/" style={{ color: T.muted, textDecoration: 'none', fontSize: 17 }}>
          Docs
        </a>
      </header>

      <div style={{ display: 'flex', maxWidth: 1320, margin: '0 auto' }}>
        <nav style={{ width: 240, flexShrink: 0, padding: '32px 24px', borderRight: `1px solid ${T.rule}` }}>
          {sectionsOf(pages).map((section) => (
            <div key={section} style={{ marginBottom: 24 }}>
              <div
                style={{
                  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                  fontSize: 9,
                  letterSpacing: '0.28em',
                  textTransform: 'uppercase',
                  color: T.muted,
                  marginBottom: 10,
                }}
              >
                {section}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {pages
                  .filter((page) => page.frontmatter.section === section)
                  .map((page) => (
                    <a
                      key={page.slug}
                      href={page.url}
                      aria-current={page.slug === current.slug ? 'page' : undefined}
                      style={{
                        color: page.slug === current.slug ? T.plasma : T.muted,
                        textDecoration: 'none',
                        fontSize: 13,
                      }}
                    >
                      {page.frontmatter.title}
                    </a>
                  ))}
              </div>
            </div>
          ))}
        </nav>

        <main style={{ flex: 1, minWidth: 0, padding: '32px 40px', lineHeight: 1.6 }}>
          <article dangerouslySetInnerHTML={{ __html: current.html }} />
        </main>
      </div>
    </div>
  );
}

function sectionsOf(pages: DocPage[]): string[] {
  const seen: string[] = [];
  for (const page of pages) {
    if (!seen.includes(page.frontmatter.section)) {
      seen.push(page.frontmatter.section);
    }
  }
  return seen;
}
