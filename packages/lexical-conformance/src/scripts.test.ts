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
  type Json,
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

test('scripts/rich-text-commands', () => {
  // `registerRichText`'s handlers for the commands that act on the model,
  // dispatched outside an update as an input layer does.
  const richText = true;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const canUndo: HandlerSpec[] = [
    {command: 'CAN_UNDO_COMMAND', id: 'canUndo', priority: 0, result: false},
    {command: 'CAN_REDO_COMMAND', id: 'canRedo', priority: 0, result: false},
  ];
  emit(
    'rich-text-commands',
    "@lexical/rich-text's registerRichText: the handlers of the commands that edit text, format it, and insert, align and indent blocks.",
    [
      {
        name: 'inserting and deleting text',
        richText,
        steps: [
          doc(p(t('hello world'))),
          update(select(['2', 11, 'text'])),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', '!'),
          // Collapsed, these need the DOM's Selection.modify, so headless
          // JS throws.
          run('DELETE_CHARACTER_COMMAND', true),
          run('DELETE_WORD_COMMAND', true),
          run('DELETE_LINE_COMMAND', false),
          // Expanded, they delete the selection.
          update(select(['2', 10, 'text'], ['2', 12, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          update(select(['2', 0, 'text'], ['2', 2, 'text'])),
          run('DELETE_WORD_COMMAND', false),
          update(select(['2', 2, 'text'], ['2', 6, 'text'])),
          run('DELETE_LINE_COMMAND', true),
          update(select(['2', 0, 'text'], ['2', 1, 'text'])),
          run('REMOVE_TEXT_COMMAND'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'X'),
        ],
      },
      {
        name: 'formatting a range, a collapsed caret and a node selection',
        richText,
        steps: [
          doc(p(t('dogs.')), p(t('cats'))),
          update(select(['2', 0, 'text'], ['2', 5, 'text'])),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          update(select(['2', 0, 'text'], ['2', 4, 'text'])),
          run('FORMAT_TEXT_COMMAND', 'italic'),
          run('SET_TEXT_FORMAT_COMMAND', {bold: true, italic: false}),
          run('SET_TEXT_FORMAT_COMMAND', {subscript: true, superscript: true}),
          update(select(['4', 2, 'text'])),
          run('FORMAT_TEXT_COMMAND', 'underline'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'u'),
          run('FORMAT_TEXT_COMMAND', 'underline'),
          run('FORMAT_TEXT_COMMAND', 'underline'),
          update({do: 'selectNodes', keys: ['4']}),
          run('FORMAT_TEXT_COMMAND', 'strikethrough'),
        ],
      },
      {
        name: 'paragraphs, line breaks and tabs',
        richText,
        steps: [
          doc(h('h1', t('Title')), p(t('ab'))),
          update(select(['2', 5, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          update(select(['4', 1, 'text'])),
          run('INSERT_LINE_BREAK_COMMAND', false),
          run('INSERT_LINE_BREAK_COMMAND', true),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          run('INSERT_TAB_COMMAND'),
          update(select(['2', 1, 'text'], ['2', 3, 'text'])),
          run('INSERT_TAB_COMMAND'),
        ],
      },
      {
        name: 'aligning and indenting blocks',
        richText,
        steps: [
          doc(p(t('one')), h('h2', t('two')), q(t('three'))),
          update(select(['2', 1, 'text'], ['4', 1, 'text'])),
          run('FORMAT_ELEMENT_COMMAND', 'center'),
          run('INDENT_CONTENT_COMMAND'),
          run('INDENT_CONTENT_COMMAND'),
          update(select(['6', 2, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          run('FORMAT_ELEMENT_COMMAND', 'end'),
          update(select(['2', 0, 'text'], ['6', 5, 'text'])),
          run('OUTDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          run('FORMAT_ELEMENT_COMMAND', ''),
        ],
      },
      {
        name: 'select all, deleting a node selection, and no selection',
        richText,
        steps: [
          doc(p(t('one')), p(t('two'))),
          update({do: 'clearSelection'}),
          run('DELETE_CHARACTER_COMMAND', true),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          run('INDENT_CONTENT_COMMAND'),
          run('INSERT_PARAGRAPH_COMMAND'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'x'),
          run('SELECT_ALL_COMMAND'),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          update({do: 'selectNodes', keys: ['4']}),
          run('DELETE_CHARACTER_COMMAND', false),
          update({do: 'selectNodes', keys: ['1']}),
          run('DELETE_CHARACTER_COMMAND', true),
        ],
      },
      {
        handlers: canUndo,
        history: {delay: 300},
        name: 'bold on and off in two presses, with history',
        richText,
        steps: [
          doc(p(t('The quick dogs.'))),
          {...update(select(['2', 10, 'text'], ['2', 15, 'text'])), now: 1000},
          {...run('FORMAT_TEXT_COMMAND', 'bold'), now: 2000},
          {...run('FORMAT_TEXT_COMMAND', 'bold'), now: 3000},
          // At a collapsed caret, only the pending format changes.
          {...update(select(['2', 0, 'text'])), now: 4000},
          {...run('FORMAT_TEXT_COMMAND', 'bold'), now: 5000},
          {...run('FORMAT_TEXT_COMMAND', 'bold'), now: 6000},
          {...run('FORMAT_TEXT_COMMAND', 'bold'), now: 7000},
          {...run('CONTROLLED_TEXT_INSERTION_COMMAND', 'A'), now: 8000},
          {...run('UNDO_COMMAND'), now: 9000},
          {...run('UNDO_COMMAND'), now: 9100},
          {...run('UNDO_COMMAND'), now: 9200},
          {...run('REDO_COMMAND'), now: 9300},
        ],
      },
    ],
  );
});

test('scripts/plain-text', () => {
  // `registerPlainText`'s handlers for the commands that act on the model,
  // dispatched outside an update as an input layer does. Where they differ
  // from rich text: Enter inserts a line break, and nothing handles
  // formatting, a node selection or a missing selection.
  const plainText = true;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  emit(
    'plain-text',
    "@lexical/plain-text's registerPlainText: the handlers of the commands that edit text and insert line breaks.",
    [
      {
        name: 'typing, Enter and line breaks',
        plainText,
        steps: [
          doc(p(t('one two'))),
          update(select(['2', 3, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'x'),
          run('INSERT_LINE_BREAK_COMMAND', true),
          run('INSERT_LINE_BREAK_COMMAND', false),
          update(select(['2', 1, 'text'], ['4', 1, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          run('SELECT_ALL_COMMAND'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'all'),
        ],
      },
      {
        name: 'deleting',
        plainText,
        steps: [
          doc(p(t('hello world')), p(t('second'))),
          // Collapsed, these need the DOM's Selection.modify, so headless
          // JS throws.
          update(select(['2', 5, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          run('DELETE_WORD_COMMAND', false),
          run('DELETE_LINE_COMMAND', true),
          update(select(['2', 9, 'text'], ['4', 2, 'text'])),
          run('DELETE_CHARACTER_COMMAND', false),
          update(select(['2', 0, 'text'], ['2', 3, 'text'])),
          run('DELETE_WORD_COMMAND', true),
          update(select(['2', 1, 'text'], ['2', 4, 'text'])),
          run('DELETE_LINE_COMMAND', false),
          update(select(['2', 0, 'text'], ['2', 1, 'text'])),
          run('REMOVE_TEXT_COMMAND'),
        ],
      },
      {
        name: 'what plain text leaves unhandled',
        plainText,
        steps: [
          doc(p(t('one')), p(t('two'))),
          update(select(['2', 0, 'text'], ['2', 3, 'text'])),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          run('FORMAT_ELEMENT_COMMAND', 'center'),
          run('INDENT_CONTENT_COMMAND'),
          run('INSERT_TAB_COMMAND'),
          update({do: 'selectNodes', keys: ['4']}),
          run('DELETE_CHARACTER_COMMAND', true),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'x'),
          run('INSERT_PARAGRAPH_COMMAND'),
          update({do: 'clearSelection'}),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'x'),
          run('REMOVE_TEXT_COMMAND'),
          run('SELECT_ALL_COMMAND'),
        ],
      },
      {
        handlers: [
          {
            command: 'CAN_UNDO_COMMAND',
            id: 'canUndo',
            priority: 0,
            result: false,
          },
        ],
        history: {delay: 300},
        name: 'typing and Enter with history',
        plainText,
        steps: [
          doc(p(t('ab'))),
          {...update(select(['2', 2, 'text'])), now: 1000},
          {...run('CONTROLLED_TEXT_INSERTION_COMMAND', 'c'), now: 2000},
          {...run('CONTROLLED_TEXT_INSERTION_COMMAND', 'd'), now: 2100},
          {...run('INSERT_PARAGRAPH_COMMAND'), now: 2200},
          {...run('CONTROLLED_TEXT_INSERTION_COMMAND', 'e'), now: 3000},
          {...run('UNDO_COMMAND'), now: 4000},
          {...run('UNDO_COMMAND'), now: 4100},
          {...run('REDO_COMMAND'), now: 4200},
        ],
      },
    ],
  );
});

function li(...children: Node[]): Node {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    type: 'listitem',
    value: 1,
    version: 1,
  } as Node;
}

function list(listType: 'number' | 'bullet' | 'check', ...items: Node[]): Node {
  return {
    children: items,
    direction: null,
    format: '',
    indent: 0,
    listType,
    start: 1,
    tag: listType === 'number' ? 'ol' : 'ul',
    type: 'list',
    version: 1,
  } as Node;
}

test('scripts/list', () => {
  // `@lexical/list`'s nodes and `registerList`'s and `registerCheckList`'s
  // model command handlers, dispatched outside an update as an input layer
  // does, with rich text's handlers where Enter, Backspace and indenting need
  // them.
  const richText = true;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  emit(
    'list',
    "@lexical/list's ListNode and ListItemNode, registerList's and registerCheckList's command handlers and transforms: inserting and removing lists, Enter and Backspace in items, indenting, check items, numbering and merging.",
    [
      {
        list: true,
        name: 'inserting lists over paragraphs and a heading',
        richText,
        steps: [
          doc(
            p(t('one')),
            p(t('two')),
            {...h('h2', t('three')), format: 'center', indent: 1} as Node,
            p(t('four')),
          ),
          update(select(['2', 1, 'text'])),
          run('INSERT_ORDERED_LIST_COMMAND'),
          update(select(['4', 0, 'text'], ['6', 2, 'text'])),
          run('INSERT_UNORDERED_LIST_COMMAND'),
          // Next to a bullet list, the new item joins it.
          update(select(['8', 4, 'text'])),
          run('INSERT_UNORDERED_LIST_COMMAND'),
          // Lists become check lists, and the transform merges them.
          run('SELECT_ALL_COMMAND'),
          run('INSERT_CHECK_LIST_COMMAND'),
          run('INSERT_ORDERED_LIST_COMMAND'),
        ],
      },
      {
        list: true,
        name: 'inserting a list at an element point and in an empty item',
        richText,
        steps: [
          doc(p(), p(t('x'))),
          update(select(['1', 0, 'element'])),
          run('INSERT_UNORDERED_LIST_COMMAND'),
          // In an empty item, its list changes type.
          run('INSERT_ORDERED_LIST_COMMAND'),
          // At the root, the first block's start.
          update(select(['root', 1, 'element'])),
          run('INSERT_CHECK_LIST_COMMAND'),
          // No selection: nothing changes, but the command is handled.
          update({do: 'clearSelection'}),
          run('INSERT_UNORDERED_LIST_COMMAND'),
          run('REMOVE_LIST_COMMAND'),
        ],
      },
      {
        list: true,
        name: 'removing a list',
        richText,
        steps: [
          doc(
            list(
              'bullet',
              {
                ...li(t('a')),
                direction: 'rtl',
                format: 'center',
                style: 'color: red',
              } as Node,
              li(t('b')),
            ),
            p(t('c')),
            list('number', li(t('d')), li(list('number', li(t('e'))))),
            list('bullet', li()),
          ),
          update(select(['3', 1, 'text'])),
          run('REMOVE_LIST_COMMAND'),
          // A nested item's whole list goes.
          update(select(['14', 1, 'text'])),
          run('REMOVE_LIST_COMMAND'),
          // An empty item, at its element point.
          update(select(['16', 0, 'element'])),
          run('REMOVE_LIST_COMMAND'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'f'),
        ],
      },
      {
        list: true,
        name: 'Enter in a list item',
        richText,
        steps: [
          doc(list('number', li(t('first')), li(t('second'))), p(t('after'))),
          update(select(['3', 5, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'x'),
          update(select(['5', 3, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          run('INSERT_PARAGRAPH_COMMAND'),
          // In the empty item Enter left, Enter leaves the list.
          update(select(['5', 3, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          run('INSERT_PARAGRAPH_COMMAND'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'out'),
        ],
      },
      {
        list: true,
        name: 'Enter in an empty item, a blank one and a nested one',
        richText,
        steps: [
          doc(
            list('bullet', li(t('a')), li(), li(t('c'))),
            list('bullet', li(t('d')), li(list('bullet', li())), li(t('f'))),
            list('number', li(t('g')), li(t('  ')), li(t('i'))),
          ),
          update(select(['4', 0, 'element'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          update(select(['12', 0, 'element'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          update(select(['19', 1, 'text'])),
          {...run('FORMAT_TEXT_COMMAND', 'bold')},
          run('INSERT_PARAGRAPH_COMMAND'),
        ],
      },
      {
        list: true,
        name: 'Backspace at the start of an item',
        richText,
        steps: [
          doc(
            p(t('p')),
            list('bullet', li(t('a')), li(t('b')), li(t('c'))),
            list('bullet', li(t('d')), li(list('bullet', li(t('e'))))),
          ),
          // From a text point: the item becomes a paragraph between two
          // lists.
          update(select(['7', 0, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          // From the first item's element point.
          update(select(['4', 0, 'element'])),
          run('DELETE_CHARACTER_COMMAND', true),
          // A nested item outdents.
          update(select(['16', 0, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          // Expanded, across items.
          update(select(['9', 0, 'text'], ['12', 1, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          // Collapsed inside text, headless JS needs the DOM and throws.
          update(select(['12', 1, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
        ],
      },
      {
        list: true,
        name: "Backspace at the start of the document's first item",
        richText,
        steps: [
          doc(
            list('bullet', li(list('bullet', li(t('a')))), li(t('b'))),
            p(t('c')),
          ),
          // With nothing before the item, the deletion needs the DOM's
          // Selection.modify before it gets to ListItemNode.collapseAtStart,
          // so headless JS throws, from a text point and from an empty
          // item's element point.
          update(select(['5', 0, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          update({do: 'clearSelection'}, node('root', 'clear'), {
            do: 'append',
            key: 'root',
            nodes: [list('number', li())],
          }),
          update(select(['11', 0, 'element'])),
          run('DELETE_CHARACTER_COMMAND', true),
        ],
      },
      {
        list: true,
        name: 'indenting and outdenting items',
        richText,
        steps: [
          doc(
            list('bullet', li(t('a')), li(t('b')), li(t('c')), li(t('d'))),
            p(t('p')),
          ),
          update(select(['5', 0, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          // Next to the nested list, the item joins it.
          update(select(['7', 0, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          update(select(['5', 0, 'text'], ['11', 1, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          // The first item, with nothing before it.
          update(select(['3', 0, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          // setIndent nests an item and unwraps it again.
          update(node('8', 'setIndent', 2)),
          update(node('8', 'setIndent', 0)),
        ],
      },
      {
        list: true,
        name: 'check items',
        richText,
        steps: [
          doc(
            list(
              'check',
              {...li(t('done')), checked: true} as Node,
              {...li(t('todo')), checked: false} as Node,
            ),
          ),
          update(node('4', 'toggleChecked')),
          update(node('2', 'toggleChecked'), node('2', 'toggleChecked')),
          // A new item after a checked one starts unchecked.
          update(select(['3', 4, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          // In a bullet list the transform clears `checked`.
          update(select(['5', 0, 'text'])),
          run('INSERT_UNORDERED_LIST_COMMAND'),
          run('INSERT_CHECK_LIST_COMMAND'),
        ],
      },
      {
        list: true,
        name: 'list start, numbering and merging adjacent lists',
        steps: [
          doc(
            {...list('number', li(t('a')), li(t('b'))), start: 3} as Node,
            list('number', li(t('c'))),
            list('bullet', li(t('d'))),
            list('bullet', li(t('e'))),
          ),
          run('UPDATE_LIST_START_COMMAND', {listNodeKey: '1', newStart: 10}),
          // A bullet list keeps its start, and a key that is no list is
          // not handled.
          run('UPDATE_LIST_START_COMMAND', {listNodeKey: '9', newStart: 5}),
          run('UPDATE_LIST_START_COMMAND', {listNodeKey: '3', newStart: 2}),
          // Nested lists at the seam of two merged lists join.
          update(
            {do: 'clearSelection'},
            {
              do: 'append',
              key: 'root',
              nodes: [
                list('bullet', li(t('f')), li(list('bullet', li(t('g'))))),
                list('bullet', li(list('bullet', li(t('h')))), li(t('i'))),
              ],
            },
          ),
        ],
      },
      {
        list: true,
        name: 'appending to, replacing in and splitting lists',
        richText,
        steps: [
          // A list wraps what isn't an item; an item takes a paragraph's
          // children.
          doc(
            list('bullet', p(t('wrapped')), li(p(t('merged')), t('x'))),
            p(t('end')),
          ),
          // An item outside a list gets one.
          update({do: 'append', key: 'root', nodes: [li(t('orphan'))]}),
          update(node('1', 'append', [li(t('b')), li(t('c')), li(t('d'))])),
          // Replacing a middle item splits the list.
          update(node('16', 'replace', p(t('replaced')))),
          // A block after an item splits the list too.
          update(node('5', 'insertAfter', p(t('between')))),
          update(node('4', 'remove')),
          // Removing an item between two nested lists joins them.
          update({
            do: 'append',
            key: 'root',
            nodes: [
              list(
                'bullet',
                li(list('bullet', li(t('x')))),
                li(t('y')),
                li(list('bullet', li(t('z')))),
              ),
            ],
          }),
          update(node('31', 'remove')),
        ],
      },
      {
        history: {delay: 300},
        list: true,
        name: 'undoing a list insert',
        richText,
        steps: [
          doc(p(t('one')), p(t('two'))),
          {...update(select(['2', 0, 'text'], ['4', 3, 'text'])), now: 1000},
          {...run('INSERT_ORDERED_LIST_COMMAND'), now: 2000},
          {...run('UNDO_COMMAND'), now: 3000},
          {...run('REDO_COMMAND'), now: 4000},
          {...run('REMOVE_LIST_COMMAND'), now: 5000},
          {...run('UNDO_COMMAND'), now: 6000},
        ],
      },
    ],
  );
});

function a(url: string, ...children: Node[]): Node {
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
  } as Node;
}

function autolink(url: string, ...children: Node[]): Node {
  return {...a(url, ...children), isUnlinked: false, type: 'autolink'} as Node;
}

test('scripts/link', () => {
  // `@lexical/link`'s nodes and `registerLink`'s TOGGLE_LINK_COMMAND
  // handler and `$linkNodeTransform`, dispatched outside an update as an
  // input layer does, with rich text's handlers for typing and Enter.
  const richText = true;
  const link = true;
  const url = 'https://lexical.dev';
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const toggle = (payload: Json): Step => run('TOGGLE_LINK_COMMAND', payload);
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  emit(
    'link',
    "@lexical/link's LinkNode and AutoLinkNode, $toggleLink through registerLink's TOGGLE_LINK_COMMAND handler, and $linkNodeTransform: linking, changing and unlinking, typing at a link's edges, Enter in a link, autolinks and undo.",
    [
      {
        link,
        name: 'linking part of one text node, then changing and removing it',
        richText,
        steps: [
          doc(p(t('Hello world'))),
          update(select(['2', 6, 'text'], ['2', 11, 'text'])),
          toggle(url),
          // The selection now covers the link's text: the link changes.
          toggle('https://example.com'),
          toggle(null),
          // Backward, in the middle of the text.
          update(select(['2', 4, 'text'], ['2', 1, 'text'])),
          toggle(url),
        ],
      },
      {
        link,
        name: 'linking across text nodes and blocks',
        richText,
        steps: [
          doc(
            p(t('one '), t('bold', {format: 1}), t(' two')),
            p(t('three')),
            h('h2', t('four')),
          ),
          update(select(['2', 2, 'text'], ['8', 2, 'text'])),
          toggle(url),
          // Over the same blocks, a new URL updates every link.
          run('SELECT_ALL_COMMAND'),
          toggle('https://example.com'),
          toggle(null),
        ],
      },
      {
        link,
        name: 'linking a range with element points, forward and backward',
        richText,
        steps: [
          doc(
            p(t('a'), t('b', {format: 1}), t('c')),
            p(t('d'), t('e', {format: 2})),
          ),
          update(select(['1', 0, 'element'], ['1', 2, 'element'])),
          toggle(url),
          update(select(['5', 2, 'element'], ['5', 0, 'element'])),
          toggle('https://example.com'),
          // From inside the first paragraph's link to the end of the
          // second's: the two links join one URL.
          update(select(['1', 0, 'element'], ['5', 2, 'element'])),
          toggle('https://lexical.dev/docs'),
        ],
      },
      {
        link,
        name: 'link attributes',
        richText,
        steps: [
          doc(p(t('abc def'))),
          update(select(['2', 0, 'text'], ['2', 3, 'text'])),
          toggle({rel: 'noopener', target: '_blank', title: 'Lexical', url}),
          // A string keeps the target and title, and rel goes back to
          // noreferrer.
          toggle('https://example.com'),
          // An object sets only what it carries; null clears it.
          toggle({target: null, title: 'Example', url: 'https://example.com'}),
          // Over the link and the text after it, both get the attributes
          // given.
          run('SELECT_ALL_COMMAND'),
          toggle({rel: null, title: 'All', url}),
        ],
      },
      {
        link,
        name: 'changing the URL of an existing link at a caret',
        richText,
        steps: [
          doc(
            p(t('a '), a('https://old.example', t('link')), t(' b')),
            p(a('https://old.example', t('c')), a(url, t('d'))),
          ),
          update(select(['4', 2, 'text'])),
          toggle('https://new.example'),
          // At the caret, null removes the whole link.
          toggle(null),
          // Now equal to the next link, which isn't dirty: the transform
          // merges it in.
          update(select(['8', 1, 'text'])),
          toggle({rel: null, url}),
        ],
      },
      {
        link,
        name: 'unlinking part of a link, at its start, middle and end',
        richText,
        steps: [
          doc(
            p(a(url, t('abcdef'))),
            p(a(url, t('ghijkl'))),
            p(a(url, t('mnopqr'))),
          ),
          update(select(['3', 2, 'text'], ['3', 4, 'text'])),
          toggle(null),
          update(select(['6', 0, 'text'], ['6', 2, 'text'])),
          toggle(null),
          update(select(['9', 4, 'text'], ['9', 6, 'text'])),
          toggle(null),
        ],
      },
      {
        link,
        name: 'unlinking across links, and a range that only touches one',
        richText,
        steps: [
          doc(
            p(
              a(url, t('one')),
              t(' and '),
              a('https://example.com', t('two'), t('2', {format: 1})),
            ),
            p(t('x'), a(url, t('three'))),
            p(autolink(url, t('auto'))),
          ),
          update(select(['3', 1, 'text'], ['6', 2, 'text'])),
          toggle(null),
          // Ending at the start of the link's text: nothing to unlink.
          update(select(['9', 0, 'text'], ['11', 0, 'text'])),
          toggle(null),
          // A collapsed caret in an autolink: only LinkNodes are removed.
          update(select(['14', 2, 'text'])),
          toggle(null),
        ],
      },
      {
        link,
        name: 'typing at the start and end of a link',
        richText,
        steps: [
          doc(p(a(url, t('link'))), p(t('x'), a(url, t('two')), t('y'))),
          // Text typed at a link's edge goes outside it.
          update(select(['3', 0, 'text'])),
          type('a'),
          update(select(['3', 4, 'text'])),
          type('b'),
          update(select(['7', 0, 'text'])),
          type('c'),
          update(select(['7', 3, 'text'])),
          type('d'),
          // Inside, it goes in.
          update(select(['7', 1, 'text'])),
          type('e'),
          // Replacing all of a link's text.
          update(select(['3', 0, 'text'], ['3', 4, 'text'])),
          type('f'),
        ],
      },
      {
        link,
        name: 'Enter in a link',
        richText,
        steps: [
          doc(p(t('x'), a(url, t('hello world')), t('y'))),
          update(select(['4', 5, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          type('z'),
          // At the end of the link and at its start.
          update(select(['4', 5, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          update(select(['root', 0, 'element'])),
          update({do: 'selection', method: 'insertParagraph'}),
        ],
      },
      {
        link,
        name: 'a link over formatted text',
        richText,
        steps: [
          doc(
            p(t('plain '), t('bold', {format: 1}), t(' italic', {format: 2})),
          ),
          update(select(['2', 2, 'text'], ['4', 4, 'text'])),
          toggle(url),
          run('FORMAT_TEXT_COMMAND', 'underline'),
          // Typing inside stays in the link.
          update(select(['3', 2, 'text'])),
          type('B'),
          // Unlinking the bold text before it only.
          update(select(['3', 0, 'text'], ['3', 2, 'text'])),
          toggle(null),
        ],
      },
      {
        link,
        name: 'an autolink in a parsed document',
        richText,
        steps: [
          doc(
            p(t('see '), autolink(url, t('lexical.dev')), t(' now')),
            p({
              ...autolink('https://example.com', t('example.com')),
              isUnlinked: true,
              rel: 'noreferrer',
            } as Node),
          ),
          // Typing at an autolink's edges goes outside it too.
          update(select(['4', 11, 'text'])),
          type('!'),
          update(select(['4', 0, 'text'])),
          type('>'),
          update(select(['4', 3, 'text'])),
          type('x'),
          // Enter in an autolink copies it.
          update(select(['9', 7, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
          // Linking a range in an autolink updates the autolink.
          update(select(['4', 0, 'text'], ['4', 3, 'text'])),
          toggle('https://example.org'),
        ],
      },
      {
        link,
        name: 'the link transform merges adjacent links and lifts blocks',
        richText,
        steps: [
          doc(
            p(a(url, t('a')), a(url, t('b', {format: 1}))),
            p(a(url, t('c')), a('https://example.com', t('d'))),
            p(autolink(url, t('e')), autolink(url, t('f'))),
          ),
          update(select(['3', 1, 'text'])),
          // Linking text between two equal links joins the three.
          update(
            {do: 'clearSelection'},
            {
              do: 'append',
              key: 'root',
              nodes: [p(a(url, t('g')), t('h'), a(url, t('i')))],
            },
          ),
          update(select(['19', 0, 'text'], ['19', 1, 'text'])),
          toggle({rel: null, url}),
          // A block in a link is lifted out, keeping its text linked.
          update(
            {do: 'clearSelection'},
            {
              do: 'append',
              key: 'root',
              nodes: [p(a(url, t('j'), p(t('k')), t('l')))],
            },
          ),
        ],
      },
      {
        link,
        name: 'node selections and no selection',
        richText,
        steps: [
          doc(p(t('a'), t('b', {format: 1})), p(a(url, t('c')))),
          {update: [{do: 'selectNodes', keys: ['3']}]},
          toggle('https://example.com'),
          {update: [{do: 'selectNodes', keys: ['6']}]},
          toggle('https://example.org'),
          {update: [{do: 'selectNodes', keys: ['3', '6']}]},
          toggle(null),
          update({do: 'clearSelection'}),
          toggle(url),
        ],
      },
      {
        link,
        name: 'Backspace after a link',
        richText,
        steps: [
          doc(p(a(url, t('link')), t('x'))),
          // Collapsed inside text, headless JS needs the DOM and throws.
          update(select(['4', 0, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          update(select(['3', 1, 'text'], ['4', 1, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
        ],
      },
      {
        history: {delay: 300},
        link,
        name: 'undoing a toggle',
        richText,
        steps: [
          doc(p(t('one two'))),
          {...update(select(['2', 0, 'text'], ['2', 3, 'text'])), now: 1000},
          {...toggle(url), now: 2000},
          {...run('UNDO_COMMAND'), now: 3000},
          {...run('REDO_COMMAND'), now: 4000},
          {...toggle(null), now: 5000},
          {...run('UNDO_COMMAND'), now: 6000},
        ],
      },
    ],
  );
});

test('scripts/autolink', () => {
  // `registerAutoLink` (@lexical/link) with `autoLinkUrlMatcher` and
  // `autoLinkEmailMatcher`, over `registerLink` and rich text: text becomes
  // an autolink as it is typed or set, an autolink follows edits to its text
  // and its neighbors, and TOGGLE_LINK_COMMAND with null flips `isUnlinked`.
  // The change handler logs `['autoLink', url, prevUrl]`.
  const autoLink = true;
  const link = true;
  const richText = true;
  const url = 'https://lexical.dev';
  const insert = (text: string): Step => update(call('insertText', text));
  const typed = (text: string): Step[] => [...text].map(insert);
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const lines = (...texts: string[]): Step =>
    doc(...texts.map(text => p(t(text))));
  emit(
    'autolink',
    "@lexical/link's registerAutoLink with autoLinkUrlMatcher and autoLinkEmailMatcher: autolinks made from typed and set text (URLs, www. URLs, emails, in many scripts, at separators and at the bounds of the matchers' repetitions), edited until they stop matching, split, unlinked with TOGGLE_LINK_COMMAND, and across formatted text.",
    [
      {
        autoLink,
        link,
        name: 'typing a URL a character at a time',
        richText,
        steps: [
          doc(p()),
          update(select(['1', 0, 'element'])),
          ...typed('go https://lex.dev/x y'),
        ],
      },
      {
        autoLink,
        link,
        name: 'www URLs and URLs with a path, query and fragment',
        richText,
        steps: [
          lines(
            'visit www.lexical.dev, or lexical.dev.',
            'see https://lexical.dev/docs/intro?a=1&b=two#part now',
            'http://www.lexical.dev:8080/a/b.html',
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'parentheses, and two URLs in one text node',
        richText,
        steps: [
          lines(
            'https://en.wikipedia.org/wiki/Fish_(disambiguation) and (https://en.wikipedia.org/wiki/Fish_(disambiguation))',
            '(see https://lexical.dev) or (https://lexical.dev/a_(b))',
            'www.one.com www.two.com',
            'https://lexical.dev/(open',
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'emails',
        richText,
        steps: [
          lines(
            'mail user@example.com or "Jo Doe"@example.org, or a@b',
            'user@exämple.de and üser@example.de',
            'user@[192.168.0.1]',
            'a..b@c.de',
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'letters and numbers of other scripts',
        richText,
        steps: [
          lines(
            'https://例え.jp/パス',
            'see https://ru.wikipedia.org/wiki/Москва.',
            'https://x.com/٣٤ and https://x.com/１２３',
            'https://٣٤.com',
            'https://𝐀.com/𝐀 𝐀',
            '😀 https://lexical.dev 😀',
            '😀https://lexical.dev',
            'https://lexical.dev😀',
            'www.ŝ.com/ß',
            '😀😀 user@example.com',
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'punctuation and separators around a URL',
        richText,
        steps: [
          lines(
            `${url}.`,
            `${url}, next`,
            `${url};`,
            `x${url}`,
            `${url}!`,
            `a\ufeff${url}`,
            `${url}\u0085`,
            `${url}\u00a0x`,
            `tab\t${url}`,
            `.${url}`,
          ),
          update(
            {do: 'clearSelection'},
            {
              do: 'append',
              key: 'root',
              nodes: [p(t('one'), br(), t(url), br(), t('two'))],
            },
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'the bounds of the repetitions',
        richText,
        steps: [
          lines(
            `https://${'a'.repeat(256)}.com`,
            `https://${'a'.repeat(257)}.com`,
            'https://example.abcdef x',
            'https://example.abcdefg x',
            `https://${'𝐀'.repeat(200)}.com`,
            `${'a'.repeat(64)}@x.com`,
            `${'a'.repeat(65)}@x.com`,
            `a@b.${'c'.repeat(63)}`,
            `a@b.${'c'.repeat(64)}`,
            `${'😀'.repeat(33)}@x.com`,
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'editing an autolink until it stops matching',
        richText,
        steps: [
          lines('go https://lexical.dev now'),
          update(node('6', 'setTextContent', 'https://lexical.org')),
          update(node('6', 'setTextContent', 'www.lexical.org/x')),
          update(node('6', 'setTextContent', 'https://lexical')),
        ],
      },
      {
        autoLink,
        link,
        name: 'splitting an autolink with a space',
        richText,
        steps: [
          lines('https://lexical.dev/docs', 'https://lexical.dev/docs'),
          update(select(['6', 19, 'text'])),
          insert(' '),
          update(select(['8', 10, 'text'])),
          insert(' '),
        ],
      },
      {
        autoLink,
        link,
        name: 'an unlinked autolink stays unlinked',
        richText,
        steps: [
          lines('go https://lexical.dev now'),
          update(select(['6', 0, 'text'], ['6', 19, 'text'])),
          run('TOGGLE_LINK_COMMAND', null),
          // Edits still update its URL, and text after it doesn't unwrap it.
          update(select(['6', 8, 'text'])),
          insert('x'),
          update(select(['5', 0, 'text'])),
          insert('z'),
          update(select(['6', 0, 'text'], ['6', 20, 'text'])),
          run('TOGGLE_LINK_COMMAND', null),
          update(
            {do: 'clearSelection'},
            {
              do: 'append',
              key: 'root',
              nodes: [
                p({...autolink(url, t(url)), isUnlinked: true} as Node, t(' ')),
                p({...autolink(url, t(url)), isUnlinked: true} as Node, t('x')),
              ],
            },
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'typing before and after an autolink',
        richText,
        steps: [
          lines('go https://lexical.dev now', 'to https://lexical.dev now'),
          update(select(['2', 3, 'text'])),
          insert('x'),
          update(select(['11', 0, 'text'])),
          insert('y'),
        ],
      },
      {
        autoLink,
        link,
        name: "continuing an autolink's top-level domain",
        richText,
        steps: [
          lines('see https://lexical.dev', 'mail a@b.co'),
          update(select(['7', 19, 'text'])),
          ...typed('.uk'),
          update(select(['10', 6, 'text'])),
          ...typed('.uk'),
        ],
      },
      {
        autoLink,
        link,
        name: 'a URL across formatted text nodes',
        richText,
        steps: [
          doc(
            p(
              t('see https://lex'),
              t('ical.dev', {format: 1}),
              t('/docs', {format: 2}),
              t(' end'),
            ),
            p(t('https://lexical'), t('dev', {format: 1})),
            p(t('https://lexical'), t('dev', {format: 1})),
          ),
          update(
            select(['7', 5, 'text']),
            node('7', 'setTextContent', 'https://lexical.'),
          ),
          update(
            {do: 'selectNodes', keys: ['10']},
            node('10', 'setTextContent', 'https://lexical.'),
          ),
        ],
      },
      {
        autoLink,
        link,
        name: 'TOGGLE_LINK_COMMAND and Enter in an autolink',
        richText,
        steps: [
          lines('https://lexical.dev', 'https://lexical.dev/docs'),
          update(select(['6', 0, 'text'], ['6', 19, 'text'])),
          run('TOGGLE_LINK_COMMAND', 'https://example.com'),
          update(select(['8', 19, 'text'])),
          run('INSERT_PARAGRAPH_COMMAND'),
        ],
      },
    ],
  );
});

function codeBlock(language: string | null, ...children: Node[]): Node {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    type: 'code',
    version: 1,
    ...(language === null ? {} : {language}),
  } as Node;
}

function ch(text: string, extra: Record<string, unknown> = {}): Node {
  return {...t(text), type: 'code-highlight', ...extra};
}

test('scripts/code', () => {
  // `@lexical/code-core`'s nodes, `CodeExtension`'s TabNode transform and
  // `registerCodeIndentation`'s command handlers, over rich text's, with
  // commands dispatched outside an update as an input layer does.
  const code = true;
  const richText = true;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const enter = run('INSERT_PARAGRAPH_COMMAND');
  emit(
    'code',
    "@lexical/code-core's CodeNode and CodeHighlightNode: Enter in a code block (keeping a line's leading tabs and spaces, and leaving the block after two line breaks), Backspace at its start, INSERT_TAB_COMMAND, INDENT_CONTENT_COMMAND and OUTDENT_CONTENT_COMMAND through registerCodeIndentation, formats a code block ignores, and parsing what setFormat and the setters drop.",
    [
      {
        code,
        name: "Enter keeps the line's leading tabs and spaces",
        richText,
        steps: [
          doc(codeBlock('javascript', tab(), ch('  if (x) {'), br(), ch('y'))),
          // At the end of the indented line.
          update(select(['3', 10, 'text'])),
          enter,
          // In the middle of it: the text splits.
          update(select(['3', 5, 'text'])),
          enter,
          // At the start of an unindented line.
          update(select(['5', 0, 'text'])),
          enter,
          // A range: rich text removes it first.
          update(select(['3', 1, 'text'], ['5', 0, 'text'])),
          enter,
        ],
      },
      {
        code,
        name: 'Enter three times at the end leaves the block',
        richText,
        steps: [
          doc(codeBlock(null, ch('a')), p(t('after'))),
          update(select(['2', 1, 'text'])),
          enter,
          enter,
          enter,
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'b'),
        ],
      },
      {
        code,
        name: 'Enter in an empty code block',
        richText,
        steps: [
          doc(codeBlock(null)),
          update(select(['1', 0, 'element'])),
          enter,
          enter,
          run('INSERT_LINE_BREAK_COMMAND', false),
        ],
      },
      {
        code,
        name: 'INSERT_TAB_COMMAND in and out of a code block',
        richText,
        steps: [
          doc(codeBlock(null, ch('ab')), p(t('cd'))),
          update(select(['2', 1, 'text'])),
          run('INSERT_TAB_COMMAND'),
          // Rich text's handler, outside the block.
          update(select(['4', 1, 'text'])),
          run('INSERT_TAB_COMMAND'),
          // From the block into the paragraph: neither block handles it.
          update(select(['2', 0, 'text'], ['4', 0, 'text'])),
          run('INSERT_TAB_COMMAND'),
        ],
      },
      {
        code,
        name: 'indenting and outdenting lines',
        richText,
        steps: [
          doc(codeBlock(null, ch('a'), br(), ch('b'), br(), ch('c'))),
          // All three lines, forward.
          update(select(['2', 0, 'text'], ['6', 1, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          run('INDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          // Backward, ending at the start of the last line, which is left.
          update(select(['6', 0, 'text'], ['2', 1, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
        ],
      },
      {
        code,
        name: 'indenting at a caret and over a line break',
        richText,
        steps: [
          doc(codeBlock(null, ch('a'), br(), br(), ch('  b'))),
          // A caret at a line's start inserts a tab there.
          update(select(['2', 0, 'text'])),
          run('INDENT_CONTENT_COMMAND'),
          run('OUTDENT_CONTENT_COMMAND'),
          // A caret on the blank line has no line to outdent.
          update(select(['1', 2, 'element'])),
          run('OUTDENT_CONTENT_COMMAND'),
          // Only the line break between the two lines.
          update(select(['1', 2, 'element'], ['1', 3, 'element'])),
          run('INDENT_CONTENT_COMMAND'),
          // Backward over a line break.
          update(select(['1', 3, 'element'], ['1', 2, 'element'])),
          run('INDENT_CONTENT_COMMAND'),
          // Leading spaces aren't outdented without a tab size.
          update(select(['5', 3, 'text'])),
          run('OUTDENT_CONTENT_COMMAND'),
          // Outside a code block, rich text indents the paragraph.
          update(
            {do: 'append', key: 'root', nodes: [p(t('x'))]},
            select(['root', 1, 'element']),
          ),
          run('INDENT_CONTENT_COMMAND'),
        ],
      },
      {
        code,
        name: 'formats a code block ignores',
        richText,
        steps: [
          doc(codeBlock(null, tab(), ch('a'), br(), ch('b'))),
          update(select(['1', 0, 'element'], ['1', 4, 'element'])),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          run('FORMAT_TEXT_COMMAND', 'italic'),
          // A tab set bold outside a command: the transform clears it.
          update(node('2', 'setFormat', 1)),
          // Typing takes the selection's format.
          update(select(['3', 1, 'text'])),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          run('CONTROLLED_TEXT_INSERTION_COMMAND', 'c'),
        ],
      },
      {
        code,
        name: 'Backspace at the start of a code block',
        richText,
        steps: [
          doc(p(t('x')), codeBlock(null, ch('a'), br(), ch('b'))),
          update(select(['4', 0, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
        ],
      },
      {
        code,
        name: 'parsing what setFormat and the setters drop',
        richText,
        steps: [
          doc(
            codeBlock(
              '',
              ch('a', {format: 1, highlightType: ''}),
              ch('b', {format: 'bold', highlightType: 'keyword'}),
              ch('c', {highlightType: null}),
            ),
            {...codeBlock(null, ch('d')), language: null, theme: ''} as Node,
            {...codeBlock('rust', tab()), theme: 'dark'} as Node,
          ),
          update(select(['root', 3, 'element']), {
            do: 'insertNodes',
            nodes: [ch('e')],
          }),
        ],
      },
    ],
  );
});

function overflow(...children: Node[]): Node {
  return {
    children,
    direction: null,
    format: '',
    indent: 0,
    type: 'overflow',
    version: 1,
  } as Node;
}

test('scripts/overflow', () => {
  // `@lexical/overflow`'s OverflowNode in a paragraph, as `@lexical/react`'s
  // character limit leaves it, with rich text's handlers dispatched outside
  // an update as an input layer does. Keys: the paragraph 1, "within " 2,
  // the overflow node 3, "past" (italic, so it doesn't merge) 4, " the
  // limit" 5.
  const richText = true;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const enter = run('INSERT_PARAGRAPH_COMMAND');
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  const start = doc(
    p(t('within '), overflow(t('past', {format: 2}), t(' the limit'))),
  );
  emit(
    'overflow',
    "@lexical/overflow's OverflowNode: Enter in it (its insertNewAfter is its parent's), typing at its edges, the $transform that removes it once empty and where that leaves the selection, removing text across it, formatting, Backspace at its start, and inserting one.",
    [
      {
        name: 'Enter in an overflow node',
        overflow: true,
        richText,
        steps: [
          start,
          // In the middle of its first text.
          update(select(['4', 2, 'text'])),
          enter,
          // At its end.
          update(select(['5', 10, 'text'])),
          enter,
          // At its start.
          update(select(['4', 0, 'text'])),
          enter,
        ],
      },
      {
        name: 'Enter with an element point in an overflow node',
        overflow: true,
        richText,
        steps: [
          start,
          update(select(['3', 1, 'element'])),
          enter,
          update(select(['3', 0, 'element'])),
          run('INSERT_LINE_BREAK_COMMAND', false),
        ],
      },
      {
        name: "typing at an overflow node's edges",
        overflow: true,
        richText,
        steps: [
          start,
          update(select(['2', 7, 'text'])),
          type('a'),
          update(select(['4', 0, 'text'])),
          type('b'),
          update(select(['5', 10, 'text'])),
          type('c'),
          update(select(['1', 2, 'element'])),
          type('d'),
        ],
      },
      {
        name: 'an emptied overflow node removes itself',
        overflow: true,
        richText,
        steps: [
          start,
          // All its text, with a range in it.
          update(select(['4', 0, 'text'], ['5', 10, 'text'])),
          run('REMOVE_TEXT_COMMAND'),
          type('e'),
        ],
      },
      {
        name: 'removing the last child of an overflow node',
        overflow: true,
        richText,
        steps: [
          doc(p(t('a'), overflow(t('b'))), p(t('c'))),
          update(select(['4', 1, 'text']), node('4', 'remove')),
          // An empty one appended in an update goes too.
          update(
            node('1', 'append', [overflow()]),
            node('5', 'append', [overflow(t('x'))]),
          ),
          update(select(['9', 0, 'text'], ['9', 1, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
        ],
      },
      {
        name: 'removing text across an overflow node',
        overflow: true,
        richText,
        steps: [
          doc(
            p(t('one '), overflow(t('two', {format: 2}), t(' three'))),
            p(overflow(t('four')), t(' five')),
          ),
          update(select(['2', 2, 'text'], ['5', 3, 'text'])),
          run('REMOVE_TEXT_COMMAND'),
          update(select(['2', 2, 'text'], ['9', 2, 'text'])),
          run('DELETE_CHARACTER_COMMAND', false),
        ],
      },
      {
        name: 'formatting across an overflow node',
        overflow: true,
        richText,
        steps: [
          start,
          update(select(['2', 3, 'text'], ['4', 2, 'text'])),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          run('SELECT_ALL_COMMAND'),
          run('FORMAT_TEXT_COMMAND', 'italic'),
          run('FORMAT_ELEMENT_COMMAND', 'center'),
          run('INDENT_CONTENT_COMMAND'),
        ],
      },
      {
        name: 'Backspace at the start of an overflow node',
        overflow: true,
        richText,
        steps: [
          start,
          update(select(['4', 0, 'text'])),
          run('DELETE_CHARACTER_COMMAND', true),
          update(select(['3', 0, 'element'])),
          run('DELETE_CHARACTER_COMMAND', true),
        ],
      },
      {
        name: 'inserting an overflow node',
        overflow: true,
        richText,
        steps: [
          doc(p(t('abcd'))),
          update(
            select(['2', 2, 'text']),
            call('insertNodes', [overflow(t('x'))]),
          ),
          update(select(['root', 1, 'element']), {
            do: 'insertNodes',
            nodes: [overflow(t('y'))],
          }),
        ],
      },
    ],
  );
});

test('scripts/node-state', () => {
  // NodeState (LexicalNodeState.ts) on the core nodes, with the configs of
  // ops.ts's STATES: setting and reading values, updaters, defaults left out
  // of the JSON, the `$` JSON round trip with keys no config reads (and what
  // a read does to them), copies (`$copyNode`) and history, text nodes that
  // merge or split, a key collision, a flat state (QuoteNode's
  // `shadowRoot`), the NodeState that text keeps when a segmented node turns
  // into plain text, and the NodeState hashtags take from their text
  // (registerLexicalTextEntity).
  const set = (key: string, name: string, value: Json): Action => ({
    do: 'setState',
    key,
    state: name,
    value,
  });
  const apply = (key: string, name: string, updater: string): Action => ({
    do: 'updateState',
    key,
    state: name,
    updater,
  });
  const get = (key: string, name: string): Action => ({
    do: 'logState',
    key,
    state: name,
  });
  const change = (key: string, name: string): Action => ({
    do: 'logStateChange',
    key,
    state: name,
  });
  const withState = (n: Node, $: Json): Node => ({...n, $}) as Node;
  const tag = (text: string, extra: Record<string, unknown> = {}): Node =>
    t(text, {...extra, type: 'hashtag'});
  const at = (now: number, step: Step): Step => ({...step, now});
  emit(
    'node-state',
    'NodeState: createState configs set and read on nodes (values and updaters), defaults left out of the JSON, the `$` JSON round trip with keys no config reads, reading and copying such keys, $copyNode and history, text nodes merging or splitting by their NodeState, a key collision, a flat state, segmented text made plain, and hashtags taking NodeState from their text.',
    [
      {
        listen: true,
        name: 'set, get and defaults',
        steps: [
          doc(p(t('one'), t('two', {format: 1}))),
          update(
            get('2', 'count'),
            get('2', 'label'),
            get('2', 'words'),
            get('1', 'fresh'),
          ),
          // A value equal to the default is set, but not written to JSON.
          update(
            set('2', 'label', 'x'),
            set('2', 'count', 1),
            get('2', 'label'),
            get('2', 'count'),
            change('2', 'label'),
            change('3', 'label'),
          ),
          update(
            set('2', 'count', 5),
            set('3', 'words', ['a', 'b']),
            set('1', 'fresh', true),
            change('2', 'count'),
            change('3', 'words'),
          ),
          // Back to the defaults: only the label is left.
          update(
            set('2', 'count', 1),
            set('3', 'words', []),
            set('1', 'fresh', false),
          ),
          update(get('2', 'label'), get('3', 'words'), get('1', 'fresh')),
        ],
      },
      {
        listen: true,
        name: 'updaters',
        steps: [
          doc(p(t('a'), t('b', {format: 1}))),
          update(
            apply('2', 'count', 'count:increment'),
            apply('2', 'count', 'count:increment'),
            get('2', 'count'),
          ),
          // An updater that returns an equal value writes nothing.
          update(apply('3', 'count', 'count:same')),
          update(apply('3', 'words', 'words:copy')),
          update(
            apply('3', 'words', 'words:push-x'),
            apply('3', 'label', 'label:exclaim'),
            get('3', 'words'),
          ),
          update(apply('3', 'words', 'words:copy')),
          update(apply('1', 'label', 'label:exclaim'), change('1', 'label')),
        ],
      },
      {
        name: 'JSON with keys no config has read',
        steps: [
          doc(
            withState(
              p(
                t('x', {
                  $: {count: 'bad', label: 'kept', mystery: {deep: [1, 2]}},
                }),
                t('y', {$: {words: 'b c'}, format: 1}),
                t('z', {$: {'2': 'two', a: 3, b: 1}, format: 2}),
                t('w', {$: 'ab', format: 4}),
                t('v', {$: null, format: 8}),
              ),
              {label: 'para'},
            ),
          ),
          // A read parses the value and keeps it, without changing the JSON.
          update(get('2', 'label'), get('1', 'label')),
          // `bad` is no count: the read gives the default, which the JSON
          // leaves out.
          update(get('2', 'count')),
          // A write copies the state, parsing the keys a config has read.
          update(set('2', 'label', 'new')),
          update(get('3', 'words'), set('3', 'count', 2)),
          update(set('4', 'count', 7), set('5', 'label', 's')),
        ],
      },
      {
        name: 'text nodes merge only with equivalent NodeState',
        steps: [
          doc(
            p(
              t('a', {$: {label: 'x'}}),
              t('b'),
              t('c', {$: {label: 'x'}}),
              t('d', {$: {label: 'x'}}),
            ),
          ),
          update(set('3', 'label', 'x')),
          update(set('2', 'count', 2)),
          update(set('2', 'count', 1)),
        ],
      },
      {
        name: 'merging compares unread JSON values by identity',
        steps: [
          doc(p(t('ef', {$: {m: {z: 1}}}), t('g', {$: {m: {z: 1}}}))),
          // The parts of a split share the values, so they merge back.
          update(node('2', 'splitText', 1), node('2', 'toggleFormat', 'bold')),
          update(node('2', 'toggleFormat', 'bold')),
        ],
      },
      {
        name: 'splitText gives the parts the NodeState',
        steps: [
          doc(p(t('abcdef', {$: {label: 'x', other: 1}}))),
          update(node('2', 'splitText', 2, 4), set('3', 'label', 'y')),
          update(node('4', 'setMode', 'segmented'), node('4', 'splitText', 1)),
          update(set('2', 'count', 3), node('2', 'splitText', 1)),
          update(get('5', 'label'), get('5', 'count')),
        ],
      },
      {
        handlers: [
          {
            command: 'CAN_UNDO_COMMAND',
            id: 'canUndo',
            priority: 0,
            result: false,
          },
        ],
        history: {delay: 300},
        list: true,
        name: '$copyNode and history',
        richText: true,
        steps: [
          at(
            0,
            doc(
              list(
                'bullet',
                withState(li(t('item', {$: {label: 'x'}})), {
                  fresh: true,
                  label: 'li',
                }),
                withState(li(t('two')), {fresh: true}),
              ),
            ),
          ),
          at(1000, update(select(['3', 4, 'text']), get('2', 'fresh'))),
          // The new item is a copy: it keeps `label` and resets `fresh`,
          // which was read.
          at(2000, {dispatch: 'INSERT_PARAGRAPH_COMMAND'}),
          // This item's `fresh` was never read, but the type's config for it
          // is known now: the copy parses it and resets it too.
          at(2500, update(select(['5', 3, 'text']))),
          at(2600, {dispatch: 'INSERT_PARAGRAPH_COMMAND'}),
          at(3000, update(set('3', 'count', 4), get('2', 'fresh'))),
          at(4000, {dispatch: 'UNDO_COMMAND'}),
          at(5000, update(get('3', 'count'))),
          at(6000, {dispatch: 'UNDO_COMMAND'}),
          at(7000, {dispatch: 'REDO_COMMAND'}),
          at(8000, {dispatch: 'REDO_COMMAND'}),
        ],
      },
      {
        name: 'a key collision',
        steps: [
          doc(p(t('a'))),
          update(set('2', 'label', 'x')),
          update(set('2', 'otherLabel', 'y')),
          update(get('2', 'otherLabel')),
          update(get('2', 'label')),
          // The paragraph has not used `label`: the other config takes it.
          update(set('1', 'otherLabel', 'p')),
          update(get('1', 'label')),
        ],
      },
      {
        name: 'a flat state',
        richText: true,
        steps: [
          doc(withState(q(t('a')), {shadowRoot: true}), {
            ...withState(q(p(t('b'))), {label: 'q', shadowRoot: false}),
            shadowRoot: true,
          } as Node),
          update(set('4', 'label', 'r'), set('2', 'count', 3)),
        ],
      },
      {
        name: 'segmented text made plain keeps its NodeState',
        steps: [
          doc(
            p(t('seg ment', {$: {label: 'x'}, mode: 'segmented'}), t(' after')),
            p(t('two seg', {$: {count: 2}, mode: 'segmented'})),
          ),
          update(select(['2', 3, 'text']), call('insertText', 'X')),
          update(select(['5', 0, 'text'], ['5', 4, 'text']), {
            do: 'removeTextFromCaretRange',
          }),
        ],
      },
      {
        hashtag: true,
        name: 'hashtags take the NodeState of their text',
        steps: [
          doc(
            p(t('see #tag now', {$: {label: 'x'}})),
            p(t('hello ', {$: {label: 'x'}})),
            p(t('hi ', {$: {label: 'x'}})),
          ),
          // Text typed at once is not the text's: its hashtag takes nothing.
          update(select(['4', 6, 'text']), call('insertText', '#ab')),
          // A `#` already there is: the hashtag takes the NodeState.
          update(select(['6', 3, 'text']), call('insertText', '#')),
          update(call('insertText', 'a')),
        ],
      },
      {
        hashtag: true,
        name: 'text split off a hashtag and a hashtag turned into text',
        steps: [
          doc(
            p(tag('#tag', {$: {label: 'x'}})),
            p(tag('#t', {$: {label: 'y'}}), t(' x')),
          ),
          update(select(['2', 4, 'text']), call('insertText', ' #b')),
          // `#` is no hashtag: the text it turns into keeps its NodeState.
          update(
            select(['4', 1, 'text'], ['4', 2, 'text']),
            call('removeText'),
          ),
        ],
      },
    ],
  );
});

test('scripts/set-blocks-type', () => {
  // `@lexical/selection`'s `$setBlocksType` with its default
  // `$afterCreateElement` (`$copyBlockFormatIndent`), turning the blocks a
  // selection touches into paragraphs, quotes and headings, with rich
  // text's nodes, and lists and links where a case needs them.
  const richText = true;
  const set = (block: 'paragraph' | 'quote' | `h${1 | 2 | 3 | 4 | 5 | 6}`) =>
    update({block, do: 'setBlocksType'});
  const url = 'https://lexical.dev';
  emit(
    'set-blocks-type',
    "@lexical/selection's $setBlocksType with $copyBlockFormatIndent: carets, forward and backward ranges, a focus that overshoots into the next block, format and indent, links, list items, element points, node selections, no selection and undo.",
    [
      {
        name: 'a caret in a paragraph: to h1, h2, a quote and back',
        richText,
        steps: [
          doc(p(t('Hello')), p(t('world'))),
          update(select(['2', 2, 'text'])),
          set('h1'),
          set('h2'),
          set('quote'),
          set('paragraph'),
          // At the end of the second paragraph.
          update(select(['4', 5, 'text'])),
          set('h6'),
        ],
      },
      {
        name: 'a range across several blocks',
        richText,
        steps: [
          doc(p(t('one')), p(t('two')), h('h2', t('three')), p(t('four'))),
          update(select(['2', 1, 'text'], ['6', 2, 'text'])),
          set('h3'),
          set('quote'),
          // With the root's element points, every block.
          update(select(['root', 0, 'element'], ['root', 4, 'element'])),
          set('h5'),
          update({do: 'selectAll'}),
          set('paragraph'),
        ],
      },
      {
        name: 'a backward range',
        richText,
        steps: [
          doc(p(t('one')), p(t('two')), p(t('three'))),
          update(select(['6', 2, 'text'], ['2', 1, 'text'])),
          set('h4'),
          // Backward within one block.
          update(select(['4', 3, 'text'], ['4', 0, 'text'])),
          set('quote'),
        ],
      },
      {
        name: 'a focus that overshoots to the edge of the next block',
        richText,
        steps: [
          doc(p(t('one')), p(t('two')), p(t('three')), p()),
          // Forward, to the start of the next block's text: that block is
          // skipped.
          update(select(['2', 1, 'text'], ['4', 0, 'text'])),
          set('h1'),
          // To an element point at the start of the next block.
          update(select(['4', 1, 'text'], ['5', 0, 'element'])),
          set('h2'),
          // Backward, to the end of the previous block's text.
          update(select(['6', 2, 'text'], ['4', 3, 'text'])),
          set('quote'),
          // Into an empty block, which is the whole block: not skipped.
          update(select(['6', 2, 'text'], ['7', 0, 'element'])),
          set('h3'),
          // From the start of a block to the start of the next: only the
          // anchor's block.
          update(select(['2', 0, 'text'], ['4', 0, 'text'])),
          set('paragraph'),
        ],
      },
      {
        name: 'format and indent are copied',
        richText,
        steps: [
          doc(
            {...p(t('centered')), format: 'center', indent: 2} as Node,
            {...h('h2', t('right')), format: 'right'} as Node,
            {...q(t('indented')), indent: 1} as Node,
          ),
          update(select(['2', 0, 'text'], ['6', 3, 'text'])),
          set('h1'),
          set('paragraph'),
          update(select(['4', 1, 'text'])),
          set('quote'),
        ],
      },
      {
        link: true,
        name: 'a caret and a range inside a link',
        richText,
        steps: [
          doc(
            p(t('see '), a(url, t('link')), t(' here')),
            p(a(url, t('two')), t(' x')),
          ),
          update(select(['4', 2, 'text'])),
          set('h2'),
          update(select(['4', 1, 'text'], ['8', 2, 'text'])),
          set('quote'),
        ],
      },
      {
        list: true,
        name: 'a list item to a heading, at the start, middle and end',
        richText,
        steps: [
          doc(
            list('bullet', li(t('one')), li(t('two')), li(t('three'))),
            list('number', li(t('a')), li(t('b'))),
          ),
          // The middle item: the list splits around the heading.
          update(select(['5', 1, 'text'])),
          set('h1'),
          update(select(['3', 0, 'text'])),
          set('h2'),
          // The only item left in a list: the list goes.
          update(select(['7', 5, 'text'])),
          set('quote'),
          // Backward across the second list's items.
          update(select(['12', 1, 'text'], ['10', 0, 'text'])),
          set('paragraph'),
        ],
      },
      {
        list: true,
        name: 'a range across list items and a paragraph',
        richText,
        steps: [
          doc(
            p(t('before')),
            list('number', li(t('a')), li(t('b')), li(t('c'))),
            p(t('after')),
          ),
          update(select(['5', 0, 'text'], ['7', 1, 'text'])),
          set('quote'),
          update(select(['2', 3, 'text'], ['9', 1, 'text'])),
          set('h3'),
        ],
      },
      {
        list: true,
        name: 'a nested list item',
        richText,
        steps: [
          doc(
            list('bullet', li(t('a')), {
              ...li(list('bullet', li(t('b')), li(t('c')))),
              value: 2,
            } as Node),
          ),
          update(select(['7', 0, 'text'])),
          set('h1'),
          update(select(['9', 1, 'text'])),
          set('paragraph'),
        ],
      },
      {
        name: 'an element point in an empty paragraph',
        richText,
        steps: [
          doc(p(t('a')), p(), p()),
          update(select(['3', 0, 'element'])),
          set('h1'),
          // A range of element points over the two empty blocks.
          update(select(['5', 0, 'element'], ['4', 0, 'element'])),
          set('quote'),
          // An element point in a block with text.
          update(select(['1', 1, 'element'])),
          set('h2'),
        ],
      },
      {
        name: 'node selections',
        richText,
        steps: [
          doc(p(t('a')), p(t('b')), p(t('c'), t('d', {format: 1}))),
          // Text nodes: their blocks.
          update({do: 'selectNodes', keys: ['2', '6']}),
          set('h1'),
          // A block itself.
          update({do: 'selectNodes', keys: ['3']}),
          set('quote'),
          // Two nodes in one block.
          update({do: 'selectNodes', keys: ['6', '7']}),
          set('paragraph'),
        ],
      },
      {
        name: 'no selection',
        richText,
        steps: [
          doc(p(t('a')), p(t('b'))),
          update({do: 'clearSelection'}),
          set('h1'),
        ],
      },
      {
        history: {delay: 300},
        name: 'undoing a conversion',
        richText,
        steps: [
          doc(p(t('one')), p(t('two'))),
          {...update(select(['2', 1, 'text'], ['4', 1, 'text'])), now: 1000},
          {...set('h1'), now: 2000},
          {...set('quote'), now: 3000},
          {dispatch: 'UNDO_COMMAND', now: 4000},
          {dispatch: 'UNDO_COMMAND', now: 5000},
          {dispatch: 'REDO_COMMAND', now: 6000},
        ],
      },
    ],
  );
});

test('scripts/decorator', () => {
  // DecoratorNode (LexicalDecoratorNode.ts) and the places the model treats
  // decorators specially, with @lexical/extension's two decorators: a
  // HorizontalRuleNode (a block decorator whose text is '\n') and a
  // DecoratorTextNode (an inline one, formattable, with a flat `format`
  // state). Commands are dispatched outside an update, as an input layer
  // does; a headless editor has no `Selection.modify`, so a deletion or move
  // the model does not resolve around a decorator or block throws.
  const hr = (): Node => ({type: 'horizontalrule', version: 1}) as Node;
  const dt = (format?: number): Node =>
    ({
      type: 'decorator-text',
      version: 1,
      ...(format === undefined ? {} : {format}),
    }) as Node;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const backspace = run('DELETE_CHARACTER_COMMAND', true);
  const del = run('DELETE_CHARACTER_COMMAND', false);
  const enter = run('INSERT_PARAGRAPH_COMMAND');
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  const selectNodes = (...keys: string[]): Action => ({
    do: 'selectNodes',
    keys,
  });
  const selectionText: Action = {do: 'logSelectionText'};
  const setBlocksType = (
    block: 'paragraph' | 'quote' | `h${1 | 2 | 3 | 4 | 5 | 6}`,
  ): Action => ({
    block,
    do: 'setBlocksType',
  });
  const modify = (alter: string, isBackward: boolean, granularity: string) =>
    call('modify', alter, isBackward, granularity);
  const extension = true;
  const richText = true;
  // Keys: the paragraph 1, "ab" 2, the inline decorator 3, "cd" 4, the rule
  // 5, the paragraph 6, "ef" 7.
  const start = doc(p(t('ab'), dt(), t('cd')), hr(), p(t('ef')));
  emit(
    'decorator',
    "DecoratorNode, with @lexical/extension's HorizontalRuleNode (block) and DecoratorTextNode (inline): their JSON and text content, inserting them, typing and Enter around them, a NodeSelection of one, Backspace and Delete next to one (and the headless editor's throw where the model does not resolve the deletion), modify around them, removing text over them, formatting, $setBlocksType across a block decorator, and undo.",
    [
      {
        extension,
        name: 'JSON and text content',
        richText,
        steps: [
          doc(p(t('ab'), dt(3), t('cd')), hr(), p(dt(), br(), t('ef')), hr()),
          update(select(['2', 0, 'text'], ['9', 2, 'text']), selectionText),
          update(select(['2', 1, 'text'], ['4', 1, 'text']), selectionText),
          update(selectNodes('5'), selectionText),
          update(selectNodes('3', '5', '7'), selectionText),
          update(
            select(['root', 1, 'element'], ['root', 2, 'element']),
            selectionText,
          ),
        ],
      },
      {
        extension,
        name: 'inserting inline and block decorators',
        richText,
        steps: [
          doc(p(t('hello world')), p()),
          update(select(['2', 5, 'text']), call('insertNodes', [dt()])),
          update(select(['2', 2, 'text']), call('insertNodes', [hr()])),
          update(select(['3', 0, 'element']), call('insertNodes', [dt(1)])),
          update(select(['3', 1, 'element']), call('insertNodes', [hr()])),
          update(call('insertNodes', [hr(), p(t('x'))])),
          update(select(['root', 0, 'element']), call('insertNodes', [hr()])),
          update(select(['root', 1, 'element']), call('insertNodes', [dt()])),
        ],
      },
      {
        extension,
        name: 'inserting decorators at the end of a block and in a node selection',
        richText,
        steps: [
          doc(p(t('one')), p(t('two'))),
          update(select(['2', 3, 'text']), call('insertNodes', [hr()])),
          update(select(['4', 0, 'text']), call('insertNodes', [hr()])),
          update(selectNodes('5'), call('insertNodes', [dt()])),
        ],
      },
      {
        extension,
        name: 'inserting a block in a node selection of a decorator',
        richText,
        steps: [
          start,
          update(selectNodes('5'), call('insertNodes', [p(t('new'))])),
        ],
      },
      {
        extension,
        name: 'inserting a block decorator in a node selection of an inline one',
        richText,
        steps: [start, update(selectNodes('3'), call('insertNodes', [hr()]))],
      },
      {
        extension,
        name: 'typing around decorators',
        richText,
        steps: [
          start,
          update(select(['1', 1, 'element'])),
          type('x'),
          update(select(['1', 2, 'element'])),
          type('y'),
          update(select(['root', 1, 'element'])),
          type('z'),
          update(select(['root', 2, 'element'])),
          type('w'),
        ],
      },
      {
        extension,
        name: 'typing in a paragraph that holds only a decorator',
        richText,
        steps: [
          doc(p(dt()), p(dt())),
          update(select(['1', 0, 'element'])),
          type('a'),
          update(select(['3', 1, 'element'])),
          type('b'),
        ],
      },
      {
        extension,
        name: 'Enter around decorators',
        richText,
        steps: [
          start,
          update(select(['1', 2, 'element'])),
          enter,
          update(select(['root', 1, 'element'])),
          enter,
          update(select(['root', 3, 'element'])),
          enter,
        ],
      },
      {
        extension,
        name: 'a node selection of a decorator',
        richText,
        steps: [
          start,
          update(selectNodes('3')),
          type('q'),
          update(selectNodes('5')),
          backspace,
          update(selectNodes('3')),
          del,
          update(node('7', 'selectStart'), selectNodes('7')),
          update(node('7', 'remove')),
        ],
      },
      {
        extension,
        name: 'selecting next to a decorator',
        richText,
        steps: [
          start,
          update(node('3', 'selectNext')),
          update(node('3', 'selectPrevious')),
          update(node('5', 'selectNext')),
          update(node('5', 'selectPrevious')),
          update(node('5', 'selectStart')),
          update(node('3', 'selectEnd')),
        ],
      },
      {
        extension,
        name: 'Backspace and Delete next to an inline decorator',
        richText,
        steps: [
          // Keys: "ab" 2, decorators 3, 5 and 7, "cd" 4, "ef" 6.
          doc(p(t('ab'), dt(), t('cd'), dt(), t('ef'), dt())),
          update(select(['6', 0, 'text'])),
          backspace,
          update(select(['2', 2, 'text'])),
          del,
          update(select(['1', 2, 'element'])),
          backspace,
          // In the middle of text: headless JS has no Selection.modify.
          update(select(['2', 1, 'text'])),
          backspace,
        ],
      },
      {
        extension,
        name: 'Backspace and Delete next to a block decorator',
        richText,
        steps: [
          doc(p(t('ab')), hr(), p(t('cd')), hr(), p(), hr()),
          update(select(['2', 2, 'text'])),
          del,
          update(select(['5', 0, 'text'])),
          backspace,
          update(select(['7', 0, 'element'])),
          backspace,
        ],
      },
      {
        extension,
        name: 'Backspace and Delete in an empty paragraph next to a block decorator',
        richText,
        steps: [
          doc(hr(), p(), p(t('x')), p(), hr()),
          update(select(['2', 0, 'element'])),
          backspace,
          update(select(['5', 0, 'element'])),
          del,
          update(select(['root', 0, 'element'])),
          del,
        ],
      },
      {
        extension,
        name: 'modify around decorators',
        richText,
        steps: [
          start,
          update(select(['2', 2, 'text']), modify('move', false, 'character')),
          update(select(['4', 0, 'text']), modify('move', true, 'character')),
          update(
            select(['2', 2, 'text']),
            modify('extend', false, 'character'),
          ),
          update(select(['4', 2, 'text']), modify('move', false, 'character')),
          update(select(['7', 0, 'text']), modify('move', true, 'character')),
          update(
            select(['4', 2, 'text']),
            modify('extend', false, 'character'),
          ),
          update(
            select(['2', 2, 'text']),
            modify('move', false, 'lineboundary'),
          ),
          update(select(['2', 1, 'text']), modify('move', false, 'character')),
          update(
            select(['2', 0, 'text'], ['4', 1, 'text']),
            modify('move', false, 'character'),
          ),
        ],
      },
      {
        extension,
        name: 'removing text over decorators',
        richText,
        steps: [
          start,
          update(
            select(['2', 1, 'text'], ['4', 1, 'text']),
            call('removeText'),
          ),
          update(
            select(['2', 1, 'text'], ['7', 1, 'text']),
            call('removeText'),
          ),
        ],
      },
      {
        extension,
        name: 'removing a block decorator with element points',
        richText,
        steps: [
          doc(p(t('ab')), hr(), hr(), p(t('cd'))),
          update(
            select(['root', 1, 'element'], ['root', 2, 'element']),
            call('removeText'),
          ),
          update(
            select(['2', 2, 'text'], ['root', 2, 'element']),
            call('removeText'),
          ),
        ],
      },
      {
        extension,
        name: 'formatting over an inline decorator',
        richText,
        steps: [
          start,
          update(select(['2', 1, 'text'], ['4', 1, 'text'])),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          update(selectNodes('3')),
          run('FORMAT_TEXT_COMMAND', 'italic'),
          update(selectNodes('5')),
          run('FORMAT_TEXT_COMMAND', 'italic'),
        ],
      },
      {
        extension,
        name: '$setBlocksType across a block decorator',
        richText,
        steps: [
          start,
          update(
            select(['2', 1, 'text'], ['7', 1, 'text']),
            setBlocksType('h1'),
          ),
          update(selectNodes('5'), setBlocksType('quote')),
          update(selectNodes('3'), setBlocksType('quote')),
          update(
            select(['root', 1, 'element'], ['root', 2, 'element']),
            setBlocksType('paragraph'),
          ),
          update(
            select(['root', 2, 'element'], ['root', 1, 'element']),
            setBlocksType('quote'),
          ),
        ],
      },
      {
        extension,
        name: 'Backspace removing the last block decorator of a shadow root quote',
        richText,
        steps: [
          // Keys: the quote 1, the rule 2, the paragraph 3, "x" 4.
          doc({...q(hr()), shadowRoot: true} as Node, p(t('x'))),
          update(select(['1', 1, 'element'])),
          backspace,
        ],
      },
      {
        extension,
        list: true,
        name: 'a list over a block decorator',
        richText,
        steps: [
          start,
          update(select(['2', 1, 'text'], ['7', 1, 'text'])),
          run('INSERT_UNORDERED_LIST_COMMAND'),
        ],
      },
      {
        extension,
        list: true,
        name: 'a list from a node selection of a block decorator',
        richText,
        steps: [
          start,
          update(selectNodes('5')),
          run('INSERT_UNORDERED_LIST_COMMAND'),
        ],
      },
      {
        extension,
        history: {delay: 0},
        name: 'undo a decorator deleted by a node selection',
        richText,
        steps: [
          start,
          update(selectNodes('5')),
          backspace,
          run('UNDO_COMMAND'),
          run('REDO_COMMAND'),
        ],
      },
    ],
  );
});
