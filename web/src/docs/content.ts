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

const SECTION_ORDER = ['Pod Docs', 'Guide', 'Features', 'Reference'];

const md = new MarkdownIt({
  html: false,
  linkify: true,
  typographer: false,
});

const PUBLIC_DIR = path.resolve(process.cwd(), 'public');

interface ImageDimensions {
  width: number;
  height: number;
}

const dimensionCache = new Map<string, ImageDimensions | null>();

// Read intrinsic dimensions from a PNG's IHDR chunk (a dev-only, build-time read
// over web/public). Avoids an extra dependency: the PNG header layout is fixed —
// the 8-byte signature is followed by the IHDR chunk whose data begins at byte 16
// with big-endian uint32 width and height. Returns null for non-PNG or unreadable
// sources so the image still renders, just without width/height.
function readPngDimensions(absPath: string): ImageDimensions | null {
  if (dimensionCache.has(absPath)) {
    return dimensionCache.get(absPath) ?? null;
  }

  let dimensions: ImageDimensions | null = null;
  try {
    const fd = fs.openSync(absPath, 'r');
    try {
      const header = Buffer.alloc(24);
      const read = fs.readSync(fd, header, 0, 24, 0);
      const isPng =
        read >= 24 &&
        header.readUInt32BE(0) === 0x89504e47 &&
        header.readUInt32BE(4) === 0x0d0a1a0a;
      if (isPng) {
        const width = header.readUInt32BE(16);
        const height = header.readUInt32BE(20);
        if (width > 0 && height > 0) {
          dimensions = { width, height };
        }
      }
    } finally {
      fs.closeSync(fd);
    }
  } catch {
    dimensions = null;
  }

  dimensionCache.set(absPath, dimensions);
  return dimensions;
}

// Resolve a doc image src (e.g. "/docs/img/foo/bar.png") to a path under web/public.
function resolvePublicAsset(src: string): string | null {
  if (!src.startsWith('/')) {
    return null;
  }
  return path.join(PUBLIC_DIR, src);
}

const defaultImageRenderer =
  md.renderer.rules.image ??
  ((tokens, idx, options, _env, self) => self.renderToken(tokens, idx, options));

md.renderer.rules.image = (tokens, idx, options, env, self) => {
  const token = tokens[idx];

  token.attrSet('loading', 'lazy');
  token.attrSet('decoding', 'async');

  const src = token.attrGet('src');
  if (src && token.attrIndex('width') < 0 && token.attrIndex('height') < 0) {
    const assetPath = resolvePublicAsset(src);
    const dimensions = assetPath ? readPngDimensions(assetPath) : null;
    if (dimensions) {
      token.attrSet('width', String(dimensions.width));
      token.attrSet('height', String(dimensions.height));
    }
  }

  return defaultImageRenderer(tokens, idx, options, env, self);
};

export function loadDocPages(contentDir: string): DocPage[] {
  const pages = collectMarkdownFiles(contentDir).map((sourcePath) => loadDocPage(contentDir, sourcePath));

  pages.sort((a, b) => {
    if (a.frontmatter.section !== b.frontmatter.section) {
      const rankDelta = sectionRank(a.frontmatter.section) - sectionRank(b.frontmatter.section);
      if (rankDelta !== 0) {
        return rankDelta;
      }
      return a.frontmatter.section.localeCompare(b.frontmatter.section);
    }
    if (a.frontmatter.order !== b.frontmatter.order) {
      return a.frontmatter.order - b.frontmatter.order;
    }
    return a.frontmatter.title.localeCompare(b.frontmatter.title);
  });

  return pages;
}

function sectionRank(section: string): number {
  const index = SECTION_ORDER.indexOf(section);
  return index === -1 ? SECTION_ORDER.length : index;
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
