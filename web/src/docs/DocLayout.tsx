import { Footer } from '../components/Footer';
import { TopNav } from '../components/TopNav';
import { RELEASE } from '../generated/release';
import { T } from '../tokens';
import type { DocPage } from './content';

const ACCENT = T.plasma;

const ARTICLE_CSS = `
.doc-article {
  font-family: 'Space Grotesk', system-ui, sans-serif;
  font-size: 16px;
  line-height: 1.7;
  color: ${T.ink};
}
.doc-article > :first-child { margin-top: 0; }
.doc-article > :last-child { margin-bottom: 0; }
.doc-article h1 {
  font-family: 'Space Grotesk', sans-serif;
  font-size: clamp(30px, 3.4vw, 42px);
  font-weight: 500;
  letter-spacing: -0.02em;
  line-height: 1.08;
  margin: 0 0 24px;
  text-wrap: balance;
}
.doc-article h2 {
  font-family: 'Space Grotesk', sans-serif;
  font-size: 24px;
  font-weight: 500;
  letter-spacing: -0.015em;
  line-height: 1.2;
  margin: 48px 0 16px;
  padding-top: 28px;
  border-top: 1px solid ${T.rule};
}
.doc-article h3 {
  font-size: 18px;
  font-weight: 500;
  letter-spacing: -0.01em;
  margin: 32px 0 12px;
}
.doc-article h4 {
  font-size: 15px;
  font-weight: 500;
  margin: 24px 0 10px;
}
.doc-article p { margin: 0 0 18px; color: rgba(244,242,236,0.82); }
.doc-article a {
  color: ${T.plasma};
  text-decoration: none;
  border-bottom: 1px solid ${T.plasmaSoft};
}
.doc-article a:hover { border-bottom-color: ${T.plasma}; }
.doc-article strong { color: ${T.ink}; font-weight: 700; }
.doc-article em { font-style: italic; }
.doc-article ul, .doc-article ol {
  margin: 0 0 18px;
  padding-left: 24px;
  color: rgba(244,242,236,0.82);
}
.doc-article li { margin: 6px 0; }
.doc-article li::marker { color: ${T.veryMuted}; }
.doc-article code {
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  font-size: 0.86em;
  background: ${T.paperSunk};
  border: 1px solid ${T.rule};
  border-radius: 5px;
  padding: 2px 6px;
  color: ${T.ink};
}
.doc-article pre {
  background: ${T.paperSunk};
  border: 1px solid ${T.rule};
  border-radius: 12px;
  padding: 18px 20px;
  margin: 0 0 24px;
  overflow-x: auto;
}
.doc-article pre code {
  background: none;
  border: none;
  border-radius: 0;
  padding: 0;
  font-size: 13px;
  line-height: 1.6;
  color: rgba(244,242,236,0.9);
}
.doc-article blockquote {
  margin: 0 0 24px;
  padding: 4px 20px;
  border-left: 3px solid ${T.plasma};
  background: ${T.plasmaSoft};
  border-radius: 0 8px 8px 0;
  color: ${T.muted};
}
.doc-article blockquote p:last-child { margin-bottom: 0; }
.doc-article img {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 28px 0;
  border: 1px solid ${T.rule};
  border-radius: 12px;
  background: ${T.paperSunk};
}
.doc-article hr {
  border: none;
  border-top: 1px solid ${T.rule};
  margin: 40px 0;
}
.doc-article table {
  width: 100%;
  border-collapse: collapse;
  margin: 0 0 24px;
  font-size: 14px;
}
.doc-article th, .doc-article td {
  text-align: left;
  padding: 10px 14px;
  border-bottom: 1px solid ${T.rule};
}
.doc-article th {
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  font-size: 10px;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: ${T.muted};
}
@media (max-width: 900px) {
  .doc-shell { flex-direction: column; }
  .doc-sidebar {
    width: auto;
    border-right: none;
    border-bottom: 1px solid ${T.rule};
  }
}
`;

export interface DocLayoutProps {
  current: DocPage;
  pages: DocPage[];
}

interface DocSection {
  pages: DocPage[];
  section: string;
}

export function DocLayout({ current, pages }: DocLayoutProps) {
  return (
    <div
      style={{
        background: T.paper,
        color: T.ink,
        minHeight: '100dvh',
        display: 'flex',
        flexDirection: 'column',
        fontFamily: "'Space Grotesk', system-ui, sans-serif",
      }}
    >
      <style dangerouslySetInnerHTML={{ __html: ARTICLE_CSS }} />

      <TopNav accent={ACCENT} basePath="/" release={RELEASE} />

      <div
        className="doc-shell"
        style={{ display: 'flex', flex: 1, width: '100%', maxWidth: 1320, margin: '0 auto' }}
      >
        <nav
          className="doc-sidebar"
          style={{
            width: 256,
            flexShrink: 0,
            padding: '48px 24px',
            borderRight: `1px solid ${T.rule}`,
          }}
        >
          {sectionsOf(pages).map(({ pages: sectionPages, section }) => (
            <div key={section} style={{ marginBottom: 28 }}>
              <div
                style={{
                  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
                  fontSize: 9,
                  letterSpacing: '0.28em',
                  textTransform: 'uppercase',
                  color: T.plasma,
                  marginBottom: 14,
                }}
              >
                {section}
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                {sectionPages.map((page) => {
                  const active = page.slug === current.slug;
                  return (
                    <a
                      key={page.slug}
                      href={page.url}
                      aria-current={active ? 'page' : undefined}
                      style={{
                        display: 'block',
                        padding: '6px 12px',
                        marginLeft: -12,
                        borderRadius: 8,
                        fontSize: 14,
                        textDecoration: 'none',
                        color: active ? T.ink : T.muted,
                        background: active ? T.plasmaSoft : 'transparent',
                        boxShadow: active ? `inset 2px 0 0 ${T.plasma}` : 'none',
                        fontWeight: active ? 500 : 400,
                      }}
                    >
                      {page.frontmatter.title}
                    </a>
                  );
                })}
              </div>
            </div>
          ))}
        </nav>

        <main style={{ flex: 1, minWidth: 0, padding: '48px 56px' }}>
          <article
            className="doc-article"
            style={{ maxWidth: 760 }}
            dangerouslySetInnerHTML={{ __html: current.html }}
          />
        </main>
      </div>

      <Footer basePath="/" release={RELEASE} />
    </div>
  );
}

function sectionsOf(pages: DocPage[]): DocSection[] {
  const sections: DocSection[] = [];
  for (const page of pages) {
    const existing = sections.find((entry) => entry.section === page.frontmatter.section);
    if (existing) {
      existing.pages.push(page);
    } else {
      sections.push({ pages: [page], section: page.frontmatter.section });
    }
  }
  return sections;
}
