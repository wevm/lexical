/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

// Full stack in a browser: every editor is mounted on a contenteditable
// root in the page, so a discrete update's sample runs through the DOM
// reconcile (the DOM is updated when it returns; layout is not forced, as
// the Swift benchmark's storage does no layout).

import type {LexicalEditor} from 'lexical';

import {$getRoot} from 'lexical';

import {checksum, fixture, makeEditor, paragraphCount, run} from './harness';

function mountedEditor() {
  const editor = makeEditor();
  const root = document.createElement('div');
  root.contentEditable = 'true';
  document.body.replaceChildren(root);
  editor.setRootElement(root);
  return editor;
}

/** The DOM holds the model's text, so the reconcile really ran. */
function check(editor: LexicalEditor) {
  const root = editor.getRootElement()!;
  const blocks = editor.read(() => $getRoot().getChildrenSize());
  if (root.children.length !== blocks) {
    throw new Error(`DOM has ${root.children.length} blocks, model ${blocks}`);
  }
  const text = editor.read(() => $getRoot().getTextContent());
  if (root.textContent!.replace(/\n/g, '') !== text.replace(/\n/g, '')) {
    throw new Error('DOM text differs from the model');
  }
}

export function runBrowser(only?: string) {
  const json = fixture();
  const results = run(
    {
      check,
      editor: mountedEditor,
      names: {
        bold: 'select all and toggle bold',
        delete: 'keystroke: delete a character',
        enter: 'keystroke: split a paragraph (Enter)',
        insert: 'keystroke: insert a character',
        join: 'keystroke: join paragraphs (Backspace at start)',
        load: 'load: JSON to rendered storage',
        parse: 'load: parseEditorState',
        undo: 'undo and redo the last change',
      },
      now: () => performance.now(),
    },
    json,
    only,
  );
  return {
    fixture: `Fixture: ${paragraphCount} blocks, ${Math.floor(
      new TextEncoder().encode(json).length / 1024,
    )} KB of JSON, FNV-1a ${checksum(json)}`,
    results,
  };
}

(globalThis as {runBrowser?: typeof runBrowser}).runBrowser = runBrowser;
