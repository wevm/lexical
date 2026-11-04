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
  | {press: string; times?: number}
  /**
   * The e2e helper `advanceHistoryClock`: history's clock moves past its
   * merge delay (plus 50ms) and stays there, so the next change starts a
   * new undo entry.
   */
  | {advanceHistoryClock: true}
  /**
   * The playground toolbar's block type menu (`formatParagraph`,
   * `formatHeading`, `formatQuote`): `$setBlocksType` on the selection, and
   * for a heading `$patchStyleText` clearing `font-size`.
   */
  | {block: ScenarioBlockType};

export type ScenarioBlockType =
  | 'paragraph'
  | 'h1'
  | 'h2'
  | 'h3'
  | 'h4'
  | 'h5'
  | 'h6'
  | 'quote';

export type ScenarioMode = 'rich' | 'plain';

/**
 * Extensions a scenario's editor adds to the views' set, for specs that
 * depend on a playground plugin with a Lexical extension equivalent.
 */
export type ScenarioExtension = 'tab-indentation';

export type Scenario = {
  /** `<spec file name> > <test title>`, with the describe titles. */
  name: string;
  modes: ScenarioMode[];
  steps: ScenarioStep[];
  extensions?: ScenarioExtension[];
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

const advance: ScenarioStep = {advanceHistoryClock: true};
const undo: ScenarioStep = {press: 'Meta+z'};
const redo: ScenarioStep = {press: 'Meta+Shift+z'};

export const HISTORY: Scenario[] = [
  {
    modes: both,
    name: 'History > Can type two paragraphs of text and correctly undo and redo',
    steps: [
      advance,
      {type: 'hello'},
      advance,
      {type: ' world'},
      {press: 'Enter'},
      {type: 'hello world again'},
      {press: 'ArrowLeft', times: 6},
      {type: ', again and'},
      undo,
      undo,
      undo,
      undo,
      undo,
      redo,
      redo,
      redo,
      redo,
      redo,
      {press: 'Backspace', times: 4},
      undo,
    ],
  },
  {
    modes: ['rich'],
    name: 'History > Can coalesce when switching inline styles (#1151)',
    steps: [
      advance,
      {press: 'Meta+b'},
      {type: 'foo'},
      {press: 'Meta+b'},
      {type: 'bar'},
      {press: 'Meta+b'},
      {type: 'baz'},
      undo,
      undo,
      undo,
      redo,
      redo,
      redo,
    ],
  },
];

const bold: ScenarioStep = {press: 'Meta+b'};
const italic: ScenarioStep = {press: 'Meta+i'};

// TextFormatting.spec.mjs requires rich text. Its toolbar (font size and
// family) and decorator (#2523) tests are out of scope.
export const TEXT_FORMATTING: Scenario[] = [
  {
    modes: ['rich'],
    name: 'TextFormatting > Can create bold text using the shortcut',
    steps: [{type: 'Hello'}, bold, {type: ' World'}, bold, {type: '!'}],
  },
  {
    modes: ['rich'],
    name: 'TextFormatting > Can select multiple text parts and format them with shortcuts',
    steps: [
      {type: 'Hello world!'},
      {press: 'ArrowLeft'},
      {press: 'Shift+ArrowLeft', times: 5},
      bold,
      {press: 'ArrowLeft'},
      {press: 'ArrowRight'},
      {press: 'Shift+ArrowRight', times: 2},
      italic,
      bold,
      {press: 'ArrowLeft', times: 2},
      {press: 'Shift+ArrowRight', times: 5},
      bold,
      italic,
      italic,
    ],
  },
  {
    modes: ['rich'],
    name: 'TextFormatting > Can insert range of formatted text and select part and replace with character',
    steps: [
      {type: '123'},
      bold,
      {type: '456'},
      bold,
      {type: '789'},
      {press: 'Shift+Enter'},
      {type: 'abc'},
      bold,
      {type: 'def'},
      bold,
      {type: 'ghi'},
      {press: 'ArrowUp'},
      {press: 'Meta+ArrowLeft'},
      {press: 'ArrowRight', times: 2},
      {press: 'Shift+ArrowDown'},
      {press: 'Shift+ArrowRight', times: 8},
      {type: 'z'},
    ],
  },
  {
    modes: ['rich'],
    name: 'TextFormatting > Regression #2439: can format backwards when at first text node boundary',
    steps: [
      {type: '123456'},
      {press: 'ArrowLeft', times: 3},
      {press: 'Shift+ArrowLeft', times: 3},
      bold,
      {press: 'Meta+ArrowRight'},
      {press: 'Shift+ArrowLeft', times: 4},
      bold,
      bold,
    ],
  },
  {
    modes: ['rich'],
    name: 'TextFormatting > The active state of the button in the toolbar should to be displayed correctly',
    steps: [
      {type: 'A'},
      {press: 'Enter'},
      {type: 'B'},
      {press: 'Shift+ArrowLeft', times: 3},
      bold,
      italic,
    ],
  },
  {
    modes: ['rich'],
    name: 'TextFormatting > Multiline selection format ignores new lines',
    steps: [
      {type: 'Fist'},
      {press: 'Enter'},
      {press: 'Meta+u'},
      {type: 'Second'},
      {press: 'ArrowLeft', times: 7},
      {press: 'Shift+ArrowRight', times: 7},
    ],
  },
  // The playground's TextFormatting browser test
  // (src/__tests__/browser/TextFormatting.test.ts), its core shortcuts.
  {
    modes: ['rich'],
    name: 'TextFormatting > Can create italic text using the shortcut',
    steps: [
      {type: 'Hello'},
      {press: 'Meta+i'},
      {type: ' World'},
      {press: 'Meta+i'},
      {type: '!'},
    ],
  },
  ...(
    [
      ['boldify', 'Meta+b'],
      ['italicify', 'Meta+i'],
    ] as const
  ).map(
    ([verb, chord]): Scenario => ({
      modes: ['rich'],
      name: `TextFormatting > Can select text and ${verb} it with the shortcut`,
      steps: [
        {type: 'Hello world!'},
        {press: 'ArrowLeft'},
        {press: 'Shift+ArrowLeft', times: 5},
        {press: chord},
        {press: chord},
      ],
    }),
  ),
];

const threeLines: ScenarioStep[] = [
  {type: 'One'},
  {press: 'Enter'},
  {type: 'Two'},
  {press: 'Enter', times: 2},
  {type: 'Three'},
];

/** The keyboard-only tests of Selection.spec.mjs (Mac key bindings). */
export const SELECTION: Scenario[] = [
  {
    modes: ['rich'],
    name: 'Selection > can delete text by line backwards with CMD+delete',
    steps: [
      ...threeLines,
      {press: 'Meta+Backspace'},
      {press: 'Backspace'},
      {press: 'Meta+Backspace'},
      {press: 'Meta+Backspace'},
      {press: 'Backspace'},
      {press: 'Meta+Backspace'},
    ],
  },
  ...(
    [
      ['opt+CMD+delete', 'Meta+Delete'],
      ['control+K', 'Control+k'],
    ] as const
  ).map(
    ([title, chord]): Scenario => ({
      modes: ['rich'],
      name: `Selection > can delete text by line forwards with ${title}`,
      steps: [
        ...threeLines,
        {press: 'ArrowUp', times: 2},
        {press: chord, times: 4},
        {press: 'Meta+ArrowUp'},
        {press: chord, times: 4},
      ],
    }),
  ),
  {
    modes: ['plain'],
    name: 'Selection > Can delete at boundary #4221',
    steps: [
      {type: 'aaa'},
      {press: 'Enter'},
      {type: 'b'},
      {press: 'Enter'},
      {type: 'c'},
      {press: 'Shift+ArrowLeft', times: 3},
      {press: 'Delete'},
      {press: 'Shift+ArrowLeft'},
      {press: 'Delete'},
    ],
  },
  {
    modes: ['rich'],
    name: 'Selection > Backspace at the start of the first paragraph keeps content that follows a blank text node',
    steps: [
      {type: '  '},
      {press: 'Meta+b'},
      {type: 'hello'},
      {press: 'Enter'},
      {press: 'Meta+b'},
      {type: 'world'},
      {press: 'Meta+ArrowUp'},
      {press: 'Backspace'},
    ],
  },
  {
    modes: ['rich'],
    name: 'Selection > Can persist the text format from the paragraph',
    steps: [
      {press: 'Meta+b'},
      {type: 'Line1'},
      {press: 'Enter', times: 3},
      {type: 'Line2'},
      {press: 'ArrowUp'},
      {type: 'Line3'},
    ],
  },
  {
    modes: ['rich'],
    name: 'Selection > toggle format at the start of paragraph to a different format persists the format',
    steps: [
      {press: 'Meta+b'},
      {type: 'Line1'},
      {press: 'Enter', times: 3},
      {press: 'Meta+i'},
      {type: 'Line2'},
      {press: 'ArrowUp'},
      {press: 'Meta+b'},
      {type: 'Line3'},
    ],
  },
  {
    modes: ['rich'],
    name: 'Selection > formatting is persisted after deleting all nodes from the paragraph node',
    steps: [
      {press: 'Meta+b'},
      {type: 'Line1'},
      {press: 'Enter'},
      {press: 'Meta+b'},
      {type: 'Line2'},
      {press: 'Alt+Shift+ArrowLeft'},
      {press: 'Backspace'},
      {type: 'Line3'},
    ],
  },
];

function typeParagraphs(paragraphs: string[]): ScenarioStep[] {
  return paragraphs.flatMap((text, i): ScenarioStep[] =>
    i > 0 ? [{press: 'Enter'}, {type: text}] : [{type: text}],
  );
}

/** Each press its own step, so every caret position is compared. */
function presses(chord: string, times: number): ScenarioStep[] {
  return Array.from({length: times}, () => ({press: chord}));
}

const longParagraphs = typeParagraphs([
  'Lorem Ipsum is simply dummy text of the printing and typesetting industry.',
  'It has survived not only five centuries, but also the leap into electronic typesetting, remaining essentially unchanged. ',
  'It was popularised in the 1960s with the release of Letraset sheets containing lorem ipsum passages.',
]);

/**
 * Navigation.spec.mjs and the browser test it defers to
 * (lexical-playground/src/__tests__/browser/Navigation.test.ts, "native
 * navigation"). That test fills the editor with an update; typing the same
 * paragraphs gives the same document and caret. Its root is 1400px wide,
 * as the recording's is, so no paragraph wraps.
 */
export const NAVIGATION: Scenario[] = [
  {
    modes: both,
    name: 'Navigation > can type and navigate several paragraphs',
    steps: [
      ...typeParagraphs(['one', 'two', 'three']),
      {press: 'Meta+ArrowUp'},
      {press: 'Meta+ArrowDown'},
    ],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can type several paragraphs',
    steps: typeParagraphs([
      'First paragraph.',
      'Second paragraph.',
      'Third paragraph.',
    ]),
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can move to the top of the editor',
    steps: [...longParagraphs, {press: 'Meta+ArrowUp'}],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can move to the bottom of the editor',
    steps: [
      ...longParagraphs,
      {press: 'Meta+ArrowUp'},
      {press: 'Meta+ArrowDown'},
    ],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can move to the beginning of the current line, then back to the end of the current line',
    steps: [
      ...longParagraphs,
      {press: 'Meta+ArrowLeft'},
      {press: 'Meta+ArrowRight'},
    ],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can move one word to the right',
    steps: [
      ...longParagraphs,
      {press: 'Meta+ArrowUp'},
      {press: 'Alt+ArrowRight'},
    ],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can move to the beginning of the previous word',
    steps: [...longParagraphs, ...presses('Alt+ArrowLeft', 2)],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can move to the top of the editor, then to the bottom of the current paragraph',
    steps: [
      ...longParagraphs,
      {press: 'Meta+ArrowUp'},
      {press: 'Alt+ArrowDown'},
    ],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can move to the beginning of the current paragraph',
    steps: [...longParagraphs, {press: 'Alt+ArrowUp'}],
  },
  {
    modes: both,
    name: 'Navigation > native navigation > can navigate through the plain text word by word',
    steps: [
      {type: '  123 abc 456  def  '},
      ...presses('Alt+ArrowLeft', 5),
      ...presses('Alt+ArrowRight', 5),
    ],
  },
  {
    modes: ['rich'],
    name: 'Navigation > native navigation > can navigate through the formatted text word by word',
    steps: [
      {type: '  123 abc 456  def  '},
      {press: 'Alt+ArrowLeft'},
      ...presses('Shift+ArrowRight', 2),
      {press: 'Meta+b'},
      ...presses('Alt+ArrowLeft', 3),
      ...presses('Shift+ArrowRight', 2),
      {press: 'Meta+b'},
      {press: 'Meta+ArrowRight'},
      ...presses('Alt+ArrowLeft', 5),
      ...presses('Alt+ArrowRight', 5),
    ],
  },
];

/** Tab.spec.mjs, with the playground's TabIndentationPlugin. */
export const TAB: Scenario[] = [
  {
    extensions: ['tab-indentation'],
    modes: ['rich'],
    name: 'Tab > can go to start of line after a tab character',
    steps: [{type: 'Foo'}, {press: 'Tab'}, {press: 'Meta+ArrowLeft'}],
  },
];

/**
 * Headings/*.spec.mjs: what Enter and Backspace do to a heading made with the
 * toolbar's block type menu.
 */
export const HEADINGS: Scenario[] = [
  {
    modes: ['rich'],
    name: 'HeadingsBackspaceAtStart > Headings - stays as a heading when you backspace at the start of a heading with no previous sibling nodes present',
    steps: [
      {block: 'h1'},
      {type: 'Welcome to the playground'},
      {press: 'Meta+ArrowUp'},
      {press: 'Backspace'},
    ],
  },
  {
    modes: ['rich'],
    name: 'HeadingsBackspaceAtStart > Headings - removes only the empty previous paragraph and preserves heading on backspace at start (#4359)',
    steps: [
      {press: 'Enter'},
      {block: 'h1'},
      {type: 'Welcome to the playground'},
      {press: 'Meta+ArrowLeft'},
      {press: 'Backspace'},
    ],
  },
  {
    modes: ['rich'],
    name: 'HeadingsEnterAtEnd > Headings - changes to a paragraph when you press enter at the end of a heading',
    steps: [
      {block: 'h1'},
      {type: 'Welcome to the playground'},
      {press: 'Enter'},
    ],
  },
  {
    modes: ['rich'],
    name: 'HeadingsEnterInMiddle > Headings - stays as a heading when you press enter in the middle of a heading',
    steps: [
      {block: 'h1'},
      {type: 'Welcome to the playground'},
      {press: 'Meta+ArrowUp'},
      {press: 'ArrowRight', times: 5},
      {press: 'Enter'},
    ],
  },
];

/**
 * SelectAllDelete.spec.mjs (#5835): select all and delete leaves one empty
 * paragraph. The spec makes its heading or quote with a markdown shortcut
 * ("# ", "> "); markdown is not in v0.1.0, so the block type menu makes it
 * here. The list case waits for `LexicalList`.
 */
export const SELECT_ALL_DELETE: Scenario[] = [
  {
    modes: ['rich'],
    name: 'SelectAllDelete > Select all + delete clears the editor (#5835) > a heading, with Backspace',
    steps: [
      {block: 'h1'},
      {type: 'Just a heading'},
      {press: 'Meta+a'},
      {press: 'Backspace'},
    ],
  },
  {
    modes: ['rich'],
    name: 'SelectAllDelete > Select all + delete clears the editor (#5835) > a heading, with forward Delete',
    steps: [
      {block: 'h1'},
      {type: 'Just a heading'},
      {press: 'Meta+a'},
      {press: 'Delete'},
    ],
  },
  {
    modes: ['rich'],
    name: 'SelectAllDelete > Select all + delete clears the editor (#5835) > a quote, with forward Delete',
    steps: [
      {block: 'quote'},
      {type: 'Just a quote'},
      {press: 'Meta+a'},
      {press: 'Delete'},
    ],
  },
  {
    modes: ['rich'],
    name: 'SelectAllDelete > Select all + delete clears the editor (#5835) > a heading, with delete-by-word',
    steps: [
      {block: 'h1'},
      {type: 'Just a heading'},
      {press: 'Meta+a'},
      {press: 'Alt+Delete'},
    ],
  },
  {
    modes: ['rich'],
    name: 'SelectAllDelete > Select all + delete clears the editor (#5835) > delete-by-word from a caret keeps the heading',
    steps: [
      {block: 'h1'},
      {type: 'Heading'},
      {press: 'Meta+ArrowUp'},
      {press: 'Alt+Delete'},
    ],
  },
];

export const SCENARIO_FILES: {name: string; scenarios: Scenario[]}[] = [
  {name: 'text-entry', scenarios: TEXT_ENTRY},
  {name: 'history', scenarios: HISTORY},
  {name: 'text-formatting', scenarios: TEXT_FORMATTING},
  {name: 'selection', scenarios: SELECTION},
  {name: 'navigation', scenarios: NAVIGATION},
  {name: 'tab', scenarios: TAB},
  {name: 'headings', scenarios: HEADINGS},
  {name: 'select-all-delete', scenarios: SELECT_ALL_DELETE},
];

/**
 * The keys a Linux WebKit needs for a native caret move that a Mac makes
 * with `chord`. Playwright's WebKit uses the GTK key bindings on Linux, so
 * the Cocoa word and line moves (Option and Command with an arrow) are
 * pressed as their GTK equivalents, and the paragraph moves (Option with
 * up or down) as Home and End. Chords that Lexical handles itself on
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
  if (modifiers.has('Alt') && !modifiers.has('Meta')) {
    // GTK has no paragraph move. Scenario lines never wrap, so Home and End
    // (which the upstream tests press off Apple platforms) land where
    // Option-Up and Option-Down do, from inside a paragraph.
    return `${shift}${key === 'ArrowUp' ? 'Home' : 'End'}`;
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
