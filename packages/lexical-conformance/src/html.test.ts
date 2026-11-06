/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {JSDOM} from 'jsdom';

import {createHeadlessEditor} from '@lexical/headless';
import {$generateHtmlFromNodes, $generateNodesFromDOM} from '@lexical/html';
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createTabNode,
  $createTextNode,
  $getRoot,
  $isElementNode,
  type EditorThemeClasses,
  type ElementFormatType,
  type LexicalNode,
  TEXT_TYPE_TO_FORMAT,
} from 'lexical';
import {readFileSync} from 'node:fs';
import {test} from 'vitest';

import {browserWindow} from './browserWindow';
import {emitVectors} from './vectors';

type Case = {
  name: string;
  /** The theme the editor was created with, if any. */
  theme?: EditorThemeClasses;
  /** `JSON.stringify(editorState.toJSON())`. */
  editorState: string;
  /** `$generateHtmlFromNodes(editor)`. */
  html: string;
};

/**
 * Runs `f` with a jsdom window as the global `window` and `document`, the
 * DOM the upstream unit tests run on (`@lexical/headless/dom` would give
 * happy-dom, which normalizes inline styles differently).
 */
function withJSDOM<T>(f: (dom: JSDOM['window']) => T): T {
  const dom = browserWindow();
  const globals = globalThis as {window?: unknown; document?: unknown};
  const prev = [globals.window, globals.document];
  globals.window = dom;
  globals.document = dom.document;
  try {
    return f(dom);
  } finally {
    [globals.window, globals.document] = prev;
    dom.close();
  }
}

/** mulberry32, as in fuzz.test.ts. */
function prng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const ALIGNMENTS: ElementFormatType[] = [
  '',
  'left',
  'center',
  'right',
  'justify',
  'start',
  'end',
];
const STYLES = [
  '',
  'color: red',
  'font-size: 15px; color: #f00',
  'background-color: rgb(1,2,3) !important',
  'font-family: Arial, "Times New Roman"',
  '--custom: value',
];
const TEXTS = ['a', 'Hello world', '', 'é😀', '<b>&amp; "q"</b>', '\u00a0x '];
const FORMATS = Object.values(TEXT_TYPE_TO_FORMAT);

const THEME: EditorThemeClasses = {
  indent: 'my-indent',
  paragraph: 'my-paragraph',
  tab: 'my-tab',
  text: {
    base: 'my-text',
    bold: 'my-bold',
    code: 'my-code',
    italic: 'my-italic',
    strikethrough: 'my-strikethrough',
    underline: 'my-underline',
    underlineStrikethrough: 'my-underline-strikethrough',
  },
};

function $randomDocument(random: () => number): void {
  const int = (n: number) => Math.floor(random() * n);
  const pick = <T>(items: readonly T[]): T => items[int(items.length)];
  const paragraphs = 1 + int(4);
  for (let p = 0; p < paragraphs; p++) {
    const paragraph = $createParagraphNode()
      .setIndent(int(3))
      .setFormat(pick(ALIGNMENTS))
      .setDirection(pick([null, null, 'ltr', 'rtl'] as const));
    const children: LexicalNode[] = [];
    const count = int(5);
    for (let c = 0; c < count; c++) {
      const kind = int(6);
      if (kind === 0) {
        children.push($createLineBreakNode());
      } else if (kind === 1) {
        children.push($createTabNode());
      } else {
        let format = 0;
        for (let f = int(3); f > 0; f--) {
          format |= pick(FORMATS);
        }
        children.push(
          $createTextNode(pick(TEXTS)).setFormat(format).setStyle(pick(STYLES)),
        );
      }
    }
    $getRoot().append(paragraph.append(...children));
  }
}

test('html/export', () => {
  const documents: {
    name: string;
    theme?: EditorThemeClasses;
    build: () => void;
  }[] = [
    {build: () => $getRoot().append($createParagraphNode()), name: 'empty'},
    {
      build: () =>
        $getRoot().append(
          $createParagraphNode().append(
            $createTextNode('a'),
            $createLineBreakNode(),
          ),
        ),
      name: 'trailing line break',
    },
  ];
  for (let seed = 1; seed <= 40; seed++) {
    const random = prng(seed);
    documents.push({
      build: () => $randomDocument(random),
      name: `seed ${seed}`,
      theme: seed % 4 === 0 ? THEME : undefined,
    });
  }
  const cases: Case[] = documents.map(({name, theme, build}) => {
    const editor = createHeadlessEditor({
      onError: e => {
        throw e;
      },
      // A copy: the editor caches class names on the theme it is given.
      theme: theme && JSON.parse(JSON.stringify(theme)),
    });
    editor.update(build, {discrete: true});
    const html = withJSDOM(() =>
      editor.read(() => $generateHtmlFromNodes(editor)),
    );
    return {
      editorState: JSON.stringify(editor.getEditorState().toJSON()),
      html,
      name,
      ...(theme ? {theme} : {}),
    };
  });
  emitVectors(
    'html/export',
    'html-export',
    'Core-node documents (seeded random, some with a theme) and the HTML $generateHtmlFromNodes writes for them in jsdom, the DOM of the upstream unit tests.',
    cases,
  );
});

/** A node and its subtree as JSON, as `exportJSON` writes each node. */
function $nodeToJSON(node: LexicalNode): Record<string, unknown> {
  const json: Record<string, unknown> = {...node.exportJSON()};
  if ($isElementNode(node)) {
    json.children = node.getChildren().map($nodeToJSON);
  }
  return json;
}

type ImportCase = {html: string; nodes: string};

const IMPORT_CORPUS = [
  '<p>Hello world</p>',
  '<p>a<p>b',
  'plain text',
  '<p>  leading and   collapsed   spaces  </p>',
  '<span>a </span><span> b</span>',
  '<p>one<br>two<br></p>',
  '<p><br></p>',
  '<div>x<br><br></div>',
  '<b>bold</b><i>italic</i><u>u</u><s>s</s><code>c</code><mark>m</mark><sub>2</sub><sup>3</sup>',
  '<strong>s</strong><em>e</em>',
  '<b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:700;font-style:italic;text-decoration:underline line-through;">Docs</span></p></b>',
  '<span style="vertical-align: super">sup</span><span style="vertical-align:sub">sub</span><span style="text-transform: uppercase">up</span>',
  '<p style="text-align: center">centered</p><p align="right">legacy</p><p dir="rtl">rtl</p>',
  '<p style="padding-inline-start: 80px">indent</p><p data-lexical-indent="3">attr</p>',
  '<div><p>nested</p>loose text<p>again</p></div>',
  '<div style="text-align: right">a<p>b</p>c</div>',
  '<pre>  keep\n\tspaces</pre>',
  '<span style="white-space: pre-wrap">a  \t b</span>',
  '<ul><li>one</li><li>two</li></ul>',
  '<h1>Title</h1><blockquote>quote</blockquote>',
  '<a href="https://example.com">link</a> after',
  'a<div></div>b',
  '<table><tr><td>cell</td></tr></table>',
  '<p>tab\there</p>',
  '<p>&amp; &lt;tag&gt; &nbsp;nbsp</p>',
  '<script>x</script><p>after</p>',
];

test('html/import', () => {
  const htmls = [
    ...IMPORT_CORPUS,
    // Everything html/export writes must import too.
    ...JSON.parse(
      readFileSync(
        new URL('../vectors/html/export.json', import.meta.url),
        'utf8',
      ),
    ).cases.map((c: Case) => c.html),
  ];
  const cases: ImportCase[] = htmls.map(html => {
    const editor = createHeadlessEditor({
      onError: e => {
        throw e;
      },
    });
    let nodes = '';
    withJSDOM(win => {
      const dom = new win.DOMParser().parseFromString(html, 'text/html');
      editor.update(
        () => {
          nodes = JSON.stringify(
            $generateNodesFromDOM(editor, dom).map($nodeToJSON),
          );
        },
        {discrete: true},
      );
    });
    return {html, nodes};
  });
  emitVectors(
    'html/import',
    'html-import',
    'HTML (curated, plus every html/export output) and the nodes $generateNodesFromDOM makes from it in jsdom, each as exportJSON with its children.',
    cases,
  );
});
