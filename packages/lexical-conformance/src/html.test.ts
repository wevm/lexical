/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {createHeadlessEditor} from '@lexical/headless';
import {$generateHtmlFromNodes} from '@lexical/html';
import {JSDOM} from 'jsdom';
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createTabNode,
  $createTextNode,
  $getRoot,
  type EditorThemeClasses,
  type ElementFormatType,
  type LexicalNode,
  TEXT_TYPE_TO_FORMAT,
} from 'lexical';
import {test} from 'vitest';

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
function withJSDOM<T>(f: () => T): T {
  const dom = new JSDOM('').window;
  const globals = globalThis as {window?: unknown; document?: unknown};
  const prev = [globals.window, globals.document];
  globals.window = dom;
  globals.document = dom.document;
  try {
    return f();
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
