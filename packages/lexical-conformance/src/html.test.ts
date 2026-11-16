/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {
  buildEditorFromExtensions,
  HorizontalRuleExtension,
  HorizontalRuleNode,
} from '@lexical/extension';
import {createHeadlessEditor} from '@lexical/headless';
import {
  $generateHtmlFromNodes,
  $generateNodesFromDOM,
  $generateNodesFromDOMViaExtension,
  CoreImportExtension,
} from '@lexical/html';
import {JSDOM} from 'jsdom';
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createTabNode,
  $createTextNode,
  $getRoot,
  $isElementNode,
  defineExtension,
  type EditorThemeClasses,
  type ElementFormatType,
  type LexicalNode,
  TEXT_TYPE_TO_FORMAT,
} from 'lexical';
import {readFileSync} from 'node:fs';
import {test} from 'vitest';

import {browserWindow} from './browserWindow';
import {DOCUMENT_NODES} from './property';
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
  const globals = globalThis as {
    window?: unknown;
    document?: unknown;
    CSSStyleRule?: unknown;
  };
  const prev = [globals.window, globals.document, globals.CSSStyleRule];
  globals.window = dom;
  globals.document = dom.document;
  // `$inlineStylesFromStyleSheets` reads the global, as in a browser.
  globals.CSSStyleRule = dom.CSSStyleRule;
  try {
    return f(dom);
  } finally {
    [globals.window, globals.document, globals.CSSStyleRule] = prev;
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

/** HTML for the importers of the node packages (lists, rich text, links, code, the horizontal rule). */
const NODES_IMPORT_CORPUS = [
  '<ul><li>one</li><li>two<ul><li>nested</li></ul></li></ul>',
  '<ol start="3"><li>three</li><li value="7">seven</li></ol>',
  '<ol start=" -2x"><li>a</li></ol><ol start="99999999999"><li>b</li></ol>',
  '<ul __lexicallisttype="check"><li aria-checked="true">done</li><li aria-checked="false">todo</li></ul>',
  '<ul class="contains-task-list"><li class="task-list-item"><input type="checkbox" checked> gh</li><li class="task-list-item"><input type="checkbox"> gh2</li></ul>',
  '<ul data-is-checklist="1"><li class="joplin-checkbox"><div class="checkbox-wrapper"><input type="checkbox" checked></div>j</li></ul>',
  '<ul><li><p style="text-align: center">docs</p></li><li dir="rtl" style="text-align: right">r</li></ul>',
  '<ul><li>a</li>loose<li>b</li></ul>',
  '<h1>one</h1><h2 style="text-align: center">two</h2><h3 dir="rtl">three</h3><h4 data-lexical-indent="2">four</h4><h5>five</h5><h6>six</h6>',
  '<p><span style="font-size: 26pt">Docs title</span></p>',
  '<blockquote style="text-align: right" dir="ltr">quote<br>more</blockquote>',
  '<blockquote><p>a</p><p>b</p></blockquote>',
  '<p><a href="https://example.com" target="_blank" rel="noopener" title="t">link</a> <a href="/x"></a><a href="#y"><b>bold</b></a></p>',
  '<pre data-language="js" data-theme="dark">const a = 1;\n\tb();</pre>',
  '<code>one line</code><code>two\nlines</code><code>a<br>b</code>',
  '<div style="font-family: monospace">mono<br>code</div><div style="font-family: Menlo, monospace"><div>child</div></div>',
  '<table class="js-file-line-container"><tr><td class="blob-num">1</td><td class="js-file-line">line one</td></tr><tr><td>2</td><td class="js-file-line">line two</td></tr></table>',
  '<p>before</p><hr><p>after</p>',
  '<p>a<hr>b</p>',
];

test('html/import-nodes', () => {
  const htmls = [
    ...NODES_IMPORT_CORPUS,
    // Everything property/documents' HTML export writes must import too.
    // That file comes from property.test.ts, so a `generate` that changes it
    // needs a second run to bring this one up to date.
    ...JSON.parse(
      readFileSync(
        new URL('../vectors/property/documents.json', import.meta.url),
        'utf8',
      ),
    )
      .cases.map((c: {html?: string | null}) => c.html)
      .filter((html: unknown): html is string => typeof html === 'string'),
  ];
  const cases: ImportCase[] = htmls.map(html => {
    const editor = createHeadlessEditor({
      namespace: '',
      nodes: [...DOCUMENT_NODES, HorizontalRuleNode],
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
    'html/import-nodes',
    'html-import',
    'HTML for the node packages (curated, plus every property/documents HTML export) and the nodes $generateNodesFromDOM makes from it in jsdom with the property/documents nodes and HorizontalRuleNode, each as exportJSON with its children.',
    cases,
  );
});

/**
 * HTML for `DOMImportExtension`'s walk and `CoreImportRules` that the legacy
 * corpus doesn't reach: whitespace against unknown and block neighbors, the
 * style overrides of the inline format rule, `<hr>`, and `<style>` sheets
 * (`$inlineStylesFromStyleSheets`).
 */
const IMPORT_EXTENSION_CORPUS = [
  '<p>a <foo> b </foo> c</p>',
  '<p>x<span> </span>y</p>',
  '<p> <b> bold </b> </p>',
  '<div> one </div><div> two </div>',
  '<p>end <br> next</p>',
  '<p>a\r\nb</p>',
  '<b><span style="font-weight: normal">plain</span> bold</b>',
  '<b><span style="font-weight: 400">400</span></b><span style="font-weight: 700">700</span>',
  '<i><span style="font-style: normal">upright</span></i>',
  '<u><span style="text-decoration: none">none</span></u><span style="text-decoration: underline line-through">both</span>',
  '<span style="text-transform: lowercase">L</span><span style="text-transform: capitalize">c</span><span style="text-transform: uppercase"><span style="text-transform: none">n</span></span>',
  '<sup><span style="vertical-align: sub">sub</span></sup><sub><span style="vertical-align: baseline">base</span></sub>',
  '<code><b>cb</b></code><mark><i>mi</i></mark>',
  '<span style="color: red; font-weight: bold">styled</span>',
  '<p align="justify">j</p><p align="middle">bad</p><p style="text-align: left" align="right">both</p>',
  '<section style="text-align: center"><h1>h</h1>inline<p style="text-align: right">p</p></section>',
  '<div><br></div><div>a<br></div><div><br>b</div>',
  '<br>',
  'a<br>',
  '<hr>',
  '<p>a<hr>b</p>',
  '<div>x<hr>y</div>',
  '<script>s</script><style>p { color: red }</style><p>visible</p>',
  '<style>p { color: red; font-weight: bold }</style><p>red bold</p>',
  '<style>.big { font-size: 20px } #i { font-style: italic }</style><p class="big" id="i">both</p>',
  '<style>span { color: blue }</style><p><span style="color: green">own wins</span><span>blue</span></p>',
  '<style>b, i { text-decoration: underline }</style><b>b</b><i>i</i>',
  '<style>.xl1 { font-weight: 700; }</style><p><span class="xl1">bold</span></p>',
  '<style>span.a[data-a="1"] { font-weight: 700 } p > span { font-style: italic } div span { text-decoration: underline }</style><p><span class="a" data-a="1">s</span></p><div><b><span>deep</span></b></div>',
  '<style>b + i { font-style: normal } b ~ u { text-decoration: none } * { vertical-align: super }</style><p><b>b</b><i>i</i><u>u</u></p>',
  '<style>span { FONT-WEIGHT: BOLD } .a, .b { font-style: italic }</style><p><span class="b">case</span></p>',
  '<style>span { font-weight: bold</style><p><span>unterminated</span></p>',
  '<style>span { content: "}"; font-weight: bold }</style><p><span>quoted</span></p>',
  '<style>@media print { p { color: red } } p { font-style: italic }</style><p>media</p>',
  '<style>/* c */ p { color: red !important } <!-- --></style><p>important</p>',
  '<style>p { white-space: pre }</style><p>  pre   ws </p>',
  '<style>div { display: inline }</style><p>a <div> b </div> c</p>',
  '<html><head><style>p { font-weight: bold }</style></head><body><p>head sheet</p></body></html>',
  '<!-- comment --><p>after comment</p>',
  '<p>a<!-- c -->b</p>',
  '<unknown>stray text</unknown>',
  '<table><tr><td>a</td><td>b</td></tr></table>',
];

test('html/import-extension', () => {
  const htmls = [
    ...IMPORT_CORPUS,
    ...IMPORT_EXTENSION_CORPUS,
    ...JSON.parse(
      readFileSync(
        new URL('../vectors/html/export.json', import.meta.url),
        'utf8',
      ),
    ).cases.map((c: Case) => c.html),
  ];
  const cases: ImportCase[] = htmls.map(html => {
    const editor = buildEditorFromExtensions(
      defineExtension({
        dependencies: [CoreImportExtension, HorizontalRuleExtension],
        name: '[conformance]',
      }),
    );
    let nodes = '';
    withJSDOM(() => {
      // A document of its own, not DOMParser's: jsdom gives those no
      // styleSheets, where a browser does.
      const dom = new JSDOM(html).window.document;
      editor.update(
        () => {
          nodes = JSON.stringify(
            $generateNodesFromDOMViaExtension(dom).map($nodeToJSON),
          );
        },
        {discrete: true},
      );
    });
    editor.dispose();
    return {html, nodes};
  });
  emitVectors(
    'html/import-extension',
    'html-import',
    "HTML (html/import's corpus, curated cases for the walk, the inline format rule, <hr> and <style> sheets, and every html/export output) and the nodes DOMImportExtension's $generateNodesFromDOM makes from it in a jsdom document, in an editor built from CoreImportExtension and HorizontalRuleExtension, each as exportJSON with its children.",
    cases,
  );
});
