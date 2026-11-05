/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

// Headless: the model only, as the Swift benchmark's `model:` rows measure
// it (no view, no reconcile).

import {checksum, fixture, makeEditor, paragraphCount, run} from './harness';

/** Deleting a character first measures the move with the DOM selection of
 * the editor's window (`$extendSelectionForDeletion`), and throws without a
 * window. Node has no DOM selection (`CAN_USE_DOM` is false), so with any
 * window object the deletion takes upstream's model path, as the Swift
 * model does. */
function headlessEditor() {
  const editor = makeEditor();
  (editor as unknown as {_window: object})._window = {};
  return editor;
}

export function runNode(only?: string) {
  const json = fixture();
  const results = run(
    {
      editor: headlessEditor,
      extendInModel: true,
      names: {
        bold: 'model: select all and toggle bold',
        delete: 'model: delete a character',
        enter: 'model: split a paragraph (Enter)',
        insert: 'model: insert a character',
        join: 'model: join paragraphs (Backspace at start)',
        load: 'model: load (parse and setEditorState)',
        parse: 'model: parseEditorState',
        undo: 'model: undo and redo the last change',
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
