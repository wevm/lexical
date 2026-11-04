/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

// The JS side of swift/Benchmarks/LexicalBenchmarks: the same editor (rich
// text and history, with history's clock at 0), the same 5,000-block
// document, the same operations in the same order with the same counts, no
// warmup, and the same p50/p95 (the sample at index min(n - 1, n * p / 100)
// of the sorted samples). `node.ts` runs it headless (the model only);
// `browser.ts` runs it on a mounted contenteditable root, so each sample
// includes the DOM reconcile.

import {buildEditorFromExtensions} from '@lexical/extension';
import {HistoryExtension} from '@lexical/history';
import {
  $createHeadingNode,
  $createQuoteNode,
  RichTextExtension,
} from '@lexical/rich-text';
import {
  $createParagraphNode,
  $createTextNode,
  $getRoot,
  $getSelection,
  $isRangeSelection,
  $selectAll,
  configExtension,
  CONTROLLED_TEXT_INSERTION_COMMAND,
  defineExtension,
  DELETE_CHARACTER_COMMAND,
  type ElementNode,
  FORMAT_TEXT_COMMAND,
  INSERT_PARAGRAPH_COMMAND,
  type LexicalCommand,
  type LexicalEditor,
  REDO_COMMAND,
  UNDO_COMMAND,
} from 'lexical';

export const paragraphCount = 5_000;

export interface Result {
  name: string;
  p50: number;
  p95: number;
  target?: number;
}

export function makeEditor(): LexicalEditor & {dispose(): void} {
  return buildEditorFromExtensions(
    defineExtension({
      dependencies: [
        RichTextExtension,
        // History disables itself where there is no `window` (Node).
        configExtension(HistoryExtension, {disabled: false, now: () => 0}),
      ],
      name: '[benchmark]',
    }),
  );
}

/** Swift's `fixture()`: 5,000 blocks of prose, every tenth a heading and
 * every seventh a quote, each with plain, bold and italic runs. */
export function fixture(): string {
  const editor = makeEditor();
  editor.update(
    () => {
      const root = $getRoot().clear();
      for (let i = 0; i < paragraphCount; i++) {
        const block: ElementNode =
          i % 10 === 0
            ? $createHeadingNode('h2')
            : i % 7 === 0
              ? $createQuoteNode()
              : $createParagraphNode();
        block.append(
          $createTextNode(`Paragraph ${i} has some plain text, `),
          $createTextNode('a bold part').setFormat('bold'),
          $createTextNode(' and '),
          $createTextNode('an italic one').setFormat('italic'),
          $createTextNode(' before it ends.'),
        );
        root.append(block);
      }
    },
    {discrete: true},
  );
  const json = JSON.stringify(editor.getEditorState().toJSON());
  editor.dispose();
  return json;
}

/** FNV-1a (32-bit) of the UTF-8 bytes, which the Swift benchmark prints
 * too, so the two outputs show they ran on the same document. */
export function checksum(text: string): string {
  let h = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(text)) {
    h = Math.imul(h ^ byte, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0');
}

export function percentile(samples: number[], p: number): number {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[
    Math.min(sorted.length - 1, Math.floor((sorted.length * p) / 100))
  ];
}

export interface Host {
  /** A fresh editor, mounted on a root element in the browser. */
  editor(): LexicalEditor & {dispose(): void};
  /** Names rows the way the Swift benchmark does for this layer. */
  names: Record<
    'parse' | 'load' | 'insert' | 'delete' | 'enter' | 'join' | 'bold' | 'undo',
    string
  >;
  /** Checks the rendered document after a run, in the browser. */
  check?(editor: LexicalEditor): void;
  /** Without a DOM selection, `deleteCharacter` cannot measure the
   * character before a collapsed caret (it uses the native
   * `Selection.modify`), so a headless host extends the selection over it
   * in the model first, as the Swift model does, and the command then
   * removes the selected text. */
  extendInModel?: boolean;
  now(): number;
}

/** Runs the benchmarks in the Swift benchmark's order, with its counts. */
export function run(host: Host, json: string, only?: string): Result[] {
  const results: Result[] = [];
  const selected = (name: string) =>
    only === undefined || name.startsWith(only);
  const report = (name: string, samples: number[], target?: number) => {
    const result = {
      name,
      p50: percentile(samples, 50),
      p95: percentile(samples, 95),
      target,
    };
    results.push(result);
    return result;
  };

  const {names} = host;
  if (selected(names.parse) || selected(names.load)) {
    const parses: number[] = [];
    const loads: number[] = [];
    for (let i = 0; i < 10; i++) {
      const editor = host.editor();
      const start = host.now();
      const state = editor.parseEditorState(json);
      parses.push(host.now() - start);
      editor.setEditorState(state);
      loads.push(host.now() - start);
      if (host.check) {
        host.check(editor);
      }
      editor.dispose();
    }
    report(names.parse, parses);
    report(names.load, loads, 300);
  }

  const editor = host.editor();
  editor.setEditorState(editor.parseEditorState(json));
  const discrete = (fn: () => void) => editor.update(fn, {discrete: true});
  const caretAtEndOfMiddle = () =>
    discrete(() => {
      $getRoot()
        .getChildAtIndex(paragraphCount / 2)!
        .selectEnd();
    });
  const bench = (
    name: string,
    count: number,
    body: () => void,
    target?: number,
  ) => {
    if (!selected(name)) {
      return;
    }
    const samples: number[] = [];
    for (let i = 0; i < count; i++) {
      const start = host.now();
      body();
      samples.push(host.now() - start);
    }
    report(name, samples, target);
  };
  const command =
    <T>(type: LexicalCommand<T>, payload: T) =>
    () =>
      discrete(() => {
        editor.dispatchCommand(type, payload);
      });

  const middleText = () =>
    editor.read(() =>
      $getRoot()
        .getChildAtIndex(paragraphCount / 2)!
        .getTextContent(),
    );
  const blockCount = () => editor.read(() => $getRoot().getChildrenSize());
  const expect = (condition: boolean, message: string) => {
    if (!condition) {
      throw new Error(`benchmark check failed: ${message}`);
    }
  };
  const original = middleText();

  caretAtEndOfMiddle();
  bench(names.insert, 200, command(CONTROLLED_TEXT_INSERTION_COMMAND, 'a'), 8);
  // The deletes remove what the inserts typed, so they stay in one paragraph.
  if (!selected(names.insert)) {
    for (let i = 0; i < 200; i++) {
      command(CONTROLLED_TEXT_INSERTION_COMMAND, 'a')();
    }
  }
  caretAtEndOfMiddle();
  bench(
    names.delete,
    200,
    host.extendInModel
      ? () =>
          discrete(() => {
            const selection = $getSelection();
            if ($isRangeSelection(selection) && selection.isCollapsed()) {
              const {focus} = selection;
              if (focus.type === 'text' && focus.offset > 0) {
                focus.set(focus.key, focus.offset - 1, 'text');
              }
            }
            editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true);
          })
      : command(DELETE_CHARACTER_COMMAND, true),
    8,
  );
  if (selected(names.delete)) {
    expect(
      middleText() === original,
      `the deletes remove what was typed: ${JSON.stringify(middleText().slice(-60))} vs ${JSON.stringify(original.slice(-60))}`,
    );
  }
  caretAtEndOfMiddle();
  bench(names.enter, 100, command(INSERT_PARAGRAPH_COMMAND, undefined), 8);
  if (selected(names.enter)) {
    expect(blockCount() === paragraphCount + 100, 'Enter adds a block');
  }
  bench(names.join, 100, command(DELETE_CHARACTER_COMMAND, true), 8);
  if (selected(names.enter) && selected(names.join)) {
    expect(blockCount() === paragraphCount, 'Backspace joins the blocks');
  }
  if (host.check) {
    host.check(editor);
  }

  bench(names.bold, 10, () =>
    discrete(() => {
      $selectAll();
      editor.dispatchCommand(FORMAT_TEXT_COMMAND, 'bold');
    }),
  );
  bench(names.undo, 10, () => {
    editor.dispatchCommand(UNDO_COMMAND, undefined);
    editor.dispatchCommand(REDO_COMMAND, undefined);
    // A command dispatched outside an update may commit in a microtask;
    // the forcing read commits it within the sample.
    editor.read(() => {});
  });
  if (selected(names.undo)) {
    // The samples are this fast only if undo restores a recorded state
    // rather than doing nothing.
    const serialized = () => JSON.stringify(editor.getEditorState().toJSON());
    const redone = serialized();
    editor.dispatchCommand(UNDO_COMMAND, undefined);
    editor.read(() => {});
    expect(serialized() !== redone, 'undo changes the document');
    editor.dispatchCommand(REDO_COMMAND, undefined);
    editor.read(() => {});
    expect(serialized() === redone, 'redo restores it');
  }
  if (host.check) {
    host.check(editor);
  }
  editor.dispose();
  return results;
}
