/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {createHeadlessEditor} from '@lexical/headless';
import {
  $createListItemNode,
  $createListNode,
  ListItemNode,
  ListNode,
} from '@lexical/list';
import {
  $createLineBreakNode,
  $createParagraphNode,
  $createTabNode,
  $createTextNode,
  $getRoot,
} from 'lexical';
import {test} from 'vitest';

import {emitVectors} from './vectors';

type Case = {
  name: string;
  /** `JSON.stringify(editorState.toJSON())`: the legacy form, the default. */
  legacy: string;
  /** `JSON.stringify(editorState.toJSON(true))`: the compact form. */
  compact: string;
};

// Documents built from the core nodes alone, and the exact bytes JS writes for
// them in both forms. The Swift port must import each form and write the same
// bytes back.
test('serialization/core', () => {
  const documents: Record<string, () => void> = {
    'empty paragraph': () => {
      $getRoot().append($createParagraphNode());
    },
    'formatted text, line break, tab': () => {
      $getRoot().append(
        $createParagraphNode().append(
          $createTextNode('Hello ').toggleFormat('bold'),
          $createTextNode('world')
            .toggleFormat('italic')
            .setStyle('color: red'),
          $createLineBreakNode(),
          $createTabNode(),
          $createTextNode('UTF-16: 😀 é'),
        ),
      );
    },
    'indented, aligned paragraphs': () => {
      $getRoot().append(
        $createParagraphNode()
          .setIndent(2)
          .setFormat('center')
          .append($createTextNode('a')),
        $createParagraphNode()
          .setDirection('rtl')
          .append($createTextNode('שלום')),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/core',
    'serialization',
    'Core-node documents and the exact JSON text JS writes for them, legacy and compact.',
    cases,
  );
});

// Documents with @lexical/list nodes: list types, start, checked items and
// nesting (a list item's indent is derived from it).
test('serialization/list', () => {
  const $item = (text: string, checked?: boolean) =>
    $createListItemNode(checked).append($createTextNode(text));
  const documents: Record<string, () => void> = {
    'aligned item, empty item': () => {
      $getRoot().append(
        $createListNode('bullet').append(
          $item('centered').setFormat('center'),
          $createListItemNode(),
        ),
        $createParagraphNode(),
      );
    },
    'bullet list': () => {
      $getRoot().append(
        $createListNode('bullet').append($item('a'), $item('b')),
      );
    },
    checklist: () => {
      $getRoot().append(
        $createListNode('check').append($item('done', true), $item('todo')),
      );
    },
    'nested list': () => {
      const nested = $createListItemNode().append(
        $createListNode('bullet').append($item('inner')),
      );
      $getRoot().append(
        $createListNode('number').append(
          $item('outer'),
          nested,
          $item('after'),
        ),
      );
    },
    'numbered list from 3': () => {
      $getRoot().append(
        $createListNode('number', 3).append($item('one'), $item('two')),
      );
    },
  };
  const cases: Case[] = Object.entries(documents).map(([name, build]) => {
    const editor = createHeadlessEditor({
      nodes: [ListNode, ListItemNode],
      onError: e => {
        throw e;
      },
    });
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    return {
      compact: JSON.stringify(state.toJSON(true)),
      legacy: JSON.stringify(state.toJSON()),
      name,
    };
  });
  emitVectors(
    'serialization/list',
    'serialization',
    'Documents with @lexical/list nodes and the exact JSON text JS writes for them.',
    cases,
  );
});
