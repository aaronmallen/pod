import assert from 'node:assert/strict';
import test from 'node:test';
import { slugForSource, urlForSlug } from './content';

test('slugForSource maps a root index.md to the empty slug', () => {
  assert.equal(slugForSource('content', 'content/index.md'), '');
});

test('slugForSource maps a nested index.md to its directory slug', () => {
  assert.equal(slugForSource('content', 'content/guide/index.md'), 'guide');
});

test('slugForSource maps a regular page to its filename slug', () => {
  assert.equal(slugForSource('content', 'content/skills.md'), 'skills');
});

test('slugForSource keeps a non-index filename ending in index intact', () => {
  assert.equal(slugForSource('content', 'content/reindex.md'), 'reindex');
});

test('urlForSlug routes the empty slug to the docs root', () => {
  assert.equal(urlForSlug(''), '/docs/');
});
