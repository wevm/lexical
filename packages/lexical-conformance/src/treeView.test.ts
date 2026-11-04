/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {generateContent} from '@lexical/devtools-core';
import {createHeadlessEditor} from '@lexical/headless';
import {
  $createHeadingNode,
  $createQuoteNode,
  HeadingNode,
  QuoteNode,
} from '@lexical/rich-text';
import {
  $createLineBreakNode,
  $createNodeSelection,
  $createParagraphNode,
  $createTabNode,
  $createTextNode,
  $getRoot,
  $isElementNode,
  $isTextNode,
  $setSelection,
  type LexicalEditor,
  type LexicalNode,
} from 'lexical';
import {test} from 'vitest';

import {emitVectors} from './vectors';

type Case = {
  name: string;
  /** `JSON.stringify(editorState.toJSON())`, selection excluded. */
  editorState: string;
  /** The selection, set after the state is parsed, as text node paths. */
  selection:
    | null
    | {
        type: 'range';
        anchor: {path: number[]; offset: number; type: 'text' | 'element'};
        focus: {path: number[]; offset: number; type: 'text' | 'element'};
        format?: number;
        style?: string;
      }
    | {type: 'node'; paths: number[][]};
  editable: boolean;
  /**
   * The node keys in the order the tree prints them (root excluded), so a
   * port whose keys differ can map them.
   */
  keys: string[];
  /** `LexicalEditor.version` in JS; the Swift port prints its own. */
  version: string;
  /** `generateContent(editor, [], false)`: the playground's tree view. */
  tree: string;
};

type Builder = () => void;

/** Documents built like the playground's: rich text nodes, formats, breaks. */
const DOCUMENTS: Record<string, Builder> = {
  breaksAndTabs: () => {
    $getRoot().append(
      $createParagraphNode().append(
        $createTextNode('one'),
        $createLineBreakNode(),
        $createTabNode(),
        $createTextNode('two\tthree'),
      ),
      $createHeadingNode('h3').append($createTextNode('x')),
    );
  },
  empty: () => {
    $getRoot().append($createParagraphNode());
  },
  formatted: () => {
    const p = $createParagraphNode();
    p.append(
      $createTextNode('Hello '),
      $createTextNode('bold').setFormat('bold'),
      $createTextNode(' and '),
      $createTextNode('all').setFormat(
        1 | 2 | 4 | 8 | 16 | 32 | 64 | 128, // every inline format
      ),
    );
    $getRoot().append(
      $createHeadingNode('h1').append($createTextNode('Title')),
      p,
      $createQuoteNode().append($createTextNode('A quote')),
    );
  },
  modesAndDetails: () => {
    $getRoot().append(
      $createParagraphNode().append(
        $createTextNode('token').setMode('token'),
        $createTextNode('segmented').setMode('segmented'),
        $createTextNode('plain'),
        $createTextNode('unmergeable').toggleUnmergeable(),
        $createTextNode('directionless').toggleDirectionless(),
      ),
    );
  },
  nested: () => {
    $getRoot().append(
      $createQuoteNode().append(
        $createTextNode('a'),
        $createTextNode('b').setFormat('italic'),
      ),
      $createParagraphNode().append($createTextNode('c')),
      $createQuoteNode(),
    );
  },
  paragraphFormat: () => {
    const p = $createParagraphNode().setTextFormat(1 | 2);
    p.setStyle('color: red');
    $getRoot().append(
      p.append($createTextNode('styled').setFormat(3).setStyle('color: red')),
      $createParagraphNode(),
    );
  },
};

const SELECTIONS: Record<string, Case['selection']> = {
  acrossBlocks: {
    anchor: {offset: 2, path: [0, 0], type: 'text'},
    focus: {offset: 1, path: [1, 0], type: 'text'},
    format: 1,
    style: 'color: blue',
    type: 'range',
  },
  backward: {
    anchor: {offset: 3, path: [0, 0], type: 'text'},
    focus: {offset: 1, path: [0, 0], type: 'text'},
    type: 'range',
  },
  caretStart: {
    anchor: {offset: 0, path: [0, 0], type: 'text'},
    focus: {offset: 0, path: [0, 0], type: 'text'},
    type: 'range',
  },
  element: {
    anchor: {offset: 0, path: [0], type: 'element'},
    focus: {offset: 0, path: [0], type: 'element'},
    type: 'range',
  },
  node: {paths: [[0, 0]], type: 'node'},
  none: null,
  withinText: {
    anchor: {offset: 1, path: [0, 0], type: 'text'},
    focus: {offset: 3, path: [0, 0], type: 'text'},
    type: 'range',
  },
};

function $nodeAt(path: number[]) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let node: any = $getRoot();
  for (const index of path) {
    node = node.getChildAtIndex(index);
    if (node === null) {
      return null;
    }
  }
  return node;
}

function $applySelection(selection: Case['selection']): boolean {
  if (selection === null) {
    $setSelection(null);
    return true;
  }
  if (selection.type === 'node') {
    const nodes = selection.paths.map($nodeAt);
    if (nodes.some(n => n === null)) {
      return false;
    }
    const sel = $createNodeSelection();
    for (const node of nodes) {
      sel.add(node.getKey());
    }
    $setSelection(sel);
    return true;
  }
  const anchor = $nodeAt(selection.anchor.path);
  const focus = $nodeAt(selection.focus.path);
  if (anchor === null || focus === null) {
    return false;
  }
  for (const [point, node] of [
    [selection.anchor, anchor],
    [selection.focus, focus],
  ] as const) {
    const size =
      point.type === 'text'
        ? $isTextNode(node)
          ? node.getTextContentSize()
          : -1
        : $isElementNode(node)
          ? node.getChildrenSize()
          : -1;
    if (size < 0 || point.offset > size) {
      return false;
    }
    if (point.type === 'text' && node.getType() !== 'text') {
      return false;
    }
  }
  const sel = anchor.select(0, 0);
  sel.anchor.set(
    anchor.getKey(),
    selection.anchor.offset,
    selection.anchor.type,
  );
  sel.focus.set(focus.getKey(), selection.focus.offset, selection.focus.type);
  if (selection.format !== undefined) {
    sel.format = selection.format;
  }
  if (selection.style !== undefined) {
    sel.style = selection.style;
  }
  return true;
}

function makeEditor(): LexicalEditor {
  return createHeadlessEditor({
    namespace: 'playground',
    nodes: [HeadingNode, QuoteNode],
    onError: e => {
      throw e;
    },
  });
}

test('tree view vectors', () => {
  const cases: Case[] = [];
  for (const [docName, build] of Object.entries(DOCUMENTS)) {
    for (const [selName, selection] of Object.entries(SELECTIONS)) {
      for (const editable of selName === 'none' ? [true, false] : [true]) {
        const builder = makeEditor();
        builder.update(build, {discrete: true});
        const editorState = JSON.stringify(builder.getEditorState().toJSON());
        // Parsed, as the port does, so the keys follow the tree's order.
        const editor = makeEditor();
        editor.setEditorState(editor.parseEditorState(editorState));
        let applied = true;
        editor.update(
          () => {
            applied = $applySelection(selection);
          },
          {discrete: true},
        );
        if (!applied) {
          continue;
        }
        editor.setEditable(editable);
        const keys: string[] = [];
        editor.read(() => {
          const visit = (node: LexicalNode) => {
            for (const child of $isElementNode(node)
              ? node.getChildren()
              : []) {
              keys.push(child.getKey());
              visit(child);
            }
          };
          visit($getRoot());
        });
        cases.push({
          editable,
          editorState,
          keys,
          name: `${docName} / ${selName}${editable ? '' : ' / read-only'}`,
          selection,
          tree: generateContent(editor, [], false),
          version: String(
            (editor.constructor as {version?: string}).version ?? '',
          ),
        });
      }
    }
  }
  emitVectors(
    'devtools/tree',
    'devtools-tree',
    "generateContent(editor, [], false) from @lexical/devtools-core: the playground's tree view of the editor state, its selection and the editor's flags. The selection is set after the state is parsed; Swift prints its own LexicalEditor.version in place of `version`.",
    cases,
  );
});
