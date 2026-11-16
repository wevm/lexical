/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {
  Action,
  DomEventJson,
  ExtensionArg,
  HandlerSpec,
  Json,
  Script,
  ShortcutJson,
  ShortcutsJson,
  Step,
} from './ops';

// The scripts of `vectors/scripts/input-extensions`: the extensions of
// `@lexical/extension` and `@lexical/link` that listen to keys, clicks,
// composition and focus (`KeyboardShortcutsExtension` with
// `formatKeyboardShortcut`, `ClickAfterLastBlockExtension`,
// `NormalizeTripleClickSelectionExtension`'s `$fixFocusOverselection`,
// `IMEExtension`, `AutoFocusExtension`, `ClickableLinkExtension`), and the
// builder's `namespace`, `editable` and disposal.
//
// Where JS listens to the root element rather than to a command (a click
// below the last block, a click on a link, `compositionend`), the scripts
// dispatch the DOM event at the element the reconciler made (`domEvent`),
// and the Rust and Swift runners dispatch the command their views send for
// it (CLICK_COMMAND with the node's key as `targetKey`, the root's `root`,
// or COMPOSITION_END_COMMAND). In Node an update of an editor with a root
// element starts with no range selection (see `setRootElement` in ops.ts),
// so these sessions click with no selection. A composition that starts on
// an element point isn't recorded: JS inserts a placeholder there for the
// browser to compose into (the empty space heuristic), which the ports
// leave out (rust/PARITY.md).

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

function p(...children: Node[]): Node {
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

function br(): Node {
  return {type: 'linebreak', version: 1};
}

function hr(): Node {
  return {type: 'horizontalrule', version: 1};
}

function a(
  url: string,
  extra: Record<string, Json>,
  ...children: Node[]
): Node {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    rel: null,
    target: null,
    title: null,
    type: 'link',
    url,
    version: 1,
    ...extra,
  };
}

function update(...actions: Action[]): Step {
  return {update: actions};
}

/** Clears the root (the builder's empty paragraph is key 1), then appends. */
function start(...blocks: Node[]): Step {
  return update(
    {args: [], do: 'node', key: 'root', method: 'clear'},
    {do: 'append', key: 'root', nodes: blocks as never},
  );
}

function select(
  anchor: [string, number, 'text' | 'element'],
  focus?: [string, number, 'text' | 'element'],
): Action {
  return {anchor, do: 'select', focus};
}

function keydown(payload: Record<string, Json>): Step {
  return {dispatch: 'KEY_DOWN_COMMAND', payload};
}

function domEvent(event: DomEventJson): Step {
  return {domEvent: event};
}

function format(shortcut: ShortcutJson, isApple?: boolean): Action {
  return isApple === undefined
    ? {do: 'formatKeyboardShortcut', shortcut}
    : {do: 'formatKeyboardShortcut', isApple, shortcut};
}

/** A handler that logs the command (and its event) without handling it. */
function logger(command: string, result = false): HandlerSpec {
  return {command, id: command, priority: 1, result};
}

/** The commands the shortcuts dispatch, each logged. */
const SHORTCUT_COMMANDS = [
  'KEY_ESCAPE_COMMAND',
  'KEY_ARROW_UP_COMMAND',
  'KEY_ARROW_DOWN_COMMAND',
  'MOVE_TO_END',
];

function shortcuts(
  table: ShortcutsJson,
  ...more: ShortcutsJson[]
): ExtensionArg {
  return [
    'KeyboardShortcutsExtension',
    {shortcuts: table as never},
    ...more.map(m => ({shortcuts: m as never})),
  ];
}

const CONTROL_OR_META = {
  controlOrOther: 'metaKey',
  ctrlKey: true,
  metaKey: false,
} as const;

function keyboardShortcutScripts(): Script[] {
  const handlers = SHORTCUT_COMMANDS.map(c => logger(c));
  return [
    {
      extensions: [
        shortcuts({
          down: {command: 'KEY_ARROW_DOWN_COMMAND', key: 'j'},
          end: {
            command: 'MOVE_TO_END',
            key: 'e',
            modifiers: CONTROL_OR_META,
          },
          escape: {command: 'KEY_ESCAPE_COMMAND', key: 'q'},
          up: {
            command: 'KEY_ARROW_UP_COMMAND',
            key: 'K',
            modifiers: {shiftKey: 'any'},
          },
        }),
      ],
      handlers,
      name: 'KeyboardShortcutsExtension dispatches the command of a matching shortcut',
      steps: [
        start(p(t('ab'))),
        keydown({code: 'KeyQ', key: 'q'}),
        // Keys match case-insensitively, and 'any' takes either state.
        keydown({code: 'KeyK', key: 'k'}),
        keydown({code: 'KeyK', key: 'K', shiftKey: true}),
        // A modifier that is not in the mask must not be pressed.
        keydown({code: 'KeyJ', ctrlKey: true, key: 'j'}),
        keydown({code: 'KeyJ', key: 'j'}),
        // CONTROL_OR_META is Ctrl off Apple platforms.
        keydown({code: 'KeyE', ctrlKey: true, key: 'e'}),
        keydown({code: 'KeyE', key: 'e', metaKey: true}),
        // A non-Latin layout matches by its code.
        keydown({code: 'KeyQ', key: 'й'}),
        // No shortcut: the core's handler takes it.
        keydown({code: 'KeyX', key: 'x'}),
      ],
    },
    {
      extensions: [
        shortcuts({
          first: {command: 'KEY_ESCAPE_COMMAND', key: 'q'},
          second: {command: 'KEY_ARROW_UP_COMMAND', key: 'q'},
          two: [
            {command: 'KEY_ARROW_DOWN_COMMAND', key: 'w'},
            {command: 'MOVE_TO_END', key: 'w'},
          ],
        }),
      ],
      handlers: [
        logger('KEY_ESCAPE_COMMAND'),
        logger('KEY_ARROW_UP_COMMAND', true),
        logger('KEY_ARROW_DOWN_COMMAND', true),
        logger('MOVE_TO_END', true),
      ],
      name: 'an unhandled shortcut falls through to the next match',
      steps: [
        start(p(t('ab'))),
        keydown({code: 'KeyQ', key: 'q'}),
        keydown({code: 'KeyW', key: 'w'}),
      ],
    },
    {
      extensions: [
        shortcuts(
          {
            first: {command: 'KEY_ESCAPE_COMMAND', key: 'q'},
            gone: {command: 'KEY_ARROW_DOWN_COMMAND', key: 'w'},
            moved: {command: 'KEY_ARROW_UP_COMMAND', key: 'e'},
          },
          {
            added: {command: 'MOVE_TO_END', key: 'q'},
            gone: null,
            moved: {command: 'KEY_ARROW_UP_COMMAND', key: 'r'},
          },
        ),
      ],
      handlers,
      name: 'shortcuts merge by name: remapped, disabled, and the overrides first',
      steps: [
        start(p(t('ab'))),
        keydown({code: 'KeyQ', key: 'q'}),
        keydown({code: 'KeyW', key: 'w'}),
        keydown({code: 'KeyE', key: 'e'}),
        keydown({code: 'KeyR', key: 'r'}),
      ],
    },
    {
      extensionDefs: {
        dep: {
          dependencies: [
            shortcuts({
              fromDep: {command: 'KEY_ARROW_DOWN_COMMAND', key: 'w'},
              shared: {command: 'KEY_ESCAPE_COMMAND', key: 'q'},
            }),
          ],
          name: 'dep',
          phases: [],
        },
      },
      extensions: [
        'dep',
        shortcuts({shared: {command: 'KEY_ARROW_UP_COMMAND', key: 'q'}}),
      ],
      handlers,
      name: 'an extension and the app both configure the shortcuts',
      steps: [
        start(p(t('ab'))),
        keydown({code: 'KeyQ', key: 'q'}),
        keydown({code: 'KeyW', key: 'w'}),
      ],
    },
    {
      extensions: [
        shortcuts({
          disabled: {
            $disabled: 'always',
            command: 'KEY_ESCAPE_COMMAND',
            key: 'q',
          },
          logged: {
            $dispatch: 'log',
            command: 'KEY_ARROW_DOWN_COMMAND',
            key: 'w',
          },
          range: {
            $disabled: 'notRange',
            command: 'KEY_ARROW_UP_COMMAND',
            key: 'q',
          },
          unhandled: {
            $dispatch: 'unhandled',
            command: 'MOVE_TO_END',
            key: 'e',
          },
        }),
      ],
      handlers: [
        logger('KEY_ESCAPE_COMMAND', true),
        logger('KEY_ARROW_UP_COMMAND', true),
        logger('KEY_ARROW_DOWN_COMMAND', true),
        logger('MOVE_TO_END', true),
      ],
      name: '$disabled and $dispatch',
      steps: [
        start(p(t('ab'))),
        // No selection: both are disabled.
        keydown({code: 'KeyQ', key: 'q'}),
        update(select(['3', 1, 'text'])),
        keydown({code: 'KeyQ', key: 'q'}),
        keydown({code: 'KeyW', key: 'w'}),
        keydown({code: 'KeyE', key: 'e'}),
      ],
    },
    {
      extensions: [
        [
          'KeyboardShortcutsExtension',
          {
            disabled: true,
            shortcuts: {escape: {command: 'KEY_ESCAPE_COMMAND', key: 'q'}},
          } as never,
        ],
      ],
      handlers,
      name: 'a disabled KeyboardShortcutsExtension registers nothing',
      steps: [start(p(t('ab'))), keydown({code: 'KeyQ', key: 'q'})],
    },
    {
      extensions: [
        [
          'KeyboardShortcutsExtension',
          {
            priority: 0,
            shortcuts: {escape: {command: 'KEY_ESCAPE_COMMAND', key: 'q'}},
          } as never,
        ],
      ],
      handlers,
      name: 'at COMMAND_PRIORITY_EDITOR the core handler comes first',
      steps: [start(p(t('ab'))), keydown({code: 'KeyQ', key: 'q'})],
    },
    {
      name: 'formatKeyboardShortcut',
      steps: [
        update(
          format({key: 'k'}),
          format({key: 'k', modifiers: CONTROL_OR_META}),
          format({key: 'k', modifiers: CONTROL_OR_META}, true),
          format({key: 'k', modifiers: CONTROL_OR_META}, false),
          format(
            {
              key: 'k',
              modifiers: {
                altKey: true,
                ctrlKey: true,
                metaKey: true,
                shiftKey: true,
              },
            },
            true,
          ),
          format(
            {
              key: 'k',
              modifiers: {
                altKey: true,
                ctrlKey: true,
                metaKey: true,
                shiftKey: true,
              },
            },
            false,
          ),
          format(
            {key: '?', modifiers: {shiftKey: true}, unshiftedKey: '/'},
            true,
          ),
          format(
            {key: '?', modifiers: {shiftKey: true}, unshiftedKey: '/'},
            false,
          ),
          format({key: '?', modifiers: {shiftKey: true}}, true),
          format({key: 'Tab', modifiers: {shiftKey: true}}, true),
          format({key: 'Tab'}, true),
          format({key: 'Tab', modifiers: {shiftKey: true}}, false),
          format({key: ' ', modifiers: {ctrlKey: true}}, false),
          format({key: 'ArrowUp', modifiers: {altKey: true}}, true),
          format({key: 'ArrowUp', modifiers: {altKey: true}}, false),
          format(
            {key: 'Enter', modifiers: {metaKey: 'any', shiftKey: 'any'}},
            true,
          ),
          format(
            {
              key: 'Backspace',
              modifiers: {
                altKey: true,
                controlOrOther: 'altKey',
                ctrlKey: false,
              },
            },
            false,
          ),
          format({key: 'é', modifiers: {ctrlKey: true}}, false),
          format({key: 'F5'}, false),
        ),
      ],
    },
  ];
}

function clickAfterLastBlockScripts(): Script[] {
  const extensionDefs = {
    rule: {name: 'rule', nodes: ['horizontalrule'], phases: []},
  };
  const below: DomEventJson = {clientY: 10, type: 'click'};
  return [
    {
      extensionDefs,
      extensions: ['rule', 'ClickAfterLastBlockExtension'],
      listen: true,
      name: 'ClickAfterLastBlockExtension inserts a paragraph after a last decorator',
      steps: [
        // Keys: the paragraph 2, its text 3, the rule 4.
        start(p(t('ab')), hr()),
        {setRootElement: true},
        domEvent(below),
        // Now the last block is the new paragraph: nothing happens.
        domEvent(below),
      ],
    },
    {
      extensionDefs,
      extensions: ['rule', 'ClickAfterLastBlockExtension'],
      name: 'a click on a block, or in a read-only editor, is left alone',
      steps: [
        start(p(t('ab')), hr()),
        {setRootElement: true},
        domEvent({...below, targetKey: '4'}),
        domEvent({...below, targetKey: '2'}),
        {setEditable: false},
        domEvent(below),
        {setEditable: true},
        domEvent(below),
      ],
    },
    {
      extensionDefs,
      extensions: [
        'rule',
        ['ClickAfterLastBlockExtension', {$shouldInsertAfter: 'always'}],
      ],
      name: '$shouldInsertAfter',
      steps: [start(p(t('ab'))), {setRootElement: true}, domEvent(below)],
    },
    {
      extensionDefs,
      extensions: ['rule', ['ClickAfterLastBlockExtension', {disabled: true}]],
      name: 'a disabled ClickAfterLastBlockExtension',
      steps: [start(p(t('ab')), hr()), {setRootElement: true}, domEvent(below)],
    },
  ];
}

function tripleClickScripts(): Script[] {
  const extensions = ['NormalizeTripleClickSelectionExtension'];
  const fix = (
    anchor: [string, number, 'text' | 'element'],
    focus: [string, number, 'text' | 'element'],
  ): Step => update(select(anchor, focus), {do: 'fixFocusOverselection'});
  return [
    {
      extensions,
      name: '$fixFocusOverselection moves the focus back into the clicked block',
      steps: [
        // Keys: the paragraphs 2, 4 and 6, their texts 3, 5 and 7.
        start(p(t('abc')), p(t('def')), p(t('ghi'))),
        // The start of the next block's text.
        fix(['3', 0, 'text'], ['5', 0, 'text']),
        // The start of the next block.
        fix(['3', 0, 'text'], ['4', 0, 'element']),
        fix(['2', 0, 'element'], ['4', 0, 'element']),
        // Into the next block's text: left as it is.
        fix(['3', 0, 'text'], ['5', 1, 'text']),
        // Collapsed: left as it is.
        fix(['3', 1, 'text'], ['3', 1, 'text']),
        // Across two blocks to the start of a third.
        fix(['3', 1, 'text'], ['7', 0, 'text']),
        // Backward: the anchor is the later point.
        fix(['5', 0, 'text'], ['3', 0, 'text']),
      ],
    },
    {
      extensions,
      name: '$fixFocusOverselection with line breaks and an empty block',
      steps: [
        // Keys: the paragraph 2, its text 3, the line break 4; the
        // paragraph 5 and its text 6; the empty paragraph 7.
        start(p(t('ab'), br()), p(t('cd')), p()),
        fix(['3', 0, 'text'], ['5', 0, 'element']),
        fix(['3', 0, 'text'], ['2', 2, 'element']),
        fix(['6', 0, 'text'], ['7', 0, 'element']),
        fix(['2', 0, 'element'], ['6', 0, 'text']),
      ],
    },
    // RichTextExtension and PlainTextExtension depend on it, so an editor
    // built from either has its output. (A triple click itself can't be
    // recorded: JS arms on the document's mousedown and trims the next
    // selection change that matches the DOM selection, which the session's
    // detached root has no counterpart of.)
    ...['RichTextExtension', 'PlainTextExtension'].map(
      (extension): Script => ({
        extensions: [extension],
        name: `${extension} brings NormalizeTripleClickSelectionExtension`,
        steps: [
          start(p(t('abc')), p(t('def'))),
          fix(['3', 0, 'text'], ['5', 0, 'text']),
        ],
      }),
    ),
  ];
}

function imeScripts(): Script[] {
  return [
    {
      extensions: ['RichTextExtension', 'IMEExtension'],
      name: 'IMEExtension follows a composition in a text node',
      steps: [
        {logIME: true},
        // Keys: the paragraph 2, its text 3.
        start(p(t('Hello'))),
        {now: 1000, setRootElement: true},
        {
          update: [
            select(['3', 5, 'text']),
            {
              command: 'COMPOSITION_START_COMMAND',
              do: 'dispatch',
              payload: {data: ''},
            },
          ],
        },
        {logIME: true},
        {
          update: [
            select(['3', 5, 'text']),
            {
              command: 'CONTROLLED_TEXT_INSERTION_COMMAND',
              do: 'dispatch',
              payload: 'か',
            },
          ],
        },
        {logIME: true},
        // JS's core handler of compositionend runs in an update that, with a
        // root element in Node, starts with no selection; the ports' start
        // with the last one, so the script clears it first.
        update({do: 'clearSelection'}),
        domEvent({data: 'か', type: 'compositionend'}),
        {logIME: true},
      ],
    },
    {
      extensions: ['RichTextExtension', 'IMEExtension'],
      name: 'removing the root element ends the composition',
      steps: [
        start(p(t('ab'))),
        {now: 1000, setRootElement: true},
        {
          update: [
            select(['3', 1, 'text']),
            {
              command: 'COMPOSITION_START_COMMAND',
              do: 'dispatch',
              payload: {data: ''},
            },
          ],
        },
        {logIME: true},
        {setRootElement: false},
        {logIME: true},
      ],
    },
  ];
}

function autoFocusScripts(): Script[] {
  return [
    {
      extensions: ['AutoFocusExtension'],
      listen: true,
      name: 'AutoFocusExtension focuses the editor when the root element is set',
      rootFocus: true,
      steps: [
        // Keys: the paragraphs 2 and 4, their texts 3 and 5.
        start(p(t('ab')), p(t('cd'))),
        {setRootElement: true},
        {setRootElement: false},
      ],
    },
    {
      extensions: [['AutoFocusExtension', {defaultSelection: 'rootStart'}]],
      name: 'AutoFocusExtension at the root start',
      rootFocus: true,
      steps: [start(p(t('ab')), p(t('cd'))), {setRootElement: true}],
    },
    {
      extensions: [['AutoFocusExtension', {disabled: true}]],
      name: 'a disabled AutoFocusExtension',
      rootFocus: true,
      steps: [start(p(t('ab')), p(t('cd'))), {setRootElement: true}],
    },
  ];
}

function clickableLinkScripts(): Script[] {
  const url = 'https://lexical.dev/';
  const log = ['ClickableLinkExtension', {getWindow: 'log'}] as ExtensionArg;
  // Keys: the paragraph 2, its text 3, the link 4 and its text 5.
  const doc = (extra: Record<string, Json> = {}, linkUrl = url): Step =>
    start(p(t('see '), a(linkUrl, extra, t('link'))));
  const click = (targetKey: string, extra: Partial<DomEventJson> = {}): Step =>
    domEvent({targetKey, type: 'click', ...extra});
  return [
    {
      extensions: [log],
      name: 'ClickableLinkExtension opens a clicked link',
      steps: [
        doc(),
        {setRootElement: true},
        click('5'),
        click('4'),
        // Not a link.
        click('3'),
        click('2'),
        // A new tab: Command or Control, or the middle button.
        click('5', {metaKey: true}),
        click('5', {ctrlKey: true}),
        domEvent({button: 1, targetKey: '5', type: 'auxclick'}),
        // The right button.
        domEvent({button: 2, targetKey: '5', type: 'auxclick'}),
      ],
    },
    {
      // AutoLinkNode alone, without AutoLinkExtension's transforms, which
      // would unlink an autolink no matcher finds.
      extensionDefs: {
        autolink: {name: 'autolink', nodes: ['autolink'], phases: []},
      },
      extensions: [log, 'autolink'],
      name: "a link's target, an unsafe URL and an unlinked autolink",
      steps: [
        doc({target: '_blank'}),
        {setRootElement: true},
        click('5'),
        // Keys: the paragraph 6, its text 7, the link 8 and its text 9.
        // eslint-disable-next-line no-script-url
        doc({}, 'javascript:alert(1)'),
        click('9'),
        start(
          p(
            t('see '),
            {
              ...(a(url, {}, t('auto')) as object),
              isUnlinked: true,
              type: 'autolink',
            },
            {
              ...(a(url, {}, t('link')) as object),
              isUnlinked: false,
              type: 'autolink',
            },
          ),
        ),
        // Keys: the paragraph 10, its text 11, the autolinks 12 and 14, their
        // texts 13 and 15.
        click('13'),
        click('15'),
      ],
    },
    {
      extensions: [
        ['ClickableLinkExtension', {getWindow: 'log', newTab: true}],
      ],
      name: 'newTab',
      steps: [doc(), {setRootElement: true}, click('5')],
    },
    {
      extensions: [
        ['ClickableLinkExtension', {disabled: true, getWindow: 'log'}],
      ],
      name: 'a disabled ClickableLinkExtension',
      steps: [doc(), {setRootElement: true}, click('5')],
    },
    {
      extensions: [['ClickableLinkExtension', {getWindow: 'none'}]],
      name: 'getWindow returning null leaves the click alone',
      steps: [doc(), {setRootElement: true}, click('5')],
    },
  ];
}

function builderScripts(): Script[] {
  return [
    {
      extensionDefs: {
        a: {editable: false, name: 'a', namespace: 'first', phases: []},
        b: {dependencies: ['a'], name: 'b', namespace: 'second', phases: []},
      },
      extensions: ['b'],
      name: "the builder's namespace and editable: the last one set wins",
      steps: [{logEditor: true}],
    },
    {
      extensionDefs: {
        a: {listen: true, name: 'a', namespace: 'ns', phases: ['register']},
      },
      extensions: ['a'],
      name: 'disposing a built editor removes its root element first',
      steps: [
        start(p(t('ab'))),
        {setRootElement: true},
        {logEditor: true},
        {dispose: true},
        {logEditor: true},
      ],
    },
  ];
}

export function extensionScripts(): Script[] {
  return [
    ...keyboardShortcutScripts(),
    ...clickAfterLastBlockScripts(),
    ...tripleClickScripts(),
    ...imeScripts(),
    ...autoFocusScripts(),
    ...clickableLinkScripts(),
    ...builderScripts(),
  ];
}
