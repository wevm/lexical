/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {Action, HandlerSpec, Json, Script, Step} from './ops';

// The scripts of `vectors/scripts/events` and `vectors/scripts/events-apple`:
// the core's KEY_DOWN_COMMAND handler (`$handleKeyDown`, its compiled
// `buildKeyDownShortcuts` table) for every key of the table with each
// modifier combination, on the platform `isApple` names, and the commands
// those keys reach in rich text. Off Apple platforms (as Node is) they also
// cover `$handleBeforeInput` and the composition handlers. scripts.test.ts
// records them for Node's platform; scriptsApple.test.ts mocks `IS_APPLE`.

function t(text: string, extra: Record<string, Json> = {}): Json {
  return {
    detail: 0,
    format: 0,
    mode: 'normal',
    style: '',
    text,
    type: 'text',
    version: 1,
    ...extra,
  };
}

function p(...children: Json[]): Json {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    textFormat: 0,
    textStyle: '',
    type: 'paragraph',
    version: 1,
  };
}

function select(
  anchor: [string, number, 'text' | 'element'],
  focus?: [string, number, 'text' | 'element'],
): Action {
  return {anchor, do: 'select', focus};
}

function selectionChange(
  anchor: [string, number, 'text' | 'element'],
  focus?: [string, number, 'text' | 'element'],
): Action {
  return {anchor, do: 'selectionChange', focus};
}

/** The root gets these blocks, and the selection is set. */
function doc(selection: Action, ...blocks: Json[]): Step {
  return {
    update: [{do: 'append', key: 'root', nodes: blocks as never}, selection],
  };
}

type Modifiers = {
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

function keyDown(key: string, code: string, modifiers: Modifiers = {}): Step {
  return {dispatch: 'KEY_DOWN_COMMAND', payload: {code, key, ...modifiers}};
}

function dispatchIn(command: string, payload: Json): Action {
  return {command, do: 'dispatch', payload};
}

/** `[key, code]` of every key the keydown table binds, and some it doesn't. */
const KEYS: [string, string][] = [
  ['ArrowRight', 'ArrowRight'],
  ['ArrowLeft', 'ArrowLeft'],
  ['ArrowUp', 'ArrowUp'],
  ['ArrowDown', 'ArrowDown'],
  ['Enter', 'Enter'],
  [' ', 'Space'],
  ['Backspace', 'Backspace'],
  ['Escape', 'Escape'],
  ['Delete', 'Delete'],
  ['Tab', 'Tab'],
  ['Home', 'Home'],
  ['End', 'End'],
  ['a', 'KeyA'],
  ['b', 'KeyB'],
  ['c', 'KeyC'],
  ['d', 'KeyD'],
  ['h', 'KeyH'],
  ['i', 'KeyI'],
  ['k', 'KeyK'],
  ['o', 'KeyO'],
  ['u', 'KeyU'],
  ['x', 'KeyX'],
  ['y', 'KeyY'],
  ['z', 'KeyZ'],
  // Shift+Z reports the capital, which matches case-insensitively.
  ['Z', 'KeyZ'],
  ['q', 'KeyQ'],
  // A non-Latin layout: matched by `code` (KeyB, KeyA).
  ['и', 'KeyB'],
  ['ф', 'KeyA'],
  // A remapped Latin layout (Dvorak's `x` on KeyB): matched by `key` only.
  ['x', 'KeyB'],
  ['1', 'Digit1'],
  ['Shift', 'ShiftLeft'],
  ['', ''],
];

const MODIFIER_COMBINATIONS: Modifiers[] = [
  {},
  {shiftKey: true},
  {ctrlKey: true},
  {altKey: true},
  {metaKey: true},
  {ctrlKey: true, shiftKey: true},
  {metaKey: true, shiftKey: true},
  {altKey: true, shiftKey: true},
];

/** Every command a keydown or beforeinput can dispatch. */
const DISPATCHED = [
  'KEY_DOWN_COMMAND',
  'KEY_ARROW_RIGHT_COMMAND',
  'KEY_ARROW_LEFT_COMMAND',
  'KEY_ARROW_UP_COMMAND',
  'KEY_ARROW_DOWN_COMMAND',
  'MOVE_TO_START',
  'MOVE_TO_END',
  'KEY_ENTER_COMMAND',
  'KEY_SPACE_COMMAND',
  'KEY_BACKSPACE_COMMAND',
  'KEY_ESCAPE_COMMAND',
  'KEY_DELETE_COMMAND',
  'KEY_TAB_COMMAND',
  'KEY_MODIFIER_COMMAND',
  'SELECT_ALL_COMMAND',
  'COPY_COMMAND',
  'CUT_COMMAND',
  'PASTE_COMMAND',
  'DELETE_CHARACTER_COMMAND',
  'DELETE_WORD_COMMAND',
  'DELETE_LINE_COMMAND',
  'FORMAT_TEXT_COMMAND',
  'INSERT_LINE_BREAK_COMMAND',
  'INSERT_PARAGRAPH_COMMAND',
  'CONTROLLED_TEXT_INSERTION_COMMAND',
  'UNDO_COMMAND',
  'REDO_COMMAND',
];

/**
 * A handler for each command, before any other (critical priority), that
 * logs the command's payload and lets it go on.
 */
const LOGGERS: HandlerSpec[] = DISPATCHED.map(command => ({
  command,
  id: command,
  priority: 4,
  result: false,
}));

/** The scripts of the platform `isApple` names. */
export function eventScripts(isApple: boolean): Script[] {
  const platform = {isApple};
  const primary: Modifiers = isApple ? {metaKey: true} : {ctrlKey: true};
  const scripts: Script[] = [];

  // The table: each key, with each combination of modifiers, in an editor
  // with only the core's handlers, at a range selection (where the copy and
  // cut keys are left to the platform).
  for (const [key, code] of KEYS) {
    scripts.push({
      ...platform,
      handlers: LOGGERS,
      name: `keydown ${JSON.stringify(key)} (${code})`,
      snapshot: 'final',
      steps: [
        doc(select(['2', 1, 'text'], ['2', 3, 'text']), p(t('Hello'))),
        ...MODIFIER_COMBINATIONS.map(modifiers =>
          keyDown(key, code, modifiers),
        ),
      ],
    });
  }

  scripts.push(
    {
      ...platform,
      handlers: LOGGERS,
      name: 'copy and cut keys at a node selection dispatch the commands with the key event',
      steps: [
        doc({do: 'selectNodes', keys: ['1']}, p(t('Hello'))),
        keyDown('c', 'KeyC', primary),
        keyDown('x', 'KeyX', primary),
        keyDown('c', 'KeyC', {...primary, shiftKey: true}),
      ],
    },
    {
      ...platform,
      handlers: LOGGERS,
      name: 'copy and cut keys without a selection are left to the platform',
      steps: [
        {
          update: [
            {do: 'append', key: 'root', nodes: [p(t('Hello'))] as never},
          ],
        },
        keyDown('c', 'KeyC', primary),
        keyDown('x', 'KeyX', primary),
      ],
    },
    {
      ...platform,
      handlers: LOGGERS,
      history: {delay: 0},
      name: 'rich text: formatting keys, undo and redo',
      richText: true,
      steps: [
        doc(select(['2', 0, 'text'], ['2', 5, 'text']), p(t('Hello world'))),
        {...keyDown('b', 'KeyB', primary), now: 1000},
        {...keyDown('i', 'KeyI', primary), now: 2000},
        {...keyDown('u', 'KeyU', primary), now: 3000},
        {...keyDown('z', 'KeyZ', primary), now: 4000},
        {...keyDown('z', 'KeyZ', primary), now: 5000},
        {...keyDown('Z', 'KeyZ', {...primary, shiftKey: true}), now: 6000},
        {...keyDown('y', 'KeyY', primary), now: 7000},
        {...keyDown('и', 'KeyB', primary), now: 8000},
      ],
    },
    {
      ...platform,
      handlers: LOGGERS,
      name: 'rich text: Enter, Shift+Enter and select all',
      richText: true,
      steps: [
        doc(select(['2', 5, 'text']), p(t('Hello world'))),
        keyDown('Enter', 'Enter'),
        keyDown('Enter', 'Enter', {shiftKey: true}),
        keyDown('Enter', 'Enter', {altKey: true}),
        keyDown('o', 'KeyO', {ctrlKey: true}),
        keyDown('a', 'KeyA', primary),
      ],
    },
    {
      ...platform,
      handlers: LOGGERS,
      name: 'rich text: deletion keys at a range selection',
      richText: true,
      steps: [
        doc(
          select(['2', 0, 'text'], ['2', 2, 'text']),
          p(t('abcdefghijklmnop')),
        ),
        // Ctrl+H, Ctrl+D and Ctrl+K on Apple platforms.
        keyDown('h', 'KeyH', {ctrlKey: true}),
        {update: [select(['2', 0, 'text'], ['2', 2, 'text'])]},
        keyDown('d', 'KeyD', {ctrlKey: true}),
        {update: [select(['2', 0, 'text'], ['2', 2, 'text'])]},
        keyDown('k', 'KeyK', {ctrlKey: true}),
        {update: [select(['2', 0, 'text'], ['2', 2, 'text'])]},
        // Word deletion (Option on Apple platforms, Ctrl elsewhere).
        keyDown(
          'Backspace',
          'Backspace',
          isApple ? {altKey: true} : {ctrlKey: true},
        ),
        {update: [select(['2', 0, 'text'], ['2', 2, 'text'])]},
        keyDown('Delete', 'Delete', isApple ? {altKey: true} : {ctrlKey: true}),
        {update: [select(['2', 0, 'text'], ['2', 2, 'text'])]},
        // Line deletion (⌘ on Apple platforms).
        keyDown('Backspace', 'Backspace', {metaKey: true}),
        {update: [select(['2', 0, 'text'], ['2', 2, 'text'])]},
        keyDown('Delete', 'Delete', {metaKey: true}),
      ],
    },
  );

  if (isApple) {
    return scripts;
  }

  // `$handleBeforeInput`, with only the core's handlers.
  const beforeInput = (inputType: string, data?: string): Step => ({
    dispatch: 'BEFORE_INPUT_COMMAND',
    payload: data === undefined ? {inputType} : {data, inputType},
  });
  scripts.push(
    {
      handlers: LOGGERS,
      name: 'beforeinput at a range selection',
      snapshot: 'final',
      steps: [
        doc(select(['2', 2, 'text']), p(t('Hello'))),
        ...[
          'deleteContentBackward',
          'insertParagraph',
          'insertLineBreak',
          'formatBold',
          'formatItalic',
          'formatUnderline',
          'formatStrikeThrough',
          'deleteWordBackward',
          'deleteWordForward',
          'deleteSoftLineBackward',
          'deleteHardLineBackward',
          'deleteSoftLineForward',
          'deleteHardLineForward',
          'historyUndo',
          'historyRedo',
          'insertFromPaste',
          'insertFromPasteAsQuotation',
          'insertCompositionText',
          'somethingElse',
        ].map(inputType => beforeInput(inputType)),
        beforeInput('insertText', '\n'),
        beforeInput('insertText', '\n\n'),
        beforeInput('insertTranspose', '\n'),
      ],
    },
    {
      handlers: LOGGERS,
      name: 'Shift+Enter makes the next insertParagraph a line break',
      steps: [
        doc(select(['2', 2, 'text']), p(t('Hello'))),
        keyDown('Enter', 'Enter', {shiftKey: true}),
        beforeInput('insertParagraph'),
        beforeInput('insertParagraph'),
        keyDown('Enter', 'Enter', {shiftKey: true}),
        keyDown('Enter', 'Enter'),
        beforeInput('insertParagraph'),
      ],
    },
    {
      handlers: LOGGERS,
      name: 'beforeinput without a selection',
      steps: [
        {
          update: [
            {do: 'append', key: 'root', nodes: [p(t('Hello'))] as never},
          ],
        },
        beforeInput('historyUndo'),
        beforeInput('historyRedo'),
        beforeInput('formatBold'),
        beforeInput('deleteContentBackward'),
        beforeInput('insertParagraph'),
      ],
    },
    {
      handlers: LOGGERS,
      name: 'deleteContentBackward without a selection takes the previous one and stops',
      steps: [
        doc(select(['2', 2, 'text']), p(t('Hello'))),
        {
          update: [
            {do: 'clearSelection'},
            dispatchIn('BEFORE_INPUT_COMMAND', {
              inputType: 'deleteContentBackward',
            }),
          ],
        },
      ],
    },
    {
      // `onSelectionChange`: the points become the selection, a caret takes
      // the format and style of where it is, a range the format its text
      // shares, and a change is announced once.
      handlers: [
        {
          command: 'SELECTION_CHANGE_COMMAND',
          id: 'SELECTION_CHANGE_COMMAND',
          priority: 4,
          result: false,
        },
      ],
      name: 'selectionchange: points, format and style, and the notification',
      richText: true,
      steps: [
        {setRootElement: true},
        doc(
          select(['2', 0, 'text']),
          p(t('bold', {format: 1, style: 'color: red'}), t('plain')),
          p(t('next')),
        ),
        // A caret in bold text.
        {update: [selectionChange(['2', 2, 'text'])]},
        // The same caret again: nothing changed.
        {update: [selectionChange(['2', 2, 'text'])]},
        // Bold and plain text: no format they share; the style is kept.
        {update: [selectionChange(['2', 1, 'text'], ['3', 2, 'text'])]},
        // Backward from the start of "plain", an empty boundary.
        {update: [selectionChange(['3', 0, 'text'], ['2', 1, 'text'])]},
        // A caret at an element point of a block with text: the format is
        // kept and the style cleared.
        {update: [selectionChange(['4', 0, 'element'])]},
        // Into the next paragraph's text.
        {update: [selectionChange(['5', 4, 'text'])]},
      ],
    },
    {
      handlers: LOGGERS,
      name: 'rich text: insertText and deleteContentBackward with a root element',
      richText: true,
      steps: [
        {setRootElement: true},
        doc(select(['2', 5, 'text']), p(t('Hello'))),
        {
          update: [
            select(['2', 5, 'text']),
            dispatchIn('BEFORE_INPUT_COMMAND', {
              data: '!',
              inputType: 'insertText',
            }),
          ],
        },
        {
          update: [
            select(['2', 1, 'text'], ['2', 4, 'text']),
            dispatchIn('BEFORE_INPUT_COMMAND', {
              data: 'ipp',
              inputType: 'insertText',
            }),
          ],
        },
        {
          update: [
            select(['2', 2, 'text'], ['2', 4, 'text']),
            dispatchIn('BEFORE_INPUT_COMMAND', {
              inputType: 'deleteContentBackward',
            }),
          ],
        },
        {
          update: [
            select(['2', 2, 'text']),
            dispatchIn('BEFORE_INPUT_COMMAND', {inputType: 'insertParagraph'}),
          ],
        },
      ],
    },
  );

  // The composition handlers. Composition needs a root element: without one
  // JS reads the composed text from a DOM it doesn't have
  // (`$updateSelectedTextFromDOM`) and throws. In Node it has no DOM
  // selection, so the text is the one inserted through the model. The clock
  // starts at 1000 so that a composition doesn't start within 30ms of the
  // last keydown (at 0): that would apply the empty space heuristic, which
  // inserts a placeholder for the browser to compose into, and a native
  // view composes through the model instead.
  const composition = (
    command: 'COMPOSITION_START_COMMAND' | 'COMPOSITION_END_COMMAND',
    data: string,
  ): Action => dispatchIn(command, {data});
  const composing = {
    handlers: LOGGERS,
    listen: true,
    richText: true,
  };
  scripts.push(
    {
      ...composing,
      name: 'composition: the composed text stays selected until it ends',
      steps: [
        {now: 1000, setRootElement: true},
        doc(select(['2', 5, 'text']), p(t('Hello'))),
        {
          update: [
            select(['2', 5, 'text']),
            composition('COMPOSITION_START_COMMAND', ''),
          ],
        },
        {
          update: [
            select(['2', 5, 'text']),
            dispatchIn('CONTROLLED_TEXT_INSERTION_COMMAND', 'か'),
          ],
        },
        {
          update: [
            select(['2', 5, 'text'], ['2', 6, 'text']),
            dispatchIn('CONTROLLED_TEXT_INSERTION_COMMAND', '漢字'),
          ],
        },
        {
          update: [
            select(['2', 5, 'text'], ['2', 7, 'text']),
            // A second start while composing changes nothing.
            composition('COMPOSITION_START_COMMAND', ''),
          ],
        },
        {
          update: [
            select(['2', 7, 'text']),
            composition('COMPOSITION_END_COMMAND', '漢字'),
          ],
        },
        {
          update: [
            select(['2', 7, 'text']),
            dispatchIn('CONTROLLED_TEXT_INSERTION_COMMAND', '!'),
          ],
        },
      ],
    },
    {
      ...composing,
      name: 'composition: the composed node is not transformed, then merges',
      steps: [
        {now: 1000, setRootElement: true},
        doc(select(['2', 2, 'text']), p(t('ab'), t('cd', {format: 1}))),
        {
          update: [
            select(['2', 2, 'text']),
            composition('COMPOSITION_START_COMMAND', ''),
          ],
        },
        // The bold node becomes plain, so it could merge into the composed
        // one, which normalization still does (only transforms skip it).
        {
          update: [
            select(['2', 2, 'text']),
            {args: [0], do: 'node', key: '3', method: 'setFormat'},
          ],
        },
        {
          update: [
            select(['2', 4, 'text']),
            composition('COMPOSITION_END_COMMAND', 'cd'),
          ],
        },
      ],
    },
    {
      ...composing,
      name: 'composition: ending with a line break inserts a paragraph',
      steps: [
        {now: 1000, setRootElement: true},
        doc(select(['2', 5, 'text']), p(t('Hello'))),
        {
          update: [
            select(['2', 2, 'text']),
            composition('COMPOSITION_START_COMMAND', ''),
          ],
        },
        {
          update: [
            select(['2', 2, 'text'], ['2', 4, 'text']),
            composition('COMPOSITION_END_COMMAND', 'x\n'),
          ],
        },
      ],
    },
    {
      ...composing,
      name: 'composition: cancelled',
      steps: [
        {now: 1000, setRootElement: true},
        doc(select(['2', 5, 'text']), p(t('Hello'))),
        {
          update: [
            select(['2', 3, 'text']),
            composition('COMPOSITION_START_COMMAND', ''),
          ],
        },
        {
          update: [
            select(['2', 3, 'text']),
            composition('COMPOSITION_END_COMMAND', ''),
          ],
        },
        {
          update: [
            select(['2', 3, 'text']),
            dispatchIn('CONTROLLED_TEXT_INSERTION_COMMAND', 'x'),
          ],
        },
      ],
    },
    {
      ...composing,
      name: 'composition: ending on a token node puts the text beside it',
      steps: [
        {now: 1000, setRootElement: true},
        doc(select(['2', 5, 'text']), p(t('Hello'))),
        {
          update: [
            select(['2', 5, 'text']),
            composition('COMPOSITION_START_COMMAND', ''),
          ],
        },
        {
          update: [
            select(['2', 5, 'text']),
            {args: ['token'], do: 'node', key: '2', method: 'setMode'},
          ],
        },
        {
          update: [
            select(['2', 5, 'text']),
            composition('COMPOSITION_END_COMMAND', 'x'),
          ],
        },
      ],
    },
    {
      ...composing,
      name: 'composition: does not start without a range selection',
      steps: [
        {now: 1000, setRootElement: true},
        doc({do: 'selectNodes', keys: ['1']}, p(t('Hello'))),
        {
          update: [
            {do: 'selectNodes', keys: ['1']},
            composition('COMPOSITION_START_COMMAND', ''),
          ],
        },
        {
          update: [
            select(['2', 5, 'text']),
            dispatchIn('CONTROLLED_TEXT_INSERTION_COMMAND', 'x'),
          ],
        },
      ],
    },
  );
  return scripts;
}
