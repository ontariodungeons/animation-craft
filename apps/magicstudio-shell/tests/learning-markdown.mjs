import test from 'node:test';
import assert from 'node:assert/strict';
import { splitDocument, joinDocument, editableProblem } from '../frontend/learning-markdown.mjs';

test('unknown frontmatter survives body edits byte for byte', () => {
  const prefix = '\ufeff---\r\nid: trail\r\ncustom: {future: [1, 2]}\r\n---\r\n';
  const d = splitDocument(prefix + '# Trail\n\nOriginal.\n');
  assert.equal(d.prefix, prefix);
  assert.equal(joinDocument(d, '# Trail\n\nRevised.\n'), prefix + '# Trail\n\nRevised.\n');
});
test('ordinary Markdown exposes literal code, prompts and links for editing', () => {
  const body = '# A title\n\n1. First step\n2. Second step\n\n> Preserve the supplied timing.\n\n```sh\nmc "$PROJECT" storyboard.init\n```\n\n[Source](https://example.com/guide)\n';
  assert.equal(editableProblem(body), null);
  assert.equal(joinDocument(splitDocument(body), body), body);
});
test('unsupported body syntax blocks editing rather than dropping content', () => {
  for (const body of ['<future-widget key="x">keep me</future-widget>', '[^n]: preserve footnote', ':::custom\nkeep\n:::']) {
    assert.match(editableProblem(body), /unsupported/i);
    assert.equal(joinDocument(splitDocument(body), body), body);
  }
});
test('code examples containing markup are supported literal content', () => {
  assert.equal(editableProblem('```html\n<future-widget>example</future-widget>\n```\n'), null);
});

test('unsafe link schemes are retained but refused',()=>{assert.match(editableProblem('[unsafe](javascript:alert(1))'),/unsafe link/i);});
