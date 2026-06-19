import fs from 'fs';
import path from 'path';
import { createElement, StrictMode } from 'react';
import { renderToString } from 'react-dom/server';
import type { Plugin } from 'vite';
import { App } from '../App';
import { FAQ_ITEMS } from '../components/Faq';
import { loadDocPages } from './content';
import type { DocPage } from './content';
import { renderDocPage } from './render';

export interface DocsPluginOptions {
  contentDir: string;
  root: string;
}

// Canonical production origin. Kept in sync with render.tsx's ORIGIN so the
// sitemap, robots.txt, and per-page canonical links all agree.
const ORIGIN = 'https://pod.aaronmallen.dev';

// Builds the FAQPage JSON-LD from the plain-text Q&A projection so crawlers see
// rich results for the landing page. Kept next to the prerender path because it
// is injected into the same dist/index.html document.
function faqJsonLd(): string {
  const data = {
    '@context': 'https://schema.org',
    '@type': 'FAQPage',
    mainEntity: FAQ_ITEMS.map((item) => ({
      '@type': 'Question',
      name: item.question,
      acceptedAnswer: { '@type': 'Answer', text: item.answer },
    })),
  };

  // JSON.stringify escapes the only character (<) that could break out of the
  // script context; embed it as a standalone ld+json block.
  return `<script type="application/ld+json">\n${JSON.stringify(data, null, 2)}\n</script>`;
}

// Renders the live App component to hydration-compatible HTML and injects it
// (plus the FAQPage JSON-LD) into the Vite-emitted dist/index.html, replacing
// the empty SPA shell so non-JS crawlers see the full landing page.
function prerenderLanding(outDir: string, info: (msg: string) => void): void {
  const indexPath = path.join(outDir, 'index.html');
  if (!fs.existsSync(indexPath)) return;

  const body = renderToString(createElement(StrictMode, null, createElement(App)));

  let html = fs.readFileSync(indexPath, 'utf8');
  html = html.replace('<div id="root"></div>', `<div id="root">${body}</div>`);
  html = html.replace('</head>', `${faqJsonLd()}\n  </head>`);

  fs.writeFileSync(indexPath, html, 'utf8');
  info('prerendered landing page into dist/index.html');
}

// Emits dist/sitemap.xml covering the landing page plus every doc page. Each
// page.url is already the site-absolute path (e.g. "/docs/" or "/docs/guide/"),
// so we just prefix the canonical origin. The landing page ("/") is included
// explicitly since it is not part of the doc page set.
function emitSitemap(outDir: string, pages: DocPage[], info: (msg: string) => void): void {
  const paths = ['/', ...pages.map((page) => page.url)];
  const urls = paths
    .map((p) => `  <url>\n    <loc>${ORIGIN}${p}</loc>\n  </url>`)
    .join('\n');

  const xml = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`;

  fs.writeFileSync(path.join(outDir, 'sitemap.xml'), xml, 'utf8');
  info(`emitted sitemap.xml with ${paths.length} url(s)`);
}

export function docsPlugin(options: DocsPluginOptions): Plugin {
  let outDir = path.resolve(options.root, 'dist');

  return {
    name: 'pod-docs',

    apply: 'build',

    configResolved(config) {
      outDir = path.resolve(config.root, config.build.outDir);
    },

    closeBundle() {
      // Landing prerender: turn the empty SPA shell into real static markup.
      prerenderLanding(outDir, (msg) => this.info(msg));

      const contentDir = path.resolve(options.root, options.contentDir);
      if (!fs.existsSync(contentDir)) return;

      const pages = loadDocPages(contentDir);
      for (const page of pages) {
        const dest = path.join(outDir, 'docs', ...page.slug.split('/'), 'index.html');
        fs.mkdirSync(path.dirname(dest), { recursive: true });
        fs.writeFileSync(dest, renderDocPage(page, pages), 'utf8');
      }

      this.info(`emitted ${pages.length} doc page(s)`);

      // Sitemap: landing page + every doc URL, from the same page set above.
      emitSitemap(outDir, pages, (msg) => this.info(msg));
    },
  };
}
