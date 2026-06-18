import { renderToStaticMarkup } from 'react-dom/server';
import { DocLayout } from './DocLayout';
import type { DocPage } from './content';

const FONTS_HREF =
  'https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;700&family=JetBrains+Mono:wght@400;500&display=swap';

export function renderDocPage(current: DocPage, pages: DocPage[]): string {
  const body = renderToStaticMarkup(<DocLayout current={current} pages={pages} />);

  return [
    '<!doctype html>',
    '<html lang="en">',
    '<head>',
    '<meta charset="UTF-8" />',
    '<meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    '<link rel="icon" type="image/svg+xml" href="/favicon.svg" />',
    `<title>${escapeHtml(current.frontmatter.title)} · Pod Docs</title>`,
    '<link rel="preconnect" href="https://fonts.googleapis.com" />',
    '<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />',
    `<link href="${FONTS_HREF}" rel="stylesheet" />`,
    '<style>*,*::before,*::after{box-sizing:border-box;}html{scroll-behavior:smooth;}body{margin:0;padding:0;}</style>',
    '</head>',
    '<body>',
    body,
    '</body>',
    '</html>',
    '',
  ].join('\n');
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
