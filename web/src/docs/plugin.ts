import fs from 'fs';
import path from 'path';
import { createElement, StrictMode } from 'react';
import { renderToString } from 'react-dom/server';
import type { Plugin } from 'vite';
import { App } from '../App';
import { FAQ_ITEMS } from '../components/Faq';
import { loadDocPages } from './content';
import { renderDocPage } from './render';

export interface DocsPluginOptions {
  contentDir: string;
  root: string;
}

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
    },
  };
}
