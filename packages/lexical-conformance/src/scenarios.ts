/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

/**
 * Tier 2 scenarios: upstream Playwright specs (packages/lexical-playground/
 * __tests__/e2e) translated into key presses, keeping their names. Each runs
 * on an editor built like the native views' (rich or plain text, with
 * history) that starts with one empty, selected paragraph, as the
 * playground's editor is after `focusEditor`. Keys are written as a person
 * presses them on a Mac (Playwright chord syntax, `Meta` for Command):
 * `__tests__/webkit/scenarios.test.ts` records what WebKit makes of them, and the
 * Swift runner replays the same keys through the native input mapping.
 */

export type ScenarioStep =
  /** Typed text, one key per character, as `page.keyboard.type`. */
  | {type: string}
  /** A key chord, as `page.keyboard.press`, `times` times in a row. */
  | {press: string; times?: number};

export type ScenarioMode = 'rich' | 'plain';

export type Scenario = {
  /** `<spec file name> > <test title>`, with the describe titles. */
  name: string;
  modes: ScenarioMode[];
  steps: ScenarioStep[];
};

const both: ScenarioMode[] = ['rich', 'plain'];

export const TEXT_ENTRY: Scenario[] = [
  {
    modes: both,
    name: "TextEntry > Can type 'Hello Lexical' in the editor",
    steps: [{type: 'Hello Lexical'}],
  },
  {
    modes: ['rich'],
    name: 'TextEntry > Can insert a paragraph between two text nodes',
    steps: [
      {type: 'Hello '},
      {press: 'Meta+b'},
      {type: 'world'},
      {press: 'ArrowLeft', times: 5},
      {press: 'Enter'},
    ],
  },
  {
    modes: both,
    name: "TextEntry > Can type 'Hello Lexical' in the editor and replace it with foo",
    steps: [{type: 'Hello Lexical'}, {press: 'Meta+a'}, {type: 'Foo'}],
  },
  {
    modes: both,
    name: "TextEntry > Can type 'Hello Lexical' in the editor and replace it with an empty space",
    steps: [{type: 'Hello Lexical'}, {press: 'Meta+a'}, {type: ' '}],
  },
  {
    modes: both,
    name: "TextEntry > Can delete characters after they're typed",
    steps: [
      {type: 'Delete some of these characters.'},
      {press: 'Backspace', times: 3},
    ],
  },
  {
    modes: both,
    name: 'TextEntry > Can type characters, and select and replace a part',
    steps: [
      {type: 'Hello foobar.'},
      {press: 'ArrowLeft', times: 7},
      {press: 'Shift+ArrowRight', times: 3},
      {type: 'lol'},
    ],
  },
  {
    modes: both,
    name: 'TextEntry > Can select and delete a word',
    steps: [
      {type: 'Delete some of these characters.'},
      {press: 'Alt+Shift+ArrowLeft'},
      {press: 'Backspace'},
    ],
  },
  {
    modes: both,
    name: 'TextEntry > Paragraphed text entry and selection',
    steps: [
      {type: 'Hello World.'},
      {press: 'Enter'},
      {type: 'This is another block.'},
      {press: 'Shift+ArrowLeft', times: 6},
      {type: 'paragraph.'},
      {type: ' :)'},
    ],
  },
  {
    modes: both,
    name: 'TextEntry > First paragraph backspace handling',
    steps: [
      {type: '  '},
      {press: 'Enter'},
      {press: 'ArrowUp'},
      {press: 'Backspace'},
    ],
  },
  {
    modes: both,
    name: 'TextEntry > Mix of paragraphs and break points',
    steps: [
      {press: 'Shift+Enter', times: 3},
      {press: 'ArrowUp', times: 3},
      {press: 'Enter'},
      {press: 'ArrowUp'},
      {type: '\u0647\u064E'},
    ],
  },
  {
    modes: both,
    name: 'TextEntry > Empty paragraph and new line node selection',
    steps: [
      {press: 'Enter'},
      {press: 'ArrowLeft'},
      {press: 'ArrowRight'},
      {press: 'ArrowLeft'},
      {press: 'Delete'},
      {press: 'Shift+Enter'},
      {press: 'ArrowLeft'},
      {press: 'Delete'},
    ],
  },
];

export const SCENARIO_FILES: {name: string; scenarios: Scenario[]}[] = [
  {name: 'text-entry', scenarios: TEXT_ENTRY},
];

/**
 * The keys a Linux WebKit needs for a native caret move that a Mac makes
 * with `chord`. Playwright's WebKit uses the GTK key bindings on Linux, so
 * the Cocoa word and line moves (Option and Command with an arrow) are
 * pressed as their GTK equivalents. Chords that Lexical handles itself on
 * an Apple platform (Command-B, Command-Z, Option-Backspace, …) go through
 * unchanged, since `navigator.platform` is a Mac's.
 */
export function linuxWebKitChord(chord: string): string {
  const keys = chord.split('+');
  const key = keys[keys.length - 1];
  const modifiers = new Set(keys.slice(0, -1));
  const shift = modifiers.has('Shift') ? 'Shift+' : '';
  const isArrow = key.startsWith('Arrow');
  if (!isArrow) {
    return chord;
  }
  if (modifiers.has('Alt') && (key === 'ArrowLeft' || key === 'ArrowRight')) {
    return `Control+${shift}${key}`;
  }
  if (modifiers.has('Meta')) {
    switch (key) {
      case 'ArrowLeft':
        return `${shift}Home`;
      case 'ArrowRight':
        return `${shift}End`;
      case 'ArrowUp':
        return `Control+${shift}Home`;
      case 'ArrowDown':
        return `Control+${shift}End`;
    }
  }
  return chord;
}
