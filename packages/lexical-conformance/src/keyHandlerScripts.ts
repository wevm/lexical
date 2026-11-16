/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {
  Action,
  ExtensionArg,
  HandlerSpec,
  Json,
  Script,
  Step,
} from './ops';

// The scripts of `vectors/scripts/key-handlers`: the packages' handlers of
// the commands the core's keydown handler dispatches (the arrow keys,
// Backspace, Delete, Escape, Space, Tab, MOVE_TO_START and MOVE_TO_END), and
// of a click, a paste and drag and drop: `registerRichText` (with
// `RichTextExtension`'s `escapeFormatTriggers` and
// `shouldHandlePasteAsFiles`), `registerPlainText`, `registerList`'s
// Backspace, `registerCodeIndentation` (with `escapeWithArrows`),
// `TabIndentationExtension` and `registerLink`'s paste of a URL. Each key
// command is dispatched as the keydown handler dispatches it, with its
// event; a headless editor has no `Selection.modify`, so the cases stay
// where the model decides.

type Node = Json;

function t(text: string, extra: Record<string, Json> = {}): Node {
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

function block(type: string, children: Node[], extra: Record<string, Json>) {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    textFormat: 0,
    textStyle: '',
    type,
    version: 1,
    ...extra,
  };
}

function p(...children: Node[]): Node {
  return block('paragraph', children, {});
}

function indented(indent: number, ...children: Node[]): Node {
  return block('paragraph', children, {indent});
}

/** A quote that is a shadow root (`quoteShadowRootState`). */
function shadowQuote(...children: Node[]): Node {
  return block('quote', children, {shadowRoot: true});
}

const hr = (): Node => ({type: 'horizontalrule', version: 1});
const dt = (): Node => ({type: 'decorator-text', version: 1});
const br = (): Node => ({type: 'linebreak', version: 1});
const tab = (): Node => ({...(t('\t') as object), type: 'tab'});

function ch(text: string): Node {
  return {...(t(text) as object), type: 'code-highlight'};
}

function code(...children: Node[]): Node {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    type: 'code',
    version: 1,
  };
}

function li(...children: Node[]): Node {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    type: 'listitem',
    value: 1,
    version: 1,
  };
}

function ul(...items: Node[]): Node {
  return {
    children: items,
    direction: null,
    format: '',
    indent: 0,
    listType: 'bullet',
    start: 1,
    tag: 'ul',
    type: 'list',
    version: 1,
  };
}

function select(
  anchor: [string, number, 'text' | 'element'],
  focus?: [string, number, 'text' | 'element'],
): Action {
  return {anchor, do: 'select', focus};
}

function selectNodes(...keys: string[]): Action {
  return {do: 'selectNodes', keys};
}

function doc(selection: Action | null, ...blocks: Node[]): Step {
  const append: Action = {do: 'append', key: 'root', nodes: blocks as never};
  return {update: selection === null ? [append] : [append, selection]};
}

/**
 * `doc` for an editor built from extensions, whose root starts with an empty
 * paragraph (key 1): it is cleared first, so the blocks' keys start at 2.
 */
function start(selection: Action | null, ...blocks: Node[]): Step {
  const clear: Action = {do: 'node', key: 'root', method: 'clear'};
  const [append, ...rest] = (doc(selection, ...blocks) as {update: Action[]})
    .update;
  return {update: [clear, append, ...rest]};
}

function update(...actions: Action[]): Step {
  return {update: actions};
}

type Modifiers = {
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
};

/** The key each key command stands for, as `[key, code]`. */
const KEYS: Record<string, [string, string]> = {
  KEY_ARROW_DOWN_COMMAND: ['ArrowDown', 'ArrowDown'],
  KEY_ARROW_LEFT_COMMAND: ['ArrowLeft', 'ArrowLeft'],
  KEY_ARROW_RIGHT_COMMAND: ['ArrowRight', 'ArrowRight'],
  KEY_ARROW_UP_COMMAND: ['ArrowUp', 'ArrowUp'],
  KEY_BACKSPACE_COMMAND: ['Backspace', 'Backspace'],
  KEY_DELETE_COMMAND: ['Delete', 'Delete'],
  KEY_ENTER_COMMAND: ['Enter', 'Enter'],
  KEY_ESCAPE_COMMAND: ['Escape', 'Escape'],
  KEY_SPACE_COMMAND: [' ', 'Space'],
  KEY_TAB_COMMAND: ['Tab', 'Tab'],
  MOVE_TO_END: ['End', 'End'],
  MOVE_TO_START: ['Home', 'Home'],
};

/** The key command, with the event of its key. */
function key(command: string, modifiers: Modifiers = {}): Step {
  const [k, c] = KEYS[command];
  return {dispatch: command, payload: {code: c, key: k, ...modifiers}};
}

/** `key`, dispatched inside an update, after the update's selection. */
function keyIn(command: string, modifiers: Modifiers = {}): Action {
  const [k, c] = KEYS[command];
  return {command, do: 'dispatch', payload: {code: c, key: k, ...modifiers}};
}

function dispatch(command: string, payload: Json = null): Step {
  return {dispatch: command, payload};
}

/** A data transfer's JSON (ops.ts's `eventPayload`). */
function dataTransfer(
  data: Record<string, string>,
  files: {name: string; type: string; text?: string}[] = [],
): Json {
  return {data, files};
}

function paste(data: Json): Step {
  return dispatch('PASTE_COMMAND', {clipboardData: data});
}

function drag(command: string, data: Json, x = 10, y = 20): Step {
  return dispatch(command, {clientX: x, clientY: y, dataTransfer: data});
}

/** A handler for each command that logs its payload and lets it go on. */
function loggers(...commands: string[]): HandlerSpec[] {
  return commands.map(command => ({
    command,
    id: command,
    priority: 4,
    result: false,
  }));
}

/** The commands the key handlers dispatch. */
const DISPATCHED = loggers(
  'DELETE_CHARACTER_COMMAND',
  'INDENT_CONTENT_COMMAND',
  'OUTDENT_CONTENT_COMMAND',
  'INSERT_TAB_COMMAND',
  'INSERT_PARAGRAPH_COMMAND',
  'INSERT_LINE_BREAK_COMMAND',
  'TOGGLE_LINK_COMMAND',
);

/** A `DRAG_DROP_PASTE` handler that takes the files (and logs them). */
const TAKE_FILES: HandlerSpec = {
  command: 'DRAG_DROP_PASTE',
  id: 'DRAG_DROP_PASTE',
  priority: 1,
  result: true,
};

const richText = true;

function richTextScripts(): Script[] {
  const handlers = DISPATCHED;
  return [
    {
      handlers,
      name: 'rich text: Backspace outdents an indented block at its start',
      richText,
      steps: [
        doc(select(['2', 0, 'text']), indented(2, t('ab')), p(t('cd'))),
        key('KEY_BACKSPACE_COMMAND'),
        key('KEY_BACKSPACE_COMMAND'),
        // At the block's element point.
        update(
          {args: [1], do: 'node', key: '1', method: 'setIndent'},
          select(['1', 0, 'element']),
        ),
        key('KEY_BACKSPACE_COMMAND'),
        // Not at the start: a deletion.
        update(
          {args: [1], do: 'node', key: '1', method: 'setIndent'},
          select(['2', 1, 'text'], ['2', 2, 'text']),
        ),
        key('KEY_BACKSPACE_COMMAND'),
      ],
    },
    {
      extension: true,
      handlers,
      name: 'rich text: Backspace and Delete at a range, a node selection and none',
      richText,
      steps: [
        doc(
          select(['2', 0, 'text'], ['2', 1, 'text']),
          p(t('ab')),
          hr(),
          p(t('cd')),
          hr(),
        ),
        key('KEY_DELETE_COMMAND'),
        update(selectNodes('3')),
        key('KEY_BACKSPACE_COMMAND'),
        update(selectNodes('6')),
        key('KEY_DELETE_COMMAND'),
        update({do: 'clearSelection'}),
        key('KEY_BACKSPACE_COMMAND'),
        key('KEY_DELETE_COMMAND'),
      ],
    },
    {
      extension: true,
      name: 'rich text: Escape at a range selection blurs the editor',
      richText,
      steps: [
        doc(select(['2', 1, 'text']), p(t('ab')), hr()),
        key('KEY_ESCAPE_COMMAND'),
        update(selectNodes('3')),
        key('KEY_ESCAPE_COMMAND'),
        update({do: 'clearSelection'}),
        key('KEY_ESCAPE_COMMAND'),
      ],
    },
    {
      handlers,
      name: 'rich text: Space and Tab escape the default formats',
      richText,
      steps: [
        doc(select(['2', 2, 'text']), p(t('ab'))),
        dispatch('FORMAT_TEXT_COMMAND', 'uppercase'),
        dispatch('FORMAT_TEXT_COMMAND', 'bold'),
        key('KEY_SPACE_COMMAND'),
        dispatch('FORMAT_TEXT_COMMAND', 'capitalize'),
        key('KEY_TAB_COMMAND'),
        dispatch('FORMAT_TEXT_COMMAND', 'lowercase'),
        // Arrows and clicks escape none of them by default.
        key('KEY_ARROW_RIGHT_COMMAND'),
        dispatch('CLICK_COMMAND', {}),
        key('KEY_ENTER_COMMAND'),
      ],
    },
    {
      name: 'rich text: the vertical arrows at the edges of the root',
      richText,
      steps: [
        doc(select(['root', 0, 'element']), p(t('ab')), p(t('cd'))),
        key('KEY_ARROW_UP_COMMAND'),
        key('KEY_ARROW_DOWN_COMMAND'),
        update(select(['root', 2, 'element'])),
        key('KEY_ARROW_DOWN_COMMAND'),
        key('KEY_ARROW_UP_COMMAND'),
        // Within text the platform moves the caret.
        update(select(['2', 1, 'text'])),
        key('KEY_ARROW_UP_COMMAND'),
        key('KEY_ARROW_DOWN_COMMAND'),
        key('KEY_ARROW_LEFT_COMMAND'),
        key('KEY_ARROW_RIGHT_COMMAND'),
      ],
    },
    {
      extension: true,
      name: 'rich text: the arrows at a block cursor beside a decorator at the edge of the root',
      richText,
      steps: [
        doc(select(['root', 0, 'element']), hr(), p(t('ab')), hr()),
        key('KEY_ARROW_LEFT_COMMAND'),
        key('KEY_ARROW_LEFT_COMMAND', {shiftKey: true}),
        update(select(['root', 3, 'element'])),
        key('KEY_ARROW_RIGHT_COMMAND'),
        // Not at the edge.
        update(select(['root', 1, 'element'])),
        key('KEY_ARROW_UP_COMMAND'),
      ],
    },
    {
      extension: true,
      name: 'rich text: the arrows leave a node selection',
      richText,
      steps: [
        doc(selectNodes('3'), p(t('ab')), hr(), p(t('cd'))),
        key('KEY_ARROW_DOWN_COMMAND'),
        update(selectNodes('3')),
        key('KEY_ARROW_UP_COMMAND'),
        update(selectNodes('3')),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(selectNodes('3')),
        key('KEY_ARROW_LEFT_COMMAND'),
      ],
    },
    {
      extension: true,
      name: 'rich text: Shift+arrows turn a node selection of contiguous blocks into a range',
      richText,
      steps: [
        doc(selectNodes('4', '3'), p(t('ab')), hr(), hr(), p(t('cd'))),
        key('KEY_ARROW_UP_COMMAND', {shiftKey: true}),
        update(selectNodes('3', '4')),
        key('KEY_ARROW_DOWN_COMMAND', {shiftKey: true}),
        // Not contiguous: the selection collapses beside the first node.
        update(selectNodes('3', '7')),
        key('KEY_ARROW_UP_COMMAND', {shiftKey: true}),
      ],
    },
    {
      extension: true,
      name: 'rich text: an arrow selects the block decorator an empty block is beside',
      richText,
      steps: [
        doc(select(['1', 0, 'element']), p(), hr(), p(t('ab')), p()),
        key('KEY_ARROW_DOWN_COMMAND'),
        update(select(['5', 0, 'element'])),
        key('KEY_ARROW_UP_COMMAND'),
        // A block with text needs a line move, which a headless editor has
        // no layout for.
        update(select(['4', 1, 'text'])),
        key('KEY_ARROW_UP_COMMAND'),
        // With Shift the platform extends.
        update(select(['1', 0, 'element'])),
        key('KEY_ARROW_DOWN_COMMAND', {shiftKey: true}),
      ],
    },
    {
      extension: true,
      name: 'rich text: the arrows enter and leave a shadow root beside a block cursor',
      richText,
      steps: [
        doc(
          select(['root', 1, 'element']),
          hr(),
          shadowQuote(p(t('ab'))),
          hr(),
        ),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(select(['4', 2, 'text'])),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(select(['4', 0, 'text'])),
        key('KEY_ARROW_LEFT_COMMAND'),
        update(select(['root', 2, 'element'])),
        key('KEY_ARROW_UP_COMMAND'),
        // Not at the edge of the shadow root.
        update(select(['4', 1, 'text'])),
        key('KEY_ARROW_RIGHT_COMMAND'),
      ],
    },
    {
      extension: true,
      name: 'rich text: the horizontal arrows step over an inline decorator',
      richText,
      steps: [
        doc(select(['2', 2, 'text']), p(t('ab'), dt(), t('cd'))),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(select(['4', 0, 'text'])),
        key('KEY_ARROW_LEFT_COMMAND'),
        update(select(['2', 2, 'text'])),
        key('KEY_ARROW_RIGHT_COMMAND', {shiftKey: true}),
      ],
    },
    {
      // `$isParentRTL`, `$isParentElementRTL` and the vertical writing mode
      // read the computed styles of the nodes' elements, so the editor has
      // a root element, and each update selects before its key.
      extension: true,
      name: 'rich text: the horizontal arrows in right-to-left and vertical text',
      richText,
      steps: [
        {setRootElement: true},
        doc(null, hr(), p(t('ab'), dt(), t('cd')), hr()),
        {computedStyles: {'2': {direction: 'rtl'}, root: {direction: 'rtl'}}},
        // A node selection leaves toward the previous node on the right.
        update(selectNodes('1'), keyIn('KEY_ARROW_RIGHT_COMMAND')),
        update(selectNodes('6'), keyIn('KEY_ARROW_LEFT_COMMAND')),
        update(selectNodes('6'), keyIn('KEY_ARROW_RIGHT_COMMAND')),
        // A block cursor at the root's edge, the right arrow at its start.
        update(
          select(['root', 0, 'element']),
          keyIn('KEY_ARROW_RIGHT_COMMAND'),
        ),
        update(select(['root', 3, 'element']), keyIn('KEY_ARROW_LEFT_COMMAND')),
        update(select(['root', 0, 'element']), keyIn('KEY_ARROW_LEFT_COMMAND')),
        // Beside the inline decorator, the right arrow moves back over
        // text, which the platform does, and the left arrow toward the
        // decorator, which needs `Selection.modify`.
        update(select(['3', 2, 'text']), keyIn('KEY_ARROW_RIGHT_COMMAND')),
        update(select(['3', 2, 'text']), keyIn('KEY_ARROW_LEFT_COMMAND')),
        update(select(['5', 0, 'text']), keyIn('KEY_ARROW_RIGHT_COMMAND')),
        // An element point reads its own element's style.
        update(select(['2', 0, 'element']), keyIn('KEY_ARROW_LEFT_COMMAND')),
        // Vertical writing reverses the arrows; with right-to-left too,
        // `$shouldOverrideDefaultCharacterSelection` reverses them back.
        {computedStyles: {'2': {writingMode: 'vertical-rl'}}},
        update(select(['3', 2, 'text']), keyIn('KEY_ARROW_RIGHT_COMMAND')),
        update(select(['3', 2, 'text']), keyIn('KEY_ARROW_LEFT_COMMAND')),
        {
          computedStyles: {
            '2': {direction: 'rtl', writingMode: 'vertical-rl'},
          },
        },
        update(select(['3', 2, 'text']), keyIn('KEY_ARROW_RIGHT_COMMAND')),
        update(select(['3', 2, 'text']), keyIn('KEY_ARROW_LEFT_COMMAND')),
        // Left to right again.
        {computedStyles: {}},
        update(select(['3', 2, 'text']), keyIn('KEY_ARROW_RIGHT_COMMAND')),
        update(selectNodes('6'), keyIn('KEY_ARROW_LEFT_COMMAND')),
      ],
    },
    {
      extension: true,
      name: 'rich text: MOVE_TO_START and MOVE_TO_END past an inline decorator at the start of a block',
      richText,
      steps: [
        doc(select(['1', 0, 'element']), p(dt(), t('ab')), p(t('cd'))),
        key('MOVE_TO_END'),
        key('MOVE_TO_START'),
        update(select(['1', 0, 'element'])),
        key('MOVE_TO_END', {shiftKey: true}),
        key('MOVE_TO_START', {shiftKey: true}),
        // Already at the start of the block.
        update(select(['1', 0, 'element'])),
        key('MOVE_TO_START'),
        // Across blocks the platform moves.
        update(select(['3', 1, 'text'], ['5', 1, 'text'])),
        key('MOVE_TO_START'),
        // No inline decorator first.
        update(select(['5', 1, 'text'])),
        key('MOVE_TO_START'),
        key('MOVE_TO_END'),
      ],
    },
    {
      extension: true,
      name: 'rich text: MOVE_TO_START and MOVE_TO_END at a node selection collapse it to its block',
      richText,
      steps: [
        doc(selectNodes('3', '2'), p(t('ab'), dt(), dt()), hr(), p(t('cd'))),
        key('MOVE_TO_START'),
        update(selectNodes('3', '2')),
        key('MOVE_TO_END'),
        update(selectNodes('5')),
        key('MOVE_TO_START'),
        update(selectNodes('5')),
        key('MOVE_TO_END', {shiftKey: true}),
      ],
    },
    {
      extension: true,
      name: 'rich text: a click clears a node selection',
      richText,
      steps: [
        doc(selectNodes('3'), p(t('ab')), hr()),
        dispatch('CLICK_COMMAND', {clientX: 5, clientY: 5}),
        update(select(['2', 1, 'text'])),
        dispatch('CLICK_COMMAND', {}),
      ],
    },
  ];
}

/** RichTextExtension, configured. */
function richTextExtension(...configs: {[key: string]: Json}[]): ExtensionArg {
  return configs.length === 0
    ? 'RichTextExtension'
    : ['RichTextExtension', ...configs];
}

function richTextConfigScripts(): Script[] {
  const boundaryCode = {
    escapeFormatTriggers: {
      code: {arrow: true, click: true, onlyAtBoundary: true},
    },
  };
  return [
    {
      extensions: [richTextExtension(boundaryCode)],
      name: 'escapeFormatTriggers: code escapes at a text node boundary on arrows and clicks',
      steps: [
        start(
          select(['3', 2, 'text']),
          p(t('ab', {format: 16, style: 'color: red'})),
        ),
        update(
          select(['3', 2, 'text']),
          {args: [16], do: 'selection', method: 'setFormat'},
          {args: ['color: red'], do: 'selection', method: 'setStyle'},
        ),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(
          select(['3', 0, 'text']),
          {args: [16], do: 'selection', method: 'setFormat'},
          {args: ['color: red'], do: 'selection', method: 'setStyle'},
        ),
        key('KEY_ARROW_LEFT_COMMAND'),
        update(select(['3', 1, 'text']), {
          args: [16],
          do: 'selection',
          method: 'setFormat',
        }),
        // Not at a boundary.
        key('KEY_ARROW_RIGHT_COMMAND'),
        dispatch('CLICK_COMMAND', {}),
        update(select(['3', 2, 'text']), {
          args: [16],
          do: 'selection',
          method: 'setFormat',
        }),
        dispatch('CLICK_COMMAND', {}),
        // With Shift the arrows escape nothing.
        update(select(['3', 0, 'text']), {
          args: [16],
          do: 'selection',
          method: 'setFormat',
        }),
        key('KEY_ARROW_LEFT_COMMAND', {shiftKey: true}),
      ],
    },
    {
      extensions: [
        richTextExtension(
          {escapeFormatTriggers: {lowercase: null, uppercase: {space: false}}},
          {escapeFormatTriggers: {bold: {tab: true}}},
        ),
      ],
      name: 'escapeFormatTriggers: overrides merge format by format',
      steps: [
        start(select(['3', 2, 'text']), p(t('ab'))),
        dispatch('FORMAT_TEXT_COMMAND', 'uppercase'),
        dispatch('FORMAT_TEXT_COMMAND', 'bold'),
        key('KEY_SPACE_COMMAND'),
        key('KEY_TAB_COMMAND'),
        dispatch('FORMAT_TEXT_COMMAND', 'lowercase'),
        key('KEY_ENTER_COMMAND'),
      ],
    },
  ];
}

function pasteScripts(): Script[] {
  const image = {name: 'a.png', type: 'image/png'};
  return [
    {
      handlers: [TAKE_FILES],
      name: 'rich text: a paste of text, of files and of both',
      richText,
      steps: [
        doc(select(['2', 1, 'text']), p(t('ab'))),
        paste(dataTransfer({'text/plain': 'xy'})),
        paste(dataTransfer({}, [image])),
        paste(dataTransfer({'text/plain': 'zw'}, [image])),
        paste(null),
      ],
    },
    {
      name: 'rich text: a paste of files nothing takes',
      richText,
      steps: [
        doc(select(['2', 1, 'text']), p(t('ab'))),
        paste(dataTransfer({}, [image])),
      ],
    },
    {
      extensions: [richTextExtension({shouldHandlePasteAsFiles: 'always'})],
      handlers: [TAKE_FILES],
      name: 'shouldHandlePasteAsFiles: files with text',
      steps: [
        start(select(['3', 1, 'text']), p(t('ab'))),
        paste(
          dataTransfer({'text/plain': 'zw'}, [
            image,
            {name: 'b.txt', text: 'b', type: 'text/plain'},
          ]),
        ),
        paste(dataTransfer({'text/plain': 'zw'})),
      ],
    },
    {
      extensions: [
        'RichTextExtension',
        ['LinkExtension', {validateUrl: 'https'}],
      ],
      handlers: DISPATCHED,
      name: 'LinkExtension: a paste of a URL over selected text links it',
      steps: [
        start(
          select(['3', 6, 'text'], ['3', 11, 'text']),
          p(t('hello world')),
          p(t('two')),
        ),
        // Not a URL validateUrl accepts.
        paste(dataTransfer({'text/plain': 'http://lexical.dev'})),
        // A collapsed selection.
        paste(dataTransfer({'text/plain': 'https://a.dev'})),
        update(select(['3', 0, 'text'], ['3', 5, 'text'])),
        paste(dataTransfer({'text/plain': 'https://lexical.dev'})),
        // Across blocks.
        update({do: 'selectAll'}),
        paste(dataTransfer({'text/plain': 'https://b.dev'})),
      ],
    },
    {
      extensions: [['LinkExtension', {validateUrl: 'https'}]],
      handlers: loggers('TOGGLE_LINK_COMMAND'),
      name: 'LinkExtension: the paste of a URL without rich text',
      steps: [
        start(select(['3', 0, 'text'], ['3', 5, 'text']), p(t('hello world'))),
        paste(dataTransfer({'text/plain': 'https://lexical.dev'})),
        paste(null),
      ],
    },
  ];
}

function dragScripts(): Script[] {
  const file = {name: 'a.png', type: 'image/png'};
  const marker = (editorKey: string) =>
    dataTransfer({
      'application/x-lexical-drag': JSON.stringify({editorKey}),
      'text/plain': 'dropped',
    });
  return [
    {
      extension: true,
      handlers: [TAKE_FILES],
      // The drag start of a range selection writes the selection's clipboard
      // data, its HTML among it, which `$generateHtmlFromNodes` makes only
      // with a global DOM, which Node lacks.
      name: 'rich text: drag start at a node selection and at none',
      richText,
      steps: [
        doc(selectNodes('3'), p(t('abcd')), hr()),
        drag('DRAGSTART_COMMAND', dataTransfer({}, [file])),
        drag('DRAGSTART_COMMAND', dataTransfer({})),
        drag('DRAGOVER_COMMAND', dataTransfer({}, [file])),
        drag('DRAGEND_COMMAND', dataTransfer({})),
        update({do: 'clearSelection'}),
        drag('DRAGSTART_COMMAND', dataTransfer({})),
        drag('DRAGSTART_COMMAND', dataTransfer({}, [file])),
      ],
    },
    {
      // A drop and a drag over read the caret at the drop point, which
      // needs the root element; an editor with one has no selection in
      // Node, and jsdom resolves no caret.
      extension: true,
      handlers: [TAKE_FILES],
      name: 'rich text: drag over and drops at no caret',
      richText,
      steps: [
        {setRootElement: true},
        doc(null, p(t('abcd')), hr()),
        drag('DRAGOVER_COMMAND', dataTransfer({'text/plain': 'x'})),
        drag('DRAGOVER_COMMAND', dataTransfer({}, [file])),
        drag('DROP_COMMAND', dataTransfer({}, [file])),
        // A drag from no Lexical editor, and one from another editor.
        drag('DROP_COMMAND', dataTransfer({'text/plain': 'x'})),
        drag('DROP_COMMAND', marker('other')),
        drag('DROP_COMMAND', null),
        drag('DRAGSTART_COMMAND', dataTransfer({})),
        drag('DRAGSTART_COMMAND', dataTransfer({}, [file])),
      ],
    },
    {
      name: 'plain text: drag start marks a range selection as the drag source',
      plainText: true,
      steps: [
        doc(select(['2', 1, 'text'], ['2', 3, 'text']), p(t('abcd'))),
        drag('DRAGSTART_COMMAND', dataTransfer({})),
        drag('DRAGSTART_COMMAND', null),
        update(select(['2', 1, 'text'])),
        drag('DRAGSTART_COMMAND', dataTransfer({})),
        update({do: 'clearSelection'}),
        drag('DRAGSTART_COMMAND', dataTransfer({})),
      ],
    },
    {
      name: 'plain text: drops at no caret',
      plainText: true,
      steps: [
        {setRootElement: true},
        doc(null, p(t('abcd'))),
        drag('DROP_COMMAND', marker('other')),
        drag('DROP_COMMAND', dataTransfer({}, [file])),
        drag('DROP_COMMAND', null),
      ],
    },
  ];
}

function plainTextScripts(): Script[] {
  return [
    {
      extension: true,
      handlers: DISPATCHED,
      name: 'plain text: Backspace, Delete and the horizontal arrows',
      plainText: true,
      steps: [
        doc(select(['2', 0, 'text'], ['2', 1, 'text']), p(t('abcd'), dt())),
        key('KEY_BACKSPACE_COMMAND'),
        update(select(['2', 1, 'text'], ['2', 2, 'text'])),
        key('KEY_DELETE_COMMAND'),
        update(select(['2', 1, 'text'])),
        key('KEY_ARROW_LEFT_COMMAND'),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(select(['2', 2, 'text'])),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(select(['2', 2, 'text'])),
        key('KEY_ARROW_RIGHT_COMMAND', {shiftKey: true}),
        key('KEY_ENTER_COMMAND'),
        update(selectNodes('3')),
        key('KEY_BACKSPACE_COMMAND'),
        key('KEY_DELETE_COMMAND'),
        key('KEY_ARROW_LEFT_COMMAND'),
      ],
    },
  ];
}

function listScripts(): Script[] {
  return [
    {
      handlers: DISPATCHED,
      list: true,
      name: 'list: Backspace at the start of an item collapses it',
      richText,
      steps: [
        doc(
          select(['5', 0, 'text']),
          ul(li(t('one')), li(t('two')), li(t('three'))),
        ),
        key('KEY_BACKSPACE_COMMAND'),
        // At the start of the first item.
        update(select(['3', 0, 'text'])),
        key('KEY_BACKSPACE_COMMAND'),
        // Not at the start of an item.
        update(select(['5', 1, 'text'], ['5', 3, 'text'])),
        key('KEY_BACKSPACE_COMMAND'),
      ],
    },
    {
      handlers: DISPATCHED,
      list: true,
      name: 'list: Backspace at a range in an item deletes',
      richText,
      steps: [
        doc(
          select(['3', 1, 'text'], ['3', 2, 'text']),
          ul(li(t('one', {format: 1}), t('two'))),
        ),
        key('KEY_BACKSPACE_COMMAND'),
        // At the start of the second text node of the item.
        update(select(['4', 0, 'text'], ['4', 1, 'text'])),
        key('KEY_BACKSPACE_COMMAND'),
      ],
    },
  ];
}

function codeScripts(): Script[] {
  const handlers = DISPATCHED;
  const code3 = () =>
    code(ch('one'), br(), tab(), ch('two'), br(), ch('  three'));
  // Keys: code 1, one 2, br 3, tab 4, two 5, br 6, three 7.
  return [
    {
      code: true,
      handlers,
      name: 'code: Tab and Shift+Tab',
      richText,
      steps: [
        // Outside code.
        doc(select(['9', 1, 'text']), code3(), p(t('x'))),
        key('KEY_TAB_COMMAND'),
        // A whole line.
        update(select(['5', 0, 'text'], ['5', 3, 'text'])),
        key('KEY_TAB_COMMAND'),
        // Across lines.
        update(select(['2', 1, 'text'], ['7', 2, 'text'])),
        key('KEY_TAB_COMMAND', {shiftKey: true}),
        key('KEY_TAB_COMMAND'),
        // Collapsed.
        update(select(['2', 1, 'text'])),
        key('KEY_TAB_COMMAND'),
        key('KEY_TAB_COMMAND', {shiftKey: true}),
        // Within a line.
        update(select(['7', 3, 'text'], ['7', 5, 'text'])),
        key('KEY_TAB_COMMAND'),
      ],
    },
    {
      code: true,
      name: 'code: the vertical arrows at the edges of a code block',
      richText,
      steps: [
        doc(select(['2', 0, 'text']), code3()),
        key('KEY_ARROW_UP_COMMAND'),
        update(select(['7', 7, 'text'])),
        key('KEY_ARROW_DOWN_COMMAND'),
        // Within the block, the platform moves the caret.
        update(select(['5', 1, 'text'])),
        key('KEY_ARROW_UP_COMMAND'),
        key('KEY_ARROW_DOWN_COMMAND'),
      ],
    },
    {
      code: true,
      name: 'code: Alt+Up and Alt+Down move lines',
      richText,
      steps: [
        doc(select(['5', 1, 'text']), code3()),
        key('KEY_ARROW_UP_COMMAND', {altKey: true}),
        key('KEY_ARROW_UP_COMMAND', {altKey: true}),
        key('KEY_ARROW_DOWN_COMMAND', {altKey: true}),
        key('KEY_ARROW_DOWN_COMMAND', {altKey: true}),
        key('KEY_ARROW_DOWN_COMMAND', {altKey: true}),
        // Two lines at once.
        update(select(['2', 1, 'text'], ['5', 1, 'text'])),
        key('KEY_ARROW_DOWN_COMMAND', {altKey: true}),
      ],
    },
    {
      code: true,
      name: 'code: Alt+arrows past a blank line',
      richText,
      steps: [
        doc(select(['2', 1, 'text']), code(ch('one'), br(), br(), ch('two'))),
        key('KEY_ARROW_DOWN_COMMAND', {altKey: true}),
        update(select(['5', 1, 'text'])),
        key('KEY_ARROW_UP_COMMAND', {altKey: true}),
      ],
    },
    {
      code: true,
      name: 'code: MOVE_TO_START and MOVE_TO_END in a line',
      richText,
      steps: [
        doc(select(['7', 5, 'text']), code3()),
        key('MOVE_TO_START'),
        key('MOVE_TO_START'),
        key('MOVE_TO_END'),
        update(select(['5', 2, 'text'])),
        key('MOVE_TO_START'),
        update(select(['5', 2, 'text'])),
        key('MOVE_TO_START', {shiftKey: true}),
        key('MOVE_TO_END', {shiftKey: true}),
        update(select(['2', 2, 'text'])),
        key('MOVE_TO_START'),
      ],
    },
    {
      extensions: [
        'RichTextExtension',
        ['CodeIndentExtension', {escapeWithArrows: true}],
      ],
      name: 'CodeIndentExtension: escapeWithArrows leaves a code block at its edges',
      steps: [
        start(select(['5', 3, 'text']), code(ch('one'), br(), ch('two'))),
        key('KEY_ARROW_DOWN_COMMAND', {altKey: true}),
        key('KEY_ARROW_DOWN_COMMAND'),
        update(select(['3', 0, 'text'])),
        key('KEY_ARROW_UP_COMMAND', {altKey: true}),
        key('KEY_ARROW_LEFT_COMMAND'),
        update(select(['5', 3, 'text'])),
        key('KEY_ARROW_RIGHT_COMMAND'),
        update(select(['5', 3, 'text'])),
        key('KEY_ARROW_UP_COMMAND'),
      ],
    },
    {
      extensions: ['RichTextExtension', 'CodeIndentExtension'],
      name: 'CodeIndentExtension: without escapeWithArrows the arrows stay',
      steps: [
        start(select(['5', 3, 'text']), code(ch('one'), br(), ch('two'))),
        key('KEY_ARROW_DOWN_COMMAND'),
        key('KEY_ARROW_RIGHT_COMMAND'),
        key('KEY_TAB_COMMAND'),
      ],
    },
  ];
}

function tabIndentationScripts(): Script[] {
  return [
    {
      extensions: ['RichTextExtension', 'TabIndentationExtension'],
      handlers: DISPATCHED,
      name: 'TabIndentationExtension: Tab indents at the start of a block and inserts a tab elsewhere',
      steps: [
        start(select(['3', 0, 'text']), p(t('ab')), p(t('cd'))),
        key('KEY_TAB_COMMAND'),
        key('KEY_TAB_COMMAND', {shiftKey: true}),
        key('KEY_TAB_COMMAND', {shiftKey: true}),
        update(select(['3', 1, 'text'])),
        key('KEY_TAB_COMMAND'),
        // Across blocks.
        update(select(['3', 1, 'text'], ['6', 1, 'text'])),
        key('KEY_TAB_COMMAND'),
        update({do: 'clearSelection'}),
        key('KEY_TAB_COMMAND'),
      ],
    },
    {
      extensions: [
        'RichTextExtension',
        ['TabIndentationExtension', {$canIndent: 'never'}],
      ],
      handlers: DISPATCHED,
      name: 'TabIndentationExtension: blocks that cannot indent get a tab',
      steps: [
        start(select(['3', 0, 'text']), p(t('ab'))),
        key('KEY_TAB_COMMAND'),
      ],
    },
  ];
}

/** The scripts of `vectors/scripts/key-handlers`. */
export function keyHandlerScripts(): Script[] {
  return [
    ...richTextScripts(),
    ...richTextConfigScripts(),
    ...pasteScripts(),
    ...dragScripts(),
    ...plainTextScripts(),
    ...listScripts(),
    ...codeScripts(),
    ...tabIndentationScripts(),
  ];
}
