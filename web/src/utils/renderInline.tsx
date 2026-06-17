import { T } from '../tokens';

const codeStyle: React.CSSProperties = {
  fontFamily: '"JetBrains Mono", ui-monospace, monospace',
  background: T.plasmaSoft,
  padding: '1px 6px',
  borderRadius: 4,
  fontSize: '0.85em',
};

export function renderInline(text: string, accent: string): React.ReactNode[] {
  const nodes: React.ReactNode[] = [];
  let buf = '';
  let key = 0;
  let i = 0;

  const flush = () => {
    if (buf) {
      nodes.push(buf);
      buf = '';
    }
  };

  while (i < text.length) {
    if (text[i] === '`') {
      const end = text.indexOf('`', i + 1);
      if (end > i) {
        flush();
        nodes.push(<code key={key++} style={codeStyle}>{text.slice(i + 1, end)}</code>);
        i = end + 1;
        continue;
      }
    }

    if (text[i] === '*' && text[i + 1] === '*') {
      const end = text.indexOf('**', i + 2);
      if (end > i + 1) {
        flush();
        nodes.push(
          <strong key={key++} style={{ fontWeight: 600 }}>
            {renderInline(text.slice(i + 2, end), accent)}
          </strong>,
        );
        i = end + 2;
        continue;
      }
    }

    if (text[i] === '_') {
      const end = text.indexOf('_', i + 1);
      if (end > i) {
        flush();
        nodes.push(<em key={key++}>{renderInline(text.slice(i + 1, end), accent)}</em>);
        i = end + 1;
        continue;
      }
    }

    if (text[i] === '[') {
      const close = text.indexOf(']', i + 1);
      if (close > i && text[close + 1] === '(') {
        const paren = text.indexOf(')', close + 2);
        if (paren > close) {
          flush();
          const url = text.slice(close + 2, paren);
          nodes.push(
            <a
              key={key++}
              href={url}
              target="_blank"
              rel="noreferrer"
              style={{ color: accent, textDecoration: 'none' }}
            >
              {renderInline(text.slice(i + 1, close), accent)}
            </a>,
          );
          i = paren + 1;
          continue;
        }
      }
    }

    buf += text[i];
    i++;
  }

  flush();
  return nodes;
}
