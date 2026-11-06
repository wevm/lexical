/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {
  editorStateFromSerializedDocument,
  type SerializedDocument,
  serializedDocumentFromEditorState,
} from '@lexical/file';
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
  $createTextNode,
  $getRoot,
} from 'lexical';
import {test} from 'vitest';

import {emitVectors} from './vectors';

type ExportCase = {
  name: string;
  /** The editor state the document is made from, `JSON.stringify(toJSON())`. */
  editorState: string;
  /** `serializedDocumentFromEditorState`'s config; absent keys are left out. */
  config: {source?: string; lastSaved?: number};
  /** `JSON.stringify(serializedDocumentFromEditorState(state, config))`. */
  document: string;
};

type ImportCase = {
  name: string;
  /** The text of a `.lexical` file. */
  document: string;
  /** `JSON.stringify(editorStateFromSerializedDocument(editor, document).toJSON())`. */
  editorState: string;
};

const newEditor = () =>
  createHeadlessEditor({
    nodes: [ListNode, ListItemNode],
    onError: e => {
      throw e;
    },
  });

const documents: Record<string, () => void> = {
  'empty paragraph': () => {
    $getRoot().append($createParagraphNode());
  },
  'formatted text and a list': () => {
    $getRoot().append(
      $createParagraphNode().append(
        $createTextNode('Hello ').toggleFormat('bold'),
        $createTextNode('"world" ✨').toggleFormat('italic'),
        $createLineBreakNode(),
        $createTextNode('line two'),
      ),
      $createListNode('check').append(
        $createListItemNode(true).append($createTextNode('done')),
        $createListItemNode(false).append($createTextNode('todo')),
      ),
    );
  },
};

// The `.lexical` file format of @lexical/file: the exact text JS writes for
// a document, with the fixed `lastSaved`, `source` and `version` handling
// (an empty source is "Lexical"; `lastSaved` is written as given, so these
// cases pass one: 0 or none would be `Date.now()`), and the editor state JS
// reads back from a document.
test('file/documents', () => {
  const configs: Record<string, ExportCase['config']> = {
    'default source': {lastSaved: 1762401787158},
    'empty source': {lastSaved: 1762401787158, source: ''},
    'fractional lastSaved': {lastSaved: 1.5, source: 'Playground'},
    'named source': {lastSaved: 1762401787158, source: 'Playground'},
  };
  const exportCases: ExportCase[] = [];
  for (const [documentName, build] of Object.entries(documents)) {
    const editor = newEditor();
    editor.update(build, {discrete: true});
    const state = editor.getEditorState();
    for (const [configName, config] of Object.entries(configs)) {
      exportCases.push({
        config,
        document: JSON.stringify(
          serializedDocumentFromEditorState(state, config),
        ),
        editorState: JSON.stringify(state.toJSON()),
        name: `${documentName}, ${configName}`,
      });
    }
  }
  emitVectors(
    'file/export',
    'file',
    'serializedDocumentFromEditorState: the exact text of a .lexical document for an editor state and a config.',
    exportCases,
  );

  // Documents as other tools write them: another source and version, a
  // compact editor state, and keys in another order.
  const editor = newEditor();
  editor.update(documents['formatted text and a list'], {discrete: true});
  const state = editor.getEditorState();
  const files: Record<string, string> = {
    // SerializedDocument's type has no compact editor state, but
    // editorStateFromSerializedDocument parses one.
    'compact editor state': JSON.stringify({
      editorState: state.toJSON(true),
      lastSaved: 1,
      source: 'Lexical',
      version: '0.53.0',
    }),
    'other source and version': JSON.stringify({
      editorState: state.toJSON(),
      lastSaved: 1700000000000,
      source: 'Playground',
      version: '0.12.0',
    } satisfies SerializedDocument),
    'reordered keys': JSON.stringify({
      version: '0.53.0',
      // eslint-disable-next-line sort-keys-fix/sort-keys-fix
      source: 'Lexical',
      // eslint-disable-next-line sort-keys-fix/sort-keys-fix
      lastSaved: 1,
      // eslint-disable-next-line sort-keys-fix/sort-keys-fix
      editorState: state.toJSON(),
    }),
  };
  const importCases: ImportCase[] = Object.entries(files).map(
    ([name, document]) => ({
      document,
      editorState: JSON.stringify(
        editorStateFromSerializedDocument(newEditor(), document).toJSON(),
      ),
      name,
    }),
  );
  emitVectors(
    'file/import',
    'file',
    'editorStateFromSerializedDocument: the editor state JS reads from the text of a .lexical document.',
    importCases,
  );
});
