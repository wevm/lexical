/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {SerializedLexicalNode} from 'lexical';

import {test} from 'vitest';

import {
  type Action,
  type HandlerSpec,
  runScript,
  type Script,
  type Step,
} from './ops';
import {emitVectors} from './vectors';

// Hand-written editor sessions over the core nodes, recorded step by step by
// the real JS code (see ops.ts). Keys are deterministic: in a session that
// starts with `doc(...)`, the root's descendants get keys 1, 2, 3… in
// document order.

type Node = SerializedLexicalNode & Record<string, unknown>;

function t(text: string, extra: Record<string, unknown> = {}): Node {
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

function tab(): Node {
  return {...t('\t'), type: 'tab'};
}

function br(): Node {
  return {type: 'linebreak', version: 1};
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
  } as Node;
}

/** The first step of most sessions: the root gets these blocks. */
function doc(...blocks: Node[]): Step {
  return {update: [{do: 'append', key: 'root', nodes: blocks}]};
}

function update(...actions: Action[]): Step {
  return {update: actions};
}

function select(
  anchor: [string, number, 'text' | 'element'],
  focus?: [string, number, 'text' | 'element'],
): Action {
  return {anchor, do: 'select', focus};
}

function call(method: string, ...args: unknown[]): Action {
  return {args: args as never, do: 'selection', method};
}

function node(key: string, method: string, ...args: unknown[]): Action {
  return {args: args as never, do: 'node', key, method};
}

function emit(name: string, description: string, scripts: Script[]) {
  emitVectors(`scripts/${name}`, 'script', description, scripts.map(runScript));
}

test('scripts/updates', () => {
  emit(
    'updates',
    'Node mutations in discrete updates: the state, selection and text after each step.',
    [
      {
        name: 'append, then edit text',
        steps: [
          doc(p(t('Hello')), p(t('world'))),
          update(node('2', 'setTextContent', 'Hello there')),
          update(node('4', 'toggleFormat', 'bold')),
          update(node('3', 'remove')),
        ],
      },
      {
        name: 'splitText and normalization merges',
        steps: [
          doc(p(t('abcdef'))),
          update(node('2', 'splitText', 2, 4)),
          // The split parts merge back at commit, and keys only grow, so
          // the second split's parts are 2, 5 and 6.
          update(
            node('2', 'splitText', 2, 4),
            node('5', 'toggleFormat', 'italic'),
          ),
          update(node('5', 'setTextContent', 'XY')),
          update(node('5', 'toggleFormat', 'italic')),
        ],
      },
      {
        name: 'insertBefore, insertAfter, replace',
        steps: [
          doc(p(t('middle'))),
          update(node('2', 'insertBefore', t('before ', {format: 1}))),
          update(node('2', 'insertAfter', t(' after', {format: 2}))),
          update(node('2', 'replace', t('MIDDLE'))),
          update(node('1', 'insertAfter', p(t('next')))),
        ],
      },
      {
        name: 'splice an element',
        steps: [
          doc(p(t('a'), br(), t('b'), tab(), t('c'))),
          update(node('1', 'splice', 1, 2, [t('X', {format: 1})])),
          update(node('1', 'splice', 0, 0, [br()])),
          update(node('1', 'splice', 0, 100, [])),
        ],
      },
      {
        name: 'element formats, indent, direction, text format',
        steps: [
          doc(p(t('one')), p()),
          update(
            node('1', 'setFormat', 'center'),
            node('1', 'setIndent', 3),
            node('1', 'setDirection', 'rtl'),
            node('3', 'setTextFormat', 1),
            node('3', 'setTextStyle', 'color: red'),
          ),
          update(node('1', 'setStyle', 'margin: 0')),
        ],
      },
      {
        name: 'text modes and details',
        steps: [
          doc(
            p(t('token', {format: 1}), t('segmented', {format: 2}), t('plain')),
          ),
          update(
            node('2', 'setMode', 'token'),
            node('3', 'setMode', 'segmented'),
            node('4', 'setDetail', 1),
          ),
          update(node('4', 'setDetail', 0), node('4', 'toggleDirectionless')),
        ],
      },
      {
        name: 'unmergeable text stays apart',
        steps: [
          doc(p(t('a'), t('b'))),
          update(node('2', 'toggleUnmergeable')),
          update(node('2', 'toggleUnmergeable')),
        ],
      },
      {
        name: 'mergeWithSibling',
        steps: [
          doc(p(t('a', {format: 1}), t('b'), t('c', {format: 2}))),
          update(node('3', 'mergeWithSibling', '4')),
          update(node('2', 'mergeWithSibling', '3')),
        ],
      },
      {
        name: 'spliceText moves the selection',
        steps: [
          doc(p(t('Hello world'))),
          update(node('2', 'select', 6, 6)),
          update(node('2', 'spliceText', 6, 0, 'big ', true)),
          update(node('2', 'spliceText', 0, 5, 'Bye', false)),
        ],
      },
      {
        name: 'clear the root leaves it empty',
        steps: [
          doc(p(t('a')), p(t('b'))),
          update(node('2', 'select', 1, 1)),
          update(node('root', 'clear')),
          update({do: 'append', key: 'root', nodes: [p()]}),
        ],
      },
      {
        name: 'a failing update keeps the previous state',
        steps: [
          doc(p(t('a'))),
          update(node('2', 'setTextContent', 'b'), node('missing', 'remove')),
          update(node('root', 'append', [t('text in root')])),
          update(node('2', 'setTextContent', 'c')),
        ],
      },
      {
        name: 'update tags',
        steps: [
          doc(p(t('a'))),
          {tag: ['history-merge'], update: [node('2', 'setTextContent', 'b')]},
        ],
      },
    ],
  );
});

test('scripts/selection', () => {
  emit(
    'selection',
    'RangeSelection and NodeSelection editing in discrete updates.',
    [
      {
        name: 'insertText into a collapsed selection',
        steps: [
          doc(p(t('Hello'))),
          update(select(['2', 5, 'text']), call('insertText', ' world')),
          update(select(['2', 0, 'text']), call('insertText', '> ')),
          update(
            select(['2', 2, 'text'], ['2', 7, 'text']),
            call('insertText', 'X'),
          ),
        ],
      },
      {
        name: 'insertText across blocks',
        steps: [
          doc(p(t('first')), p(t('second')), p(t('third'))),
          update(
            select(['2', 2, 'text'], ['6', 3, 'text']),
            call('insertText', '-'),
          ),
        ],
      },
      {
        name: 'backward selection across formatted text',
        steps: [
          doc(
            p(t('bold', {format: 1}), t(' plain '), t('italic', {format: 2})),
          ),
          update(
            select(['4', 3, 'text'], ['2', 1, 'text']),
            call('insertText', 'Z'),
          ),
        ],
      },
      {
        name: 'insertParagraph splits and keeps formats',
        steps: [
          doc(p(t('one two', {format: 1}))),
          update(select(['2', 3, 'text']), call('insertParagraph')),
          update(call('insertText', 'x')),
          update(select(['2', 0, 'text']), call('insertParagraph')),
        ],
      },
      {
        name: 'insertLineBreak',
        steps: [
          doc(p(t('abc'))),
          update(select(['2', 1, 'text']), call('insertLineBreak')),
          update(call('insertLineBreak', true)),
        ],
      },
      {
        name: 'insertRawText with tabs and newlines',
        steps: [
          doc(p(t('start'))),
          update(
            select(['2', 5, 'text']),
            call('insertRawText', ' a\tb\nc\r\nd'),
          ),
        ],
      },
      {
        name: 'removeText',
        steps: [
          doc(p(t('abc')), p(t('def'))),
          update(
            select(['2', 1, 'text'], ['4', 2, 'text']),
            call('removeText'),
          ),
        ],
      },
      // Deleting within text needs Selection.modify, which a headless
      // editor has no window for; these cases are resolved in the model.
      {
        name: 'deleteCharacter at block boundaries',
        steps: [
          doc(p(t('abc')), p(t('def')), p(t('ghi'))),
          update(select(['4', 0, 'text']), call('deleteCharacter', true)),
          update(select(['2', 6, 'text']), call('deleteCharacter', false)),
        ],
      },
      {
        name: 'deleteCharacter with an expanded selection',
        steps: [
          doc(p(t('a😀b'))),
          update(
            select(['2', 1, 'text'], ['2', 3, 'text']),
            call('deleteCharacter', true),
          ),
        ],
      },
      {
        name: 'formatText toggles and splits',
        steps: [
          doc(p(t('Hello world'))),
          update(
            select(['2', 0, 'text'], ['2', 5, 'text']),
            call('formatText', 'bold'),
          ),
          update(
            select(['2', 2, 'text'], ['3', 3, 'text']),
            call('formatText', 'italic'),
          ),
          update(call('formatText', 'bold')),
        ],
      },
      {
        name: 'formatText on a collapsed selection sets the selection format',
        steps: [
          doc(p(t('ab'))),
          update(select(['2', 1, 'text']), call('formatText', 'underline')),
          update(call('insertText', 'X')),
        ],
      },
      {
        name: 'toggleFormat and setStyle on the selection',
        steps: [
          doc(p(t('ab'))),
          update(
            select(['2', 1, 'text']),
            call('toggleFormat', 'code'),
            call('setStyle', 'color: blue'),
            call('insertText', 'Y'),
          ),
        ],
      },
      {
        name: 'insertNodes paragraphs into text',
        steps: [
          doc(p(t('abcd'))),
          update(
            select(['2', 2, 'text']),
            call('insertNodes', [p(t('X')), p(t('Y'))]),
          ),
        ],
      },
      {
        name: 'insertNodes inline nodes',
        steps: [
          doc(p(t('abcd'))),
          update(select(['2', 2, 'text']), {
            do: 'insertNodes',
            nodes: [t('X', {format: 1}), tab()],
          }),
        ],
      },
      {
        name: 'selectAll and delete',
        steps: [
          doc(p(t('a')), p(t('b'))),
          update({do: 'selectAll'}, call('removeText')),
        ],
      },
      {
        name: 'element points',
        steps: [
          doc(p(t('a'), t('b', {format: 1})), p()),
          update(select(['1', 1, 'element']), call('insertText', 'X')),
          update(select(['4', 0, 'element']), call('insertText', 'Y')),
          update(
            select(['root', 0, 'element'], ['root', 2, 'element']),
            call('insertText', 'Z'),
          ),
        ],
      },
      {
        name: 'NodeSelection insertText and removal of selected nodes',
        steps: [
          doc(p(t('a')), p(t('b'))),
          update({do: 'selectNodes', keys: ['1']}),
          update(node('1', 'remove')),
        ],
      },
      {
        name: 'selection moves when a selected node is removed',
        steps: [
          doc(p(t('a'), t('b', {format: 1}), t('c'))),
          update(select(['3', 1, 'text'])),
          update(node('3', 'remove')),
          update(node('2', 'remove')),
        ],
      },
      {
        name: 'node select helpers',
        steps: [
          doc(p(t('one')), p(t('two'))),
          update(node('2', 'selectEnd')),
          update(node('4', 'selectStart')),
          update(node('3', 'selectPrevious')),
          update(node('1', 'selectNext', 0, 1)),
          update(node('3', 'select', 0, 0)),
        ],
      },
    ],
  );
});

test('scripts/transforms', () => {
  emit(
    'transforms',
    'Node transforms: their call order (in the log) and the resulting states.',
    [
      {
        name: 'text transform runs on dirty text',
        steps: [
          doc(p(t('banana')), p(t('xyz'))),
          update(node('4', 'setTextContent', 'xyza')),
        ],
        transforms: ['text:upper-a'],
      },
      {
        name: 'split then bold chained transforms',
        steps: [
          doc(p(t('see #tag here'))),
          update(node('2', 'setTextContent', 'more #tags')),
        ],
        transforms: ['text:split-hash', 'text:bold-hash'],
      },
      {
        name: 'element transforms of intentionally and unintentionally dirty elements',
        steps: [
          doc(p(t('a')), p(t('b'))),
          update(node('2', 'setTextContent', 'aa')),
          update(node('3', 'setIndent', 1)),
        ],
        transforms: ['paragraph:log', 'root:log'],
      },
      {
        name: 'paragraph transform removes empty paragraphs',
        steps: [
          doc(p(t('a')), p(), p(t('b')), p()),
          update(node('5', 'remove')),
        ],
        transforms: ['paragraph:remove-empty', 'paragraph:log'],
      },
      {
        name: 'transforms see typed text',
        steps: [
          doc(p(t('x'))),
          update(select(['2', 1, 'text']), call('insertText', 'a #b')),
        ],
        transforms: ['text:upper-a', 'text:split-hash', 'text:bold-hash'],
      },
    ],
  );
});

test('scripts/commands', () => {
  emit(
    'commands',
    'Command dispatch: handler order by priority, propagation and nested dispatch.',
    [
      {
        handlers: [
          {command: 'A', id: 'low', priority: 0, result: true},
          {command: 'A', id: 'high', priority: 4, result: false},
          {command: 'A', id: 'mid', priority: 2, result: false},
          {command: 'A', id: 'never', priority: 0, result: true},
        ],
        name: 'priorities and propagation',
        steps: [doc(p(t('a'))), {dispatch: 'A', payload: 1}, {dispatch: 'B'}],
      },
      {
        handlers: [
          {
            actions: [node('2', 'setTextContent', 'changed')],
            command: 'EDIT',
            id: 'edit',
            priority: 1,
            result: true,
          },
          {
            actions: [{command: 'EDIT', do: 'dispatch', payload: 'nested'}],
            command: 'OUTER',
            id: 'outer',
            priority: 1,
            result: true,
          },
        ],
        name: 'handlers edit and dispatch inside the update',
        steps: [
          doc(p(t('a'))),
          {dispatch: 'OUTER', payload: {x: 1}},
          update({command: 'EDIT', do: 'dispatch', payload: [1, 2]}),
        ],
      },
      {
        handlers: [
          {
            actions: [node('missing', 'remove')],
            command: 'FAIL',
            id: 'fail',
            priority: 1,
            result: true,
          },
        ],
        name: 'a handler that throws',
        steps: [
          doc(p(t('a'))),
          {dispatch: 'FAIL'},
          update(node('2', 'setTextContent', 'b')),
        ],
      },
    ],
  );
});

test('scripts/listeners', () => {
  emit(
    'listeners',
    'Update, mutation and text content listener calls for each step.',
    [
      {
        listen: true,
        name: 'dirty elements and leaves',
        steps: [
          doc(p(t('a')), p(t('b'))),
          update(node('2', 'setTextContent', 'aa')),
          update(node('3', 'setIndent', 1)),
          update(node('1', 'remove')),
          update(),
          {
            tag: ['history-merge', 'collaboration'],
            update: [node('4', 'setTextContent', 'c')],
          },
        ],
      },
      {
        listen: true,
        name: 'mutations from splitting and merging',
        steps: [
          doc(p(t('abcdef'))),
          update(node('2', 'splitText', 3), node('3', 'toggleFormat', 'bold')),
          update(node('3', 'remove')),
          update(select(['2', 3, 'text']), call('insertParagraph')),
        ],
      },
      {
        listen: true,
        name: 'a failing update notifies nothing',
        steps: [
          doc(p(t('a'))),
          update(node('2', 'setTextContent', 'b'), node('x', 'remove')),
        ],
      },
    ],
  );
});

test('scripts/caret', () => {
  emit(
    'caret',
    'Caret ranges from selections (their carets and text slices), normalization, and $removeTextFromCaretRange.',
    [
      {
        name: 'caret range of a selection within one text node',
        steps: [
          doc(p(t('Hello world'))),
          update(select(['2', 2, 'text'], ['2', 7, 'text']), {
            do: 'logCaretRange',
          }),
          update(select(['2', 7, 'text'], ['2', 2, 'text']), {
            do: 'logCaretRange',
          }),
        ],
      },
      {
        name: 'caret range across blocks',
        steps: [
          doc(p(t('one'), t('two', {format: 1})), p(t('three')), p()),
          update(select(['2', 1, 'text'], ['5', 2, 'text']), {
            do: 'logCaretRange',
          }),
          update(select(['1', 0, 'element'], ['6', 0, 'element']), {
            do: 'logCaretRange',
          }),
          update(select(['5', 4, 'text'], ['2', 0, 'text']), {
            do: 'logCaretRange',
          }),
        ],
      },
      {
        name: 'explicit caret ranges',
        steps: [
          doc(p(t('ab'), t('cd', {format: 1})), p(t('ef'))),
          update({
            anchor: ['1', 'next', 'child'],
            do: 'logCaretRange',
            focus: ['5', 'next', 'text', 1],
          }),
          update({
            anchor: ['3', 'previous', 'text', 'previous'],
            do: 'logCaretRange',
            focus: ['root', 'previous', 'child'],
          }),
        ],
      },
      {
        name: 'normalizeCaret',
        steps: [
          doc(p(t('ab'), t('cd', {format: 1})), p()),
          update(
            {caret: ['1', 'next', 'child'], do: 'logNormalizeCaret'},
            {caret: ['2', 'next', 'sibling'], do: 'logNormalizeCaret'},
            {caret: ['3', 'previous', 'sibling'], do: 'logNormalizeCaret'},
            {caret: ['4', 'next', 'child'], do: 'logNormalizeCaret'},
            {caret: ['root', 'previous', 'child'], do: 'logNormalizeCaret'},
          ),
        ],
      },
      {
        name: 'removeTextFromCaretRange',
        steps: [
          doc(p(t('Hello')), p(t('big', {format: 1}), t(' world'))),
          update(select(['2', 2, 'text'], ['5', 1, 'text']), {
            do: 'removeTextFromCaretRange',
          }),
          update(select(['2', 2, 'text'], ['2', 4, 'text']), {
            do: 'removeTextFromCaretRange',
          }),
          update(select(['root', 0, 'element'], ['root', 1, 'element']), {
            do: 'removeTextFromCaretRange',
          }),
        ],
      },
    ],
  );
});

test('scripts/history', () => {
  // `registerHistory` with a 300 ms delay, on the session's clock, which each
  // step sets with `now`. The handlers log CAN_UNDO_COMMAND and
  // CAN_REDO_COMMAND.
  const history = {delay: 300};
  const handlers: HandlerSpec[] = [
    {command: 'CAN_UNDO_COMMAND', id: 'canUndo', priority: 0, result: false},
    {command: 'CAN_REDO_COMMAND', id: 'canRedo', priority: 0, result: false},
  ];
  const at = (now: number, step: Step): Step => ({...step, now});
  const type = (now: number, text: string, tag?: string[]): Step => ({
    now,
    tag,
    update: [call('insertText', text)],
  });
  const undo = (now: number): Step => ({dispatch: 'UNDO_COMMAND', now});
  const redo = (now: number): Step => ({dispatch: 'REDO_COMMAND', now});
  emit(
    'history',
    '@lexical/history: which updates merge into one undo entry and which push a new one, undo, redo and clearing, and the CAN_UNDO/CAN_REDO notifications.',
    [
      {
        handlers,
        history,
        name: 'typing merges within the delay and pushes after it, then undo and redo',
        steps: [
          at(0, doc(p(t('Hello')))),
          // A selection-only update merges into the current entry.
          at(0, update(select(['2', 5, 'text']))),
          type(100, '!'),
          type(200, 'a'),
          // 499 < 200 + 300: merges.
          type(499, 'b'),
          // 799 is not < 499 + 300: pushes.
          type(799, 'c'),
          type(800, 'd'),
          undo(900),
          undo(901),
          // The undo stack is empty: nothing changes.
          undo(902),
          redo(903),
          redo(904),
          // The redo stack is empty: nothing changes.
          redo(905),
        ],
      },
      {
        handlers,
        history,
        listen: true,
        name: 'a new edit after undo clears the redo stack',
        steps: [
          at(0, doc(p(t('ab')))),
          at(0, update(select(['2', 2, 'text']))),
          type(1000, 'c'),
          type(2000, 'd'),
          undo(2100),
          undo(2200),
          type(3000, 'x'),
          redo(3100),
          undo(3200),
        ],
      },
      {
        handlers,
        history,
        name: 'history-merge and history-push tags',
        steps: [
          at(0, doc(p(t('a')))),
          at(0, update(select(['2', 1, 'text']))),
          type(1000, 'b'),
          // Within the delay, but tagged history-push.
          type(1010, 'c', ['history-push']),
          type(1020, 'd'),
          // Long after, but tagged history-merge.
          {
            now: 5000,
            tag: ['history-merge'],
            update: [node('2', 'setTextContent', 'merged')],
          },
          // An unchanged text node marked dirty merges, unless tagged
          // history-push.
          {now: 6000, tag: ['history-push'], update: [node('2', 'markDirty')]},
          at(7000, update(node('2', 'markDirty'))),
          undo(8000),
          undo(8001),
          undo(8002),
          undo(8003),
        ],
      },
      {
        handlers,
        history,
        name: 'selection-only updates merge, and one that clears the selection is discarded',
        steps: [
          at(0, doc(p(t('one')))),
          at(10, update(select(['2', 3, 'text']))),
          at(20, update({do: 'clearSelection'})),
          at(1000, update(select(['2', 1, 'text']), call('insertText', 'X'))),
          // Back to the selection at 3: the cleared one was never recorded.
          undo(1100),
          redo(1200),
        ],
      },
      {
        handlers,
        history,
        name: 'formatText, insertParagraph and a deletion, each undone',
        steps: [
          at(0, doc(p(t('Hello world')))),
          at(
            1000,
            update(
              select(['2', 0, 'text'], ['2', 5, 'text']),
              call('formatText', 'bold'),
            ),
          ),
          at(2000, update(node('2', 'selectEnd'), call('insertParagraph'))),
          at(3000, update({do: 'selectAll'}, call('removeText'))),
          undo(4000),
          undo(4001),
          undo(4002),
          undo(4003),
          redo(4004),
          redo(4005),
          redo(4006),
        ],
      },
      {
        history,
        name: 'typing into an empty paragraph merges from the first character',
        steps: [
          at(0, doc(p())),
          at(0, update(select(['1', 0, 'element']))),
          type(10, 'a'),
          type(20, 'b'),
          type(30, 'c'),
          undo(40),
          redo(50),
        ],
      },
      {
        handlers,
        history,
        name: 'deleting backward and forward',
        steps: [
          at(0, doc(p(t('abcdef')))),
          at(0, update(select(['2', 6, 'text']))),
          // Backward: the caret moves back over the deleted character.
          at(
            10,
            update(
              select(['2', 5, 'text'], ['2', 6, 'text']),
              call('removeText'),
            ),
          ),
          at(
            20,
            update(
              select(['2', 4, 'text'], ['2', 5, 'text']),
              call('removeText'),
            ),
          ),
          // Forward: the caret stays.
          at(
            30,
            update(
              select(['2', 2, 'text'], ['2', 3, 'text']),
              call('removeText'),
            ),
          ),
          at(
            40,
            update(
              select(['2', 2, 'text'], ['2', 3, 'text']),
              call('removeText'),
            ),
          ),
          undo(50),
          undo(60),
          undo(70),
        ],
      },
      {
        handlers,
        history,
        name: 'paste and cut are their own entries',
        steps: [
          at(0, doc(p(t('ab')))),
          at(0, update(select(['2', 2, 'text']))),
          type(10, 'c'),
          type(20, 'P', ['paste']),
          type(30, 'd'),
          {
            now: 40,
            tag: ['cut'],
            update: [
              select(['2', 0, 'text'], ['2', 1, 'text']),
              call('removeText'),
            ],
          },
          type(50, 'e'),
          undo(60),
          undo(70),
          undo(80),
          undo(90),
        ],
      },
      {
        handlers,
        history,
        name: 'a composition that changes nothing merges',
        steps: [
          at(0, doc(p(t('ab')))),
          at(0, update(select(['2', 2, 'text']))),
          type(1000, 'c'),
          type(2000, 'x', ['composition-start']),
          {
            now: 3000,
            tag: ['composition-end'],
            update: [
              select(['2', 3, 'text'], ['2', 4, 'text']),
              call('removeText'),
            ],
          },
          type(3010, 'd'),
          undo(4000),
          undo(4010),
        ],
      },
      {
        handlers,
        history,
        name: 'CLEAR_HISTORY_COMMAND and CLEAR_EDITOR_COMMAND',
        steps: [
          at(0, doc(p(t('a')))),
          at(0, update(select(['2', 1, 'text']))),
          type(1000, 'b'),
          type(2000, 'c'),
          undo(2100),
          {dispatch: 'CLEAR_HISTORY_COMMAND', now: 2200},
          undo(2300),
          redo(2400),
          type(3000, 'd'),
          type(4000, 'e'),
          // History clears, but nothing handles the command.
          {dispatch: 'CLEAR_EDITOR_COMMAND', now: 4100},
          undo(4200),
        ],
      },
    ],
  );
});

function h(tag: string, ...children: Node[]): Node {
  return {...p(...children), tag, type: 'heading'} as Node;
}

function q(...children: Node[]): Node {
  return {...p(...children), type: 'quote'} as Node;
}

test('scripts/rich-text', () => {
  // `@lexical/rich-text`'s nodes: what Enter (`insertParagraph`) and
  // Backspace at a block's start (`deleteCharacter`) do in a heading and a
  // quote, and a quote that is a shadow root.
  const richText = true;
  emit(
    'rich-text',
    "@lexical/rich-text's HeadingNode and QuoteNode: insertNewAfter, collapseAtStart, the shadow root quote, and their JSON.",
    [
      {
        name: 'Enter at the end, in the middle and at the start of a heading',
        richText,
        steps: [
          doc(
            {...h('h2', t('Title')), format: 'center', indent: 1} as Node,
            p(t('body')),
          ),
          update(select(['2', 5, 'text']), call('insertParagraph')),
          update(select(['2', 2, 'text']), call('insertParagraph')),
          update(select(['2', 0, 'text']), call('insertParagraph')),
          update(select(['2', 0, 'text']), call('insertText', 'X')),
        ],
      },
      {
        name: 'Enter in an empty heading and with a selection over it',
        richText,
        steps: [
          doc(h('h3'), h('h4', t('abcdef'))),
          update(select(['1', 0, 'element']), call('insertParagraph')),
          update(
            select(['3', 1, 'text'], ['3', 4, 'text']),
            call('insertParagraph'),
          ),
        ],
      },
      {
        name: 'Backspace at the start of a heading',
        richText,
        steps: [
          doc(p(t('before')), h('h1', t('Title')), h('h2')),
          // A heading with content stays; the selection merges nothing.
          update(select(['4', 0, 'text']), call('deleteCharacter', true)),
          // An empty heading becomes a paragraph.
          update(select(['5', 0, 'element']), call('deleteCharacter', true)),
        ],
      },
      {
        name: 'Enter and Backspace in a quote',
        richText,
        steps: [
          doc(p(t('a')), {
            ...q(t('quoted')),
            direction: 'rtl',
            format: 'right',
          } as Node),
          update(
            select(['4', 3, 'text']),
            {args: ['bold'], do: 'selection', method: 'formatText'},
            call('insertParagraph'),
          ),
          update(select(['4', 0, 'text']), call('deleteCharacter', true)),
        ],
      },
      {
        name: 'a shadow root quote',
        richText,
        steps: [
          doc(
            p(t('a')),
            {...q(p(t('one')), p(t('two'))), shadowRoot: true} as Node,
            {...q(), shadowRoot: 'yes'} as Node,
          ),
          update(select(['5', 0, 'text']), call('insertParagraph')),
          // Merges the new empty paragraph back.
          update(select(['5', 0, 'text']), call('deleteCharacter', true)),
          // At the start of the quote's first block, from a text point and
          // from the block's element point: headless JS needs the DOM's
          // Selection.modify to get to the quote's collapseAtStart, and
          // throws.
          update(select(['5', 0, 'text']), call('deleteCharacter', true)),
          update(select(['4', 0, 'element']), call('deleteCharacter', true)),
          // `shadowRoot: 'yes'` isn't a boolean, so this quote is inline: it
          // becomes a paragraph.
          update(select(['8', 0, 'element']), call('deleteCharacter', true)),
        ],
      },
      {
        name: 'a shadow root quote parsed with inline children',
        richText,
        steps: [
          doc({...q(t('loose'), br(), t('text')), shadowRoot: true} as Node),
          update(select(['2', 2, 'text']), call('insertText', '!')),
        ],
      },
    ],
  );
});
