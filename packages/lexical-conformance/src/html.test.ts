/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {
  $getClipboardDataFromSelection,
  $insertDataTransferForRichText,
  ClipboardDOMImportExtension,
  ClipboardImportExtension,
  type ExportMimeTypeConfig,
  GetClipboardDataExtension,
} from '@lexical/clipboard';
import {CodeExtension} from '@lexical/code-core';
import {
  buildEditorFromExtensions,
  HorizontalRuleExtension,
  HorizontalRuleNode,
} from '@lexical/extension';
import {createHeadlessEditor} from '@lexical/headless';
import {
  $generateDOMFromRoot,
  $generateHtmlFromNodes,
  $generateNodesFromDOM,
  $generateNodesFromDOMViaExtension,
  $getRenderContextValue,
  $setRenderContextValue,
  $withRenderContext,
  contextUpdater,
  contextValue,
  CoreImportExtension,
  createRenderState,
  DOMImportExtension,
  domOverride,
  type DOMRenderConfig,
  DOMRenderExtension,
  RenderContextExport,
  RenderContextRoot,
} from '@lexical/html';
import {$createLinkNode, LinkExtension, LinkNode} from '@lexical/link';
import {CheckListExtension, WordListImportExtension} from '@lexical/list';
import {$createMarkNode, MarkNode} from '@lexical/mark';
import {RichTextExtension, ShadowRootQuoteRule} from '@lexical/rich-text';
import {JSDOM} from 'jsdom';
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createRangeSelection,
  $createTabNode,
  $createTextNode,
  $getDocument,
  $getNodeByKey,
  $getRoot,
  $getSelection,
  $isElementNode,
  $isParagraphNode,
  $isRangeSelection,
  $isTextNode,
  $setSelection,
  type AnyLexicalExtensionArgument,
  configExtension,
  defineExtension,
  type DOMExportOutput,
  type EditorThemeClasses,
  type ElementFormatType,
  isHTMLElement,
  type LexicalEditor,
  type LexicalNode,
  ParagraphNode,
  type PointType,
  TabNode,
  TEXT_TYPE_TO_FORMAT,
  TextNode,
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

/** A render state the `editor-gate` scenarios gate an override on. */
const RENDER_FLAG = createRenderState('flag', () => false);
/** A render state the `context` scenario writes into the HTML. */
const RENDER_LABEL = createRenderState('label', () => 'none');

/** Adds `value` to the element's `attr`, space-separated. */
function $tagExport(
  output: DOMExportOutput,
  attr: string,
  value: string,
): DOMExportOutput {
  const {element} = output;
  if (isHTMLElement(element)) {
    const prev = element.getAttribute(attr);
    element.setAttribute(attr, prev ? `${prev} ${value}` : value);
  }
  return output;
}

/**
 * The DOMRenderExtension configs the html/export-render vector replays,
 * by name. The Rust and Swift runners define the same scenarios.
 */
const RENDER_SCENARIOS: Record<string, Partial<DOMRenderConfig>> = {
  context: {
    contextDefaults: [contextValue(RENDER_LABEL, 'default')],
    overrides: [
      domOverride([ParagraphNode], {
        $exportDOM: (_node, $next) =>
          $tagExport(
            $next(),
            'data-label',
            [
              $getRenderContextValue(RENDER_LABEL),
              String($getRenderContextValue(RenderContextExport)),
              String($getRenderContextValue(RenderContextRoot)),
            ].join(' '),
          ),
      }),
    ],
  },
  'create-dom': {
    overrides: [
      domOverride([ParagraphNode], {
        $createDOM: (node, $next) => {
          const element = $next();
          element.setAttribute('data-size', String(node.getChildrenSize()));
          return element;
        },
      }),
    ],
  },
  'editor-gate': {
    contextDefaults: [contextValue(RENDER_FLAG, true)],
    overrides: [
      domOverride(
        '*',
        {$exportDOM: (_node, $next) => $tagExport($next(), 'data-gate', 'on')},
        {disabledForEditor: reader => reader.get(RENDER_FLAG)},
      ),
    ],
  },
  exclude: {
    overrides: [
      domOverride([ParagraphNode], {
        $shouldExclude: (node, selection, $next) =>
          node.getChildrenSize() > 2 || $next(),
      }),
    ],
  },
  include: {
    overrides: [
      domOverride('*', {
        $extractWithChild: (node, child, selection, destination, $next) =>
          ($isParagraphNode(node) &&
            $isTextNode(child) &&
            child.hasFormat('bold')) ||
          $next(),
        $shouldInclude: (node, selection, $next) =>
          !$isParagraphNode(node) && $next(),
      }),
    ],
  },
  none: {},
  order: {
    overrides: [
      domOverride('*', {
        $exportDOM: (_node, $next) =>
          $tagExport($next(), 'data-order', 'wildcard'),
      }),
      domOverride([(node: LexicalNode) => node.getTextContent().length > 2], {
        $exportDOM: (_node, $next) =>
          $tagExport($next(), 'data-order', 'predicate'),
      }),
      domOverride([TabNode], {
        $exportDOM: (_node, $next) => $tagExport($next(), 'data-order', 'tab'),
      }),
      domOverride([TextNode], {
        $exportDOM: (_node, $next) => $tagExport($next(), 'data-order', 'text'),
      }),
    ],
  },
  'session-gate': {
    overrides: [
      domOverride(
        '*',
        {
          $exportDOM: (_node, $next) =>
            $tagExport($next(), 'data-session', 'on'),
        },
        {disabledForSession: reader => reader.get(RenderContextRoot)},
      ),
    ],
  },
};

/**
 * How a case exports: `html` is `$generateHtmlFromNodes`, `root` the
 * innerHTML of a div `$generateDOMFromRoot` fills, and `scoped` is `html`
 * inside `$withRenderContext` with `contextUpdater(label, l => l + '+scoped')`.
 */
const RENDER_OPS = ['html', 'root', 'scoped'] as const;

function $renderExport(editor: LexicalEditor, op: string): string {
  if (op === 'root') {
    const container = $getDocument().createElement('div');
    $generateDOMFromRoot(container);
    return container.innerHTML;
  }
  if (op === 'scoped') {
    return $withRenderContext(
      [contextUpdater(RENDER_LABEL, label => `${label}+scoped`)],
      editor,
    )(() => $generateHtmlFromNodes(editor));
  }
  return $generateHtmlFromNodes(editor);
}

test('html/export-render', () => {
  const documents: {name: string; build: () => void}[] = [
    {
      build: () =>
        $getRoot().append(
          $createParagraphNode().append(
            $createTextNode('Hello').toggleFormat('bold'),
            $createTabNode(),
            $createTextNode('ab'),
          ),
          $createParagraphNode().append($createTextNode('plain')),
          $createParagraphNode(),
        ),
      name: 'mixed',
    },
  ];
  for (let seed = 1; seed <= 12; seed++) {
    const random = prng(seed);
    documents.push({
      build: () => $randomDocument(random),
      name: `seed ${seed}`,
    });
  }
  const cases: {
    name: string;
    scenario: string;
    /** Whether `$setRenderContextValue(flag, false)` ran before the export. */
    flagOff?: boolean;
    op: string;
    editorState: string;
    html: string;
  }[] = [];
  const scenarios: [string, boolean][] = [
    ...Object.keys(RENDER_SCENARIOS).map(s => [s, false] as [string, boolean]),
    ['editor-gate', true],
  ];
  for (const {name, build} of documents) {
    for (const [scenario, flagOff] of scenarios) {
      const editor = buildEditorFromExtensions(
        defineExtension({
          dependencies: [
            configExtension(DOMRenderExtension, RENDER_SCENARIOS[scenario]),
          ],
          name: '[conformance]',
        }),
      );
      editor.update(build, {discrete: true});
      if (flagOff) {
        $setRenderContextValue(RENDER_FLAG, false, editor);
      }
      const editorState = JSON.stringify(editor.getEditorState().toJSON());
      const ops = scenario === 'context' ? RENDER_OPS : RENDER_OPS.slice(0, 2);
      for (const op of ops) {
        const html = withJSDOM(() =>
          editor.read(() => $renderExport(editor, op)),
        );
        cases.push({
          editorState,
          html,
          name: `${name}, ${scenario}${flagOff ? ' (flag off)' : ''}, ${op}`,
          op,
          scenario,
          ...(flagOff ? {flagOff} : {}),
        });
      }
      editor.dispose();
    }
  }
  emitVectors(
    'html/export-render',
    'html-export-render',
    'Documents (a fixed one and seeded random core-node ones) exported through DOMRenderExtension overrides, by scenario: overrides by class, predicate and wildcard composed in order, $createDOM, $shouldInclude with $extractWithChild, $shouldExclude, disabledForEditor (and $setRenderContextValue lifting it), disabledForSession, and render context values. Each is exported with $generateHtmlFromNodes (`html`), $generateDOMFromRoot (`root`), and, for `context`, inside $withRenderContext (`scoped`), in jsdom. The scenarios are code in html.test.ts that each runner repeats.',
    cases,
  );
});

/**
 * HTML for the packages' import rules (`RichTextImportRules`,
 * `ListImportRules`, `LinkImportRules`, `CodeImportRules` with its VS Code
 * overlay, and `WordListImportExtension`) that NODES_IMPORT_CORPUS doesn't
 * reach.
 */
const PACKAGE_IMPORT_CORPUS = [
  '<h2 data-lexical-indent="1" style="text-align: right" dir="rtl">h <b>b</b></h2>',
  '<blockquote data-lexical-indent="1"><h1>h</h1>x<p>p</p></blockquote>',
  '<span style="font-size: 26pt">bare title</span><p><span style="font-size: 26pt">t</span> after</p>',
  '<p><b>x</b><span style="font-size: 26pt">not first</span></p>',
  '<ul><li>1<div>2</div>3</li><li>a<blockquote>q</blockquote><h3>h</h3>b</li></ul>',
  '<ul><li>a<hr>b</li><li><p>one</p><p>two</p></li></ul>',
  '<ul><li><p style="text-align: center">c</p></li><li style="text-align: left"><p style="text-align: right">kept</p></li></ul>',
  '<ul style="text-align: center" dir="rtl"><li>a</li><li style="text-align: left">b</li></ul>',
  '<ol start="0"><li>z</li></ol><ol><li>a<ol><li>b</li><li>c</li></ol></li><li>d</li></ol>',
  '<ul><li>x<ul><li>y</li></ul>z</li></ul>',
  '<ul><li class="task-list-item">no box</li><li class="task-list-item"><span><input type="checkbox" checked></span>deep</li><li class="task-list-item"><input type="radio" checked>radio</li></ul>',
  '<ul><li class="joplin-checkbox">no wrapper</li><li class="joplin-checkbox"><div class="checkbox-wrapper">none</div>x</li></ul>',
  '<ul><li><span aria-checked="true">s</span></li></ul><ul><li aria-checked="mixed">m</li></ul>',
  '<a href="https://a.test"><h1>head</h1>tail<div>div</div></a>',
  '<a href="x" rel="nofollow"></a><a href="y"><img></a><p><a>no href</a></p>',
  '<pre>a<br>b<div>c</div>d</pre><pre data-language="py"><span>x</span>\n<b>y</b></pre>',
  '<code>a\r\nb</code><code data-language="ts" data-theme="t"><span>one<br>two</span></code>',
  '<div style="font-family: Courier, monospace"><p>p1</p><p>p2</p>tail</div>',
  '<div style="font-family: monospace"><div style="font-family: monospace">inner</div></div>',
  '<table class="js-file-line-container"><tbody><tr><td>1</td><td class="js-file-line"><span>x</span>\ty</td></tr></tbody></table>',
  '<td class="js-file-line">stray cell</td><p>p</p>',
  '<div style="font-family: Menlo, Monaco, monospace; white-space: pre"><div>line 1</div><br><div>  line 3</div></div>',
  '<div style="font-family: Menlo, monospace; white-space: pre">a</div><div style="font-family: Menlo, monospace; white-space: pre">b</div><br style="font-family: Menlo, monospace; white-space: pre"><div style="font-family: Menlo, monospace; white-space: pre">d</div>',
  '<div style="font-family: monospace; white-space: pre">only one</div><p>after</p>',
  '<div style="font-family: monospace; white-space: pre-wrap"><span>x</span>y<div>z</div></div>',
  '<html><head><meta name="Generator" content="Microsoft Word 15"></head><body><p class="MsoListParagraphCxSpFirst" style="mso-list:l0 level1 lfo1"><span><span style="mso-list:Ignore">1.<span>  </span></span></span>one<o:p></o:p></p><p class="MsoListParagraphCxSpMiddle" style="mso-list:l0 level2 lfo1"><span><span style="mso-list:Ignore">a)<span> </span></span></span>nested</p><p class="MsoListParagraphCxSpLast" style="mso-list:l0 level1 lfo1"><span style="mso-list:Ignore">2.</span>two</p><p class="MsoNormal">after<o:p></o:p></p></body></html>',
  '<html><head><meta name="Generator" content="Microsoft Word 15"></head><body><p class="MsoListParagraph" style="mso-list:l1 level1 lfo2"><span style="mso-list:Ignore">·<span> </span></span>single</p><p class="MsoListParagraph" style="mso-list:l1 level1 lfo2"><span style="mso-list:Ignore">·</span>second</p></body></html>',
  '<p class="MsoListParagraph" style="mso-list:l0 level1 lfo1"><span style="mso-list:Ignore">1.</span>not word</p>',
];

/** Blockquotes, imported with `ShadowRootQuoteRule` too. */
const SHADOW_ROOT_QUOTE_CORPUS = [
  '<blockquote><p>a</p><p>b</p></blockquote>',
  '<blockquote style="text-align: right" dir="ltr">quote<br>more</blockquote>',
  '<blockquote data-lexical-indent="1"><h1>h</h1>x<p>p</p></blockquote>',
];

// Builds an editor per case (about 4.5 s in all), near vitest's 5 s default.
test('html/import-packages', {timeout: 60_000}, () => {
  const htmls: [string, boolean][] = [
    ...[
      ...IMPORT_CORPUS,
      ...IMPORT_EXTENSION_CORPUS,
      ...NODES_IMPORT_CORPUS,
      ...PACKAGE_IMPORT_CORPUS,
      ...JSON.parse(
        readFileSync(
          new URL('../vectors/property/documents.json', import.meta.url),
          'utf8',
        ),
      )
        .cases.map((c: {html?: string | null}) => c.html)
        .filter((html: unknown): html is string => typeof html === 'string'),
    ].map(html => [html, false] as [string, boolean]),
    ...SHADOW_ROOT_QUOTE_CORPUS.map(html => [html, true] as [string, boolean]),
  ];
  const cases: (ImportCase & {shadowRootQuote?: boolean})[] = htmls.map(
    ([html, shadowRootQuote]) => {
      const editor = buildEditorFromExtensions(
        defineExtension({
          dependencies: [
            RichTextExtension,
            CheckListExtension,
            LinkExtension,
            CodeExtension,
            WordListImportExtension,
            HorizontalRuleExtension,
            ...(shadowRootQuote
              ? [
                  configExtension(DOMImportExtension, {
                    rules: [ShadowRootQuoteRule],
                  }),
                ]
              : []),
          ],
          name: '[conformance]',
        }),
      );
      let nodes = '';
      withJSDOM(() => {
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
      return {html, nodes, ...(shadowRootQuote ? {shadowRootQuote} : {})};
    },
  );
  emitVectors(
    'html/import-packages',
    'html-import',
    "HTML (the html/import, html/import-extension and html/import-nodes corpora, curated cases for each package's import rules, and every property/documents HTML export) and the nodes DOMImportExtension's $generateNodesFromDOM makes from it in a jsdom document, in an editor built from RichTextExtension, CheckListExtension, LinkExtension, CodeExtension, WordListImportExtension and HorizontalRuleExtension (and, for `shadowRootQuote`, ShadowRootQuoteRule), each as exportJSON with its children.",
    cases,
  );
});

/** A point: the child indices from the root to its node, its offset and type. */
type PathPoint = [number[], number, 'text' | 'element'];

type ClipboardCase = {
  name: string;
  /**
   * The editor: `headless` (`createHeadlessEditor` over DOCUMENT_NODES),
   * `dom-import` (the package extensions with `ClipboardDOMImportExtension`),
   * `priority` (`ClipboardImportExtension` ranking `text/plain` first),
   * `next-data` (a `text/html` handler that calls `$next` with other HTML)
   * or `custom-export` (`GetClipboardDataExtension` with the handlers of
   * CUSTOM_EXPORT).
   */
  scenario: string;
  /** `JSON.stringify(editorState.toJSON())` before the operation. */
  editorState: string;
  /** The range selection the operation runs at: anchor, then focus. */
  selection: [PathPoint, PathPoint];
  /** `copy`: `$getClipboardDataFromSelection()`'s entries in order. */
  clipboard?: [string, string][];
  /** `paste`: the `DataTransfer`'s entries. */
  data?: [string, string][];
  /** `paste`: the editor state after `$insertDataTransferForRichText`. */
  state?: string;
  /** `paste`: the selection after it (`null` when it isn't a range). */
  after?: [PathPoint, PathPoint] | null;
};

function $path(node: LexicalNode): number[] {
  const path: number[] = [];
  for (
    let n: LexicalNode | null = node;
    n && n.getParent();
    n = n.getParent()
  ) {
    path.unshift(n.getIndexWithinParent());
  }
  return path;
}

function $nodeAtPath(path: number[]): LexicalNode {
  let node: LexicalNode = $getRoot();
  for (const i of path) {
    if (!$isElementNode(node)) {
      throw new Error(`no node at ${path}`);
    }
    node = node.getChildAtIndex(i)!;
  }
  return node;
}

/**
 * Every range selection point of the document, in document order, but
 * those inside a surrogate pair (half a pair can't be a Rust string, the
 * Tier 1 exception in rust/PARITY.md).
 */
function $allPoints(): PathPoint[] {
  const points: PathPoint[] = [];
  const visit = (node: LexicalNode) => {
    if ($isTextNode(node)) {
      const text = node.getTextContent();
      for (let i = 0; i <= text.length; i++) {
        const unit = text.charCodeAt(i);
        if (!(unit >= 0xdc00 && unit <= 0xdfff && i > 0)) {
          points.push([$path(node), i, 'text']);
        }
      }
    } else if ($isElementNode(node)) {
      const children = node.getChildren();
      for (let i = 0; i <= children.length; i++) {
        points.push([$path(node), i, 'element']);
        if (i < children.length) {
          visit(children[i]);
        }
      }
    }
  };
  visit($getRoot());
  return points;
}

function $selectPaths([anchor, focus]: [PathPoint, PathPoint]): void {
  const selection = $createRangeSelection();
  selection.anchor.set($nodeAtPath(anchor[0]).getKey(), anchor[1], anchor[2]);
  selection.focus.set($nodeAtPath(focus[0]).getKey(), focus[1], focus[2]);
  $setSelection(selection);
}

function $selectionPaths(): [PathPoint, PathPoint] | null {
  const selection = $getSelection();
  if (!$isRangeSelection(selection)) {
    return null;
  }
  const point = ({key, offset, type}: PointType): PathPoint => [
    $path($getNodeByKey(key)!),
    offset,
    type,
  ];
  return [point(selection.anchor), point(selection.focus)];
}

/** GetClipboardDataExtension's handlers for the `custom-export` scenario. */
const CUSTOM_EXPORT: ExportMimeTypeConfig = {
  'application/x-lexical-editor': [() => null],
  'application/x-node-count': [
    selection => (selection ? String(selection.getNodes().length) : null),
  ],
  'text/html': [
    (_selection, next) => {
      const html = next();
      return html ? `<!--wrapped-->${html}` : html;
    },
  ],
};

function clipboardEditor(scenario: string): LexicalEditor {
  const onError = (e: Error) => {
    throw e;
  };
  if (scenario === 'headless') {
    return createHeadlessEditor({
      namespace: 'conformance',
      nodes: DOCUMENT_NODES,
      onError,
    });
  }
  const scenarios: Record<string, AnyLexicalExtensionArgument[]> = {
    'custom-export': [
      configExtension(GetClipboardDataExtension, {
        $exportMimeType: CUSTOM_EXPORT,
      }),
    ],
    'dom-import': [
      RichTextExtension,
      CheckListExtension,
      LinkExtension,
      CodeExtension,
      ClipboardDOMImportExtension,
    ],
    'next-data': [
      configExtension(ClipboardImportExtension, {
        $importMimeType: {
          'text/html': [
            (html, _selection, $next) => $next(`<p><b>first</b></p>${html}`),
          ],
        },
      }),
    ],
    priority: [
      configExtension(ClipboardImportExtension, {
        priority: {'text/plain': 5},
      }),
    ],
  };
  return buildEditorFromExtensions(
    defineExtension({
      dependencies: scenarios[scenario],
      name: '[conformance]',
      nodes: DOCUMENT_NODES,
      onError,
    }),
  );
}

function dataTransferOf(entries: [string, string][]): DataTransfer {
  const data = new Map(entries);
  return {
    getData: (type: string) => data.get(type) ?? '',
    types: [...data.keys()],
  } as unknown as DataTransfer;
}

/**
 * Runs `c` (with no `selection`) at the selection `seed` picks among the
 * document's points in the case's editor (`0` for all of it), recorded as
 * `selection`.
 */
function runClipboardCase(
  c: Omit<ClipboardCase, 'selection'>,
  seed: number,
): ClipboardCase {
  const editor = clipboardEditor(c.scenario);
  const globals = globalThis as {DOMParser?: unknown};
  return withJSDOM(win => {
    const prevParser = globals.DOMParser;
    globals.DOMParser = win.DOMParser;
    try {
      editor.setEditorState(editor.parseEditorState(c.editorState));
      const points = editor.read($allPoints);
      const random = prng(seed);
      const pick = () => points[Math.floor(random() * points.length)];
      const selection: [PathPoint, PathPoint] =
        seed === 0 ? [points[0], points[points.length - 1]] : [pick(), pick()];
      if (c.data === undefined) {
        let clipboard: [string, string][] = [];
        editor.update(
          () => {
            $selectPaths(selection);
            clipboard = Object.entries($getClipboardDataFromSelection()).map(
              ([k, v]) => [k, v ?? ''],
            );
          },
          {discrete: true},
        );
        return {...c, clipboard, selection};
      }
      const dataTransfer = dataTransferOf(c.data);
      editor.update(
        () => {
          $selectPaths(selection);
          $insertDataTransferForRichText(
            dataTransfer,
            $getSelection()!,
            editor,
          );
        },
        {discrete: true},
      );
      return {
        ...c,
        after: editor.read($selectionPaths),
        selection,
        state: JSON.stringify(editor.getEditorState().toJSON()),
      };
    } finally {
      globals.DOMParser = prevParser;
      if ('dispose' in editor && typeof editor.dispose === 'function') {
        editor.dispose();
      }
    }
  });
}

/** Pastes with every importer's edge: equal plain text, bad JSON, URIs. */
const CLIPBOARD_PASTES: [string, string][][] = [
  [
    ['text/html', '<p>same</p>'],
    ['text/plain', '<p>same</p>'],
  ],
  [['text/html', '<h2>h</h2><ul><li>a</li></ul>']],
  [
    ['text/plain', 'plain\nlines'],
    ['text/html', '<p><i>rich</i> text</p>'],
  ],
  [
    ['application/x-lexical-editor', '{"namespace":"other","nodes":[]}'],
    ['text/html', '<blockquote>q</blockquote><p>p</p>'],
  ],
  [
    ['application/x-lexical-editor', 'not json'],
    ['text/plain', 'fallback'],
  ],
  [['text/uri-list', 'https://example.com']],
  [['text/html', '<a href="https://a.test">link</a> tail<pre>code</pre>']],
];

test('html/clipboard', {timeout: 120_000}, () => {
  const documents: string[] = JSON.parse(
    readFileSync(
      new URL('../vectors/property/documents.json', import.meta.url),
      'utf8',
    ),
  ).cases.map((c: {legacy: string}) => c.legacy);
  const cases: ClipboardCase[] = [];
  documents.forEach((editorState, i) => {
    for (const j of [0, 1]) {
      for (const scenario of j === 0
        ? ['headless']
        : ['headless', 'custom-export']) {
        cases.push(
          runClipboardCase(
            {
              editorState,
              name: `copy ${scenario} document ${i} selection ${j}`,
              scenario,
            },
            j === 0 ? i + 1 : 0,
          ),
        );
      }
    }
  });
  // Each document's copied HTML and text, pasted into the next document.
  documents.forEach((_, i) => {
    const copied = cases.find(
      c => c.name === `copy headless document ${i} selection 0`,
    )!.clipboard!;
    const data = copied.filter(([k]) => k !== 'application/x-lexical-editor');
    const target = (i + 1) % documents.length;
    for (const scenario of ['headless', 'dom-import']) {
      cases.push(
        runClipboardCase(
          {
            data,
            editorState: documents[target],
            name: `paste ${scenario} document ${i} into ${target}`,
            scenario,
          },
          target + 1,
        ),
      );
    }
  });
  CLIPBOARD_PASTES.forEach((data, i) => {
    for (const scenario of [
      'headless',
      'dom-import',
      'priority',
      'next-data',
    ]) {
      for (const target of [0, 7]) {
        cases.push(
          runClipboardCase(
            {
              data,
              editorState: documents[target],
              name: `paste ${scenario} data ${i} into ${target}`,
              scenario,
            },
            target + 1,
          ),
        );
      }
    }
  });
  emitVectors(
    'html/clipboard',
    'html-clipboard',
    "@lexical/clipboard's HTML paths over the property/documents documents, at seeded selections given as child-index paths: copies ($getClipboardDataFromSelection with $getHtmlContent in jsdom, and GetClipboardDataExtension with custom handlers), and pastes ($insertDataTransferForRichText of each document's copied HTML and text into the next document, and of curated payloads) with the default importers, ClipboardDOMImportExtension over the package extensions, a ClipboardImportExtension priority and a text/html handler that calls $next with other HTML.",
    cases,
  );
});

const MARK_IDS = ['a', 'b', 'c d', 'é'];
const MARK_THEME: EditorThemeClasses = {
  mark: 'mark-a mark-b',
  markOverlap: 'overlap',
};

/** Paragraphs of text, marks with zero to three ids, nested marks and links. */
function $randomMarkDocument(random: () => number): void {
  const int = (n: number) => Math.floor(random() * n);
  const pick = <T>(items: readonly T[]): T => items[int(items.length)];
  const inline = (depth: number): LexicalNode => {
    const kind = int(depth > 1 ? 3 : 6);
    if (kind === 0) {
      return $createTextNode(pick(TEXTS) || 'x').setFormat(
        pick([0, 0, ...FORMATS]),
      );
    } else if (kind === 1) {
      return $createLineBreakNode();
    } else if (kind === 2) {
      return $createTabNode();
    }
    const ids = MARK_IDS.filter(() => random() < 0.4);
    const parent =
      kind === 5
        ? $createLinkNode('https://x.test/' + depth)
        : $createMarkNode(ids);
    if (random() < 0.2) {
      parent.setIndent(1 + int(2)).setDirection(pick(['ltr', 'rtl'] as const));
    }
    const count = 1 + int(3);
    for (let c = 0; c < count; c++) {
      parent.append(inline(depth + 1));
    }
    return parent;
  };
  const paragraphs = 1 + int(3);
  for (let p = 0; p < paragraphs; p++) {
    const paragraph = $createParagraphNode();
    const count = 1 + int(4);
    for (let c = 0; c < count; c++) {
      paragraph.append(inline(0));
    }
    $getRoot().append(paragraph);
  }
}

test('html/export-mark', () => {
  const cases: Case[] = [];
  for (let seed = 1; seed <= 30; seed++) {
    for (const theme of [undefined, MARK_THEME]) {
      const random = prng(seed);
      const editor = createHeadlessEditor({
        nodes: [MarkNode, LinkNode],
        onError: e => {
          throw e;
        },
        theme: theme && JSON.parse(JSON.stringify(theme)),
      });
      editor.update(() => $randomMarkDocument(random), {discrete: true});
      const html = withJSDOM(() =>
        editor.read(() => $generateHtmlFromNodes(editor)),
      );
      cases.push({
        editorState: JSON.stringify(editor.getEditorState().toJSON()),
        html,
        name: `seed ${seed}${theme ? ' themed' : ''}`,
        ...(theme ? {theme} : {}),
      });
    }
  }
  emitVectors(
    'html/export-mark',
    'html-export',
    "Documents of MarkNodes (zero to three ids, nested, around and inside links, some indented), with and without a theme's mark and markOverlap classes, and the HTML $generateHtmlFromNodes writes for them in jsdom, which leaves the marks out (MarkNode.excludeFromCopy('html')) and keeps their children.",
    cases,
  );
});
