import fs from 'fs';
import path from 'path';

interface Note {
  tag: string;
  tone: string;
  text: string;
}

const SECTION_MAP: Record<string, { tag: string; tone: string }> = {
  Added:   { tag: 'NEW',    tone: 'plasma'  },
  Fixed:   { tag: 'FIX',    tone: 'success' },
  Changed: { tag: 'CHANGE', tone: 'warning' },
};

function parseChangelog(content: string): { version: string; notes: Note[] } {
  const lines = content.split('\n');

  let versionLine = -1;
  let version = '';
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^## \[(\d+\.\d+\.\d+)\]/);
    if (m) {
      versionLine = i;
      version = m[1];
      break;
    }
  }

  if (versionLine === -1) throw new Error('No versioned section found in CHANGELOG.md');

  let endLine = lines.length;
  for (let i = versionLine + 1; i < lines.length; i++) {
    if (lines[i].startsWith('## ')) {
      endLine = i;
      break;
    }
  }

  const section = lines.slice(versionLine + 1, endLine);

  const notes: Note[] = [];
  let currentMeta: { tag: string; tone: string } | null = null;

  for (const line of section) {
    const subHeading = line.match(/^### (.+)/);
    if (subHeading) {
      currentMeta = SECTION_MAP[subHeading[1].trim()] ?? null;
      continue;
    }

    if (!currentMeta) continue;

    if (line.startsWith('- ')) {
      notes.push({ tag: currentMeta.tag, tone: currentMeta.tone, text: line.slice(2) });
      continue;
    }

    if (notes.length > 0 && line.trim() !== '' && !line.startsWith('#')) {
      notes[notes.length - 1].text += ' ' + line.trim();
    }
  }

  return { version, notes };
}

function serialize(notes: Note[]): string {
  const items = notes.map(n => {
    return `  { tag: '${n.tag}', tone: '${n.tone}', text: ${JSON.stringify(n.text)} }`;
  });
  return `[\n${items.join(',\n')},\n]`;
}

function main(): void {
  const changelogPath = path.resolve(
    path.dirname(new URL(import.meta.url).pathname),
    '../../CHANGELOG.md',
  );
  const content = fs.readFileSync(changelogPath, 'utf8');
  const { version, notes } = parseChangelog(content);

  console.log(`Version: ${version}`);
  console.log(`Entries: ${notes.length}`);

  const outDir = path.resolve(
    path.dirname(new URL(import.meta.url).pathname),
    '../src/generated',
  );
  fs.mkdirSync(outDir, { recursive: true });
  const outFile = path.join(outDir, 'notes.ts');

  const fileContent =
    `import type { Note } from '../types';\n\n` +
    `export const NOTES: Note[] = ${serialize(notes)};\n`;

  fs.writeFileSync(outFile, fileContent, 'utf8');
  console.log(`Wrote: ${outFile}`);
}

main();
