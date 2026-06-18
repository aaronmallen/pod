export interface DocFrontmatter {
  order: number;
  section: string;
  title: string;
}

const FRONTMATTER_FIELDS = ['order', 'section', 'title'] as const;

export function parseFrontmatter(data: Record<string, unknown>, sourcePath: string): DocFrontmatter {
  for (const field of FRONTMATTER_FIELDS) {
    if (!(field in data)) {
      throw new Error(`${sourcePath}: missing required frontmatter field "${field}"`);
    }
  }

  const { order, section, title } = data;

  if (typeof title !== 'string' || title.trim() === '') {
    throw new Error(`${sourcePath}: frontmatter "title" must be a non-empty string`);
  }

  if (typeof section !== 'string' || section.trim() === '') {
    throw new Error(`${sourcePath}: frontmatter "section" must be a non-empty string`);
  }

  if (typeof order !== 'number' || !Number.isFinite(order)) {
    throw new Error(`${sourcePath}: frontmatter "order" must be a finite number`);
  }

  return { order, section, title };
}
