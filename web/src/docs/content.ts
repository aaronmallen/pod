import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import MarkdownIt from 'markdown-it';
import { parseFrontmatter } from './frontmatter';
import type { DocFrontmatter } from './frontmatter';

export interface DocPage {
  frontmatter: DocFrontmatter;
  html: string;
  slug: string;
  sourcePath: string;
  url: string;
}

const DOCS_BASE = '/docs/';

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
});

export function loadDocPages(contentDir: string): DocPage[] {
  const pages = collectMarkdownFiles(contentDir).map((sourcePath) => loadDocPage(contentDir, sourcePath));

  pages.sort((a, b) => {
    if (a.frontmatter.section !== b.frontmatter.section) {
      return a.frontmatter.section.localeCompare(b.frontmatter.section);
    }
    if (a.frontmatter.order !== b.frontmatter.order) {
      return a.frontmatter.order - b.frontmatter.order;
    }
    return a.frontmatter.title.localeCompare(b.frontmatter.title);
  });

  return pages;
}

export function urlForSlug(slug: string): string {
  return slug === '' ? DOCS_BASE : `${DOCS_BASE}${slug}/`;
}

function collectMarkdownFiles(dir: string): string[] {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  const files: string[] = [];

  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectMarkdownFiles(full));
    } else if (entry.isFile() && entry.name.endsWith('.md')) {
      files.push(full);
    }
  }

  return files;
}

function loadDocPage(contentDir: string, sourcePath: string): DocPage {
  const raw = fs.readFileSync(sourcePath, 'utf8');
  const parsed = matter(raw);
  const frontmatter = parseFrontmatter(parsed.data, path.relative(contentDir, sourcePath));
  const slug = slugForSource(contentDir, sourcePath);

  return {
    frontmatter,
    html: md.render(parsed.content),
    slug,
    sourcePath,
    url: urlForSlug(slug),
  };
}

export function slugForSource(contentDir: string, sourcePath: string): string {
  const relative = path.relative(contentDir, sourcePath).replace(/\\/g, '/');
  const withoutExt = relative.replace(/\.md$/, '');
  return withoutExt.replace(/(^|\/)index$/, '');
}
