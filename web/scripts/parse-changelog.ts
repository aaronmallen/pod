import fs from 'fs';
import path from 'path';

interface Alert {
  type: string;
  tone: string;
  icon: string;
  text: string;
}

interface Note {
  tag: string;
  tone: string;
  text: string;
}

const ALERT_MAP: Record<string, { tone: string; icon: string }> = {
  NOTE:      { tone: 'plasma',  icon: 'alert-note'      },
  TIP:       { tone: 'success', icon: 'alert-tip'       },
  IMPORTANT: { tone: 'plasma',  icon: 'alert-important' },
  WARNING:   { tone: 'warning', icon: 'alert-warning'   },
  CAUTION:   { tone: 'danger',  icon: 'alert-caution'   },
};

const SECTION_MAP: Record<string, { tag: string; tone: string }> = {
  Added:   { tag: 'NEW',    tone: 'plasma'  },
  Fixed:   { tag: 'FIX',    tone: 'success' },
  Changed: { tag: 'CHANGE', tone: 'warning' },
};

function parseChangelog(content: string): { version: string; notices: Alert[]; notes: Note[] } {
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

  // Leading blockquotes (`> ...`) before any `###` section are GitHub-style alert callouts —
  // typed banners rendered above the itemized notes. A `> [!TYPE]` marker opens a group whose
  // following `>` lines are its body; a blank or non-`>` line closes it and allows another group.
  // An untyped blockquote falls back to WARNING. Bold `**markers**` are stripped.
  const notices: Alert[] = [];
  let pending: { type: string; bodyParts: string[] } | null = null;
  const flush = () => {
    if (!pending) return;
    const meta = ALERT_MAP[pending.type] ?? ALERT_MAP.WARNING;
    const text = pending.bodyParts.filter(Boolean).join(' ').replace(/\*\*/g, '').trim();
    notices.push({ type: pending.type, tone: meta.tone, icon: meta.icon, text });
    pending = null;
  };

  for (const line of section) {
    if (line.startsWith('###')) break;

    if (line.startsWith('>')) {
      const body = line.replace(/^>\s?/, '');
      const marker = body.trim().match(/^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]$/i);
      if (marker) {
        flush();
        pending = { type: marker[1].toUpperCase(), bodyParts: [] };
      } else if (pending) {
        pending.bodyParts.push(body.trim());
      } else {
        pending = { type: 'WARNING', bodyParts: [body.trim()] };
      }
      continue;
    }

    if (line.trim() === '') {
      flush();
      continue;
    }

    flush();
    break;
  }
  flush();

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

  return { version, notices, notes };
}

function serializeNotes(notes: Note[]): string {
  if (notes.length === 0) return '[]';
  const items = notes.map(n => {
    return `  { tag: '${n.tag}', tone: '${n.tone}', text: ${JSON.stringify(n.text)} }`;
  });
  return `[\n${items.join(',\n')},\n]`;
}

function serializeNotices(notices: Alert[]): string {
  if (notices.length === 0) return '[]';
  const items = notices.map(a => {
    return `  { type: '${a.type}', tone: '${a.tone}', icon: '${a.icon}', text: ${JSON.stringify(a.text)} }`;
  });
  return `[\n${items.join(',\n')},\n]`;
}

function main(): void {
  const changelogPath = path.resolve(
    path.dirname(new URL(import.meta.url).pathname),
    '../../CHANGELOG.md',
  );
  const content = fs.readFileSync(changelogPath, 'utf8');
  const { version, notices, notes } = parseChangelog(content);

  console.log(`Version: ${version}`);
  console.log(`Notices: ${notices.length}`);
  console.log(`Entries: ${notes.length}`);

  const outDir = path.resolve(
    path.dirname(new URL(import.meta.url).pathname),
    '../src/generated',
  );
  fs.mkdirSync(outDir, { recursive: true });
  const outFile = path.join(outDir, 'notes.ts');

  const fileContent =
    `import type { Alert, Note } from '../types';\n\n` +
    `export const NOTICES: Alert[] = ${serializeNotices(notices)};\n\n` +
    `export const NOTES: Note[] = ${serializeNotes(notes)};\n`;

  fs.writeFileSync(outFile, fileContent, 'utf8');
  console.log(`Wrote: ${outFile}`);
}

main();
