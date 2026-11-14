/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {SerializedLexicalNode} from 'lexical';

import {expect, test} from 'vitest';

import {
  type Action,
  type CodeHighlightSpec,
  type ExtensionArg,
  type ExtensionSpec,
  type HandlerSpec,
  type Json,
  NAMESPACE,
  runScript,
  type Script,
  type Step,
  type TypeaheadSpec,
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

test('scripts/code-highlight', () => {
  // @lexical/code-prism's `registerCodeHighlighting` over a tokenizer: the
  // conformance tokenizer (ops.ts's CONFORMANCE_TOKENIZE, which every
  // runner implements) or tokens recorded from Prism. After each step the
  // log ends with each code block's key, language, whether it is
  // highlighted, and its children's keys, which show what the diff kept.
  const code = true;
  const richText = true;
  const conformance: CodeHighlightSpec = {
    languages: ['javascript', 'diff'],
    tokenizer: 'conformance',
  };
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  const scripts: Script[] = [
    {
      code,
      codeHighlight: conformance,
      // Keys: the block 1, then (after highlighting) "let" 3, " " 4, "x" 5,
      // " " 6, "=" 7, " " 8, "1" 9.
      name: 'typing keeps the tokens that did not change',
      richText,
      steps: [
        doc(codeBlock('javascript', ch('let x = 1'))),
        // In the middle: only the word is replaced.
        update(select(['5', 1, 'text'])),
        type('yz'),
        // A space splits the word's run.
        type(' '),
        // At the start: a keyword before.
        update(select(['3', 0, 'text'])),
        type('if '),
        // Removing the space between two words joins them.
        update(
          select(['25', 0, 'text'], ['25', 1, 'text']),
          call('removeText'),
        ),
        // A token that is the same once typed into is kept.
        update(select(['9', 1, 'text'])),
        type('0'),
        // Two tokens changed in one update are highlighted once.
        update(
          node('19', 'setTextContent', 'return'),
          node('7', 'setTextContent', '=='),
        ),
      ],
    },
    {
      code,
      codeHighlight: conformance,
      // Keys: the block 1, "return" 3, " " 4, "a" 5, "+" 6, "b" 7.
      name: 'a caret keeps its text offset',
      richText,
      steps: [
        doc(codeBlock('javascript', ch('return a+b'))),
        // Inside a keyword that stops being one.
        update(select(['3', 3, 'text'])),
        type('X'),
        // A caret at the start of a token is at the end of the one before
        // once the block is highlighted again.
        update(select(['7', 0, 'text'])),
        update(node('5', 'setTextContent', 'a b')),
        // A caret in a token that is kept, after one that changed.
        update(select(['7', 1, 'text'])),
        update(node('15', 'setTextContent', 'aa')),
        // A caret outside the block stays where it is.
        update(
          {do: 'append', key: 'root', nodes: [p(t('after'))]},
          select(['root', 1, 'element']),
        ),
        update(node('8', 'setTextContent', 'let')),
      ],
    },
    {
      code,
      codeHighlight: conformance,
      // Keys: the block 1, "let" 3, " " 4, "a" 5, " " 6, "=" 7, " " 8, and
      // the string's '"' 9, "s" 10, '"' 11.
      name: 'a range selection collapses to its anchor',
      richText,
      steps: [
        doc(codeBlock('javascript', ch('let a = "s"'))),
        // Forward, across tokens; the block changes elsewhere.
        update(select(['3', 1, 'text'], ['10', 1, 'text'])),
        update(node('7', 'setTextContent', '+=')),
        // Backward.
        update(select(['10', 1, 'text'], ['3', 1, 'text'])),
        update(node('17', 'setTextContent', '==')),
        // Typing over a range across tokens.
        update(select(['5', 0, 'text'], ['10', 0, 'text'])),
        type('b ='),
        // Nothing changes, so nothing is selected again.
        update(select(['3', 1, 'text'], ['36', 0, 'text'])),
        update(node('1', 'markDirty')),
      ],
    },
    {
      code,
      codeHighlight: conformance,
      // Keys: the block 1, then the line break 3, "let" 4, a tab 5, "a" 6,
      // a line break 7, a tab 8, "b" 9, a line break 10.
      name: 'line breaks and tabs',
      richText,
      steps: [
        // A caret before a line break becomes an element point.
        update(
          {
            do: 'append',
            key: 'root',
            nodes: [codeBlock('javascript', ch('\nlet\ta\n\tb\n'))],
          },
          select(['2', 0, 'text']),
        ),
        // A caret after a line break is kept as an element point.
        update(select(['1', 5, 'element'])),
        update(node('4', 'setTextContent', 'xy')),
        // Raw text with a tab and line breaks.
        update(select(['9', 1, 'text']), call('insertRawText', ' c\td\r\ne')),
        // INSERT_TAB_COMMAND (registerCodeIndentation) inserts a tab.
        run('INSERT_TAB_COMMAND'),
        // Enter adds a line: the caret is on the code block.
        run('INSERT_PARAGRAPH_COMMAND'),
        type('f'),
        // A text node with a tab and a line break in it is split.
        update(select(['1', 0, 'element']), {
          do: 'insertNodes',
          nodes: [t('\tg\nh')],
        }),
      ],
    },
    {
      code,
      codeHighlight: conformance,
      name: 'setLanguage and an unknown language',
      richText,
      steps: [
        doc(codeBlock('javascript', ch('+ a\n b\n!c\n-d\nx'))),
        // The second language of the tokenizer, with nested tokens.
        update(node('1', 'setLanguage', 'diff')),
        // A language the tokenizer doesn't know: plain tokens.
        update(node('1', 'setLanguage', 'nolang')),
        // No language: the tokenizer's default.
        update(node('1', 'setLanguage', null)),
        update(node('1', 'setLanguage', 'diff')),
        update(node('1', 'setLanguage', '')),
        // isSyntaxHighlightSupported follows the language, whatever it is
        // set to.
        update(node('1', 'setIsSyntaxHighlightSupported', false)),
        update(
          node('1', 'setLanguage', 'nolang'),
          node('1', 'setIsSyntaxHighlightSupported', true),
        ),
      ],
    },
    {
      code,
      codeHighlight: {...conformance, defaultLanguage: 'diff'},
      name: 'a block without a language takes defaultLanguage',
      richText,
      steps: [
        doc(codeBlock(null, ch('+a\n b')), codeBlock('javascript', t('c d'))),
        // A text node appended to a block is highlighted.
        update({do: 'append', key: '1', nodes: [t('\n-e')]}),
      ],
    },
    {
      code,
      codeHighlight: {...conformance, defaultLanguage: null},
      name: 'defaultLanguage null leaves a block without a language plain',
      richText,
      steps: [
        doc(codeBlock(null, t('let a'), ch('\tb'))),
        update(node('1', 'setLanguage', 'javascript')),
        update(node('1', 'setLanguage', null)),
      ],
    },
    {
      code,
      codeHighlight: {...conformance, defaultLanguage: 'nolang'},
      name: 'a defaultLanguage the tokenizer does not know',
      richText,
      steps: [doc(codeBlock(null, ch('let a')))],
    },
    {
      code,
      codeHighlight: conformance,
      // Keys: the block 1, "let" 3, " " 4, "a" 5, " " 6, "=" 7, " " 8,
      // "b" 9.
      name: 'NodeState of the tokens',
      richText,
      steps: [
        doc(codeBlock('javascript', ch('let a = b'))),
        update(
          {do: 'setState', key: '3', state: 'count', value: 3},
          {do: 'setState', key: '5', state: 'label', value: 'x'},
          {do: 'setState', key: '9', state: 'label', value: 'y'},
        ),
        // A token that is the same once typed into keeps its state.
        update(select(['9', 1, 'text'])),
        type('c'),
        // One that is replaced doesn't.
        update(select(['5', 1, 'text'])),
        type(' '),
        // The block becomes a paragraph: its tokens become text nodes with
        // their state, and only those with equal state merge.
        update(node('1', 'replace', p(), true)),
      ],
    },
    {
      code,
      codeHighlight: conformance,
      // Keys: the block 1, "let" 3, " " 4, the emoji 5, " " 6, "=" 7, " " 8,
      // '"' 9, the escape 10, "x" 11, '"' 12.
      name: 'text outside the BMP',
      richText,
      steps: [
        doc(codeBlock('javascript', ch('let \u{1F600} = "\\\u{1F600}x"'))),
        update(select(['5', 2, 'text'])),
        type('\u{1F601}'),
        update(select(['11', 1, 'text'])),
        type('\\\u{1F602}'),
        // The backslash left escapes the closing quote.
        update(
          select(['32', 1, 'text'], ['32', 3, 'text']),
          call('removeText'),
        ),
      ],
    },
    {
      code,
      codeHighlight: conformance,
      // Keys: the block 1, "let" 3, " " 4, "a" 5.
      name: 'unregistering',
      richText,
      steps: [
        doc(codeBlock('javascript', ch('let a'))),
        update(select(['5', 1, 'text'])),
        {unregisterCodeHighlighting: true},
        type(' = b'),
        update({do: 'append', key: '1', nodes: [t('\tc')]}),
        // Rich text's handler, now that the indent handlers are gone.
        run('INSERT_TAB_COMMAND'),
      ],
    },
  ];
  // Prism's own tokens: the script runs once with PrismTokenizer, which
  // collects what Prism returns, then is recorded replaying those tokens,
  // which must give the same results.
  const prism: Omit<Script, 'codeHighlight'> = {
    code,
    // Keys: the blocks 1 and 3.
    name: 'tokens recorded from Prism',
    richText,
    steps: [
      doc(
        codeBlock(
          'javascript',
          ch('const s = "a\\tb"; // hi\nfunction f(x) {\n\treturn x + 1;\n}'),
        ),
        codeBlock('diff', ch('+ added\n- removed\n  same\n! changed')),
      ),
      update(select(['1', 0, 'element'])),
      type('let y = 2;\n'),
      update(select(['3', 0, 'element'])),
      type('-'),
    ],
  };
  const spec: CodeHighlightSpec = {
    languages: ['javascript', 'diff'],
    tokenizer: 'prism',
  };
  const fromPrism = runScript({...prism, codeHighlight: spec});
  const recorded: Script = {
    ...prism,
    codeHighlight: {
      languages: spec.languages,
      tokenizer: 'recorded',
      tokens: spec.tokens,
    },
  };
  const replayed = runScript(recorded);
  expect(replayed.results).toEqual(fromPrism.results);
  scripts.push(recorded);
  emit(
    'code-highlight',
    "@lexical/code-prism's registerCodeHighlighting over a Tokenizer: the CodeNode, TextNode and CodeHighlightNode transforms (codeNodeTransform, textNodeTransform with $copyNodeState), $updateAndRetainSelection, getDiffRange and isEqual, $mapTokensToLexicalStructure's flattening, the nodesCurrentlyHighlighting guard, defaultLanguage, unknown languages, isSyntaxHighlightSupported and unregistering, with a deterministic tokenizer every runner implements and with tokens recorded from Prism.",
    scripts,
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

test('scripts/text', () => {
  // @lexical/text's root helpers, read inside an update: $rootTextContent,
  // $isRootTextContentEmpty (with and without trimming, composing or not)
  // and its curried form, $canShowPlaceholder and its curried form, and
  // $findTextIntersectionFromCharacters at offsets across the text nodes.
  // Keys: the root's descendants get 1, 2, 3… in document order.
  const rootText: Action = {do: 'logRootTextContent'};
  const empty = (isComposing: boolean, trim?: boolean): Action =>
    trim === undefined
      ? {do: 'logRootTextContentEmpty', isComposing}
      : {do: 'logRootTextContentEmpty', isComposing, trim};
  const placeholder = (isComposing: boolean): Action => ({
    do: 'logCanShowPlaceholder',
    isComposing,
  });
  const intersection = (characters: number): Action => ({
    characters,
    do: 'logTextIntersection',
  });
  const probe = (...characters: number[]): Step =>
    update(
      rootText,
      empty(false),
      empty(false, true),
      empty(false, false),
      empty(true),
      empty(true, false),
      placeholder(false),
      placeholder(true),
      ...characters.map(intersection),
    );
  const clear = node('root', 'clear');
  const hr = (): Node => ({type: 'horizontalrule', version: 1}) as Node;
  const dt = (): Node => ({type: 'decorator-text', version: 1}) as Node;
  emit(
    'text',
    "@lexical/text's root helpers: $rootTextContent, $isRootTextContentEmpty and $isRootTextContentEmptyCurry (trimmed with JS's String#trim or not, composing or not), $canShowPlaceholder and $canShowPlaceholderCurry, and $findTextIntersectionFromCharacters, on documents of text, whitespace, breaks, tabs, nested and empty blocks, headings and decorators.",
    [
      {
        name: 'an empty root, then an empty paragraph',
        steps: [probe(0), doc(p()), probe(0)],
      },
      {
        name: 'text in a paragraph',
        steps: [doc(p(t('ab'), t('cd', {format: 1}))), probe(0, 1, 2, 3, 4, 5)],
      },
      {
        name: 'whitespace that String#trim removes',
        steps: [doc(p(t(' \u00a0\ufeff\u2028\u3000'))), probe(0, 4, 5)],
      },
      {
        name: 'whitespace that String#trim keeps',
        steps: [doc(p(t('\u0085\u200b'))), probe(0, 1, 2)],
      },
      {
        name: 'text outside the BMP',
        steps: [doc(p(t('\ud83d\ude00a'))), probe(0, 1, 2, 3)],
      },
      {
        name: 'two paragraphs',
        steps: [doc(p(t('ab')), p(t('cd'))), probe(0, 1, 2, 3, 4)],
      },
      {
        name: 'two empty paragraphs',
        steps: [doc(p(), p()), probe(0)],
      },
      {
        name: 'an indented paragraph',
        steps: [doc({...p(), indent: 1} as Node), probe(0)],
      },
      {
        name: 'a line break',
        steps: [doc(p(br())), probe(0)],
      },
      {
        name: 'a tab',
        steps: [doc(p(tab())), probe(0, 1)],
      },
      {
        name: 'a heading and a quote',
        richText: true,
        steps: [
          doc(h('h1')),
          probe(0),
          update(node('1', 'append', [t('x')])),
          probe(0, 1),
          update(clear),
          doc(q(t('ab')), p(t('c'))),
          probe(0, 1, 2, 3),
        ],
      },
      {
        list: true,
        name: 'nested lists',
        steps: [
          doc(
            list('bullet', li(t('a')), li(list('bullet', li(t('b'))))),
            p(t('c')),
          ),
          probe(0, 1, 2, 3),
        ],
      },
      {
        extension: true,
        name: 'decorators',
        steps: [
          doc(p(dt())),
          probe(0),
          update(clear),
          doc(hr()),
          probe(0),
          update(clear),
          doc(p(dt(), t('ab'))),
          probe(0, 1, 2),
        ],
      },
    ],
  );
});

test('scripts/extensions', () => {
  // @lexical/extension's builder (buildEditorFromExtensions, LexicalBuilder.ts
  // and ExtensionRep.ts) with test extensions made from JSON (ops.ts's
  // ExtensionSpec), which log each phase they run with their merged config,
  // and a few of the package's own extensions. The build log is the
  // script's `build`; InitialStateExtension, which the builder always adds
  // first, logs nothing itself.
  const EXT: {[key: string]: ExtensionSpec} = {
    base: {
      config: {items: ['b'], label: 'base'},
      inspect: true,
      mergeConfig: 'concat',
      name: 'test/base',
    },
    conflict: {conflictsWith: ['test/base'], name: 'test/conflict'},
    dupA: {name: 'test/dup'},
    dupB: {name: 'test/dup'},
    early: {buildPeers: ['test/late'], name: 'test/early'},
    earlyInit: {initPeers: ['test/late'], name: 'test/early-init'},
    hr: {
      initialEditorState: {
        json: {
          root: {
            children: [
              p(t('rule below')),
              {type: 'horizontalrule', version: 1} as Node,
            ],
            direction: null,
            format: '',
            indent: 0,
            type: 'root',
            version: 1,
          },
        },
      },
      listen: true,
      name: 'test/hr',
      nodes: ['horizontalrule'],
    },
    hrUnregistered: {
      initialEditorState: {
        json: {
          root: {
            children: [{type: 'horizontalrule', version: 1} as Node],
            direction: null,
            format: '',
            indent: 0,
            type: 'root',
            version: 1,
          },
        },
      },
      name: 'test/hr-unregistered',
    },
    late: {config: {label: 'late'}, name: 'test/late'},
    left: {
      config: {side: 'left'},
      dependencies: [['base', {items: ['l'], label: 'from-left'}]],
      name: 'test/left',
    },
    lookup: {lookups: ['base'], name: 'test/lookup'},
    noInit: {initResult: true, name: 'test/no-init', phases: ['register']},
    nullState: {initialEditorState: null, listen: true, name: 'test/null'},
    peer: {config: {label: 'peer', other: 1}, name: 'test/peer'},
    peerUser: {
      buildPeers: ['test/peer', 'test/absent'],
      initPeers: ['test/peer', 'test/absent'],
      inspect: true,
      name: 'test/peer-user',
      peerDependencies: [['test/peer', {label: 'from-user'}], ['test/absent']],
    },
    quiet: {name: 'test/quiet', phases: []},
    right: {
      config: {side: 'right'},
      dependencies: [['base', {items: ['r']}, {items: ['r2']}]],
      lookups: ['base'],
      name: 'test/right',
    },
    textState: {
      initialEditorState: {text: 'from a function'},
      listen: true,
      name: 'test/text',
    },
    toClear: {
      dependencies: ['ClearEditorExtension'],
      initialEditorState: {text: 'clear me'},
      name: 'test/to-clear',
    },
    toggle: {
      config: {disabled: false, label: 'a'},
      name: 'test/toggle',
      signals: true,
    },
    top: {
      abort: true,
      dependencies: ['left', 'right'],
      initResult: true,
      inspect: true,
      name: 'test/top',
    },
    watch: {
      dependencies: ['EditorStateExtension'],
      editorState: true,
      name: 'test/watch',
    },
    withDecoratorText: {
      dependencies: ['DecoratorTextExtension'],
      initialEditorState: {
        json: {
          root: {
            children: [
              p(t('a'), {type: 'decorator-text', version: 1} as Node, t('b')),
            ],
            direction: null,
            format: '',
            indent: 0,
            type: 'root',
            version: 1,
          },
        },
      },
      name: 'test/with-decorator-text',
    },
  };
  // The specs `extensions` names, with their dependencies.
  const defs = (extensions: ExtensionArg[], ...more: string[]) => {
    const out: {[key: string]: ExtensionSpec} = {};
    const add = (arg: ExtensionArg) => {
      const key = typeof arg === 'string' ? arg : arg[0];
      const spec = EXT[key];
      if (spec !== undefined && out[key] === undefined) {
        out[key] = spec;
        (spec.dependencies ?? []).forEach(add);
      }
    };
    [...extensions, ...more].forEach(add);
    return out;
  };
  const script = (
    name: string,
    extensions: ExtensionArg[],
    steps: Step[] = [],
    ...more: string[]
  ): Script => ({
    extensionDefs: defs(extensions, ...more),
    extensions,
    name,
    steps,
  });
  const dispatch = (payload: Json): Step => ({
    dispatch: 'EXTENSION_COMMAND',
    payload,
  });
  const signals = (...entries: [string, Json][]): Step => ({
    setSignals: entries.map(([name, value]) => ['toggle', name, value]),
  });
  emit(
    'extensions',
    'buildEditorFromExtensions: the order of the phases across a dependency graph, config merging, peer dependencies, errors, disposal, initial states, node registration and signals.',
    [
      script(
        'a diamond: phase order, merge order and disposal',
        ['top', ['base', {label: 'root'}]],
        [
          dispatch('x'),
          {logDependency: 'base'},
          {logDependency: 'left'},
          {logDependency: 'top'},
          {logPeer: 'test/right'},
          {dispose: true},
          dispatch('y'),
          {dispose: true},
        ],
      ),
      script('an extension listed twice is added once', [
        'base',
        ['base', {items: ['again']}],
        'base',
      ]),
      script('an extension that defines no phase', ['quiet'], [dispatch(1)]),
      script(
        'a peer dependency that is present',
        ['peerUser', 'peer'],
        [{logPeer: 'test/peer'}, {logPeer: 'test/absent'}],
      ),
      script(
        'a peer dependency that is absent',
        ['peerUser'],
        [{logPeer: 'test/peer'}, {logDependency: 'peerUser'}],
      ),
      script('a peer before it is configured', ['earlyInit', 'late']),
      script('a peer before it is built', ['early', 'late']),
      script('a peer that is built first', ['late', 'early']),
      script('two extensions with one name', ['dupA', 'dupB']),
      script('a conflict with an earlier extension', ['base', 'conflict']),
      script('a conflict with a later extension', ['conflict', 'base']),
      script('a dependency outside the editor', ['lookup'], [], 'base'),
      script('getInitResult without init', ['noInit']),
      script(
        'an extension that is not in the editor',
        ['base', 'dupA'],
        [{logDependency: 'left'}, {logDependency: 'dupB'}, dispatch(1)],
        'left',
        'dupB',
      ),
      script('the default initial state', ['base']),
      script('an initial state from JSON with a registered node', ['hr']),
      script('an initial state with a node not registered', ['hrUnregistered']),
      script(
        'an initial state from a function',
        ['textState'],
        [update(node('2', 'setTextContent', 'x'))],
      ),
      script('a null initial state', ['nullState']),
      script('the last initial state wins', ['hr', 'textState']),
      script('DecoratorTextExtension registers its node', [
        'withDecoratorText',
      ]),
      script(
        'ClearEditorExtension',
        ['toClear'],
        [
          update(select(['2', 3, 'text'])),
          {dispatch: 'CLEAR_EDITOR_COMMAND'},
          {dispose: true},
          {dispatch: 'CLEAR_EDITOR_COMMAND'},
        ],
      ),
      script(
        'EditorStateExtension as a signal',
        ['watch'],
        [
          update(node('1', 'append', [t('typed')])),
          {dispose: true},
          update(node('2', 'setTextContent', 'x')),
        ],
      ),
      script(
        'namedSignals toggle a registration',
        [['toggle', {label: 'b'}]],
        [
          dispatch(1),
          signals(['label', 'c']),
          dispatch(2),
          signals(['disabled', true]),
          dispatch(3),
          signals(['disabled', false], ['label', 'd']),
          dispatch(4),
          signals(['label', 'd']),
          signals(['disabled', true], ['disabled', false]),
          signals(['label', 'e'], ['disabled', true]),
          {dispose: true},
          signals(['disabled', false]),
          dispatch(5),
        ],
      ),
      script(
        'a signal disabled by config',
        [['toggle', {disabled: true}]],
        [dispatch(1), signals(['disabled', false]), dispatch(2)],
      ),
      {
        ...script(
          'handlers registered after the build',
          ['base', 'left'],
          [dispatch('z')],
        ),
        handlers: [
          {command: 'EXTENSION_COMMAND', id: 'h', priority: 1, result: false},
        ],
      },
    ],
  );
});

function mark(ids: unknown, ...children: Node[]): Node {
  return {
    children,
    direction: null,
    format: '',
    ids,
    indent: 0,
    type: 'mark',
    version: 1,
  } as Node;
}

test('scripts/mark', () => {
  // @lexical/mark: MarkNode (its ids, its insertion behavior through typing,
  // Enter and deletion at its edges, extractWithChild and excludeFromCopy),
  // $wrapSelectionInMarkNode, $unwrapMarkNode and $getMarkIDs (index.ts),
  // and the MarkState functions of MarkStateExtension.ts that need no
  // extension ($addMarkID, $removeMarkID, $getNodeMarkIDs,
  // $getMarkIDsAtSelection, $migrateMarkNodesToMarkState), which store ids as
  // the `markIds` NodeState of the leaves. Commands are dispatched outside an
  // update, as an input layer does.
  const hr = (): Node => ({type: 'horizontalrule', version: 1}) as Node;
  const dt = (): Node => ({type: 'decorator-text', version: 1}) as Node;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  const enter = run('INSERT_PARAGRAPH_COMMAND');
  const backspace = run('DELETE_CHARACTER_COMMAND', true);
  const del = run('DELETE_CHARACTER_COMMAND', false);
  const wrap = (id: string, isBackward = false): Action => ({
    do: 'wrapSelectionInMarkNode',
    id,
    isBackward,
  });
  const unwrap = (key: string): Action => ({do: 'unwrapMarkNode', key});
  const markIDs = (key: string, offset: number): Action => ({
    do: 'logMarkIDs',
    key,
    offset,
  });
  const addMarkID = (id: string): Action => ({do: 'addMarkID', id});
  const removeMarkID = (id: string, inSelection?: boolean): Action =>
    inSelection === undefined
      ? {do: 'removeMarkID', id}
      : {do: 'removeMarkID', id, inSelection};
  const nodeMarkIDs = (key: string): Action => ({do: 'logNodeMarkIDs', key});
  const atSelection = (mode?: 'any' | 'all'): Action =>
    mode === undefined
      ? {do: 'logMarkIDsAtSelection'}
      : {do: 'logMarkIDsAtSelection', mode};
  const migrate = (key?: string): Action =>
    key === undefined
      ? {do: 'migrateMarkNodesToMarkState'}
      : {do: 'migrateMarkNodesToMarkState', key};
  const ids = (...markIds: unknown[]) => ({$: {markIds}});
  const extension = true;
  const link = true;
  const richText = true;
  // Keys: the paragraph 1, "ab" 2, the mark 3, "cd" 4, "ef" 5.
  const edges = doc(p(t('ab'), mark(['a'], t('cd')), t('ef')));
  emit(
    'mark',
    "@lexical/mark: $wrapSelectionInMarkNode across text nodes, partial text, blocks, backward selections, overlapping marks, line breaks, decorators and inline elements; $getMarkIDs; $unwrapMarkNode; MarkNode's ids (addID, deleteID down to none, setIDs, hasID, getIDs), its JSON, and its insertion behavior (canInsertTextBefore/After, canBeEmpty, isInline, insertNewAfter, extractWithChild, excludeFromCopy) through typing, Enter and deletion at its edges; and the MarkState functions that need no extension ($addMarkID, $removeMarkID, $getNodeMarkIDs, $getMarkIDsAtSelection, $migrateMarkNodesToMarkState).",
    [
      {
        mark: true,
        name: 'wrapping text nodes and partial text',
        steps: [
          doc(p(t('Hello '), t('bold', {format: 1}), t(' world')), p(t('one'))),
          update(select(['2', 2, 'text'], ['4', 3, 'text']), wrap('a')),
          update(select(['6', 1, 'text'], ['6', 2, 'text']), wrap('b')),
          update(select(['6', 0, 'text'], ['6', 0, 'text']), wrap('c')),
        ],
      },
      {
        mark: true,
        name: 'wrapping backward selections',
        steps: [
          doc(
            p(t('Hello '), t('bold', {format: 1}), t(' world')),
            p(t('abcdef')),
          ),
          update(select(['4', 3, 'text'], ['2', 2, 'text']), wrap('a', true)),
          update(select(['6', 5, 'text'], ['6', 1, 'text']), wrap('b')),
        ],
      },
      {
        mark: true,
        name: 'wrapping across blocks',
        steps: [
          doc(p(t('one')), p(t('two'), t('2', {format: 1})), p(t('three'))),
          update(select(['2', 1, 'text'], ['7', 2, 'text']), wrap('a')),
          update(
            select(['root', 0, 'element'], ['root', 3, 'element']),
            wrap('b'),
          ),
        ],
      },
      {
        mark: true,
        name: 'overlapping marks',
        steps: [
          doc(p(t('abcdef'))),
          update(select(['2', 1, 'text'], ['2', 4, 'text']), wrap('a')),
          update(select(['3', 1, 'text'], ['4', 1, 'text']), wrap('b')),
        ],
      },
      {
        extension,
        link,
        mark: true,
        name: 'line breaks, decorators and inline elements in the range',
        steps: [
          // Keys: the paragraph 1, "a" 2, the line break 3, "b" 4, the link
          // 5, "c" 6, "d" 7, the inline decorator 8, "e" 9, the rule 10, the
          // paragraph 11, "f" 12.
          doc(
            p(
              t('a'),
              br(),
              t('b'),
              a('https://x', t('c')),
              t('d'),
              dt(),
              t('e'),
            ),
            hr(),
            p(t('f')),
          ),
          update(select(['2', 0, 'text'], ['12', 1, 'text']), wrap('a')),
        ],
      },
      {
        extension,
        link,
        mark: true,
        name: 'wrapping from inside a link',
        steps: [
          doc(p(t('a'), a('https://x', t('bc')), t('d'))),
          update(select(['4', 1, 'text'], ['5', 1, 'text']), wrap('a')),
        ],
      },
      {
        mark: true,
        name: 'getMarkIDs',
        steps: [
          // Keys: the paragraph 1, "ab" 2, the mark 3, "cd" 4, the mark 5,
          // "ef" 6, "gh" 7.
          doc(
            p(
              t('ab'),
              mark(['a', 'b'], t('cd'), mark(['c'], t('ef'))),
              t('gh'),
            ),
          ),
          update(
            markIDs('2', 0),
            markIDs('2', 1),
            markIDs('2', 2),
            markIDs('4', 0),
            markIDs('4', 2),
            markIDs('6', 1),
            markIDs('7', 0),
            markIDs('7', 2),
          ),
        ],
      },
      {
        mark: true,
        name: 'typing at the start of a mark',
        richText,
        steps: [edges, update(select(['4', 0, 'text'])), type('x')],
      },
      {
        mark: true,
        name: 'typing at the end of a mark',
        richText,
        steps: [edges, update(select(['4', 2, 'text'])), type('x')],
      },
      {
        mark: true,
        name: 'typing inside a mark',
        richText,
        steps: [edges, update(select(['4', 1, 'text'])), type('x')],
      },
      {
        mark: true,
        name: 'typing at element points around a mark',
        richText,
        steps: [
          edges,
          update(select(['3', 0, 'element'])),
          type('x'),
          update(select(['3', 1, 'element'])),
          type('y'),
          update(select(['1', 1, 'element'])),
          type('z'),
        ],
      },
      {
        mark: true,
        name: 'typing over a range that ends in a mark',
        richText,
        steps: [
          edges,
          update(select(['2', 1, 'text'], ['4', 1, 'text'])),
          type('x'),
        ],
      },
      {
        mark: true,
        name: 'Enter in a mark',
        richText,
        steps: [
          edges,
          update(select(['4', 1, 'text'])),
          enter,
          update(select(['4', 1, 'text'])),
          enter,
        ],
      },
      {
        mark: true,
        name: 'removing all of a mark',
        richText,
        steps: [
          edges,
          update(select(['4', 0, 'text'], ['4', 2, 'text'])),
          run('REMOVE_TEXT_COMMAND'),
        ],
      },
      {
        mark: true,
        name: 'removing text across a mark edge',
        richText,
        steps: [
          edges,
          update(select(['2', 1, 'text'], ['4', 1, 'text'])),
          del,
          update(select(['4', 1, 'text'], ['5', 1, 'text'])),
          backspace,
        ],
      },
      {
        mark: true,
        name: 'Backspace and Delete at mark edges',
        richText,
        steps: [
          edges,
          update(select(['4', 0, 'text'])),
          backspace,
          update(select(['4', 2, 'text'])),
          del,
          update(select(['2', 2, 'text'])),
          del,
        ],
      },
      {
        mark: true,
        name: 'removing the last child of a mark',
        steps: [edges, update(node('4', 'remove'))],
      },
      {
        mark: true,
        name: 'unwrapping',
        steps: [
          doc(p(t('a'), mark(['x'], t('b'), t('c', {format: 1})), t('d'))),
          update(select(['4', 1, 'text']), unwrap('3')),
        ],
      },
      {
        mark: true,
        name: 'unwrapping nested marks',
        steps: [
          // Keys: the paragraph 1, the mark 2, "a" 3, the mark 4, "b" 5.
          doc(p(mark(['x'], t('a'), mark(['y'], t('b'))))),
          update(select(['5', 1, 'text']), unwrap('2')),
          update(unwrap('4')),
        ],
      },
      {
        mark: true,
        name: 'mark ids',
        steps: [
          doc(p(mark(['a'], t('x')))),
          update(node('2', 'addID', 'b'), node('2', 'getIDs')),
          update(node('2', 'addID', 'a'), node('2', 'getIDs')),
          update(
            node('2', 'hasID', 'a'),
            node('2', 'hasID', 'z'),
            node('2', 'deleteID', 'z'),
          ),
          update(node('2', 'deleteID', 'a'), node('2', 'getIDs')),
          update(node('2', 'setIDs', ['c', 'b', 'c']), node('2', 'getIDs')),
          update(node('2', 'deleteID', 'c'), node('2', 'getIDs')),
          update(node('2', 'addID', 'c'), node('2', 'getIDs')),
        ],
      },
      {
        mark: true,
        name: 'deleting the last id',
        steps: [
          doc(p(t('a'), mark(['x'], t('b')), t('c'))),
          // The mark stays, with no ids.
          update(node('3', 'deleteID', 'x'), node('3', 'getIDs')),
          // As the playground's CommentPlugin does next.
          update(unwrap('3')),
        ],
      },
      {
        mark: true,
        name: 'MarkNode JSON with ids that are not strings',
        steps: [
          doc(
            p(
              mark(['a', 1, null, 'b'], t('x')),
              mark('a', t('y')),
              mark(undefined, t('z')),
            ),
          ),
          update(node('2', 'getIDs'), node('4', 'getIDs'), node('6', 'getIDs')),
        ],
      },
      {
        mark: true,
        name: 'extractWithChild and excludeFromCopy',
        steps: [
          // Keys: the paragraph 1, the mark 2, "cd" 3, " " 4, the mark 5,
          // "ef" 6, "gh" 7.
          doc(
            p(
              mark(['a'], t('cd')),
              t(' '),
              mark(['b'], t('ef'), t('gh', {format: 1})),
            ),
          ),
          update(
            select(['3', 0, 'text'], ['3', 2, 'text']),
            node('2', 'extractWithChild', '3', 'clone'),
            node('2', 'extractWithChild', '3', 'html'),
          ),
          update(
            select(['3', 2, 'text'], ['3', 0, 'text']),
            node('2', 'extractWithChild', '3', 'clone'),
          ),
          update(
            select(['3', 0, 'text'], ['3', 1, 'text']),
            node('2', 'extractWithChild', '3', 'clone'),
          ),
          update(
            select(['3', 0, 'text'], ['4', 1, 'text']),
            node('2', 'extractWithChild', '3', 'clone'),
          ),
          update(
            select(['6', 0, 'text'], ['7', 2, 'text']),
            node('5', 'extractWithChild', '6', 'clone'),
          ),
          update(
            select(['6', 2, 'text'], ['7', 2, 'text']),
            node('5', 'extractWithChild', '7', 'clone'),
          ),
          update(
            {do: 'selectNodes', keys: ['3']},
            node('2', 'extractWithChild', '3', 'clone'),
            node('2', 'excludeFromCopy', 'clone'),
            node('2', 'excludeFromCopy', 'html'),
          ),
        ],
      },
      {
        extension,
        link,
        mark: true,
        name: 'addMarkID',
        steps: [
          // Keys: the paragraph 1, "Hello world" 2, the line break 3, "next"
          // 4, the link 5, "link" 6, the inline decorator 7, "end" 8, the
          // rule 9, the paragraph 10, "second" 11.
          doc(
            p(
              t('Hello world'),
              br(),
              t('next'),
              a('https://x', t('link')),
              dt(),
              t('end'),
            ),
            hr(),
            p(t('second')),
          ),
          update(select(['2', 6, 'text'], ['11', 3, 'text']), addMarkID('c1')),
          update(select(['2', 0, 'text'], ['4', 2, 'text']), addMarkID('c2')),
          // Collapsed: nothing is marked.
          update(select(['2', 1, 'text']), addMarkID('c3')),
          // A node selection of the inline decorator.
          update({do: 'selectNodes', keys: ['7']}, addMarkID('c4')),
          // Invalid ids throw.
          update(select(['2', 0, 'text'], ['2', 1, 'text']), addMarkID('a b')),
          update(select(['2', 0, 'text'], ['2', 1, 'text']), addMarkID('')),
        ],
      },
      {
        mark: true,
        name: 'getNodeMarkIDs and getMarkIDsAtSelection',
        steps: [
          // Keys: the paragraph 1, "ab" 2, "cd" 3, "ef" 4, "gh" 5.
          doc(p(t('ab'), t('cd', ids('b', 'a')), t('ef', ids('a')), t('gh'))),
          update(
            nodeMarkIDs('1'),
            nodeMarkIDs('2'),
            nodeMarkIDs('3'),
            nodeMarkIDs('4'),
          ),
          update(select(['2', 1, 'text']), atSelection()),
          update(select(['2', 2, 'text']), atSelection()),
          update(select(['3', 1, 'text']), atSelection()),
          update(select(['4', 2, 'text']), atSelection()),
          update(select(['1', 1, 'element']), atSelection()),
          update(select(['1', 0, 'element']), atSelection()),
          update(
            select(['2', 1, 'text'], ['4', 1, 'text']),
            atSelection(),
            atSelection('any'),
            atSelection('all'),
          ),
          update(
            select(['2', 2, 'text'], ['3', 2, 'text']),
            atSelection('all'),
          ),
          update(
            select(['3', 1, 'text'], ['5', 0, 'text']),
            atSelection('all'),
          ),
          update(select(['5', 0, 'text'], ['5', 2, 'text']), atSelection()),
          update({do: 'clearSelection'}, atSelection()),
        ],
      },
      {
        mark: true,
        name: 'mark ids from JSON',
        steps: [
          doc(
            p(
              t('x', ids('b', 'a', 'a', ' bad', 3, '', null)),
              t(' '),
              t('y', ids('\uff01', '\u{1f600}', 'b')),
              t(' '),
              t('z', {$: {markIds: 'a'}}),
            ),
          ),
          update(nodeMarkIDs('2'), nodeMarkIDs('4'), nodeMarkIDs('6')),
          update(select(['2', 0, 'text'], ['4', 1, 'text']), addMarkID('c')),
        ],
      },
      {
        mark: true,
        name: 'removeMarkID',
        steps: [
          doc(
            p(t('one', ids('a', 'b')), t('two', ids('a')), t('three')),
            p(t('four', ids('a'))),
          ),
          update(
            select(['3', 1, 'text'], ['6', 2, 'text']),
            removeMarkID('a', true),
          ),
          update(removeMarkID('a')),
          update(removeMarkID('b')),
          update(removeMarkID('missing')),
        ],
      },
      {
        extension,
        link,
        mark: true,
        name: 'migrateMarkNodesToMarkState',
        steps: [
          doc(
            p(
              t('a'),
              mark(['x'], t('b'), mark(['y', 'x'], t('c')), br(), dt()),
              t('d'),
            ),
            p(mark(['z'], a('https://x', t('e'))), t('f')),
          ),
          update(migrate()),
          update(migrate()),
        ],
      },
      {
        mark: true,
        name: 'migrateMarkNodesToMarkState of a subtree',
        steps: [
          doc(p(mark(['x'], t('a'))), p(mark(['y'], t('b')))),
          // Keys: the paragraph 1, the mark 2, "a" 3, the paragraph 4, the
          // mark 5, "b" 6.
          update(migrate('1')),
          update(migrate('4')),
        ],
      },
      {
        mark: true,
        name: 'migrateMarkNodesToMarkState with an invalid id',
        steps: [doc(p(mark(['a b'], t('a')))), update(migrate())],
      },
    ],
  );
});

test('scripts/clipboard', () => {
  // @lexical/clipboard's JSON and plain-text paths: what a copy puts on the
  // clipboard (`$getLexicalContent` and the selection's text, written by
  // `setLexicalClipboardDataTransfer`; `text/html` needs a DOM and is left
  // out, see ops.ts), `$generateJSONFromSelectedNodes` (`excludeFromCopy`,
  // `extractWithChild`, the text slices at the selection's edges), and
  // pasting with `$insertDataTransferForRichText` (the
  // `application/x-lexical-editor`, `text/plain` and `text/uri-list`
  // importers) and `$insertDataTransferForPlainText`, with
  // `$insertGeneratedNodes` and the selection's format after it.
  const richText = true;
  const copy: Action = {do: 'copy'};
  const generateJSON: Action = {do: 'generateJSON'};
  const whole: Action = {do: 'generateJSON', whole: true};
  const pasteLexical = (...nodes: Node[]): Action => ({
    do: 'pasteLexical',
    nodes,
  });
  const pasteData = (data: Record<string, string>, plain = false): Action =>
    plain ? {data, do: 'pasteData', plain} : {data, do: 'pasteData'};
  const pasteClipboard = (plain = false): Action =>
    plain ? {do: 'pasteClipboard', plain} : {do: 'pasteClipboard'};
  const selectNodes = (...keys: string[]): Action => ({
    do: 'selectNodes',
    keys,
  });
  const hr = (): Node => ({type: 'horizontalrule', version: 1}) as Node;
  const dt = (): Node => ({type: 'decorator-text', version: 1}) as Node;
  emit(
    'clipboard',
    "@lexical/clipboard's JSON and plain-text paths: copying ($getLexicalContent, the selection's text and setLexicalClipboardDataTransfer, without text/html), $generateJSONFromSelectedNodes across text, blocks, list items, links, code, decorators and an overflow node (excludeFromCopy, extractWithChild, the text slices at the selection's edges, backward and node selections), and pasting Lexical JSON, plain text and URI lists with $insertDataTransferForRichText and $insertDataTransferForPlainText (collapsed, over a range, in a list item, a code block and a link, in rich and plain editors, falling back past a bad payload), $insertGeneratedNodes with the selection's format after it, and copying then pasting elsewhere.",
    [
      {
        // Keys: the paragraph 1, "hello world" 2, the heading 3, "second"
        // (bold) 4, the quote 5, "quote" 6.
        name: 'copying text and blocks',
        richText,
        steps: [
          doc(
            p(t('hello world')),
            h('h2', t('second', {format: 1})),
            q(t('quote')),
          ),
          // Part of one text node.
          update(
            select(['2', 1, 'text'], ['2', 5, 'text']),
            copy,
            generateJSON,
          ),
          // Across blocks: the heading comes with its text
          // (`HeadingNode.extractWithChild`), the paragraph doesn't.
          update(select(['2', 6, 'text'], ['4', 3, 'text']), copy),
          // The same, backward.
          update(select(['4', 3, 'text'], ['2', 6, 'text']), copy),
          // All the text: the quote's text comes without it.
          update(select(['2', 0, 'text'], ['6', 5, 'text']), copy),
          // The root's children, by element points.
          update(
            select(['root', 0, 'element'], ['root', 3, 'element']),
            copy,
            generateJSON,
          ),
          // Edges that slice text nodes to nothing.
          update(select(['2', 11, 'text'], ['4', 0, 'text']), copy),
          // Collapsed: no Lexical content.
          update(select(['2', 3, 'text']), copy),
          // A node selection of a block takes its whole subtree.
          update(selectNodes('3'), copy),
          update(selectNodes('2', '6'), copy),
          // No selection, and the whole document.
          update({do: 'clearSelection'}, copy, whole),
        ],
      },
      {
        // Keys: the centered, indented paragraph 1, "centered" 2, the
        // paragraph 3, "plain" 4.
        name: 'copying a paragraph with an alignment',
        richText,
        steps: [
          doc(
            {...p(t('centered')), format: 'center', indent: 1} as Node,
            p(t('plain')),
          ),
          // All of it: it comes as a block (`ParagraphNode.extractWithChild`).
          update(select(['2', 0, 'text'], ['2', 8, 'text']), copy),
          // Part of it: just the text.
          update(select(['2', 2, 'text'], ['2', 8, 'text']), copy),
          update(select(['2', 0, 'text'], ['4', 2, 'text']), copy),
          update(select(['1', 0, 'element'], ['1', 1, 'element']), copy),
        ],
      },
      {
        // Keys: the list 1, its items 2, 4 and 6 with "one" 3, "two" 5 and
        // "three" 7, the paragraph 8, "after" 9.
        list: true,
        name: 'copying list items',
        richText,
        steps: [
          doc(
            list('bullet', li(t('one')), li(t('two')), li(t('three'))),
            p(t('after')),
          ),
          // Part of an item: just the text.
          update(select(['3', 1, 'text'], ['3', 3, 'text']), copy),
          // All of an item's text: the item and its list come with it.
          update(select(['5', 0, 'text'], ['5', 3, 'text']), copy),
          // Across items.
          update(select(['3', 1, 'text'], ['7', 2, 'text']), copy),
          // From an item to the paragraph after the list.
          update(select(['5', 1, 'text'], ['9', 2, 'text']), copy),
          update(select(['7', 5, 'text'], ['3', 0, 'text']), copy),
        ],
      },
      {
        // Keys: the paragraph 1, "see " 2, the link 3, "the docs" 4, " now"
        // 5.
        link: true,
        name: 'copying links',
        richText,
        steps: [
          doc(p(t('see '), a('https://lexical.dev', t('the docs')), t(' now'))),
          // Inside the link: it comes with its text
          // (`LinkNode.extractWithChild`).
          update(select(['4', 1, 'text'], ['4', 5, 'text']), copy),
          // Into it from before.
          update(select(['2', 1, 'text'], ['4', 3, 'text']), copy),
          // Out of it, backward.
          update(select(['5', 3, 'text'], ['4', 4, 'text']), copy),
          update(select(['5', 0, 'text'], ['5', 4, 'text']), copy),
        ],
      },
      {
        // Keys: the code block 1, "const" 2, " x = 1" 3, the line break 4,
        // "x" 5.
        code: true,
        name: 'copying code',
        richText,
        steps: [
          doc(
            codeBlock(
              'javascript',
              ch('const', {highlightType: 'keyword'}),
              ch(' x = 1'),
              br(),
              ch('x'),
            ),
          ),
          update(select(['2', 2, 'text'], ['3', 3, 'text']), copy),
          // From the end of a token: the empty slice is left out.
          update(select(['2', 5, 'text'], ['5', 1, 'text']), copy),
          update(select(['root', 0, 'element'], ['root', 1, 'element']), copy),
          update(select(['1', 0, 'element'], ['1', 4, 'element']), copy),
        ],
      },
      {
        // Keys: the paragraph 1, "ab" 2, the inline decorator 3, "cd" 4, the
        // rule 5, the paragraph 6, "ef" 7, the paragraph 8, "gh" 9, the
        // inline decorator 10.
        extension: true,
        name: 'copying decorators',
        richText,
        steps: [
          doc(p(t('ab'), dt(), t('cd')), hr(), p(t('ef')), p(t('gh'), dt())),
          update(selectNodes('5'), copy, generateJSON),
          update(selectNodes('3'), copy),
          update(selectNodes('3', '5'), copy),
          update(select(['2', 1, 'text'], ['4', 1, 'text']), copy),
          update(select(['1', 1, 'element'], ['1', 2, 'element']), copy),
          update(select(['2', 0, 'text'], ['7', 2, 'text']), copy),
          // From the end of a paragraph that ends with an inline decorator:
          // the decorator isn't selected (`isSelected`).
          update(select(['8', 2, 'element'], ['root', 4, 'element']), copy),
          update(select(['9', 0, 'text'], ['8', 2, 'element']), copy),
        ],
      },
      {
        // Keys: the paragraph 1, "within " 2, the overflow node 3, "past"
        // (italic) 4, " the limit" 5.
        name: 'copying an overflow node',
        overflow: true,
        richText,
        steps: [
          doc(
            p(t('within '), overflow(t('past', {format: 2}), t(' the limit'))),
          ),
          // `excludeFromCopy`: its children come without it.
          update(select(['2', 0, 'text'], ['5', 10, 'text']), copy),
          update(select(['4', 1, 'text'], ['5', 4, 'text']), copy),
          update(selectNodes('3'), copy),
          update(whole),
        ],
      },
      {
        // Keys: the paragraph 1, "hello world" 2, the paragraph 3, "second"
        // 4.
        name: 'pasting Lexical JSON',
        richText,
        steps: [
          doc(p(t('hello world')), p(t('second'))),
          // Collapsed: the selection takes the pasted text's format.
          update(select(['2', 5, 'text']), pasteLexical(t('X', {format: 1}))),
          // Blocks over a range.
          update(
            select(['2', 0, 'text'], ['2', 2, 'text']),
            pasteLexical(p(t('para one')), p(t('para two'))),
          ),
          // A heading at the end of the document.
          update(select(['4', 6, 'text']), pasteLexical(h('h1', t('head')))),
        ],
      },
      {
        // Keys: the paragraph 1, "hello" 2.
        name: 'pasting a payload the editor does not take',
        richText,
        steps: [
          doc(p(t('hello'))),
          // Another namespace: the text instead.
          update(
            select(['2', 5, 'text']),
            pasteData({
              'application/x-lexical-editor': JSON.stringify({
                namespace: 'other',
                nodes: [t('no')],
              }),
              'text/plain': ' other',
            }),
          ),
          // Not JSON.
          update(
            pasteData({
              'application/x-lexical-editor': '{oops',
              'text/plain': ' bad',
            }),
          ),
          // No nodes.
          update(
            pasteData({
              'application/x-lexical-editor': JSON.stringify({
                namespace: NAMESPACE,
              }),
              'text/plain': ' none',
            }),
          ),
          // A node type the editor doesn't have, after one it has.
          update(
            pasteData({
              'application/x-lexical-editor': JSON.stringify({
                namespace: NAMESPACE,
                nodes: [t('kept'), {type: 'nope', version: 1}],
              }),
              'text/plain': ' unknown',
            }),
          ),
          // Only a URI list.
          update(pasteData({'text/uri-list': ' https://lexical.dev'})),
          // HTML that is the plain text is skipped for it.
          update(pasteData({'text/html': ' same', 'text/plain': ' same'})),
          // Nothing to paste.
          update(pasteData({})),
        ],
      },
      {
        // Keys: the list 1, its items 2 and 4 with "one" 3 and "two" 5.
        list: true,
        name: 'pasting in a list item',
        richText,
        steps: [
          doc(list('bullet', li(t('one')), li(t('two')))),
          update(select(['3', 3, 'text']), pasteLexical(t(' more'))),
          update(pasteLexical(p(t('para')))),
          update(
            select(['5', 1, 'text']),
            pasteLexical(list('number', li(t('a')), li(t('b')))),
          ),
          update(pasteData({'text/plain': 'x\ny'})),
        ],
      },
      {
        // Keys: the code block 1, "let a" 2.
        code: true,
        name: 'pasting in a code block',
        richText,
        steps: [
          doc(codeBlock('javascript', ch('let a'))),
          update(select(['2', 3, 'text']), pasteData({'text/plain': 'b\n\tc'})),
          update(pasteLexical(t('plain'))),
          update(pasteLexical(p(t('para')))),
        ],
      },
      {
        // Keys: the paragraph 1, "a " 2, the link 3, "link" 4, " b" 5.
        link: true,
        name: 'pasting in a link',
        richText,
        steps: [
          doc(p(t('a '), a('https://x.test', t('link')), t(' b'))),
          update(select(['4', 2, 'text']), pasteLexical(t('IN'))),
          update(pasteData({'text/plain': 'one\ntwo'})),
          update(pasteLexical(a('https://y.test', t('other')))),
        ],
      },
      {
        // Keys: the paragraph 1, "start end" 2, the paragraph 3, "bold"
        // (bold) 4.
        name: 'plain text with line breaks in a rich text editor',
        richText,
        steps: [
          doc(p(t('start end')), p(t('bold', {format: 1}))),
          update(
            select(['2', 6, 'text']),
            pasteData({'text/plain': 'line1\nline2\r\nline3\tx'}),
          ),
          // Over bold text: it keeps the format.
          update(
            select(['4', 1, 'text'], ['4', 3, 'text']),
            pasteData({'text/plain': 'a\nb'}),
          ),
          update({do: 'pastePlainText', text: 'raw\nline'}),
        ],
      },
      {
        // Keys: the paragraph 1, "start end" 2.
        name: 'plain text with line breaks in a plain text editor',
        plainText: true,
        steps: [
          doc(p(t('start end'))),
          update(select(['2', 6, 'text']), {
            do: 'pastePlainText',
            text: 'line1\nline2\r\nline3\tx',
          }),
          // The plain text path takes only text/plain and text/uri-list.
          update(
            pasteData(
              {
                'application/x-lexical-editor': JSON.stringify({
                  namespace: NAMESPACE,
                  nodes: [t('json')],
                }),
                'text/plain': 'text',
              },
              true,
            ),
          ),
          update(pasteData({'text/uri-list': 'https://lexical.dev'}, true)),
          // An empty paste over a range changes nothing.
          update(
            select(['2', 0, 'text'], ['2', 2, 'text']),
            pasteData({}, true),
          ),
        ],
      },
      {
        // Keys: the paragraph 1, "ab" (bold) 2, "cd" 3.
        name: 'insertGeneratedNodes',
        richText,
        steps: [
          doc(p(t('ab', {format: 1}), t('cd'))),
          update(select(['3', 0, 'text']), {
            do: 'insertGeneratedNodes',
            nodes: [t('X', {format: 2})],
          }),
          update(select(['1', 0, 'element']), {
            do: 'insertGeneratedNodes',
            nodes: [p(t('blk'))],
          }),
          update(select(['2', 1, 'text'], ['3', 1, 'text']), {
            do: 'insertGeneratedNodes',
            nodes: [t('Z')],
          }),
        ],
      },
      {
        // Keys: the paragraph 1, "ab" 2, the rule 3, the paragraph 4, "cd"
        // 5.
        extension: true,
        name: 'pasting over a node selection',
        richText,
        steps: [
          doc(p(t('ab')), hr(), p(t('cd'))),
          update(selectNodes('3'), pasteLexical(p(t('in place')))),
          // Plain text into a node selection inserts nothing.
          update(selectNodes('5'), pasteData({'text/plain': 'x'})),
        ],
      },
      {
        // Keys: the paragraph 1, "Hello " 2, the link 3, "Lexical" 4, "!"
        // (bold) 5, the list 6, its items 7 and 9 with "first" 8 and
        // "second" 10, the paragraph 11, "target" 12.
        link: true,
        list: true,
        name: 'copying, then pasting elsewhere',
        richText,
        steps: [
          doc(
            p(
              t('Hello '),
              a('https://lexical.dev', t('Lexical')),
              t('!', {format: 1}),
            ),
            list('number', li(t('first')), li(t('second'))),
            p(t('target')),
          ),
          update(select(['4', 2, 'text'], ['8', 3, 'text']), copy),
          update(select(['12', 3, 'text']), pasteClipboard()),
          update(select(['root', 0, 'element'], ['root', 1, 'element']), copy),
          update(node('root', 'selectEnd'), pasteClipboard()),
          // The same clipboard as plain text.
          update(pasteClipboard(true)),
        ],
      },
    ],
  );
});

test('scripts/package-extensions', () => {
  // The packages' own extensions (RichTextExtension, ListExtension,
  // HistoryExtension...) built with buildEditorFromExtensions: what they
  // bring (`hasExtensions`), the nodes the builder collects in dependency
  // order (logged by `test/probe`'s init as `knownTypes`), their config
  // overrides and merges, their `disabled` and other signals, and commands
  // dispatched through the built editor. Every built editor starts with
  // InitialStateExtension's paragraph, key 1; `start` replaces it.
  const probe: ExtensionSpec = {
    inspect: true,
    name: 'test/probe',
    phases: ['init'],
  };
  const script = (
    name: string,
    extensions: ExtensionArg[],
    steps: Step[] = [],
  ): Script => ({
    extensionDefs: {probe},
    extensions: ['probe', ...extensions],
    name,
    steps,
  });
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  const enter = run('INSERT_PARAGRAPH_COMMAND');
  const indent = run('INDENT_CONTENT_COMMAND');
  const start = (...blocks: Node[]): Step =>
    update(node('root', 'clear'), {do: 'append', key: 'root', nodes: blocks});
  const has = (...names: string[]): Step => ({hasExtensions: names});
  const signals = (key: string, ...entries: [string, Json][]): Step => ({
    setSignals: entries.map(([name, value]) => [key, name, value]),
  });
  const history: Step = {logHistory: true};
  const at = (now: number, step: Step): Step => ({...step, now});
  emit(
    'package-extensions',
    "The packages' extensions (RichTextExtension, PlainTextExtension, HistoryExtension, ListExtension, CheckListExtension, LinkExtension, AutoLinkExtension, HashtagExtension, OverflowExtension, CodeExtension, CodeIndentExtension, NormalizeInlineElementsExtension, TabIndentationExtension, SelectBlockExtension) through buildEditorFromExtensions: dependencies and the nodes they register, config overrides and merges, disabled and other signals, and commands dispatched through the built editor.",
    [
      script(
        'RichTextExtension and its dependencies',
        ['RichTextExtension'],
        [
          has(
            '@lexical/rich-text',
            '@lexical/NormalizeInlineElements',
            '@lexical/plain-text',
          ),
          start(p(t('ab'))),
          update(select(['3', 1, 'text'])),
          type('x'),
          run('FORMAT_TEXT_COMMAND', 'bold'),
          type('y'),
          enter,
          run('SELECT_ALL_COMMAND'),
          run('FORMAT_ELEMENT_COMMAND', 'center'),
        ],
      ),
      script('RichTextExtension conflicts with PlainTextExtension', [
        'RichTextExtension',
        'PlainTextExtension',
      ]),
      script(
        'PlainTextExtension',
        ['PlainTextExtension'],
        [
          has('@lexical/plain-text', '@lexical/NormalizeInlineElements'),
          start(p(t('ab'))),
          update(select(['3', 2, 'text'])),
          enter,
          type('c'),
          // Plain text has no formatting handler.
          run('FORMAT_TEXT_COMMAND', 'bold'),
        ],
      ),
      script('the nodes of every package, in dependency order', [
        'CheckListExtension',
        'AutoLinkExtension',
        'CodeIndentExtension',
        'HashtagExtension',
        'OverflowExtension',
        'RichTextExtension',
        ['HistoryExtension', {disabled: false}],
        'TabIndentationExtension',
        'SelectBlockExtension',
      ]),
      script(
        'CheckListExtension brings ListExtension',
        ['CheckListExtension', 'RichTextExtension'],
        [
          has('@lexical/list/List', '@lexical/list/CheckList'),
          start(p(t('a'))),
          update(select(['3', 1, 'text'])),
          run('INSERT_CHECK_LIST_COMMAND'),
          run('INSERT_ORDERED_LIST_COMMAND'),
          run('REMOVE_LIST_COMMAND'),
        ],
      ),
      script(
        'ListExtension with shouldPreserveNumbering',
        [
          ['ListExtension', {shouldPreserveNumbering: true}],
          'RichTextExtension',
        ],
        [
          start(list('number', li(t('a')), li(), li(t('b')))),
          update(select(['5', 0, 'element'])),
          enter,
        ],
      ),
      script(
        'ListExtension with shouldPreserveNumbering turned off by its signal',
        [
          ['ListExtension', {shouldPreserveNumbering: true}],
          'RichTextExtension',
        ],
        [
          signals('ListExtension', ['shouldPreserveNumbering', false]),
          start(list('number', li(t('a')), li(), li(t('b')))),
          update(select(['5', 0, 'element'])),
          enter,
        ],
      ),
      script(
        'ListExtension with hasStrictIndent, then without',
        [['ListExtension', {hasStrictIndent: true}], 'RichTextExtension'],
        [
          start(list('bullet', li(t('a')), li(t('b')))),
          // The first item can't be indented.
          update(select(['4', 0, 'text'])),
          indent,
          // The second only one level past the first.
          update(select(['6', 0, 'text'])),
          indent,
          indent,
          signals('ListExtension', ['hasStrictIndent', false]),
          indent,
        ],
      ),
      script(
        'HistoryExtension: delay, and disabled by its signal',
        [
          ['HistoryExtension', {delay: 300, disabled: false}],
          'RichTextExtension',
        ],
        [
          history,
          update(select(['1', 0, 'element'])),
          at(0, type('a')),
          at(100, type('b')),
          at(1000, type('c')),
          history,
          run('UNDO_COMMAND'),
          history,
          signals('HistoryExtension', ['disabled', true]),
          history,
          at(2000, type('d')),
          run('UNDO_COMMAND'),
          signals('HistoryExtension', ['disabled', false]),
          history,
          at(3000, type('e')),
          history,
          run('UNDO_COMMAND'),
          run('REDO_COMMAND'),
          history,
        ],
      ),
      script(
        'HistoryExtension: maxDepth, and its signal',
        [
          ['HistoryExtension', {delay: 0, disabled: false, maxDepth: 2}],
          'RichTextExtension',
        ],
        [
          update(select(['1', 0, 'element'])),
          type('a'),
          type('b'),
          type('c'),
          type('d'),
          history,
          signals('HistoryExtension', ['maxDepth', 1]),
          type('e'),
          history,
          signals('HistoryExtension', ['maxDepth', null]),
          type('f'),
          type('g'),
          history,
        ],
      ),
      script(
        'HistoryExtension: createInitialHistoryState',
        [
          'RichTextExtension',
          [
            'HistoryExtension',
            {createInitialHistoryState: 'prefilled', delay: 0},
            {disabled: false},
          ],
        ],
        [
          history,
          update(select(['3', 0, 'element'])),
          type('a'),
          history,
          run('UNDO_COMMAND'),
          run('UNDO_COMMAND'),
          history,
          run('UNDO_COMMAND'),
          run('REDO_COMMAND'),
          history,
        ],
      ),
      script(
        'LinkExtension: attributes merge, and validateUrl',
        [
          [
            'LinkExtension',
            {attributes: {rel: 'noopener'}},
            {attributes: {target: '_blank'}, validateUrl: 'https'},
          ],
          'RichTextExtension',
        ],
        [
          start(p(t('click here'))),
          update(select(['3', 0, 'text'], ['3', 5, 'text'])),
          run('TOGGLE_LINK_COMMAND', 'http://lexical.dev'),
          run('TOGGLE_LINK_COMMAND', 'https://lexical.dev'),
          run('TOGGLE_LINK_COMMAND', {
            title: 'docs',
            url: 'https://lexical.dev/docs',
          }),
          run('TOGGLE_LINK_COMMAND', {url: 'http://lexical.dev'}),
          run('TOGGLE_LINK_COMMAND', null),
        ],
      ),
      script(
        'AutoLinkExtension: matchers and change handlers concatenate',
        [
          ['AutoLinkExtension', {matchers: ['url']}],
          ['AutoLinkExtension', {changeHandlers: ['log'], matchers: ['email']}],
          'RichTextExtension',
        ],
        [
          has('@lexical/link/AutoLink', '@lexical/link/Link'),
          update(select(['1', 0, 'element'])),
          type('see https://lexical.dev or mail a@b.dev '),
          type('x'),
        ],
      ),
      script(
        'NormalizeInlineElementsExtension removes an empty link',
        ['LinkExtension', 'RichTextExtension'],
        [
          start(p(t('a'), a('https://lexical.dev', t('b')), t('c'))),
          update(node('5', 'remove', true)),
        ],
      ),
      script(
        'NormalizeInlineElementsExtension disabled, then enabled',
        [
          ['NormalizeInlineElementsExtension', {disabled: true}],
          'LinkExtension',
          'RichTextExtension',
        ],
        [
          start(p(t('a'), a('https://lexical.dev', t('b')), t('c'))),
          update(node('5', 'remove', true)),
          signals('NormalizeInlineElementsExtension', ['disabled', false]),
          // The transform is added without marking links dirty.
          update(node('3', 'setTextContent', 'A')),
          update(node('4', 'markDirty')),
        ],
      ),
      script(
        'CodeIndentExtension: tabSize, and disabled',
        [['CodeIndentExtension', {tabSize: 2}], 'RichTextExtension'],
        [
          has('@lexical/code', '@lexical/code-indent'),
          start(codeBlock(null, ch('    a'), br(), ch('b'))),
          update(select(['3', 5, 'text'])),
          run('OUTDENT_CONTENT_COMMAND'),
          indent,
          signals('CodeIndentExtension', ['disabled', true]),
          indent,
          run('INSERT_TAB_COMMAND'),
          signals('CodeIndentExtension', ['disabled', false], ['tabSize', 4]),
          run('OUTDENT_CONTENT_COMMAND'),
        ],
      ),
      script(
        'CodeExtension: Enter in a code block adds a line',
        ['CodeExtension', 'RichTextExtension'],
        [
          start(codeBlock(null, ch('a')), p(t('after'))),
          update(select(['3', 1, 'text'])),
          enter,
          enter,
          enter,
          type('b'),
        ],
      ),
      script(
        'TabIndentationExtension: maxIndent, and disabled',
        [['TabIndentationExtension', {maxIndent: 2}], 'RichTextExtension'],
        [
          update(select(['1', 0, 'element'])),
          indent,
          indent,
          signals('TabIndentationExtension', ['disabled', true]),
          indent,
          signals(
            'TabIndentationExtension',
            ['disabled', false],
            ['maxIndent', null],
          ),
          indent,
          run('OUTDENT_CONTENT_COMMAND'),
        ],
      ),
      script(
        'TabIndentationExtension: $canIndent',
        [
          ['TabIndentationExtension', {$canIndent: 'never'}],
          'RichTextExtension',
        ],
        [update(select(['1', 0, 'element'])), indent],
      ),
      script(
        'SelectBlockExtension: the block, then the document',
        ['SelectBlockExtension', 'RichTextExtension'],
        [
          has('@lexical/extension/PreventSelectAll'),
          start(p(t('one')), p(t('two'))),
          update(select(['3', 1, 'text'])),
          run('SELECT_ALL_COMMAND'),
          run('SELECT_ALL_COMMAND'),
          run('SELECT_ALL_COMMAND'),
          update(select(['3', 1, 'text'], ['5', 1, 'text'])),
          run('SELECT_ALL_COMMAND'),
          signals('SelectBlockExtension', ['disabled', true]),
          update(select(['3', 1, 'text'])),
          run('SELECT_ALL_COMMAND'),
        ],
      ),
      script(
        'HashtagExtension and OverflowExtension',
        ['HashtagExtension', 'OverflowExtension', 'RichTextExtension'],
        [
          update(select(['1', 0, 'element'])),
          type('#tag and #more'),
          start(p(t('within '), overflow(t('past')))),
        ],
      ),
    ],
  );
});

test('scripts/selection-utils', () => {
  // The model part of @lexical/selection (lexical-node.ts,
  // range-selection.ts, utils.ts) and @lexical/utils, called directly: style
  // patches and reads, text slices, trimming, wrapping, caret movement
  // checks, traversals, splits, insertions and structure helpers. Each
  // logs what it returns; the state after each step shows what it changed.
  const hr = (): Node => ({type: 'horizontalrule', version: 1}) as Node;
  const dt = (): Node => ({type: 'decorator-text', version: 1}) as Node;
  const act = (action: Record<string, unknown>): Action => action as Action;
  const patch = (value: Record<string, unknown>): Action =>
    act({do: 'patchStyleText', patch: value});
  const describe = {fn: 'describe'};
  const styleValue = (property: string, defaultValue?: string): Action =>
    act(
      defaultValue === undefined
        ? {do: 'logSelectionStyleValue', property}
        : {defaultValue, do: 'logSelectionStyleValue', property},
    );
  const slice = (key: string): Action => act({do: 'logSlice', key});
  const dfs = (
    options: {start?: string; end?: string; reverse?: boolean} = {},
  ): Action => act({do: 'logDfs', ...options});
  const keyed = (kind: string, key: string): Action => act({do: kind, key});
  const extension = true;
  const link = true;
  const richText = true;
  const url = 'https://lexical.dev';
  // Keys: the paragraph 1, "abcd" 2, the list 3, its items 4 ("x" 5) and 6
  // ("yz" 7), the paragraph 8, "end" 9.
  const nestedBlocks = doc(
    p(t('abcd')),
    list('bullet', li(t('x')), li(t('yz'))),
    p(t('end')),
  );
  emit(
    'selection-utils',
    "@lexical/selection's and @lexical/utils's model functions, called directly: what each returns (logged) and the state after each step.",
    [
      {
        name: 'patchStyleText across nodes, backward, then removing a property',
        steps: [
          // Keys: the paragraph 1, "abc" 2, "def" 3, the paragraph 4, "ghi" 5.
          doc(p(t('abc'), t('def', {style: 'color: red;'})), p(t('ghi'))),
          update(
            select(['5', 1, 'text'], ['2', 1, 'text']),
            patch({color: 'blue', 'font-size': '12px'}),
            styleValue('color'),
            styleValue('font-size'),
            styleValue('font-weight', 'normal'),
          ),
          update(
            select(['root', 0, 'element'], ['root', 2, 'element']),
            patch({color: null, 'font-size': describe}),
            styleValue('color', 'black'),
            styleValue('font-size'),
          ),
        ],
      },
      {
        name: 'patchStyleText on a collapsed selection and an empty paragraph',
        steps: [
          // Keys: the paragraph 1, "ab" 2, the paragraph 3.
          doc(p(t('ab')), p()),
          update(
            select(['2', 1, 'text']),
            patch({color: describe, 'font-weight': 'bold'}),
            styleValue('font-weight'),
            styleValue('color'),
          ),
          update(select(['3', 0, 'element']), patch({color: 'green'})),
          update(
            select(['1', 0, 'element'], ['3', 0, 'element']),
            patch({color: describe}),
            styleValue('color'),
          ),
        ],
      },
      {
        link,
        name: 'patchStyleText into a link and a token',
        steps: [
          // Keys: the paragraph 1, "ab" 2, the link 3, "cd" 4, "tok" 5, "ef" 6.
          doc(p(t('ab'), a(url, t('cd')), t('tok', {mode: 'token'}), t('ef'))),
          update(
            select(['2', 1, 'text'], ['5', 1, 'text']),
            patch({'background-color': 'yellow', color: 'red'}),
            styleValue('background-color'),
          ),
          update(
            select(['4', 1, 'text'], ['6', 1, 'text']),
            styleValue('color'),
          ),
        ],
      },
      {
        name: 'getSelectionStyleValueForProperty at the edges',
        steps: [
          // Keys: the paragraph 1, "ab" 2, "cd" 3, "ef" 4.
          doc(
            p(
              t('ab', {style: 'color: red;'}),
              t('cd', {style: 'color: blue; font-size: 10px;'}),
              t('ef', {style: 'font-size: 10px; color: blue;'}),
            ),
          ),
          update(
            select(['2', 2, 'text'], ['4', 0, 'text']),
            styleValue('color'),
            styleValue('font-size'),
          ),
          update(
            select(['4', 0, 'text'], ['2', 2, 'text']),
            styleValue('color'),
          ),
          update(
            select(['2', 1, 'text'], ['3', 1, 'text']),
            styleValue('color'),
          ),
          update(
            select(['2', 1, 'text'], ['4', 2, 'text']),
            styleValue('font-size'),
            styleValue('line-height', '1.5'),
          ),
          // Ending at offset 0 of "cd" skips it, forward and backward: red,
          // not the mixed ''.
          update(
            select(['2', 0, 'text'], ['3', 0, 'text']),
            styleValue('color'),
          ),
          update(
            select(['3', 0, 'text'], ['2', 0, 'text']),
            styleValue('color'),
          ),
        ],
      },
      {
        name: 'getStyleObjectFromCSS and getCSSFromStyleObject',
        steps: [
          update(
            act({css: 'color: red; font-size : 12px ;', do: 'logStyleObject'}),
            act({
              css: 'background: url("a;b:c"); /* x: y; */ font-family: \'A;B\'; :x; y:; color: blue',
              do: 'logStyleObject',
            }),
            act({css: 'b: 1; 2: two; a: x; b: 3', do: 'logStyleObject'}),
            act({css: '', do: 'logStyleObject'}),
          ),
        ],
      },
      {
        name: 'isAtNodeEnd, sliceSelectedTextNodeContent and forEachSelectedTextNode',
        steps: [
          // Keys: the paragraph 1, "abc" 2, "def" 3, the paragraph 4, "gh" 5.
          doc(p(t('abc'), t('def', {format: 1})), p(t('gh'))),
          update(
            select(['3', 3, 'text'], ['2', 1, 'text']),
            act({do: 'logAtNodeEnd'}),
            slice('2'),
            slice('3'),
            slice('5'),
          ),
          update(
            select(['1', 2, 'element'], ['4', 0, 'element']),
            act({do: 'logAtNodeEnd'}),
            slice('2'),
            slice('3'),
          ),
          update(
            select(['3', 2, 'text'], ['2', 1, 'text']),
            act({do: 'forEachSelectedTextNode'}),
          ),
          update(
            select(['5', 2, 'text'], ['5', 0, 'text']),
            act({do: 'forEachSelectedTextNode'}),
          ),
          update(
            select(['5', 1, 'text']),
            act({do: 'forEachSelectedTextNode'}),
          ),
          update(
            select(['5', 1, 'text'], ['2', 0, 'text']),
            act({do: 'ensureForwardRangeSelection'}),
          ),
        ],
      },
      {
        name: 'trimTextContentFromAnchor',
        steps: [
          // Keys: the paragraph 1, "abc" 2, the paragraph 3, "de" 4, "fg" 5,
          // the paragraph 6, "hij" 7.
          doc(p(t('abc')), p(t('de'), t('fg', {format: 1})), p(t('hij'))),
          update(
            select(['5', 1, 'text']),
            act({count: 2, do: 'trimTextContentFromAnchor'}),
          ),
          update(
            select(['7', 3, 'text']),
            act({count: 6, do: 'trimTextContentFromAnchor'}),
          ),
          update(
            node('2', 'setTextContent', 'abcdef'),
            select(['2', 6, 'text']),
            act({count: 2, do: 'trimTextContentFromAnchor'}),
          ),
          update(
            select(['root', 1, 'element']),
            act({count: 1, do: 'trimTextContentFromAnchor'}),
          ),
        ],
      },
      {
        name: 'wrapNodes across blocks, into a wrapping element, and an empty block',
        richText,
        steps: [
          // Keys: the paragraph 1, "ab" 2, the paragraph 3, "cd" 4, the
          // paragraph 5.
          doc(p(t('ab')), p(t('cd')), p()),
          update(
            node('1', 'setFormat', 'center'),
            node('1', 'setIndent', 2),
            select(['2', 1, 'text'], ['4', 1, 'text']),
            act({block: 'quote', do: 'wrapNodes'}),
          ),
          update(
            select(['5', 0, 'element']),
            act({block: 'h1', do: 'wrapNodes'}),
          ),
        ],
      },
      {
        name: 'wrapNodes backward, ending at offset 0, with a wrapping element',
        richText,
        steps: [
          doc(p(t('ab')), p(t('cd')), p(t('ef'))),
          update(
            select(['6', 0, 'text'], ['2', 1, 'text']),
            act({block: 'paragraph', do: 'wrapNodes', wrapping: 'quote'}),
          ),
        ],
      },
      {
        link,
        list: true,
        name: 'wrapNodes over list items and a link',
        richText,
        steps: [
          // Keys: the list 1, its items 2 ("a" 3) and 4 ("b" 5), the
          // paragraph 6, "c" 7, the link 8, "d" 9.
          doc(
            list('bullet', li(t('a')), li(t('b'))),
            p(t('c'), a(url, t('d'))),
          ),
          update(
            select(['3', 0, 'text'], ['9', 1, 'text']),
            act({block: 'quote', do: 'wrapNodes'}),
          ),
        ],
      },
      {
        extension,
        name: 'shouldOverrideDefaultCharacterSelection and moveCharacter',
        steps: [
          // Keys: the paragraph 1, "ab" 2, the decorator 3, "cd" 4, the rule
          // 5, the paragraph 6, "xy" 7, "z" 8.
          doc(p(t('ab'), dt(), t('cd')), hr(), p(t('xy', {detail: 2}), t('z'))),
          update(
            select(['2', 2, 'text']),
            act({do: 'logShouldOverride', isBackward: false}),
            act({do: 'logShouldOverride', isBackward: true}),
            select(['4', 2, 'text']),
            act({do: 'logShouldOverride', isBackward: false}),
            select(['4', 0, 'text']),
            act({do: 'logShouldOverride', isBackward: true}),
            select(['7', 2, 'text']),
            act({do: 'logShouldOverride', isBackward: false}),
            select(['8', 0, 'text']),
            act({do: 'logShouldOverride', isBackward: true}),
          ),
          update(
            select(['2', 2, 'text']),
            act({do: 'moveCharacter', isBackward: false, shift: false}),
          ),
          update(
            select(['4', 0, 'text']),
            act({do: 'moveCharacter', isBackward: true, shift: true}),
          ),
          // Within text, the move needs the DOM's Selection.modify, which a
          // headless editor lacks.
          update(
            select(['2', 1, 'text']),
            act({do: 'moveCharacter', isBackward: false, shift: false}),
          ),
        ],
      },
      {
        name: 'copyBlockFormatIndent, selectAll, cloneWithProperties and isBlockFullySelected',
        steps: [
          // Keys: the paragraph 1, "ab" 2, the paragraph 3, "cd" 4.
          doc(p(t('ab', {format: 2, style: 'color: red;'})), p(t('cd'))),
          update(
            node('1', 'setFormat', 'right'),
            node('1', 'setIndent', 3),
            act({do: 'copyBlockFormatIndent', from: '1', to: '3'}),
            keyed('logCloneWithProperties', '2'),
            keyed('logCloneWithProperties', '1'),
          ),
          update(
            select(['2', 1, 'text'], ['4', 1, 'text']),
            keyed('logBlockFullySelected', '1'),
            keyed('logBlockFullySelected', '3'),
            act({do: 'selectAll', scoped: true}),
            keyed('logBlockFullySelected', '1'),
            keyed('logBlockFullySelected', 'root'),
          ),
          update(
            select(['root', 0, 'element']),
            act({do: 'selectAll', scoped: true}),
          ),
        ],
      },
      {
        link,
        list: true,
        name: 'dfs, depth and siblings over nested lists and inline elements',
        steps: [
          // Keys: the paragraph 1, "a" 2, the link 3, "b" 4, "c" 5; the list
          // 6, its item 7 ("d" 8) and its item 9 holding the list 10 with
          // the items 11 ("e" 12) and 13 ("f" 14); the paragraph 15.
          doc(
            p(t('a'), a(url, t('b')), t('c')),
            list(
              'bullet',
              li(t('d')),
              li(list('number', li(t('e')), li(t('f')))),
            ),
            p(),
          ),
          update(
            dfs(),
            dfs({start: '6'}),
            dfs({end: '9', start: '6'}),
            dfs({end: '12'}),
            dfs({start: '4'}),
            dfs({reverse: true}),
            dfs({reverse: true, start: '6'}),
            dfs({end: '11', reverse: true, start: '6'}),
          ),
          update(
            keyed('logDepth', 'root'),
            keyed('logDepth', '1'),
            keyed('logDepth', '12'),
            keyed('logNextSiblingOrParentSibling', '3'),
            keyed('logNextSiblingOrParentSibling', '14'),
            keyed('logNextSiblingOrParentSibling', '15'),
            keyed('logNextRightPreorderNode', '2'),
            keyed('logNextRightPreorderNode', '6'),
            keyed('logNextRightPreorderNode', '9'),
            keyed('logNextRightPreorderNode', '8'),
            keyed('logNearestBlockElementAncestor', '4'),
            keyed('logNearestBlockElementAncestor', '12'),
            keyed('logChildren', '1'),
            act({do: 'logChildren', key: '6', reverse: true}),
            act({do: 'logDescendantsMatching', key: 'root', types: ['text']}),
            act({
              do: 'logDescendantsMatching',
              key: 'root',
              types: ['listitem'],
            }),
            act({do: 'logDescendantsMatching', key: '6', types: ['list']}),
          ),
          update(keyed('logNearestBlockElementAncestor', 'root')),
        ],
      },
      {
        list: true,
        name: 'splitNode in the middle, at both edges and in nested blocks',
        richText,
        steps: [
          // Keys: the quote 1, "ab" 2, "cd" 3, "ef" 4, the list 5, its items
          // 6 ("x" 7) and 8 ("y" 9).
          doc(
            q(t('ab'), t('cd', {format: 1}), t('ef')),
            list('bullet', li(t('x')), li(t('y'))),
          ),
          update(act({do: 'splitNode', key: '1', offset: 1})),
          update(act({do: 'splitNode', key: '1', offset: 0})),
          update(act({do: 'splitNode', key: '1', offset: 5})),
          update(act({do: 'splitNode', key: '8', offset: 0})),
          update(act({do: 'splitNode', key: '6', offset: 1})),
          update(act({do: 'splitNode', key: 'root', offset: 0})),
        ],
      },
      {
        extension,
        list: true,
        name: 'insertNodeToNearestRoot in a text middle',
        richText,
        steps: [
          nestedBlocks,
          update(
            select(['2', 2, 'text']),
            act({do: 'insertNodeToNearestRoot', node: hr()}),
          ),
        ],
      },
      {
        extension,
        list: true,
        name: 'insertNodeToNearestRoot in a nested block',
        richText,
        steps: [
          nestedBlocks,
          update(
            select(['7', 1, 'text']),
            act({do: 'insertNodeToNearestRoot', node: hr()}),
          ),
          update(
            select(['5', 1, 'text']),
            act({do: 'insertNodeToNearestRoot', node: p(t('new'))}),
          ),
        ],
      },
      {
        extension,
        list: true,
        name: 'insertNodeToNearestRoot at the end, from a node selection and without a selection',
        richText,
        steps: [
          nestedBlocks,
          update(
            select(['9', 3, 'text']),
            act({do: 'insertNodeToNearestRoot', node: hr()}),
          ),
          update(
            act({do: 'selectNodes', keys: ['1']}),
            act({do: 'insertNodeToNearestRoot', node: hr()}),
          ),
          update(act({do: 'clearSelection'})),
          update(act({do: 'insertNodeToNearestRoot', node: t('inline')})),
        ],
      },
      {
        extension,
        name: 'insertNodeIntoLeaf',
        steps: [
          // Keys: the paragraph 1, "abcd" 2, the paragraph 3, "ef" 4.
          doc(p(t('abcd')), p(t('ef'))),
          update(
            select(['2', 1, 'text'], ['2', 3, 'text']),
            act({do: 'insertNodeIntoLeaf', node: dt()}),
          ),
          update(
            select(['4', 0, 'text']),
            act({do: 'insertNodeIntoLeaf', node: br()}),
          ),
          update(
            act({do: 'selectNodes', keys: ['3']}),
            act({do: 'insertNodeIntoLeaf', node: p(t('gh'))}),
          ),
        ],
      },
      {
        link,
        name: 'insertFirst, wrapNodeInElement, unwrapNode and unwrapAndFilterDescendants',
        richText,
        steps: [
          // Keys: the paragraph 1, "ab" 2, the quote 3, "cd" 4, the paragraph
          // 5, the link 6, "ef" 7, "gh" 8; then "X" 9, the quote 10, the link
          // 11 and "ij" 12.
          doc(p(t('ab')), q(t('cd')), p(a(url, t('ef')), t('gh'))),
          update(
            select(['1', 0, 'element']),
            act({do: 'insertFirst', key: '1', node: t('X', {format: 1})}),
          ),
          update(act({block: 'quote', do: 'wrapNodeInElement', key: '3'})),
          update(
            act({
              do: 'unwrapAndFilterDescendants',
              keep: ['paragraph', 'quote'],
              key: 'root',
            }),
            act({do: 'unwrapAndFilterDescendants', keep: ['text'], key: '5'}),
          ),
          update(
            act({do: 'insertFirst', key: '5', node: a(url, t('ij'))}),
            select(['7', 1, 'text']),
          ),
          update(keyed('unwrapNode', '11')),
        ],
      },
      {
        name: 'restoreEditorState',
        steps: [
          // Keys: the paragraph 1, "ab" 2.
          doc(p(t('ab'))),
          update(select(['2', 1, 'text'])),
          update(act({do: 'saveEditorState', name: 'one'})),
          update(
            node('2', 'setTextContent', 'xyz'),
            select(['2', 3, 'text']),
            node('root', 'append', [p(t('new'))]),
          ),
          update(
            node('2', 'setTextContent', 'pending'),
            act({do: 'restoreEditorState', name: 'one'}),
          ),
          update(select(['2', 2, 'text']), call('insertText', '!')),
        ],
      },
      {
        mark: true,
        name: 'registerNestedElementResolver',
        steps: [
          // Keys: the paragraph 1, the mark 2, "x" 3, the mark 4, "y" 5, "z"
          // 6.
          doc(p(mark(['a'], t('x'), mark(['b'], t('y')), t('z')))),
          update(node('5', 'setTextContent', 'Y')),
        ],
        transforms: ['mark:nested'],
      },
      {
        name: 'onEscapeUp and onEscapeDown',
        richText,
        steps: [
          // Keys: the quote 1, "ab" 2, the paragraph 3, "cd" 4, the quote 5,
          // "ef" 6.
          doc(q(t('ab')), p(t('cd')), q(t('ef'))),
          update(
            select(['2', 1, 'text']),
            act({direction: 'up', do: 'escape', types: ['quote']}),
            select(['4', 0, 'text']),
            act({direction: 'up', do: 'escape', types: ['quote']}),
            select(['2', 2, 'text']),
            act({direction: 'down', do: 'escape', types: ['quote']}),
          ),
          update(
            select(['2', 0, 'text']),
            act({direction: 'up', do: 'escape', types: ['quote']}),
          ),
          update(
            select(['6', 2, 'text']),
            act({direction: 'down', do: 'escape', types: ['quote']}),
          ),
        ],
      },
    ],
  );
});

test('scripts/slots', () => {
  // Named slots (LexicalSlot.ts): setting, moving, replacing and removing
  // slot values; their place in text content, JSON and traversals; the
  // refusals; GC of a removed host's slots; and typing, deleting and
  // selecting into and out of them, where the slot boundary behaves like a
  // shadow root. Keys: the root's descendants get 1, 2, 3… in document
  // order, then each parsed slot value and its text in turn.
  const act = (action: Record<string, unknown>): Action => action as Action;
  const setSlot = (key: string, name: string, value: Node | string): Action =>
    act(
      typeof value === 'string'
        ? {do: 'setSlot', key, name, valueKey: value}
        : {do: 'setSlot', key, name, node: value},
    );
  const removeSlot = (key: string, name: string): Action =>
    act({do: 'removeSlot', key, name});
  const slots = (...keys: string[]): Action[] =>
    keys.map(key => act({do: 'logSlots', key}));
  const exist = (...keys: string[]): Action => act({do: 'logNodesExist', keys});
  const frame: Action = act({do: 'logSelectionSlotFrame'});
  const selectionText: Action = act({do: 'logSelectionText'});
  const dfs = (
    options: {start?: string; end?: string; reverse?: boolean} = {},
  ): Action => act({do: 'logDfsWithSlots', ...options});
  const hr = (): Node => ({type: 'horizontalrule', version: 1}) as Node;
  const dispatch = (command: string, payload: Json = null): Step => ({
    dispatch: command,
    payload,
  });
  const richText = true;
  emit(
    'slots',
    'Named slots: their values, JSON, text, traversal, GC and the selection and editing rules at a slot boundary.',
    [
      {
        name: 'set, move, replace and remove',
        steps: [
          // Keys: the host paragraph 1, "Body" 2.
          doc(p(t('Body'))),
          // "Title" 3 and 4, "Aside" 5 and 6: canonical order puts aside
          // first, whatever the order they were set in.
          update(
            setSlot('1', 'title', p(t('Title'))),
            setSlot('1', 'aside', p(t('Aside'))),
            ...slots('1', '3', '4', '2'),
          ),
          // Setting the same value again changes nothing.
          update(setSlot('1', 'title', '3'), ...slots('1')),
          // Move: the title's paragraph goes to "sub" and leaves "title".
          update(setSlot('1', 'sub', '3'), ...slots('1', '3')),
          // Replace: the previous value is removed, then collected.
          update(setSlot('1', 'sub', p(t('New'))), ...slots('1')),
          update(exist('3', '4', '7', '8')),
          update(removeSlot('1', 'aside'), removeSlot('1', 'missing')),
          update(exist('5', '6'), ...slots('1')),
        ],
      },
      {
        name: 'refusals',
        steps: [
          // Keys: the host paragraph 1, "Body" 2, the slot value 3, "T" 4.
          doc(p(t('Body'))),
          update(setSlot('1', 'title', p(t('T')))),
          // A slot value can't be removed or replaced through the tree.
          update(node('3', 'remove')),
          update(node('3', 'replace', p(t('X')))),
          // Inline values and reserved names are refused.
          update(setSlot('1', 'inline', t('x'))),
          update(setSlot('1', '__proto__', p())),
          update(setSlot('1', 'constructor', p())),
          // A host can't be slotted into its own slot value.
          update(setSlot('3', 'loop', '1')),
          // A slot value takes children as any block does.
          update(node('3', 'append', [p(t('child'))])),
        ],
      },
      {
        name: 'JSON, text and depth',
        steps: [
          // Keys: the host 1, "Body" 2; from the parsed $slots, "b" 3 and 4,
          // "10" 5 and 6, "a" 7, its slot "inner" 8 and 9, and "A" 10.
          doc(p(t('Body'))),
          update(
            setSlot('1', 'b', p(t('B'))),
            setSlot('1', '10', p(t('Ten'))),
            setSlot('1', 'a', {
              ...p(t('A')),
              $slots: {inner: p(t('In'))},
            } as Node),
            ...slots('1', '7', '9', '10'),
            act({do: 'logDepth', key: '10'}),
            act({do: 'logDepth', key: '9'}),
          ),
          // A document that carries $slots parses back to the same JSON.
          update(
            act({
              do: 'append',
              key: 'root',
              nodes: [
                {
                  ...p(t('Second')),
                  // Out of order on purpose: canonical order sorts them.
                  // eslint-disable-next-line sort-keys-fix/sort-keys-fix
                  $slots: {z: p(t('Z')), y: p(t('Y'))},
                } as Node,
              ],
            }),
          ),
        ],
      },
      {
        name: 'dfs with slots',
        steps: [
          // Keys: the paragraph 1, "x" 2, the host 3, "Body" 4; the slots "s"
          // 5 and 6, "r" 7, its slot "q" 8 and 9, and "R" 10.
          doc(p(t('x')), p(t('Body'))),
          update(
            setSlot('3', 's', p(t('S'))),
            setSlot('3', 'r', {...p(t('R')), $slots: {q: p(t('Q'))}} as Node),
          ),
          update(
            dfs(),
            dfs({reverse: true}),
            dfs({start: '3'}),
            dfs({end: '3'}),
            dfs({reverse: true, start: '3'}),
            dfs({start: '7'}),
          ),
        ],
      },
      {
        extension: true,
        name: 'GC of a removed host',
        steps: [
          // Keys: the paragraph 1, "a" 2, the host 3, "b" 4, the rule 5; the
          // host's slot 6 and 7 with its own slot 8 and 9; the rule's slot
          // 10 and 11.
          doc(p(t('a')), p(t('b')), hr()),
          update(
            setSlot('3', 'one', {
              ...p(t('One')),
              $slots: {deep: p(t('Deep'))},
            } as Node),
            setSlot('5', 'caption', p(t('Caption'))),
            ...slots('5', '10'),
          ),
          // A decorator host's text is its slots'.
          update(act({do: 'logRootTextContent'})),
          update(node('3', 'remove'), node('5', 'remove')),
          update(exist('3', '4', '5', '6', '7', '8', '9', '10', '11')),
        ],
      },
      {
        name: 'typing and selecting in a slot',
        richText,
        steps: [
          // Keys: "A" paragraph 1 and 2, the host 3, "Body" 4, its slot 5
          // and "Title" 6.
          doc(p(t('A')), p(t('Body'))),
          update(setSlot('3', 'title', p(t('Title')))),
          update(select(['6', 5, 'text']), call('insertText', '!'), frame),
          // Enter in a block slot value is a no-op.
          update(select(['6', 2, 'text']), call('insertParagraph')),
          // Blocks pasted into it are flattened.
          update(
            select(['6', 1, 'text']),
            call('insertNodes', [p(t('X')), p(t('Y'))]),
          ),
          // Select all stays in the slot.
          update(select(['6', 1, 'text'])),
          dispatch('SELECT_ALL_COMMAND'),
          update(selectionText, frame),
          // A selection from the slot into the body is clamped to the slot.
          update(select(['6', 1, 'text'], ['4', 2, 'text']), selectionText),
          update(select(['4', 2, 'text'], ['6', 1, 'text']), selectionText),
          // A selection around the host carries the slot's text.
          update(select(['2', 0, 'text'], ['4', 4, 'text']), selectionText),
          // setBlocksType leaves a slot value alone.
          update(
            select(['6', 1, 'text']),
            act({block: 'h1', do: 'setBlocksType'}),
          ),
          // Select all in the body selects the document.
          update(select(['4', 1, 'text'])),
          dispatch('SELECT_ALL_COMMAND'),
          update(selectionText, frame),
        ],
      },
      {
        name: 'deleting at a slot boundary',
        richText,
        steps: [
          // Keys: "A" paragraph 1 and 2, the host 3, "Body" 4, its slot 5
          // and "Title" 6.
          doc(p(t('A')), p(t('Body'))),
          update(setSlot('3', 'title', p(t('Title')))),
          // Backspace at the start of the slot stops there.
          update(select(['6', 0, 'text'])),
          dispatch('DELETE_CHARACTER_COMMAND', true),
          // Backspace at the start of the host would merge it away with its
          // slots: it stays.
          update(select(['4', 0, 'text'])),
          dispatch('DELETE_CHARACTER_COMMAND', true),
          // Removing a range in the slot edits only the slot.
          update(
            select(['6', 1, 'text'], ['6', 4, 'text']),
            call('removeText'),
          ),
          // Removing a range around the host keeps it: merging it would lose
          // its slots.
          update(
            select(['2', 0, 'text'], ['4', 4, 'text']),
            call('removeText'),
          ),
          update(exist('3', '5', '6')),
        ],
      },
      {
        mark: true,
        name: 'copy and mark through slots',
        richText,
        steps: [
          // Keys: "A" paragraph 1 and 2, the host 3, "Body" 4, its slot 5
          // and "Title" 6.
          doc(p(t('A')), p(t('Body'))),
          update(setSlot('3', 'title', p(t('Title')))),
          // A copy around the host carries its slots whole.
          update(select(['2', 0, 'text'], ['4', 2, 'text']), act({do: 'copy'})),
          // A copy in the slot walks the slot.
          update(select(['6', 1, 'text'], ['6', 3, 'text']), act({do: 'copy'})),
          // Marking around the host marks the slot's text too.
          update(
            select(['2', 0, 'text'], ['4', 4, 'text']),
            act({do: 'addMarkID', id: 'm1'}),
          ),
        ],
      },
    ],
  );
});

test('scripts/block-id', () => {
  // BlockIdExtension (@lexical/extension): ids assigned to the blocks an
  // update creates, kept through splits, merges and setBlocksType, repaired
  // when a paste or a clone repeats one, positional ids for a document
  // loaded without ids (and derived ids for repeated ones), handles, and
  // the config options. `generateId: 'counter'` makes ids id1, id2...; the
  // state JSON of each step shows every id (`"$": {"blockId": ...}`).
  const act = (action: Record<string, unknown>): Action => action as Action;
  const ids = (...keys: string[]): Action => act({do: 'logBlockIds', keys});
  const byId = (id: string): Action => act({do: 'logBlockById', id});
  const setId = (key: string, id: string | null): Action =>
    act({do: 'setBlockId', id, key});
  const identified = (key: string): Action => act({do: 'logIdentified', key});
  const handles = (name?: string): Action =>
    act(
      name === undefined
        ? {do: 'logBlockHandles'}
        : {do: 'logBlockHandles', name},
    );
  const handle = (key: string): Action => act({do: 'logBlockHandle', key});
  const resolve = (value: string, read?: string): Action =>
    act(
      read === undefined
        ? {do: 'resolveBlockHandle', handle: value}
        : {do: 'resolveBlockHandle', handle: value, read},
    );
  const withId = (block: Node, id: string): Node =>
    ({...block, $: {blockId: id}}) as Node;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  const enter = run('INSERT_PARAGRAPH_COMMAND');
  const backspace = run('DELETE_CHARACTER_COMMAND', true);
  const start = (...blocks: Node[]): Step =>
    update(node('root', 'clear'), {do: 'append', key: 'root', nodes: blocks});
  const blockId = (config: {[key: string]: Json} = {}): ExtensionArg => [
    'BlockIdExtension',
    {generateId: 'counter', ...config},
  ];
  const root = (...children: Node[]) => ({
    root: {
      children,
      direction: null,
      format: '',
      indent: 0,
      type: 'root',
      version: 1,
    },
  });
  // A test extension that loads `json` as the initial editor state.
  const loads = (json: object): ExtensionSpec => ({
    initialEditorState: {json: json as never},
    name: 'test/load',
    phases: [],
  });
  emit(
    'block-id',
    'BlockIdExtension: ids on creation, kept through split, merge and setBlocksType, repaired after a paste or a clone repeats one, positional and derived ids for a document loaded without ids, handles and their resolution, and the disabled, $isIdentifiedBlock, generateId and repairTransforms options.',
    [
      {
        extensions: ['RichTextExtension', blockId()],
        name: 'ids on creation, split and merge',
        steps: [
          // The initial paragraph got a positional id at the load.
          update(ids('1'), identified('1')),
          // Keys: the paragraph 2, its text 3. It sits where the initial
          // paragraph sat, so it takes its id.
          start(p(t('abcd'))),
          update(select(['3', 2, 'text'])),
          // Enter in the middle: the new paragraph 5 (with the text 4)
          // gets a new id.
          enter,
          update(ids('2', '5'), byId('id1'), byId('id2')),
          // Enter at the start of 5: its id follows the content, and the
          // emptied paragraph gets a new one.
          update(select(['4', 0, 'text'])),
          enter,
          update(byId('id1'), byId('id2')),
          // Backspace at the start merges the paragraph into the one
          // before it, which keeps its id.
          backspace,
          backspace,
          update(ids('2'), byId('id1'), byId('id2')),
          type('x'),
          // setBlocksType: the heading takes the paragraph's id.
          update({block: 'h1', do: 'setBlocksType'}),
        ],
      },
      {
        extensions: ['RichTextExtension', blockId()],
        name: 'repairs after a paste and a clone',
        steps: [
          // Keys: "one" 2 and 3, "two" 4 and 5.
          start(p(t('one')), p(t('two'))),
          update(select(['5', 3, 'text'])),
          // A pasted block that carries an id already in the document
          // yields it to the block it was copied from; one with an unused
          // id keeps it. (The first block merges into "two".)
          update({
            do: 'pasteLexical',
            nodes: [
              p(t('merged')),
              withId(p(t('pasted')), 'id1'),
              withId(p(t('fresh')), 'kept'),
            ],
          }),
          update(byId('id1'), byId('kept')),
          // A clone inserted with the same id.
          update(node('2', 'insertAfter', withId(p(t('clone')), 'id1'))),
          update(byId('id1')),
          // Paste a copy of the first block and of the clone at the end of
          // the first (a copy would log the editor's random namespace): the
          // copy of the clone repeats its id.
          update(select(['3', 3, 'text']), {
            do: 'pasteLexical',
            nodes: [
              withId(p(t('one')), 'yV2Q4QYdu_-lckETHqioD'),
              withId(p(t('clone')), 'id3'),
            ],
          }),
          // An invalid id set directly reads as none, and is replaced.
          update(setId('2', 'not valid'), ids('2')),
          update(ids('2')),
        ],
      },
      {
        extensionDefs: {
          load: loads(
            root(
              p(t('first')),
              q(t('second')),
              withId(p(t('dup a')), 'same'),
              withId(p(t('dup b')), 'same'),
              list('bullet', li(t('item')), li(list('bullet', li(t('deep'))))),
            ),
          ),
        },
        extensions: ['load', 'RichTextExtension', 'ListExtension', blockId()],
        name: 'positional and derived ids on load',
        steps: [
          // Keys in document order: the list is 9, its items 10 and 12, the
          // nested list 13 and its item 14.
          update(
            identified('9'),
            identified('10'),
            identified('12'),
            identified('13'),
            identified('14'),
            byId('same'),
          ),
          // A block without an id gets a generated one after the load.
          update(select(['2', 5, 'text'])),
          enter,
        ],
      },
      {
        extensions: ['RichTextExtension', blockId()],
        name: 'slots',
        steps: [
          // Keys: the host 2, its text 3, the slot value 4.
          start(p(t('host'))),
          update(
            act({do: 'setSlot', key: '2', name: 'title', node: p(t('Title'))}),
          ),
          update(ids('2', '4'), identified('4'), byId('id1')),
        ],
      },
      {
        extensions: ['RichTextExtension', blockId()],
        name: 'handles',
        steps: [
          start(p(t('a')), p(t('b')), p(t('c'))),
          // blk6251 and blk6912 hash to keys that share 4 characters.
          update(setId('2', 'blk6251'), setId('4', 'blk6912')),
          update(
            handles('read'),
            handle('2'),
            handle('4'),
            handle('6'),
            resolve('4oXJ'),
            resolve('4oXJt'),
            resolve('abc'),
            resolve('a b'),
            resolve('ZZZZ'),
            resolve('blk6251-and-more'),
            resolve('id3'),
          ),
          // The read keeps naming what it named.
          update(node('2', 'remove')),
          update(
            resolve('4oXJ'),
            resolve('4oXJ', 'read'),
            resolve('4oXJe', 'read'),
            resolve('4oXJt', 'read'),
            handles(),
          ),
        ],
      },
      {
        extensions: ['RichTextExtension', blockId({disabled: true})],
        name: 'disabled, then enabled',
        steps: [
          start(p(t('a')), p(t('b'))),
          update(ids('2', '4'), setId('2', 'manual')),
          update(byId('manual'), handles(), handle('2')),
          {setSignals: [['BlockIdExtension', 'disabled', false]]},
          update(ids('2', '4'), byId('manual'), handles()),
          {setSignals: [['BlockIdExtension', 'disabled', true]]},
          update(select(['5', 1, 'text'])),
          enter,
        ],
      },
      {
        extensions: [
          'RichTextExtension',
          'ListExtension',
          blockId({$isIdentifiedBlock: 'rootChild'}),
        ],
        name: '$isIdentifiedBlock',
        steps: [
          start(list('number', li(t('one')), li(t('two'))), p(t('after'))),
          update(identified('2'), identified('3'), identified('7')),
        ],
      },
      {
        extensions: ['RichTextExtension', blockId({generateId: 'stutter'})],
        name: 'generateId that repeats itself',
        steps: [start(p(t('a')), p(t('b')), p(t('c')))],
      },
      {
        extensions: ['RichTextExtension', blockId({generateId: 'invalid'})],
        name: 'generateId that never makes a valid id',
        steps: [
          start(p(t('a'))),
          update(node('2', 'insertAfter', p(t('b')))),
          update(ids('2')),
        ],
      },
      {
        extensions: ['RichTextExtension', blockId()],
        name: 'collaboration and historic updates are repaired after',
        steps: [
          start(p(t('a'))),
          {
            tag: ['collaboration'],
            update: [node('2', 'insertAfter', p(t('remote')))],
          },
          {tag: ['historic'], update: [node('4', 'insertAfter', p(t('old')))]},
        ],
      },
      {
        extensions: ['RichTextExtension', blockId({repairTransforms: false})],
        name: 'repairTransforms: false',
        steps: [
          start(p(t('a'))),
          {
            tag: ['collaboration'],
            update: [
              // A copy of the initial paragraph's positional id.
              node(
                '2',
                'insertAfter',
                withId(p(t('copy')), 'yV2Q4QYdu_-lckETHqioD'),
              ),
              node('2', 'insertAfter', p(t('remote'))),
            ],
          },
        ],
      },
      {
        extensions: [
          'RichTextExtension',
          ['HistoryExtension', {disabled: false}],
          blockId(),
        ],
        name: 'undo and redo keep ids',
        steps: [
          start(p(t('ab'))),
          update(select(['3', 1, 'text'])),
          {...enter, now: 1000},
          {...run('UNDO_COMMAND'), now: 2000},
          update(byId('id1')),
          {...run('REDO_COMMAND'), now: 3000},
          update(byId('id1')),
        ],
      },
    ],
  );
});

test('scripts/a11y', () => {
  // @lexical/a11y's live region (AriaLiveRegionExtension) and the
  // extensions that announce through it: HistoryAnnounceExtension,
  // EditorModeAnnounceExtension, and the packages' HeadingAnnounceExtension
  // (@lexical/rich-text) and AutoLinkAnnounceExtension (@lexical/link). The
  // region is mounted in the body of the root element's document, so the
  // scripts set a root element (`setRootElement`); the reconciler then
  // runs, and the mutation listeners the heading and link announcers use
  // hear from it. In Node an update of an editor with a root starts with no
  // selection, so each update selects first. HistoryExtension is enabled
  // explicitly: its `disabled` defaults to `typeof window === 'undefined'`.
  // Every built editor starts with InitialStateExtension's paragraph, key 1.
  const script = (
    name: string,
    extensions: ExtensionArg[],
    steps: Step[],
    extra: Partial<Script> = {},
  ): Script => ({extensions, liveRegion: true, name, steps, ...extra});
  const root = (on: boolean): Step => ({setRootElement: on});
  const announce = (text: string): Step => ({announce: text});
  const signals = (key: string, ...entries: [string, Json][]): Step => ({
    setSignals: entries.map(([name, value]) => [key, name, value]),
  });
  const run = (dispatch: string): Step => ({dispatch});
  const history: ExtensionArg = ['HistoryExtension', {disabled: false}];
  const heading = (block: string, at: [string, number, 'text' | 'element']) =>
    update(select(at), {block: block as never, do: 'setBlocksType'});
  emit(
    'a11y',
    "@lexical/a11y's AriaLiveRegionExtension (the region's politeness and textContent, U+200B on a repeated announcement, mounting with the root element), HistoryAnnounceExtension, EditorModeAnnounceExtension, and the packages' HeadingAnnounceExtension and AutoLinkAnnounceExtension, through buildEditorFromExtensions with a root element.",
    [
      script(
        'the live region follows the root element',
        ['AriaLiveRegionExtension'],
        [
          // No root: nothing is mounted, the message is only buffered.
          announce('Hello'),
          root(true),
          // A repeat of the buffered message toggles a trailing U+200B.
          announce('Hello'),
          announce('Hello'),
          announce('Hello'),
          announce('Bye'),
          announce(''),
          signals('AriaLiveRegionExtension', ['politeness', 'assertive']),
          root(false),
          announce('Gone'),
          // A new region does not replay the last message.
          root(true),
          announce('Back'),
          {dispose: true},
          announce('After'),
        ],
      ),
      script(
        'politeness from the config',
        [['AriaLiveRegionExtension', {politeness: 'assertive'}]],
        [
          announce('Early'),
          root(true),
          announce('Early'),
          signals('AriaLiveRegionExtension', ['politeness', 'polite']),
          announce('Late'),
        ],
      ),
      script(
        'undo and redo are announced',
        ['RichTextExtension', history, 'HistoryAnnounceExtension'],
        [
          root(true),
          update(select(['1', 0, 'element']), call('insertText', 'a')),
          run('UNDO_COMMAND'),
          run('REDO_COMMAND'),
          run('UNDO_COMMAND'),
          // Undo with nothing left to undo is still announced, twice over.
          run('UNDO_COMMAND'),
          run('UNDO_COMMAND'),
          signals('HistoryAnnounceExtension', ['disabled', true]),
          run('UNDO_COMMAND'),
          signals(
            'HistoryAnnounceExtension',
            ['disabled', false],
            ['redone', 'Again'],
          ),
          run('REDO_COMMAND'),
        ],
      ),
      script(
        'history messages from the config',
        [
          'RichTextExtension',
          history,
          ['HistoryAnnounceExtension', {redone: 'R', undone: 'U'}],
        ],
        [root(true), run('UNDO_COMMAND'), run('REDO_COMMAND')],
      ),
      script(
        'editable transitions are announced',
        ['EditorModeAnnounceExtension'],
        [
          root(true),
          {setEditable: false},
          // No transition, no announcement.
          {setEditable: false},
          {setEditable: true},
          signals('EditorModeAnnounceExtension', ['disabled', true]),
          {setEditable: false},
          signals('EditorModeAnnounceExtension', ['disabled', false]),
          {setEditable: true},
          {setEditable: false},
        ],
      ),
      script(
        'editor mode messages from the config, without a root',
        [
          [
            'EditorModeAnnounceExtension',
            {editable: 'Writable', readOnly: 'Locked'},
          ],
        ],
        [{setEditable: false}, root(true), {setEditable: true}],
      ),
      script(
        'headings created, changed and removed',
        ['RichTextExtension', 'HeadingAnnounceExtension'],
        [
          root(true),
          heading('h1', ['1', 0, 'element']),
          // Typing in a heading is silent.
          update(select(['2', 0, 'element']), call('insertText', 'Title')),
          // A new level replaces the node: a creation and a removal, of which
          // the creation is announced.
          heading('h2', ['3', 2, 'text']),
          // The same node with another tag is updated, not created.
          update(select(['3', 2, 'text']), node('4', 'setTag', 'h3')),
          heading('paragraph', ['3', 1, 'text']),
          heading('h6', ['3', 0, 'text']),
          // Headings made without a root are announced when it mounts.
          root(false),
          update(select(['3', 0, 'text']), {
            do: 'insertNodes',
            nodes: [
              {
                children: [],
                direction: null,
                format: '',
                indent: 0,
                tag: 'h4',
                type: 'heading',
                version: 1,
              } as Node,
              {
                children: [],
                direction: null,
                format: '',
                indent: 0,
                tag: 'h5',
                type: 'heading',
                version: 1,
              } as Node,
            ],
          }),
          root(true),
          signals('HeadingAnnounceExtension', ['disabled', true]),
          heading('h1', ['3', 0, 'text']),
          signals(
            'HeadingAnnounceExtension',
            ['disabled', false],
            ['destroyed', 'No more h%s (%s)'],
          ),
          heading('paragraph', ['3', 0, 'text']),
        ],
        {mutations: ['heading']},
      ),
      script(
        'heading messages from the config',
        [
          'RichTextExtension',
          ['HeadingAnnounceExtension', {created: 'Level %s, %s'}],
        ],
        [root(true), heading('h3', ['1', 0, 'element'])],
      ),
      script(
        'automatic links created and removed',
        [
          'RichTextExtension',
          ['AutoLinkExtension', {matchers: ['url']}],
          'AutoLinkAnnounceExtension',
        ],
        [
          root(true),
          update(select(['1', 0, 'element']), call('insertText', 'see ')),
          update(select(['2', 4, 'text']), call('insertText', 'https://a.com')),
          // Extending the address rebuilds or updates the link: silent.
          update(select(['5', 13, 'text']), call('insertText', '/x')),
          update(
            select(['9', 15, 'text']),
            call('insertText', ' https://b.com and https://c.com '),
          ),
          update(
            select(['2', 0, 'text'], ['17', 1, 'text']),
            call('removeText'),
          ),
          update(
            select(['1', 0, 'element']),
            call('insertText', 'https://d.com '),
          ),
          update(
            select(['1', 0, 'element'], ['21', 0, 'text']),
            call('removeText'),
          ),
        ],
        {mutations: ['autolink']},
      ),
      script(
        'link messages from the config, disabled',
        [
          'RichTextExtension',
          ['AutoLinkExtension', {matchers: ['url']}],
          [
            'AutoLinkAnnounceExtension',
            {createdMany: '%s new links (%s)', disabled: true},
          ],
        ],
        [
          root(true),
          update(
            select(['1', 0, 'element']),
            call('insertText', 'https://a.com https://b.com '),
          ),
          signals('AutoLinkAnnounceExtension', ['disabled', false]),
          update(
            select(['1', 0, 'element']),
            call('insertText', 'https://c.com https://d.com '),
          ),
        ],
      ),
    ],
  );
});

test('scripts/decorator-block', () => {
  // @lexical/react's DecoratorBlockNode, through TestDecoratorBlockNode
  // (`test-decorator-block`), which extends it with nothing: a block
  // decorator with an element `format`, which is not indentable.
  const block = (format?: string): Node =>
    ({
      ...(format === undefined ? {} : {format}),
      type: 'test-decorator-block',
      version: 1,
    }) as Node;
  const info = (key: string): Action => ({do: 'logDecoratorBlock', key});
  const script = (name: string, steps: Step[]): Script => ({
    decoratorBlock: true,
    name,
    richText: true,
    steps,
  });
  emit(
    'decorator-block',
    "@lexical/react's DecoratorBlockNode, extended by TestDecoratorBlockNode: isInline, canIndent, getFormat and setFormat, its JSON with and without a format, $isDecoratorBlockNode, and blocks inserted and formatted.",
    [
      script('a block between paragraphs', [
        doc(p(t('ab'))),
        update(select(['2', 1, 'text']), {do: 'insertNodes', nodes: [block()]}),
        update(info('3'), info('1'), info('2')),
        update(node('3', 'setFormat', 'center'), info('3')),
        update(node('3', 'setFormat', 'justify'), node('2', 'setFormat', 1)),
        // A later clone keeps the format.
        update(node('3', 'markDirty'), info('3')),
        update(node('3', 'setFormat', '')),
      ]),
      script('formats from JSON', [
        doc(block('right'), block(), block('bogus'), block('start'), p(t('x'))),
        update(info('1'), info('2'), info('3'), info('4')),
        update(
          {anchor: ['6', 1, 'text'], do: 'select'},
          call('insertNodes', [block('end')]),
        ),
      ]),
      script('FORMAT_ELEMENT_COMMAND with the block selected', [
        doc(p(t('a')), block('left'), p(t('b'))),
        update({do: 'selectNodes', keys: ['3']}),
        {dispatch: 'FORMAT_ELEMENT_COMMAND', payload: 'center'},
        update(select(['2', 0, 'text'], ['5', 1, 'text'])),
        {dispatch: 'FORMAT_ELEMENT_COMMAND', payload: 'right'},
        update(info('3')),
        {dispatch: 'INDENT_CONTENT_COMMAND'},
      ]),
      script('deleting around a block', [
        doc(p(t('a')), block('center'), p(t('b'))),
        update(select(['4', 0, 'element'])),
        {dispatch: 'DELETE_CHARACTER_COMMAND', payload: true},
        {dispatch: 'DELETE_CHARACTER_COMMAND', payload: true},
        {dispatch: 'DELETE_CHARACTER_COMMAND', payload: true},
      ]),
    ],
  );
});

test('scripts/menu', () => {
  // @lexical/react's typeahead menu, without its view: the text before the
  // caret (`getTextUpToAnchor`), useBasicTypeaheadTriggerMatch's match of
  // it, `isSelectionOnEntityBoundary`, `getFullMatchOffset`, and what
  // selecting an option runs (`$splitNodeContainingQuery`, then an
  // `onSelectOption` that replaces the query).
  const at = (text: string, offset: number, ...more: Step[]): Step[] => [
    doc(p(t(text))),
    update(select(['2', offset, 'text'])),
    ...more,
  ];
  const match = (spec: TypeaheadSpec): Step => ({typeahead: spec});
  const at$ = {trigger: '@'};
  const fullMatch = (
    documentText: string,
    entryText: string,
    offset: number,
  ): Action => ({do: 'logFullMatchOffset', documentText, entryText, offset});
  const script = (name: string, steps: Step[], extra: Partial<Script> = {}) =>
    ({name, steps, ...extra}) as Script;
  emit(
    'menu',
    "@lexical/react's typeahead menu without its view: getTextUpToAnchor, useBasicTypeaheadTriggerMatch (the trigger, punctuation, minLength and maxLength, allowWhitespace, UTF-16 offsets), isSelectionOnEntityBoundary, getFullMatchOffset, and selecting an option ($splitNodeContainingQuery and a replacement).",
    [
      script('a trigger after a space', at('hi @bob', 7, match(at$))),
      script('a trigger at offset 0', at('@x', 2, match(at$))),
      script('a trigger mid-word', at('a@b', 3, match(at$))),
      script('a trigger after an open parenthesis', at('(@bo', 4, match(at$))),
      script(
        'a trigger alone',
        at('hi @', 4, match(at$), match({minLength: 0, trigger: '@'})),
      ),
      script('the caret inside the query', at('hi @bob there', 6, match(at$))),
      script('punctuation ends the query', [
        ...at('@bo.b', 5, match(at$)),
        update(select(['2', 3, 'text'])),
        match(at$),
        update(select(['2', 4, 'text'])),
        match(at$),
      ]),
      script('a custom punctuation', [
        ...at('#a,b', 4, match({trigger: '#'})),
        match({punctuation: '\\.', trigger: '#'}),
        match({punctuation: '', trigger: '#'}),
      ]),
      script('minLength', [
        ...at('@bo', 3, match({minLength: 2, trigger: '@'})),
        match({minLength: 3, trigger: '@'}),
      ]),
      script('maxLength', [
        ...at('x @abc', 6, match({maxLength: 3, trigger: '@'})),
        match({maxLength: 2, trigger: '@'}),
        match({maxLength: 0, trigger: '@'}),
      ]),
      script('allowWhitespace', [
        ...at('@bob smith', 10, match(at$)),
        match({allowWhitespace: true, trigger: '@'}),
        match({allowWhitespace: true, maxLength: 8, trigger: '@'}),
      ]),
      script('outside the BMP before the trigger', [
        ...at('😀 @bob', 7, match(at$)),
        update(select(['2', 2, 'text'])),
        match(at$),
      ]),
      script('outside the BMP in the query', [
        ...at('@😀', 3, match(at$)),
        match({maxLength: 2, trigger: '@'}),
        match({maxLength: 1, trigger: '@'}),
        match({minLength: 2, trigger: '@'}),
        match({minLength: 3, trigger: '@'}),
      ]),
      script('a slash command', at('/he', 3, match({trigger: '/'}))),
      script('the text before the caret', [
        doc(p(t('@ab'), t('@cd', {format: 1}))),
        // An element point has no text before it.
        update(select(['1', 0, 'element'])),
        match(at$),
        // Only the anchor's own text node counts.
        update(select(['3', 2, 'text'])),
        match(at$),
        // A range: the text up to its anchor.
        update(select(['2', 2, 'text'], ['3', 1, 'text'])),
        match(at$),
        // A token is not simple text.
        update(node('3', 'setMode', 'token'), select(['3', 3, 'text'])),
        match(at$),
        update(node('1', 'select', 0, 0)),
        match(at$),
      ]),
      script(
        'a query after a text entity',
        [
          doc(p(t('#tag', {type: 'hashtag'}), t('@bo'))),
          update(select(['3', 3, 'text'])),
          match(at$),
          update(select(['2', 4, 'text'])),
          match({trigger: '#'}),
        ],
        {hashtag: true},
      ),
      script('selecting an option', [
        ...at('hi @bo', 6),
        update({
          do: 'selectTypeaheadOption',
          replacement: '[Bob]',
          typeahead: at$,
        }),
      ]),
      script('selecting an option for a query at offset 0', [
        ...at('@bo and more', 3),
        update({
          do: 'selectTypeaheadOption',
          replacement: '[Bob]',
          typeahead: at$,
        }),
      ]),
      script('selecting an option mid-text', [
        ...at('x @b y', 4),
        update({
          do: 'selectTypeaheadOption',
          replacement: '[B]',
          typeahead: at$,
        }),
      ]),
      script('selecting an option without a match', [
        ...at('no trigger', 4),
        update({
          do: 'selectTypeaheadOption',
          replacement: '[none]',
          typeahead: at$,
        }),
      ]),
      script('selecting an option outside the BMP', [
        ...at('😀@x 😀 @😀b', 11),
        update({
          do: 'selectTypeaheadOption',
          replacement: '[😀]',
          typeahead: at$,
        }),
      ]),
      script('getFullMatchOffset', [
        update(
          fullMatch('hi @bo', 'bob', 3),
          fullMatch('hi @bob', 'bob', 4),
          fullMatch('hi bo', 'bob', 0),
          fullMatch('bob', 'bob', 0),
          fullMatch('xbo', 'bob', 1),
          fullMatch('', 'bob', 0),
          fullMatch('ab', '', 0),
          fullMatch('a😀', '😀b', 0),
          fullMatch('a😀', '😀b', 1),
        ),
      ]),
    ],
  );
});

test('scripts/auto-embed', () => {
  // The platform-neutral parts of @lexical/react's auto-embed that JS can
  // run without React: URL_MATCHER, the plugin's single-token test of a
  // pasted text, INSERT_EMBED_COMMAND without a handler, and the mutations
  // a pasted link makes, which the plugin's mutation listeners act on (a
  // created link in an update tagged `paste`).
  const url = (text: string): Action => ({do: 'logUrlMatcher', text});
  const token = (text: string): Action => ({do: 'logSingleTokenPaste', text});
  emit(
    'auto-embed',
    "@lexical/react's auto-embed without React: URL_MATCHER on URLs it matches and does not, the plugin's single-token test of pasted text, INSERT_EMBED_COMMAND, and the link mutations a paste makes with a root element.",
    [
      {
        name: 'URL_MATCHER',
        steps: [
          update(
            url('https://www.youtube.com/watch?v=jNQXAC9IVRw'),
            url('see www.example.com/a?b=c#d.'),
            url('http://example.com'),
            url('example.com'),
            url('ftp://example.com'),
            url('https://a.b'),
            url('https://localhost'),
            url('https://example.abcdefg'),
            url('https://example.abcdef'),
            url('https://ex-ample.co.uk/(x)/y_z~'),
            url('😀 https://a.com/😀'),
            url('HTTPS://A.COM'),
            url('https://www.'),
            url('https://' + 'a'.repeat(257) + '.com'),
            url('https://' + 'a'.repeat(256) + '.com'),
            url('https://exa mple.com'),
            url('https://example.com.'),
            url('xhttps://example.com'),
            url('https://example.c0m!x'),
            url(''),
          ),
        ],
      },
      {
        name: 'single-token pastes',
        steps: [
          update(
            token('https://a.com'),
            token(''),
            token('a b'),
            token(' a'),
            token('a\n'),
            token('a\tb'),
            token('a\u00a0b'),
            token('a\u0085b'),
            token('a\u180eb'),
            token('a\u2028b'),
            token('a\u3000b'),
            token('a\ufeffb'),
            token('a\u200bb'),
            token('😀'),
          ),
        ],
      },
      {
        link: true,
        name: 'INSERT_EMBED_COMMAND without a handler',
        steps: [{dispatch: 'INSERT_EMBED_COMMAND', payload: 'youtube-video'}],
      },
      {
        autoLink: true,
        link: true,
        mutations: ['link', 'autolink'],
        name: 'links made by a paste',
        richText: true,
        steps: [
          doc(p(t('see '))),
          {setRootElement: true},
          {
            tag: ['paste'],
            update: [
              select(['2', 4, 'text']),
              {data: {'text/plain': 'https://video.test/abc'}, do: 'pasteData'},
            ],
          },
          {
            tag: ['paste'],
            update: [
              select(['1', 0, 'element']),
              {
                data: {'text/plain': 'https://a.test https://b.test '},
                do: 'pasteData',
              },
            ],
          },
          // Typed, not pasted.
          update(
            select(['1', 0, 'element']),
            call('insertText', 'https://c.test '),
          ),
          {
            tag: ['paste'],
            update: [
              select(['1', 0, 'element']),
              {
                data: {
                  'application/x-lexical-editor': JSON.stringify({
                    namespace: NAMESPACE,
                    nodes: [
                      {
                        children: [t('https://d.test')],
                        direction: null,
                        format: '',
                        indent: 0,
                        rel: null,
                        target: null,
                        title: null,
                        type: 'link',
                        url: 'https://d.test',
                        version: 1,
                      },
                    ],
                  }),
                  'text/plain': 'https://d.test',
                },
                do: 'pasteData',
              },
            ],
          },
        ],
      },
    ],
  );
});

test('scripts/character-limit', () => {
  // @lexical/react's character limit (shared/useCharacterLimit.ts), the
  // logic of CharacterLimitPlugin: `findOffset`, whose limit falls between
  // the grapheme clusters `Intl.Segmenter` gives, `$wrapOverflowedNodes`,
  // `$mergePrevious` and `useCharacterLimit` itself, mounted with its
  // effects (`characterLimit`), which logs the remaining characters.
  const act = (action: Record<string, unknown>): Action => action as Action;
  const upTo = (n: number): number[] => [...Array(n + 1).keys()];
  const findOffset = (
    text: string,
    strlen?: 'utf8',
    maxCharacters: number[] = upTo(text.length + 1),
  ): Action =>
    act(
      strlen === undefined
        ? {do: 'findOffset', maxCharacters, text}
        : {do: 'findOffset', maxCharacters, strlen, text},
    );
  const limit = (maxCharacters: number, strlen?: 'utf8'): Step =>
    ({
      characterLimit:
        strlen === undefined ? {maxCharacters} : {maxCharacters, strlen},
    }) as Step;
  const run = (dispatch: string, payload?: Json): Step =>
    payload === undefined ? {dispatch} : {dispatch, payload};
  const type = (text: string): Step =>
    run('CONTROLLED_TEXT_INSERTION_COMMAND', text);
  const backspace = run('DELETE_CHARACTER_COMMAND', true);
  const forwardDelete = run('DELETE_CHARACTER_COMMAND', false);
  const unmount = {unmount: 'characterLimit'} as Step;
  // Logs every node's key and depth, so the steps after can name them.
  const dfs: Action = act({do: 'logDfs'});
  const overflowed = true;
  const richText = true;
  // The texts where the limit and the code units differ: combining marks,
  // emoji ZWJ sequences and modifiers, regional-indicator flags (an odd one
  // out), Hangul jamo and syllables, CR LF, text outside the BMP, Indic
  // conjuncts (GB9c), spacing marks and prepended concatenation marks.
  const combining = 'e\u0301e\u0301\u0302x';
  const family = 'a\u{1F468}\u200D\u{1F469}\u200D\u{1F467}\u200D\u{1F466}b';
  const thumbs = '\u{1F44D}\u{1F3FD}\u{1F44D}';
  const flags = '\u{1F1FA}\u{1F1F8}\u{1F1EB}\u{1F1F7}\u{1F1E9}';
  const hangul = '\u1100\u1161\u11A8\uD55C\u1100\u1100\u1161\uAC00\u11A8';
  const crlf = 'a\r\nb\r\r\n\n';
  const astral = '\u{1D4B3}\u{1D4B4}x\u{20000}';
  const devanagari = '\u0915\u094D\u0937\u093F\u0928\u094D\u200D';
  const prepend = '\u0600\u0661\u0915\u093F';
  const zwjAlone = 'x\u200D\u{1F468}\u2764\uFE0F\u200D\u{1F525}';
  emit(
    'character-limit',
    "@lexical/react's character limit (shared/useCharacterLimit.ts): findOffset at and around the limit in grapheme clusters, with UTF-16 and UTF-8 lengths; $wrapOverflowedNodes and $mergePrevious; and useCharacterLimit's count, wrapping and unwrapping as text is typed and deleted, its Backspace handler, and its clean-up.",
    [
      {
        name: 'findOffset in grapheme clusters',
        steps: [
          update(
            findOffset('hello'),
            findOffset(combining),
            findOffset(family),
            findOffset(thumbs),
            findOffset(flags),
            findOffset(hangul),
            findOffset(crlf),
            findOffset(astral),
            findOffset(devanagari),
            findOffset(prepend),
            findOffset(zwjAlone),
            findOffset(''),
          ),
        ],
      },
      {
        name: 'findOffset with UTF-8 lengths',
        steps: [
          update(
            findOffset('h\u00E9llo', 'utf8', upTo(8)),
            findOffset(combining, 'utf8', upTo(12)),
            findOffset(family, 'utf8', upTo(28)),
            findOffset(flags, 'utf8', upTo(22)),
            findOffset(hangul, 'utf8', upTo(28)),
            findOffset(astral, 'utf8', upTo(14)),
            findOffset(crlf, 'utf8', upTo(9)),
          ),
        ],
      },
      {
        name: 'under, at and over the limit as text is typed',
        overflow: overflowed,
        richText,
        steps: [
          // Keys: the paragraph 1, "hello" 2.
          doc(p(t('hello'))),
          limit(7),
          update(select(['2', 5, 'text'])),
          type('!!'),
          type('?'),
          type('??'),
          update(dfs),
          // Back under the limit: the overflow node unwraps. Without a DOM
          // selection to extend, Backspace deletes a range.
          update(select(['3', 1, 'text'], ['3', 3, 'text'])),
          backspace,
          update(dfs),
          update(select(['2', 6, 'text'], ['3', 1, 'text'])),
          backspace,
          update(dfs),
          update(select(['2', 0, 'text'])),
          type('>>>'),
          update(dfs),
        ],
      },
      {
        name: 'over the limit when it registers, across blocks',
        overflow: overflowed,
        richText,
        steps: [
          // The "\n\n" between blocks counts.
          doc(p(t('one two')), p(t('three')), p(t('four', {format: 1}))),
          limit(10),
          update(select(['4', 2, 'text'])),
          type('x'),
          // Typing into the overflow node.
          update(select(['6', 1, 'text'])),
          type('y'),
          // Removing the first block brings the rest back under.
          update(node('1', 'remove')),
          update(node('3', 'remove')),
        ],
      },
      {
        name: 'the limit between grapheme clusters',
        overflow: overflowed,
        richText,
        steps: [
          doc(
            p(t(family)),
            p(t(flags)),
            p(t(combining)),
            p(t(hangul)),
            p(t(astral)),
          ),
          limit(4),
          update(node('2', 'setTextContent', 'abc' + family)),
          update(node('2', 'setTextContent', 'ab')),
        ],
      },
      {
        name: 'the limit in a cluster after each step',
        overflow: overflowed,
        richText,
        steps: [
          doc(p(t('ab'))),
          limit(5),
          update(select(['2', 2, 'text'])),
          type('\u{1F1FA}\u{1F1F8}'),
          type('\u{1F1EB}'),
          type('\u{1F1F7}'),
          type('e\u0301'),
          type('\r\n'),
          update(dfs),
        ],
      },
      {
        name: 'UTF-8 lengths',
        overflow: overflowed,
        richText,
        steps: [
          doc(p(t('h\u00E9llo'))),
          limit(5, 'utf8'),
          update(select(['2', 6, 'text'])),
          type(' w\u00F6rld'),
          update(node('2', 'setTextContent', '\uD55C\uAD6D')),
          update(node('2', 'setTextContent', family)),
          update(node('2', 'setTextContent', 'a' + astral)),
        ],
      },
      {
        name: 'CR LF in text',
        overflow: overflowed,
        richText,
        steps: [doc(p(t('ab\r\ncd'))), limit(3), limit(2)],
      },
      {
        name: 'text that is not simple, and line breaks',
        overflow: overflowed,
        richText,
        steps: [
          // A token and a segmented text are wrapped whole, as a line
          // break is.
          doc(
            p(
              t('abc'),
              t('token', {mode: 'token'}),
              br(),
              t('seg ment', {mode: 'segmented'}),
              t('tail'),
            ),
          ),
          limit(5),
          update(node('2', 'setTextContent', 'abcdefgh')),
          update(node('2', 'setTextContent', 'a')),
        ],
      },
      {
        name: "Backspace and Delete with the limit's handler",
        overflow: overflowed,
        richText,
        steps: [
          // Keys: the paragraph 1, "abcdef" 2, the paragraph 3, "ghij" 4.
          doc(p(t('abcdef')), p(t('ghij'))),
          limit(4),
          update(dfs),
          // Across the start of the overflow node.
          update(select(['2', 3, 'text'], ['5', 1, 'text'])),
          backspace,
          update(dfs),
          // Emptying an overflow node's paragraph removes the paragraph.
          update(select(['4', 0, 'text'], ['4', 4, 'text'])),
          forwardDelete,
          update(dfs),
        ],
      },
      {
        name: 'Delete in an overflow node removes an empty block after it',
        overflow: overflowed,
        richText,
        steps: [
          // Keys: the paragraph 1, "abcdef" 2, the empty paragraph 3; the
          // limit splits off "ef" 4 into the overflow node 5.
          doc(p(t('abcdef')), p()),
          limit(4),
          update(dfs),
          update(select(['4', 0, 'text'], ['4', 1, 'text'])),
          forwardDelete,
          update(dfs),
        ],
      },
      {
        listen: true,
        name: 'updates the limit tags history-merge',
        overflow: overflowed,
        richText,
        steps: [
          doc(p(t('abcdef'))),
          limit(3),
          update(select(['2', 1, 'text'])),
          type('x'),
        ],
      },
      {
        name: 'clean-up',
        overflow: overflowed,
        richText,
        steps: [
          doc(p(t('abc'))),
          limit(2),
          unmount,
          update(select(['2', 1, 'text'])),
          type('x'),
          // Mounted again, it counts and wraps what is there.
          limit(10),
          type('y'),
        ],
      },
      {
        extension: true,
        name: 'slots count',
        overflow: overflowed,
        richText,
        steps: [
          // Keys: the host paragraph 1, "Body" 2, the slot value 3, "Title"
          // 4, the rule 5.
          doc(p(t('Body'))),
          update(
            act({do: 'setSlot', key: '1', name: 'title', node: p(t('Title'))}),
          ),
          limit(6),
          update(
            act({
              do: 'setSlot',
              key: '1',
              name: 'rule',
              node: {type: 'horizontalrule', version: 1} as Node,
            }),
          ),
          update(node('2', 'setTextContent', 'B')),
        ],
      },
      {
        name: '$wrapOverflowedNodes',
        overflow: overflowed,
        richText,
        steps: [
          // Keys: the paragraph 1, "abc" 2, an overflow 3 with "def" 4, "g"
          // 5, an overflow 6 with "hi" 7, the paragraph 8, "jk" 9.
          doc(
            p(
              t('abc'),
              overflow(t('def', {format: 1})),
              t('g'),
              overflow(t('hi', {format: 2})),
            ),
            p(t('jk')),
          ),
          update(act({do: 'wrapOverflowedNodes', offset: 8}), dfs),
          update(act({do: 'wrapOverflowedNodes', offset: 1}), dfs),
          update(act({do: 'wrapOverflowedNodes', offset: 11}), dfs),
          update(act({do: 'wrapOverflowedNodes', offset: 20}), dfs),
        ],
      },
      {
        name: '$mergePrevious',
        overflow: overflowed,
        richText,
        steps: [
          // Keys: the paragraph 1, overflows 2, 4 and 6 with "x" 3, "y" 5
          // and "z" 7.
          doc(
            p(
              overflow(t('x')),
              overflow(t('y', {format: 1})),
              overflow(t('z', {format: 2})),
            ),
          ),
          // The selection in the previous node, then in the node itself.
          update(
            select(['2', 0, 'element'], ['4', 1, 'element']),
            act({do: 'mergePrevious', key: '4'}),
          ),
          // Text points stay.
          update(
            select(['5', 0, 'text'], ['5', 1, 'text']),
            act({do: 'mergePrevious', key: '6'}),
          ),
          // Into an empty overflow node (8), in the update that makes it,
          // before its transform removes it.
          update(
            node('1', 'append', [overflow()]),
            select(['6', 1, 'element'], ['6', 3, 'element']),
            act({do: 'mergePrevious', key: '8'}),
          ),
          // Without an overflow node before it, nothing changes.
          update(act({do: 'mergePrevious', key: '8'})),
        ],
      },
    ],
  );
});

test('scripts/table-of-contents', () => {
  // @lexical/react's TableOfContentsPlugin, mounted with its effects
  // (`tableOfContents`): its entries ([key, text, tag]) at once, and as its
  // update listener and its HeadingNode and TextNode mutation listeners see
  // headings made, removed, edited and moved. Mutation listeners only hear
  // from the reconciler, so most sessions set a root element first; one
  // without shows what the update listener alone does. With a root and no
  // DOM selection, an update starts with no selection, so each update that
  // selects clears it again.
  const act = (action: Record<string, unknown>): Action => action as Action;
  const toc = {tableOfContents: true} as Step;
  const root = {setRootElement: true} as Step;
  const richText = true;
  const clear: Action = act({do: 'clearSelection'});
  const blocks = (type: string): Action =>
    act({block: type, do: 'setBlocksType'});
  emit(
    'table-of-contents',
    "@lexical/react's TableOfContentsPlugin: the initial entries, then headings created, destroyed, updated, retagged, moved and edited, kept in document order, with and without a root element, and its clean-up.",
    [
      {
        name: 'initial entries',
        richText,
        steps: [
          // Keys: h1 1 "Intro" 2, p 3 "text" 4, h2 5 "Part" 6, quote 7
          // "q" 8, h3 9 (empty).
          doc(
            h('h1', t('Intro')),
            p(t('text')),
            h('h2', t('Part')),
            q(t('q')),
            h('h3'),
          ),
          toc,
        ],
      },
      {
        name: 'created, destroyed and edited headings',
        richText,
        steps: [
          doc(h('h1', t('One')), p(t('a')), h('h2', t('Two'))),
          toc,
          root,
          // Appended at the end, inserted first, inserted in the middle.
          update(node('root', 'append', [h('h3', t('Three'))])),
          update(node('1', 'insertBefore', h('h2', t('Zero')))),
          update(node('3', 'insertAfter', h('h4', t('Half')))),
          // Text edited in a heading, and appended to one.
          update(node('2', 'setTextContent', 'Uno')),
          update(node('5', 'append', [t(' more', {format: 1})])),
          // Removed.
          update(node('1', 'remove')),
          update(node('root', 'clear')),
        ],
      },
      {
        name: 'retagged and turned into paragraphs',
        richText,
        steps: [
          doc(h('h1', t('One')), p(t('a')), h('h2', t('Two'))),
          root,
          toc,
          update(node('5', 'setTag', 'h4')),
          update(select(['4', 0, 'text']), blocks('h3'), clear),
          update(select(['4', 0, 'text']), blocks('paragraph'), clear),
          update(
            select(['2', 0, 'text'], ['6', 1, 'text']),
            blocks('h5'),
            clear,
          ),
        ],
      },
      {
        name: 'moved headings',
        richText,
        steps: [
          doc(h('h1', t('A')), h('h2', t('B')), p(t('x')), h('h3', t('C'))),
          root,
          toc,
          // C moves first, then A last.
          update(node('1', 'insertBefore', '7')),
          update(node('5', 'insertAfter', '1')),
          // B and A swap in one update.
          update(node('3', 'insertBefore', '1'), node('7', 'insertAfter', '3')),
        ],
      },
      {
        name: 'without a root element',
        richText,
        steps: [
          doc(h('h1', t('One')), p(t('a'))),
          toc,
          update(node('root', 'append', [h('h2', t('Two'))])),
          update(node('2', 'setTextContent', 'Uno')),
          update(node('1', 'remove')),
        ],
      },
      {
        name: 'clean-up',
        richText,
        steps: [
          doc(h('h1', t('One'))),
          root,
          toc,
          {unmount: 'tableOfContents'} as Step,
          update(node('root', 'append', [h('h2', t('Two'))])),
        ],
      },
    ],
  );
});
