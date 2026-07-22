import assert from 'node:assert/strict';
import test from 'node:test';
import { escapeJsonLd, firstParagraph } from './render';

test('firstParagraph decodes &amp;lt; to a literal &lt; (order-correct)', () => {
  assert.equal(firstParagraph('<p>A &amp;lt; B</p>'), 'A &lt; B');
});

test('firstParagraph does not reintroduce markup from encoded tags', () => {
  const out = firstParagraph('<p>&lt;script&gt;alert(1)&lt;/script&gt;</p>');

  assert.ok(!out.includes('<script'));
  assert.ok(!out.includes('<'));
  assert.ok(!out.includes('>'));
});

test('firstParagraph strips real tags and collapses whitespace', () => {
  assert.equal(firstParagraph('<p>Hello <a href="/x">world</a>\n  &amp; more</p>'), 'Hello world & more');
});

test('firstParagraph returns empty string when no paragraph is present', () => {
  assert.equal(firstParagraph('<h1>Heading</h1>'), '');
});

test('firstParagraph truncates long text with an ellipsis', () => {
  const out = firstParagraph(`<p>${'a'.repeat(300)}</p>`);

  assert.equal(out.length, 200);
  assert.ok(out.endsWith('...'));
});

test('escapeJsonLd escapes a </script> sequence so it cannot break out', () => {
  const json = JSON.stringify({ description: 'evil </script><script>alert(1)' });
  const escaped = escapeJsonLd(json);

  assert.ok(!escaped.includes('</script>'));
  assert.ok(!escaped.includes('<'));
  assert.ok(!escaped.includes('>'));
  assert.ok(escaped.includes('\\u003c/script\\u003e'));
});

test('escapeJsonLd round-trips to the original value through JSON.parse', () => {
  const value = { description: 'a </script> b' };
  const escaped = escapeJsonLd(JSON.stringify(value));

  assert.deepEqual(JSON.parse(escaped), value);
});
