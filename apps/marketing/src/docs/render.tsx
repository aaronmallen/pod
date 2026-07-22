import { renderToStaticMarkup } from 'react-dom/server';
import { DocLayout } from './DocLayout';
import type { DocPage } from './content';

const FONTS_HREF =
  'https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;700&family=JetBrains+Mono:wght@400;500&display=swap';

const SITE_NAME = 'Pod Docs';
const ORIGIN = 'https://pod.aaronmallen.dev';

export function renderDocPage(current: DocPage, pages: DocPage[]): string {
  const title = pageTitle(current);
  const description = pageDescription(current);
  const canonical = `${ORIGIN}${current.url}`;

  return [
    '<!doctype html>',
    '<html lang="en">',
    '<head>',
    '<meta charset="UTF-8" />',
    '<meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    '<link rel="icon" type="image/svg+xml" href="/favicon.svg" />',
    `<title>${escapeHtml(title)}</title>`,
    `<meta name="description" content="${escapeHtml(description)}" />`,
    `<link rel="canonical" href="${escapeHtml(canonical)}" />`,
    `<meta property="og:title" content="${escapeHtml(title)}" />`,
    `<meta property="og:description" content="${escapeHtml(description)}" />`,
    '<meta property="og:type" content="article" />',
    `<meta property="og:url" content="${escapeHtml(canonical)}" />`,
    `<meta property="og:site_name" content="${escapeHtml(SITE_NAME)}" />`,
    '<meta name="twitter:card" content="summary" />',
    `<meta name="twitter:title" content="${escapeHtml(title)}" />`,
    `<meta name="twitter:description" content="${escapeHtml(description)}" />`,
    structuredData(current, canonical, title, description),
    '<link rel="preconnect" href="https://fonts.googleapis.com" />',
    '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />',
    `<link href="${FONTS_HREF}" rel="stylesheet" />`,
    '<style>*,*::before,*::after{box-sizing:border-box;}html{scroll-behavior:smooth;}body{margin:0;padding:0;}</style>',
    '</head>',
    '<body>',
    renderToStaticMarkup(<DocLayout current={current} pages={pages} />),
    '</body>',
    '</html>',
    '',
  ].join('\n');
}

// The docs index already titles itself "Pod Docs"; appending the suffix there
// would double-render it as "Pod Docs · Pod Docs".
function pageTitle(page: DocPage): string {
  const { title } = page.frontmatter;
  return title === SITE_NAME ? title : `${title} · ${SITE_NAME}`;
}

// Prefer a hand-tuned frontmatter description; otherwise fall back to the first
// paragraph of the rendered page so every page still emits a meaningful summary.
function pageDescription(page: DocPage): string {
  const authored = page.frontmatter.description?.trim();
  if (authored) {
    return authored;
  }
  return firstParagraph(page.html);
}

export function firstParagraph(html: string): string {
  const match = html.match(/<p>([\s\S]*?)<\/p>/i);
  if (!match) {
    return '';
  }
  const text = stripMarkup(decodeEntities(match[1]))
    .replace(/\s+/g, ' ')
    .trim();
  return text.length > 200 ? `${text.slice(0, 197).trimEnd()}...` : text;
}

// Decode the HTML entities MarkdownIt emits, unescaping the escape character
// (&amp;) LAST so an already-escaped entity such as &amp;lt; decodes to the
// literal &lt; rather than collapsing into a bare <.
function decodeEntities(value: string): string {
  return value
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&');
}

// Remove every angle-bracketed tag and then any residual < or > so decoded
// markup (e.g. &lt;script&gt; -> <script>) cannot reappear in the plain-text
// output. Run this AFTER decodeEntities so nothing can re-form an element.
function stripMarkup(value: string): string {
  return value.replace(/<[^>]*>/g, '').replace(/[<>]/g, '');
}

// JSON.stringify does not escape <, >, or the </script> sequence, so embedding
// its output directly in a <script> block lets a </script> substring break out.
// Re-escape both angle brackets as their unicode escapes to seal the block.
export function escapeJsonLd(json: string): string {
  return json.replace(/</g, '\\u003c').replace(/>/g, '\\u003e');
}

// Emit TechArticle + BreadcrumbList JSON-LD so crawlers see the page as a
// documentation article located at Home -> Docs -> <page>.
export function structuredData(
  page: DocPage,
  canonical: string,
  title: string,
  description: string,
): string {
  const breadcrumbs: { name: string; item: string }[] = [
    { name: 'Home', item: `${ORIGIN}/` },
    { name: 'Docs', item: `${ORIGIN}/docs/` },
  ];

  // The docs index is the "Docs" crumb itself; deeper pages add their own crumb
  // under the page's section label.
  if (page.slug !== '') {
    breadcrumbs.push({ name: page.frontmatter.title, item: canonical });
  }

  const article = {
    '@context': 'https://schema.org',
    '@type': 'TechArticle',
    headline: page.frontmatter.title,
    name: title,
    description,
    url: canonical,
    inLanguage: 'en',
    articleSection: page.frontmatter.section,
    isPartOf: { '@type': 'WebSite', name: SITE_NAME, url: `${ORIGIN}/docs/` },
  };

  const breadcrumbList = {
    '@context': 'https://schema.org',
    '@type': 'BreadcrumbList',
    itemListElement: breadcrumbs.map((crumb, index) => ({
      '@type': 'ListItem',
      position: index + 1,
      name: crumb.name,
      item: crumb.item,
    })),
  };

  // JSON.stringify leaves <, >, and </script> unescaped, so each block's angle
  // brackets are re-escaped (escapeJsonLd) to keep a field value from breaking
  // out of the surrounding <script type="application/ld+json"> context.
  return [article, breadcrumbList]
    .map(
      (data) =>
        `<script type="application/ld+json">\n${escapeJsonLd(JSON.stringify(data, null, 2))}\n</script>`,
    )
    .join('\n');
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
