/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import {buildEditorFromExtensions} from '@lexical/extension';
import {HistoryExtension} from '@lexical/history';
import {PlainTextExtension} from '@lexical/plain-text';
import {RichTextExtension} from '@lexical/rich-text';
import {
  $createParagraphNode,
  $getRoot,
  $getSelection,
  $isNodeSelection,
  $isRangeSelection,
  configExtension,
  defineExtension,
  type LexicalEditor,
  type LexicalNode,
  type PointType,
} from 'lexical';
import {expect, test} from 'vitest';
import {commands} from 'vitest/browser';

import {
  linuxWebKitChord,
  type Scenario,
  SCENARIO_FILES,
  type ScenarioMode,
  type ScenarioStep,
} from '../../scenarios';

declare module 'vitest/browser' {
  interface BrowserCommands {
    keyboardPress: (chord: string) => Promise<void>;
    keyboardType: (text: string) => Promise<void>;
    emitScenarioVectors: (
      name: string,
      description: string,
      cases: unknown[],
    ) => Promise<{expected: string; actual: string} | null>;
  }
}

type ScenarioPoint = {path: number[]; offset: number; type: string};

type ScenarioSelection =
  | null
  | {
      type: 'range';
      anchor: ScenarioPoint;
      focus: ScenarioPoint;
      format: number;
      style: string;
    }
  | {nodes: number[][]; type: 'node'};

type Snapshot = {state: unknown; selection: ScenarioSelection};

type ScenarioCase = {
  name: string;
  mode: ScenarioMode;
  steps: ScenarioStep[];
  /** The editor after each step. */
  after: Snapshot[];
};

function $path(node: LexicalNode): number[] {
  const path: number[] = [];
  let current: LexicalNode = node;
  for (let parent = current.getParent(); parent; parent = current.getParent()) {
    path.unshift(current.getIndexWithinParent());
    current = parent;
  }
  return path;
}

function $point(point: PointType): ScenarioPoint {
  return {offset: point.offset, path: $path(point.getNode()), type: point.type};
}

function snapshot(editor: LexicalEditor): Snapshot {
  return editor.read(() => {
    const selection = $getSelection();
    return {
      selection: $isRangeSelection(selection)
        ? {
            anchor: $point(selection.anchor),
            focus: $point(selection.focus),
            format: selection.format,
            style: selection.style,
            type: 'range',
          }
        : $isNodeSelection(selection)
          ? {nodes: selection.getNodes().map($path), type: 'node'}
          : null,
      state: editor.getEditorState().toJSON(),
    };
  });
}

/** Lets the browser and the editor handle the last key. */
function settle(): Promise<void> {
  return new Promise(resolve =>
    requestAnimationFrame(() => setTimeout(resolve, 0)),
  );
}

async function record(
  scenario: Scenario,
  mode: ScenarioMode,
): Promise<ScenarioCase> {
  const editor = buildEditorFromExtensions(
    defineExtension({
      dependencies: [
        mode === 'rich' ? RichTextExtension : PlainTextExtension,
        // Every step merges into the same history entry unless something
        // else separates it, wherever the steps run.
        configExtension(HistoryExtension, {now: () => 0}),
      ],
      name: '[scenario]',
    }),
  );
  const root = document.createElement('div');
  root.contentEditable = 'true';
  root.style.whiteSpace = 'pre-wrap';
  document.body.append(root);
  try {
    editor.setRootElement(root);
    editor.update(
      () => {
        const paragraph = $createParagraphNode();
        $getRoot().clear().append(paragraph);
        paragraph.select();
      },
      {discrete: true},
    );
    root.focus();
    await settle();
    const after: Snapshot[] = [];
    for (const step of scenario.steps) {
      if ('type' in step) {
        await commands.keyboardType(step.type);
      } else {
        for (let i = 0; i < (step.times ?? 1); i++) {
          await commands.keyboardPress(linuxWebKitChord(step.press));
        }
      }
      await settle();
      after.push(snapshot(editor));
    }
    return {after, mode, name: scenario.name, steps: scenario.steps};
  } finally {
    editor.dispose();
    root.remove();
  }
}

for (const file of SCENARIO_FILES) {
  test(file.name, async () => {
    const cases: ScenarioCase[] = [];
    for (const scenario of file.scenarios) {
      for (const mode of scenario.modes) {
        cases.push(await record(scenario, mode));
      }
    }
    const result = await commands.emitScenarioVectors(
      `scenarios/${file.name}`,
      'Upstream playground specs as macOS key presses, and the editor (state and selection, nodes as child-index paths from the root) after each step, recorded in WebKit.',
      cases,
    );
    if (result) {
      expect(result.actual, 'stale; run `pnpm run generate-scenarios`').toBe(
        result.expected,
      );
    }
  });
}
