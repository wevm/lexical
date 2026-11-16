/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */
import type {Action, HandlerSpec, Json, Script, Step} from './ops';

// The scripts of `vectors/scripts/core-api`: the core's public API beside
// the update cycle the other script files cover: the
// `COMMAND_PRIORITY_BEFORE_*` priorities, `$addUpdateTag` and the update
// tags, `onUpdate` and `$onUpdate`, `mutatedNodes`, `hasNode(s)`,
// `$nodesOfType`, `$getAdjacentNode`, the node getters,
// `TextNode.selectionTransform`, node replacements (`$create`,
// `$createTextNode`, `importJSON`, the builder's conflict invariant),
// `console.warn`'s developer warnings, `@lexical/headless`, `editable` and
// `editor.focus`.

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

function doc(...blocks: Node[]): Step {
  return {update: [{do: 'append', key: 'root', nodes: blocks as never}]};
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

function node(key: string, method: string, ...args: Json[]): Action {
  return {args, do: 'node', key, method};
}

function handler(
  id: string,
  priority: HandlerSpec['priority'],
  result = false,
  actions?: Action[],
): HandlerSpec {
  return {actions, command: 'TEST_COMMAND', id, priority, result};
}

export function coreApiScripts(): Script[] {
  return [
    {
      // Each priority's queue runs critical first; a BEFORE priority puts
      // its listener at the front of its queue, the latest first.
      handlers: [
        handler('editor', 0),
        handler('beforeEditor', -8),
        handler('low', 1),
        handler('beforeLow', -7),
        handler('beforeLow2', -7),
        handler('critical', 4),
        handler('beforeCritical', -4),
        handler('beforeNormal', -6),
        handler('normal', 2),
        handler('beforeHigh', -5),
        handler('high', 3),
      ],
      name: 'before priorities run at the front of their queue',
      steps: [doc(p(t('a'))), {dispatch: 'TEST_COMMAND', payload: 'x'}],
    },
    {
      handlers: [
        handler('normal', 2, true),
        handler('beforeNormal', -6, false),
        handler('beforeHigh', -5, true),
      ],
      name: 'a before priority handling the command stops the queue',
      steps: [doc(p(t('a'))), {dispatch: 'TEST_COMMAND', payload: null}],
    },
    {
      listen: true,
      name: '$addUpdateTag and $hasUpdateTag',
      steps: [
        doc(p(t('a'))),
        {
          tag: ['history-push'],
          update: [
            {do: 'addUpdateTag', tag: 'skip-collab'},
            {do: 'addUpdateTag', tag: 'skip-dom-selection'},
            {do: 'logHasUpdateTag', tag: 'skip-collab'},
            {do: 'logHasUpdateTag', tag: 'history-push'},
            {do: 'logHasUpdateTag', tag: 'focus'},
            node('2', 'setTextContent', 'b'),
          ],
        },
        // An update that changes nothing still has its tags.
        update(
          {do: 'addUpdateTag', tag: 'skip-scroll-into-view'},
          {do: 'logHasUpdateTag', tag: 'skip-collab'},
        ),
      ],
    },
    {
      listen: true,
      name: 'onUpdate runs after the listeners, before $onUpdate',
      steps: [
        doc(p(t('a'))),
        {
          onUpdate: true,
          update: [{do: 'onUpdate'}, node('2', 'setTextContent', 'b')],
        },
        {onUpdate: true, update: [{do: 'onUpdate'}]},
      ],
    },
    {
      listen: true,
      mutatedNodes: true,
      name: 'mutatedNodes',
      steps: [
        doc(p(t('a')), p(t('b'))),
        {setRootElement: true},
        update(node('2', 'setTextContent', 'c')),
        update(node('3', 'remove')),
        update(),
      ],
    },
    {
      name: 'hasNode, hasNodes and $nodesOfType',
      richText: true,
      steps: [
        doc(p(t('a')), p(t('b'))),
        update(
          {do: 'logHasNodes', types: ['text', 'paragraph', 'heading', 'link']},
          {do: 'logHasNodes', types: ['text', 'heading']},
          {do: 'logHasNodes', types: []},
          {do: 'logNodesOfType', type: 'paragraph'},
          {do: 'logNodesOfType', type: 'text'},
          {do: 'logNodesOfType', type: 'heading'},
          node('3', 'remove'),
          {do: 'logNodesOfType', type: 'paragraph'},
        ),
        update({do: 'logNodesOfType', type: 'paragraph'}),
      ],
    },
    {
      name: '$getAdjacentNode',
      steps: [
        doc(p(t('ab'), t('cd', {format: 1})), p()),
        update(
          select(['2', 2, 'text']),
          {do: 'logAdjacentNode', isBackward: false},
          {do: 'logAdjacentNode', isBackward: true},
          select(['2', 0, 'text']),
          {do: 'logAdjacentNode', isBackward: false},
          {do: 'logAdjacentNode', isBackward: true},
          select(['2', 1, 'text']),
          {do: 'logAdjacentNode', isBackward: false},
          select(['3', 2, 'text']),
          {do: 'logAdjacentNode', isBackward: false},
          select(['1', 0, 'element']),
          {do: 'logAdjacentNode', isBackward: false},
          {do: 'logAdjacentNode', isBackward: true},
          select(['4', 0, 'element']),
          {do: 'logAdjacentNode', isBackward: false},
          {do: 'logAdjacentNode', isBackward: true},
          select(['root', 2, 'element']),
          {do: 'logAdjacentNode', isBackward: false},
          {do: 'logAdjacentNode', isBackward: true},
        ),
      ],
    },
    {
      name: 'node getters',
      steps: [
        doc(
          {
            ...(p(t('a', {format: 3}), t('b', {detail: 1})) as object),
            textFormat: 1,
          },
          p(),
        ),
        update(
          {do: 'logGetters', key: 'root'},
          {do: 'logGetters', key: '1'},
          {do: 'logGetters', key: '2'},
          {do: 'logGetters', key: '3'},
          {do: 'logGetters', key: '4'},
          node('3', 'remove'),
          {do: 'logGetters', key: '3'},
        ),
      ],
    },
    {
      name: 'TextNode.selectionTransform',
      steps: [
        doc(p(t('ab', {type: 'test-text'}), t('cd'))),
        update(select(['2', 1, 'text'])),
        update(select(['2', 0, 'text'], ['3', 1, 'text'])),
        update(select(['3', 1, 'text'], ['2', 2, 'text'])),
        update(select(['3', 0, 'text'])),
        update(select(['1', 0, 'element'])),
        update(select(['2', 0, 'text'], ['2', 2, 'text'])),
        update(),
      ],
      testText: true,
    },
    {
      listen: true,
      name: 'a node replacement with withKlass',
      replaceText: true,
      steps: [
        // importJSON's `$create(TextNode)` makes the replacement at once.
        doc(p(t('a')), p()),
        // `$createTextNode` makes a TextNode, then what `with` makes of it.
        update({do: 'appendText', key: '1', text: 'b'}),
        update({do: 'appendNew', key: '3', type: 'text'}),
        update(select(['3', 0, 'element']), {
          args: ['x'],
          do: 'selection',
          method: 'insertText',
        }),
        update(
          {do: 'logNodesOfType', type: 'text'},
          {do: 'logNodesOfType', type: 'test-text'},
          {do: 'logHasNodes', types: ['text', 'test-text']},
        ),
      ],
      transforms: ['text:upper-a'],
    },
    {
      extensionDefs: {
        a: {name: 'a', nodes: ['test-text', 'replace-text'], phases: []},
        b: {name: 'b', nodes: ['test-text', 'replace-text'], phases: []},
      },
      extensions: ['a', 'b'],
      name: 'two extensions replacing one node conflict',
      steps: [],
    },
    {
      extensionDefs: {
        a: {name: 'a', nodes: ['test-text', 'replace-text-legacy'], phases: []},
      },
      extensions: ['a'],
      name: 'a node replacement without withKlass warns',
      steps: [
        doc(p(t('a'))),
        update({do: 'appendText', key: '1', text: 'b'}),
        update({do: 'appendNew', key: '1', type: 'text'}),
      ],
      warnings: true,
    },
    {
      handlers: [
        handler('editor', 0, true, [node('2', 'setTextContent', 'b')]),
      ],
      listen: true,
      name: 'a command dispatched in a read warns and runs in an update',
      steps: [doc(p(t('a'))), {payload: 'x', readDispatch: 'TEST_COMMAND'}],
      warnings: true,
    },
    {
      headless: true,
      name: 'createHeadlessEditor refuses a root element',
      steps: [
        doc(p(t('a'))),
        {setRootElement: true},
        {setRootElement: false},
        update(node('2', 'setTextContent', 'b')),
      ],
    },
    {
      extensions: ['HeadlessExtension'],
      name: 'HeadlessExtension refuses a root element but allows none',
      steps: [
        doc(p(t('a'))),
        {setRootElement: true},
        {setRootElement: false},
        update(node('3', 'setTextContent', 'b')),
      ],
    },
    {
      editable: false,
      name: 'createEditor with editable false',
      steps: [
        update({do: 'logEditable'}),
        {setEditable: true},
        update({do: 'logEditable'}),
      ],
    },
    {
      listen: true,
      name: 'editor.focus',
      steps: [
        doc(p(t('ab')), p(t('cd'))),
        // Without a root element it does nothing.
        {focus: {}},
        {setRootElement: true},
        {focus: {}},
        update({do: 'clearSelection'}),
        {focus: {defaultSelection: 'rootStart'}},
        update({do: 'clearSelection'}),
        {focus: {defaultSelection: 'rootEnd'}},
      ],
    },
  ];
}
