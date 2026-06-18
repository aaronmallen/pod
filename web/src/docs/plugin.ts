import fs from 'fs';
import path from 'path';
import type { Plugin } from 'vite';
import { loadDocPages } from './content';
import { renderDocPage } from './render';

export interface DocsPluginOptions {
  contentDir: string;
  root: string;
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
