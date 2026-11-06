/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

// Property-based cases for the Swift port: random inputs drawn with
// @lexical/fast-check from the serialization schemas the upstream nodes
// declare on `$config`, run through the real upstream code. JS's answers are
// the expected values; nothing here is written by hand or computed by Swift.
// property.test.ts writes them to vectors/property/*.json from fixed seeds,
// and propertyNightly.test.ts runs fresh seeds against Swift directly.

import {CodeExtension, CodeHighlightNode, CodeNode} from '@lexical/code-core';
import {buildEditorFromExtensions} from '@lexical/extension';
import {nodeArbitrary} from '@lexical/fast-check';
import {HashtagExtension, HashtagNode} from '@lexical/hashtag';
import {createHeadlessEditor} from '@lexical/headless';
import {HistoryExtension} from '@lexical/history';
import {$generateHtmlFromNodes} from '@lexical/html';
import {
  AutoLinkNode,
  LinkExtension,
  LinkNode,
  TOGGLE_LINK_COMMAND,
} from '@lexical/link';
import {
  CheckListExtension,
  INSERT_CHECK_LIST_COMMAND,
  INSERT_ORDERED_LIST_COMMAND,
  INSERT_UNORDERED_LIST_COMMAND,
  ListExtension,
  ListItemNode,
  ListNode,
  REMOVE_LIST_COMMAND,
} from '@lexical/list';
import {$convertToMarkdownString, TRANSFORMERS} from '@lexical/markdown';
import {OverflowNode} from '@lexical/overflow';
import {
  $createHeadingNode,
  $createQuoteNode,
  HeadingNode,
  QuoteNode,
  RichTextExtension,
} from '@lexical/rich-text';
import {$setBlocksType} from '@lexical/selection';
import * as fc from 'fast-check';
import {JSDOM} from 'jsdom';
import {
  $createParagraphNode,
  $createRangeSelection,
  $getRoot,
  $getSelection,
  $isElementNode,
  $isTextNode,
  $setSelection,
  configExtension,
  CONTROLLED_TEXT_INSERTION_COMMAND,
  defineExtension,
  DELETE_CHARACTER_COMMAND,
  DELETE_LINE_COMMAND,
  DELETE_WORD_COMMAND,
  type ElementNode,
  FORMAT_ELEMENT_COMMAND,
  FORMAT_TEXT_COMMAND,
  INDENT_CONTENT_COMMAND,
  INSERT_LINE_BREAK_COMMAND,
  INSERT_PARAGRAPH_COMMAND,
  type Klass,
  type LexicalCommand,
  type LexicalEditor,
  type LexicalNode,
  LineBreakNode,
  OUTDENT_CONTENT_COMMAND,
  ParagraphNode,
  REDO_COMMAND,
  REMOVE_TEXT_COMMAND,
  resetRandomKey,
  RootNode,
  TabNode,
  TextNode,
  UNDO_COMMAND,
} from 'lexical';

import {$selectionSnapshot, type Json, type PointSpec} from './ops';

type Serialized = {[key: string]: unknown};

// --- Documents --------------------------------------------------------------

/** Every node a generated document may contain. */
export const DOCUMENT_NODES: Klass<LexicalNode>[] = [
  HeadingNode,
  QuoteNode,
  ListNode,
  ListItemNode,
  LinkNode,
  AutoLinkNode,
  CodeNode,
  CodeHighlightNode,
  OverflowNode,
  HashtagNode,
];

/**
 * Text content. The schema's own `stringValue` domain is printable ASCII
 * (fast-check's default unit); this widens it with whole graphemes beyond
 * ASCII and with the characters markdown and HTML export treat specially.
 * Lone surrogates are left out: a Swift String can't hold one (swift/DESIGN.md,
 * the Tier 1 exception).
 */
const TEXT = fc.oneof(
  {arbitrary: fc.string({maxLength: 8}), weight: 3},
  {arbitrary: fc.string({maxLength: 4, unit: 'grapheme'}), weight: 1},
  {
    arbitrary: fc.constantFrom(
      'a',
      'b c',
      '😀',
      'é',
      '*',
      '**',
      '_',
      '`',
      '#',
      '~~',
      '<b>',
      '&amp;',
      '\n',
      '\t',
      ' ',
      'https://example.com',
    ),
    weight: 2,
  },
);

/**
 * Inline CSS. A random string is mostly not a declaration, which both sides
 * drop; `name: value` with an invalid value is a known difference (jsdom
 * drops it, the Swift DOM keeps it; swift/DESIGN.md, the DOM), so random
 * strings here have no `:` and declarations come from a list.
 */
const STYLE = fc.oneof(
  fc.string({maxLength: 6}).map(s => s.replaceAll(':', '')),
  fc
    .array(
      fc.constantFrom(
        'color: red',
        'background-color: #fff',
        'font-size: 12px',
        'font-weight: bold',
        'font-family: Arial',
        'text-decoration: underline',
        'white-space: pre-wrap',
        'COLOR:BLUE',
        'margin: 0 auto',
      ),
      {maxLength: 3, minLength: 1},
    )
    .map(declarations => declarations.join('; ')),
);

/**
 * Overrides for the numeric properties, used by `tame` documents (the start
 * of command sequences): small, realistic values instead of the schemas' full
 * domains, so the commands, not the numbers, are what a case exercises.
 */
const TAME: {[key: string]: fc.Arbitrary<unknown>} = {
  detail: fc.constantFrom(0, 0, 1, 2),
  format: fc.oneof(fc.nat(31), fc.constantFrom(0, 0, 0)),
  indent: fc.nat(2),
  start: fc.integer({max: 5, min: 1}),
  textFormat: fc.constantFrom(0, 0, 1, 2),
  value: fc.integer({max: 5, min: 1}),
};

/**
 * Integer properties held in a Swift `Int`: the bit flags (32-bit, as JS's
 * bitwise operators read them), list numbers and indents (kept to 32 bits,
 * where JS's doubles and Swift's Ints compute alike). Their schemas accept any
 * finite number, which JS stores and writes back as is; Swift can't hold a
 * fraction there (swift/DESIGN.md, integer properties), so generated numbers
 * are made integers in range. Aliases (`format: 'bold'`) stay as drawn.
 */
const INT32_KEYS = new Set([
  'detail',
  'format',
  'textFormat',
  'start',
  'value',
]);
const INTEGER_KEYS = new Set(['indent']);

function integral(key: string, value: unknown): unknown {
  if (typeof value !== 'number') {
    return value;
  }
  if (INT32_KEYS.has(key)) {
    return value | 0;
  }
  if (INTEGER_KEYS.has(key)) {
    // Indents are non-negative; 32 bits keeps `indent × 40` px exact in
    // JS's doubles.
    return Math.min(2 ** 31 - 1, Math.trunc(value) || 0);
  }
  return value;
}

/** The schema-derived properties of `klass`, with `text` widened to TEXT. */
function props(klass: Klass<LexicalNode>, tame: boolean) {
  const overrides: {[key: string]: fc.Arbitrary<unknown>} = {
    style: STYLE,
    text: TEXT,
  };
  if (tame) {
    Object.assign(overrides, TAME);
  }
  return fc
    .tuple(
      nodeArbitrary(klass) as fc.Arbitrary<Serialized>,
      fc.record(overrides),
    )
    .map(([generated, override]) => {
      const p: Serialized = {};
      for (const key of Object.keys(generated)) {
        p[key] = integral(key, generated[key]);
      }
      for (const key of Object.keys(override)) {
        // Only properties fast-check chose to include: absence is in the
        // domain too.
        if (key in p) {
          p[key] = override[key];
        }
      }
      return p;
    });
}

function leaf(klass: Klass<LexicalNode>, tame: boolean) {
  return props(klass, tame).map(p => ({
    ...p,
    type: klass.getType(),
    version: 1,
  }));
}

function element(
  klass: Klass<LexicalNode>,
  children: fc.Arbitrary<Serialized[]>,
  tame: boolean,
) {
  return fc.tuple(props(klass, tame), children).map(([p, c]) => ({
    ...p,
    children: c,
    type: klass.getType(),
    version: 1,
  }));
}

/**
 * A random serialized editor state over DOCUMENT_NODES, shaped the way the
 * editor builds them (blocks under the root, inline nodes in blocks, items
 * in lists), with every node's own properties drawn by `nodeArbitrary`.
 */
export function documentArbitrary(tame: boolean): fc.Arbitrary<Serialized> {
  const text = leaf(TextNode, tame);
  const textLike = fc.oneof(
    {arbitrary: text, weight: 6},
    {arbitrary: leaf(HashtagNode, tame), weight: 1},
    {arbitrary: leaf(TabNode, tame), weight: 1},
    {arbitrary: leaf(LineBreakNode, tame), weight: 1},
  );
  const linkChildren = fc.array(text, {maxLength: 2});
  const inline = fc.oneof(
    {arbitrary: textLike, weight: 8},
    {arbitrary: element(LinkNode, linkChildren, tame), weight: 1},
    {arbitrary: element(AutoLinkNode, linkChildren, tame), weight: 1},
    {
      arbitrary: element(
        OverflowNode,
        fc.array(textLike, {maxLength: 2}),
        tame,
      ),
      weight: 1,
    },
  );
  const inlines = fc.array(inline, {maxLength: 4});
  const code = element(
    CodeNode,
    fc.array(
      fc.oneof(
        {arbitrary: leaf(CodeHighlightNode, tame), weight: 4},
        {arbitrary: leaf(LineBreakNode, tame), weight: 1},
        {arbitrary: leaf(TabNode, tame), weight: 1},
      ),
      {maxLength: 4},
    ),
    tame,
  );
  const {list} = fc.letrec<{list: Serialized; item: Serialized}>(tie => ({
    item: element(
      ListItemNode,
      fc.oneof(
        {arbitrary: inlines, weight: 4},
        {
          arbitrary: fc.array(tie('list'), {maxLength: 1}),
          depthIdentifier: 'list',
          weight: 1,
        },
      ),
      tame,
    ),
    list: element(
      ListNode,
      fc.array(tie('item'), {maxLength: 3, minLength: 1}),
      tame,
    ),
  }));
  const block = fc.oneof(
    {arbitrary: element(ParagraphNode, inlines, tame), weight: 4},
    {arbitrary: element(HeadingNode, inlines, tame), weight: 2},
    {arbitrary: element(QuoteNode, inlines, tame), weight: 1},
    {arbitrary: list, weight: 2},
    {arbitrary: code, weight: 1},
  );
  return fc
    .tuple(props(RootNode, tame), fc.array(block, {maxLength: 4, minLength: 1}))
    .map(([p, children]) => ({
      root: {...p, children, type: 'root', version: 1},
    }));
}

export type DocumentRecord = {
  /** Parsing the input threw: nothing else is recorded. */
  error?: true;
  /** `JSON.stringify(editorState.toJSON())` of the parsed input. */
  legacy?: string;
  /** `JSON.stringify(editorState.toJSON(true))`. */
  compact?: string;
  /**
   * `[legacy, compact]` after parsing `legacy` and `compact` again, when
   * that isn't a fixed point (absent when it is).
   */
  reparsed?: [string, string][];
  /** `$getRoot().getTextContent()`. */
  text?: string;
  /** `$convertToMarkdownString(TRANSFORMERS)`, or null when it throws. */
  markdown?: string | null;
  /** `$generateHtmlFromNodes(editor)` in jsdom, or null when it throws. */
  html?: string | null;
};

export type DocumentCase = {name: string; input: string} & DocumentRecord;

function attempt<T>(f: () => T): T | null {
  try {
    return f();
  } catch {
    return null;
  }
}

/**
 * Sets a jsdom window as the global `window` and `document` (the DOM of the
 * upstream unit tests, as in html.test.ts) while `f` runs.
 */
export function withJSDOM<T>(f: () => T): T {
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

/** JS's answers for one serialized document. Call inside `withJSDOM`. */
export function runDocument(input: string): DocumentRecord {
  const editor = createHeadlessEditor({
    namespace: '',
    nodes: DOCUMENT_NODES,
    onError: e => {
      throw e;
    },
  });
  let state;
  try {
    state = editor.parseEditorState(input);
  } catch {
    return {error: true};
  }
  const legacy = JSON.stringify(state.toJSON());
  const compact = JSON.stringify(state.toJSON(true));
  const record: DocumentRecord = {compact, legacy};
  const reparsed = [legacy, compact].map((json): [string, string] => {
    const again = editor.parseEditorState(json);
    return [JSON.stringify(again.toJSON()), JSON.stringify(again.toJSON(true))];
  });
  if (reparsed.some(([l, c]) => l !== legacy || c !== compact)) {
    record.reparsed = reparsed;
  }
  editor.setEditorState(state);
  record.text = editor.read(() => $getRoot().getTextContent());
  record.markdown = attempt(() =>
    editor.read(() => $convertToMarkdownString(TRANSFORMERS)),
  );
  record.html = attempt(() =>
    editor.read(() => $generateHtmlFromNodes(editor)),
  );
  return record;
}

// --- Command sequences -------------------------------------------------------

/**
 * The commands a sequence may dispatch, by the name the vectors carry, with
 * the payloads each takes.
 */
function entry<T>(
  command: LexicalCommand<T>,
  payload: fc.Arbitrary<Json | undefined>,
): [LexicalCommand<unknown>, fc.Arbitrary<Json | undefined>] {
  return [command as LexicalCommand<unknown>, payload];
}

const COMMANDS: Record<
  string,
  [LexicalCommand<unknown>, fc.Arbitrary<Json | undefined>]
> = {
  CONTROLLED_TEXT_INSERTION_COMMAND: entry(
    CONTROLLED_TEXT_INSERTION_COMMAND,
    TEXT,
  ),
  DELETE_CHARACTER_COMMAND: entry(DELETE_CHARACTER_COMMAND, fc.boolean()),
  DELETE_LINE_COMMAND: entry(DELETE_LINE_COMMAND, fc.boolean()),
  DELETE_WORD_COMMAND: entry(DELETE_WORD_COMMAND, fc.boolean()),
  FORMAT_ELEMENT_COMMAND: entry(
    FORMAT_ELEMENT_COMMAND,
    fc.constantFrom('left', 'start', 'center', 'right', 'end', 'justify', ''),
  ),
  FORMAT_TEXT_COMMAND: entry(
    FORMAT_TEXT_COMMAND,
    fc.constantFrom(
      'bold',
      'italic',
      'underline',
      'strikethrough',
      'code',
      'subscript',
      'superscript',
      'highlight',
      'lowercase',
      'uppercase',
      'capitalize',
    ),
  ),
  INDENT_CONTENT_COMMAND: entry(INDENT_CONTENT_COMMAND, fc.constant(undefined)),
  INSERT_CHECK_LIST_COMMAND: entry(
    INSERT_CHECK_LIST_COMMAND,
    fc.constant(undefined),
  ),
  INSERT_LINE_BREAK_COMMAND: entry(INSERT_LINE_BREAK_COMMAND, fc.boolean()),
  INSERT_ORDERED_LIST_COMMAND: entry(
    INSERT_ORDERED_LIST_COMMAND,
    fc.constant(undefined),
  ),
  INSERT_PARAGRAPH_COMMAND: entry(
    INSERT_PARAGRAPH_COMMAND,
    fc.constant(undefined),
  ),
  INSERT_UNORDERED_LIST_COMMAND: entry(
    INSERT_UNORDERED_LIST_COMMAND,
    fc.constant(undefined),
  ),
  OUTDENT_CONTENT_COMMAND: entry(
    OUTDENT_CONTENT_COMMAND,
    fc.constant(undefined),
  ),
  REDO_COMMAND: entry(REDO_COMMAND, fc.constant(undefined)),
  REMOVE_LIST_COMMAND: entry(REMOVE_LIST_COMMAND, fc.constant(undefined)),
  REMOVE_TEXT_COMMAND: entry(REMOVE_TEXT_COMMAND, fc.constant(undefined)),
  TOGGLE_LINK_COMMAND: entry(
    TOGGLE_LINK_COMMAND,
    fc.oneof(
      fc.constant(null),
      fc.constantFrom('https://example.com', 'https://lexical.dev/'),
      fc.record(
        {
          rel: fc.constantFrom('noopener', 'nofollow'),
          target: fc.constantFrom('_blank', '_self'),
          title: fc.constantFrom('Example', ''),
          url: fc.constantFrom('https://example.com', 'mailto:a@b.c'),
        },
        {requiredKeys: ['url']},
      ),
    ),
  ),
  UNDO_COMMAND: entry(UNDO_COMMAND, fc.constant(undefined)),
};

const BLOCKS = ['paragraph', 'h1', 'h2', 'quote'] as const;

/** A selection point before it is resolved against the live document. */
type AbstractPoint = {i: number; o: number; element: boolean};

/** A step before its selection is resolved against the live document. */
export type AbstractStep = {
  anchor: AbstractPoint;
  focus: AbstractPoint | null;
} & ({command: string; payload?: Json} | {blocks: (typeof BLOCKS)[number]});

export type AbstractSequence = {document: Serialized; steps: AbstractStep[]};

export function sequenceArbitrary(
  maxSteps = 6,
): fc.Arbitrary<AbstractSequence> {
  const point: fc.Arbitrary<AbstractPoint> = fc.record({
    element: fc.oneof(
      {arbitrary: fc.constant(false), weight: 4},
      {arbitrary: fc.constant(true), weight: 1},
    ),
    i: fc.nat(40),
    o: fc.nat(40),
  });
  const operation = fc.oneof(
    {
      arbitrary: fc
        .constantFrom(...Object.keys(COMMANDS))
        .chain(command =>
          COMMANDS[command][1].map(payload =>
            payload === undefined ? {command} : {command, payload},
          ),
        ),
      weight: 8,
    },
    {
      arbitrary: fc.constantFrom(...BLOCKS).map(blocks => ({blocks})),
      weight: 1,
    },
  );
  const step = fc
    .tuple(point, fc.option(point, {freq: 2}), operation)
    .map(([anchor, focus, op]) => ({anchor, focus, ...op}) as AbstractStep);
  return fc.record({
    document: documentArbitrary(true),
    steps: fc.array(step, {maxLength: maxSteps, minLength: 1}),
  });
}

/** One step as the vectors carry it: concrete keys and offsets. */
export type SequenceStep = {
  anchor: PointSpec;
  focus: PointSpec;
} & ({command: string; payload?: Json} | {blocks: string});

export type SequenceStepResult = {
  /** The step threw; the editor kept its previous state. */
  error?: true;
  /** What `dispatchCommand` returned. */
  handled?: boolean;
  state: string;
  compact: string;
  selection: Json;
  text: string;
};

export type SequenceCase = {
  name: string;
  /** The starting editor state (legacy JSON), set with setEditorState. */
  document: string;
  steps: SequenceStep[];
  results: SequenceStepResult[];
};

/** The editor sequences run on: rich text with lists, links and history. */
export function sequenceEditor(): LexicalEditor {
  resetRandomKey();
  const editor = buildEditorFromExtensions(
    defineExtension({
      dependencies: [
        RichTextExtension,
        ListExtension,
        CheckListExtension,
        LinkExtension,
        HashtagExtension,
        CodeExtension,
        // No time-based merging, so history doesn't depend on the clock.
        // History disables itself where there is no `window` (Node); Swift
        // enables it (swift/DESIGN.md, History).
        configExtension(HistoryExtension, {delay: 0, disabled: false}),
      ],
      name: '[root]',
      namespace: '',
      nodes: () => [AutoLinkNode, OverflowNode],
      onError: e => {
        throw e;
      },
    }),
  );
  // The window of the upstream unit tests (jsdom), with no root element: the
  // deletion commands read its DOM selection, which has no `modify` there, so
  // they resolve in the model, as the Swift editor does without a view. A
  // headless JS editor has no window at all and throws instead.
  (editor as unknown as {_window: unknown})._window = globalThis.window;
  return editor;
}

/**
 * A point of the live document for `p`: the `i`th text node (or element,
 * when `p.element` or there is no text), with an offset in it that doesn't
 * split a surrogate pair (swift/DESIGN.md, the Tier 1 exception).
 */
function $resolve(p: AbstractPoint): PointSpec {
  const texts: TextNode[] = [];
  const elements: LexicalNode[] = [];
  const visit = (node: LexicalNode) => {
    if ($isTextNode(node)) {
      texts.push(node);
    } else if ($isElementNode(node)) {
      elements.push(node);
      node.getChildren().forEach(visit);
    }
  };
  visit($getRoot());
  if (p.element || texts.length === 0) {
    const e = elements[p.i % elements.length];
    const size = $isElementNode(e) ? e.getChildrenSize() : 0;
    return [e.getKey(), p.o % (size + 1), 'element'];
  }
  const t = texts[p.i % texts.length];
  const content = t.getTextContent();
  let offset = p.o % (content.length + 1);
  const code = content.charCodeAt(offset - 1);
  if (
    offset > 0 &&
    offset < content.length &&
    code >= 0xd800 &&
    code < 0xdc00
  ) {
    offset -= 1;
  }
  return [t.getKey(), offset, 'text'];
}

function $blockCreator(blocks: string): () => ElementNode {
  switch (blocks) {
    case 'quote':
      return () => $createQuoteNode();
    case 'h1':
    case 'h2':
      return () => $createHeadingNode(blocks);
    default:
      return () => $createParagraphNode();
  }
}

/**
 * Runs `sequence` on JS: sets its document, then for each step selects the
 * resolved points in one discrete update and dispatches the command outside
 * any update (as a key handler does), or runs `$setBlocksType` in an update.
 * Records the state after every step.
 */
export function runSequence(
  name: string,
  sequence: AbstractSequence,
): SequenceCase {
  return withJSDOM(() => runSequenceInWindow(name, sequence));
}

function runSequenceInWindow(
  name: string,
  sequence: AbstractSequence,
): SequenceCase {
  const editor = sequenceEditor();
  const document = JSON.stringify(sequence.document);
  editor.setEditorState(editor.parseEditorState(document), {
    tag: 'history-merge',
  });
  const steps: SequenceStep[] = [];
  const results: SequenceStepResult[] = [];
  for (const abstract of sequence.steps) {
    const [anchor, focus] = editor.read(() => [
      $resolve(abstract.anchor),
      abstract.focus === null
        ? $resolve(abstract.anchor)
        : $resolve(abstract.focus),
    ]);
    const step: SequenceStep =
      'blocks' in abstract
        ? {anchor, blocks: abstract.blocks, focus}
        : {anchor, command: abstract.command, focus, payload: abstract.payload};
    if ('payload' in step && step.payload === undefined) {
      delete step.payload;
    }
    steps.push(step);
    results.push(runStep(editor, step));
  }
  return {document, name, results, steps};
}

/** Runs one concrete step on `editor` and records it. */
export function runStep(
  editor: LexicalEditor,
  step: SequenceStep,
): SequenceStepResult {
  let error = false;
  let handled: boolean | undefined;
  try {
    editor.update(
      () => {
        const selection = $createRangeSelection();
        selection.anchor.set(...step.anchor);
        selection.focus.set(...step.focus);
        $setSelection(selection);
        if ('blocks' in step) {
          $setBlocksType($getSelection(), $blockCreator(step.blocks));
        }
      },
      {discrete: true},
    );
    if ('command' in step) {
      handled = editor.dispatchCommand(
        COMMANDS[step.command][0],
        step.payload as never,
      );
    }
  } catch {
    error = true;
  }
  // A dispatch outside an update commits in a microtask; `editor.read`
  // commits it first.
  const record = editor.read(() => ({
    selection: $selectionSnapshot(),
    text: $getRoot().getTextContent(),
  }));
  const state = editor.getEditorState();
  const result: SequenceStepResult = {
    compact: JSON.stringify(state.toJSON(true)),
    selection: record.selection,
    state: JSON.stringify(state.toJSON()),
    text: record.text,
  };
  if (handled !== undefined) {
    result.handled = handled;
  }
  if (error) {
    result.error = true;
  }
  return result;
}
