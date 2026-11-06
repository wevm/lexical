/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import entityMap from 'entities/lib/maps/entities.json';
import legacyMap from 'entities/lib/maps/legacy.json';
import {JSDOM} from 'jsdom';
import {test} from 'vitest';

import {browserWindow} from './browserWindow';
import {emitVectors} from './vectors';

// HTML import parses with `DOMParser` (jsdom in the upstream unit tests).
// These vectors pin the trees it builds, as `head` and `body` innerHTML, for
// the kinds of HTML Lexical imports: its own export, pasted HTML from word
// processors and browsers, and malformed markup the tree builder repairs.

type Case = {html: string; head: string; body: string};

const corpus = [
  '',
  'plain text',
  '<p>Hello world</p>',
  '<p>a<p>b',
  '<p>one<div>two</div>three</p>',
  '<b>bold <i>both</b> italic</i>',
  '<span style="white-space: pre-wrap;">a</span><br><br data-lexical-managed-linebreak="true">',
  '<p dir="rtl" style="text-align: right; padding-inline-start: 80px;" data-lexical-indent="2"><span>x</span></p>',
  '<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-size:11pt;font-family:Arial;">Docs</span></p></b>',
  '<html><head><style>p { color: red; }</style></head><body><p class="c">styled</p></body></html>',
  '<!-- comment --><p>after <!-- inner --> comment</p>',
  '<p>&amp; &lt; &gt; &quot; &#39; &nbsp; &copy; &#x1F600; &unknown; &amp</p>',
  '<p title="a &amp; b" data-x=\'single\' data-y=unquoted data-z>attrs</p>',
  // A browser escapes `<` and `>` in an attribute value, as well as `&`, `"`
  // and U+00A0.
  '<img alt="a<b>c &amp; &quot;d&quot;&nbsp;e" title=\'x<y\'><p data-q="<b>">t</p>',
  '<P CLASS="Upper">case</P>',
  '<ul><li>one<li>two</ul><ol start="3"><li>three</li></ol>',
  '<table><tr><td>a</td><td>b</td></tr></table>',
  '<table><tbody><tr><th>h</th></tr><tr><td>c</td></tr></tbody></table>',
  '<pre>  keep\n  spaces\n</pre>',
  '<pre>\nleading newline</pre>',
  '<textarea>\nraw <b>text</b></textarea>',
  '<script>var x = "<p>";</script><p>after script</p>',
  '<h1>Title</h1><h2>Sub</h2><blockquote>quote</blockquote>',
  '<a href="https://example.com?a=1&b=2">link</a>',
  '<img src="x.png" alt="pic"><hr><input type="checkbox" checked>',
  '<code>let x = 1;</code><mark>hi</mark><sub>2</sub><sup>3</sup><s>del</s><u>under</u>',
  '<div><span>a</span>\n  <span>b</span>\n</div>',
  '<br/><p/>self-closing',
  '</p>stray end tag',
  '<li>orphan item</li>',
  '<td>orphan cell</td>',
  '<p>unclosed <b>bold',
  'text <svg width="10"><circle r="1"></circle></svg> more',
  '<o:p></o:p><p class="MsoNormal">Word<o:p></o:p></p>',
  '<span style="font-weight:700">w</span><span style="font-style:italic">i</span>',
  '<p>tab\there</p>',
  '<details><summary>s</summary>body</details>',
];

test('html/parse', () => {
  const {DOMParser} = browserWindow();
  const parser = new DOMParser();
  const cases: Case[] = corpus.map(html => {
    const doc = parser.parseFromString(html, 'text/html');
    return {body: doc.body.innerHTML, head: doc.head.innerHTML, html};
  });
  emitVectors(
    'html/parse',
    'html-parse',
    "HTML strings and the head and body innerHTML of the document jsdom's DOMParser builds for them.",
    cases,
  );
});

type EntityCase = {name: string; value: string; legacy: boolean};

test('html/entities', () => {
  // Every named character reference, with the text jsdom decodes it to, and
  // whether it also decodes without its semicolon in text (a legacy name).
  const {DOMParser} = new JSDOM('').window;
  const parser = new DOMParser();
  const decode = (html: string) =>
    parser.parseFromString(`<p>${html}</p>`, 'text/html').body.textContent;
  const legacyNames = new Set(Object.keys(legacyMap));
  const cases: EntityCase[] = Object.keys(entityMap)
    .sort()
    .map(name => ({
      legacy:
        legacyNames.has(name) && decode(`&${name}`) === decode(`&${name};`),
      name,
      value: decode(`&${name};`) ?? '',
    }));
  emitVectors(
    'html/entities',
    'html-entities',
    'The named character references of HTML, the text each decodes to in jsdom, and whether it decodes without a semicolon.',
    cases,
  );
});
