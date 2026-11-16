/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

import {
  CodeExtension,
  CodeHighlightNode,
  CodeNode,
  registerCodeIndentation,
} from '@lexical/code-core';
import {
  DecoratorTextNode,
  HorizontalRuleNode,
  namedSignals,
} from '@lexical/extension';
import {HashtagNode, registerLexicalHashtag} from '@lexical/hashtag';
import {createEmptyHistoryState, registerHistory} from '@lexical/history';
import {
  autoLinkEmailMatcher,
  AutoLinkNode,
  autoLinkUrlMatcher,
  LinkExtension,
  LinkNode,
  registerAutoLink,
  registerLink,
  TOGGLE_LINK_COMMAND,
} from '@lexical/link';
import {
  $isListItemNode,
  INSERT_CHECK_LIST_COMMAND,
  INSERT_ORDERED_LIST_COMMAND,
  INSERT_UNORDERED_LIST_COMMAND,
  ListItemNode,
  ListNode,
  registerCheckList,
  registerList,
  REMOVE_LIST_COMMAND,
  UPDATE_LIST_START_COMMAND,
} from '@lexical/list';
import {OverflowNode} from '@lexical/overflow';
import {registerPlainText} from '@lexical/plain-text';
import {
  $createHeadingNode,
  $createQuoteNode,
  HeadingNode,
  type HeadingTagType,
  QuoteNode,
  registerRichText,
} from '@lexical/rich-text';
import {$setBlocksType} from '@lexical/selection';
import {
  $caretRangeFromSelection,
  $createNodeSelection,
  $createParagraphNode,
  $createRangeSelection,
  $getCaretRange,
  $getChildCaret,
  $getNodeByKey,
  $getRoot,
  $getSelection,
  $getSiblingCaret,
  $getState,
  $getStateChange,
  $getTextPointCaret,
  $insertNodes,
  $isElementNode,
  $isNodeSelection,
  $isRangeSelection,
  $isTextNode,
  $normalizeCaret,
  $parseSerializedNode,
  $removeTextFromCaretRange,
  $selectAll,
  $setSelection,
  $setSelectionFromCaretRange,
  $setState,
  type AnyLexicalCommand,
  type AnyStateConfig,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  type CaretDirection,
  CLEAR_EDITOR_COMMAND,
  CLEAR_HISTORY_COMMAND,
  COMMAND_PRIORITY_EDITOR,
  CONTROLLED_TEXT_INSERTION_COMMAND,
  createCommand,
  createEditor,
  createState,
  DELETE_CHARACTER_COMMAND,
  DELETE_LINE_COMMAND,
  DELETE_WORD_COMMAND,
  type ElementNode,
  FORMAT_ELEMENT_COMMAND,
  FORMAT_TEXT_COMMAND,
  INDENT_CONTENT_COMMAND,
  INSERT_LINE_BREAK_COMMAND,
  INSERT_PARAGRAPH_COMMAND,
  INSERT_TAB_COMMAND,
  type LexicalCommand,
  type LexicalEditor,
  type LexicalNode,
  type NodeCaret,
  OUTDENT_CONTENT_COMMAND,
  ParagraphNode,
  type PointCaret,
  REDO_COMMAND,
  REMOVE_TEXT_COMMAND,
  resetRandomKey,
  RootNode,
  SELECT_ALL_COMMAND,
  type SerializedLexicalNode,
  SET_TEXT_FORMAT_COMMAND,
  TextNode,
  UNDO_COMMAND,
} from 'lexical';

// A small, JSON-only script language for editor sessions, interpreted here by
// the real JS code and by the Swift runner in
// swift/Tests/ConformanceTests/Ops.swift. Every step of a script is one
// discrete update or command dispatch, and after each one the runner records
// the editor state, the selection, the text and a log of what listeners,
// transforms and command handlers saw. Node keys are deterministic because
// each script starts from `resetRandomKey()`, so scripts name nodes by key.

export type Json =
  | null
  | boolean
  | number
  | string
  | Json[]
  | {[key: string]: Json};

/** `[key, offset, type]` of a selection point. */
export type PointSpec = [string, number, 'text' | 'element'];

/**
 * A caret: `[key, direction, 'sibling' | 'child']`, or
 * `[key, direction, 'text', offset]` where offset is a number or a direction.
 */
export type CaretSpec =
  | [string, CaretDirection, 'sibling' | 'child']
  | [string, CaretDirection, 'text', number | CaretDirection];

/** One action inside an update (or a command handler). */
export type Action =
  /** Parse each serialized node and append it to `key`. */
  | {do: 'append'; key: string; nodes: SerializedLexicalNode[]}
  /** A range selection; the focus defaults to the anchor. */
  | {do: 'select'; anchor: PointSpec; focus?: PointSpec}
  | {do: 'selectNodes'; keys: string[]}
  | {do: 'clearSelection'}
  | {do: 'selectAll'}
  /** `$insertNodes` with the parsed nodes. */
  | {do: 'insertNodes'; nodes: SerializedLexicalNode[]}
  /** A method of the current selection; see SELECTION_METHODS. */
  | {do: 'selection'; method: string; args?: Json[]}
  /** A method of the node `key`; see NODE_METHODS. */
  | {do: 'node'; key: string; method: string; args?: Json[]}
  /** Logs `$caretRangeFromSelection` and its carets and text slices. */
  | {do: 'logCaretRange'; anchor?: CaretSpec; focus?: CaretSpec}
  /** `$removeTextFromCaretRange` of the selection's caret range. */
  | {do: 'removeTextFromCaretRange'}
  /** Logs `$normalizeCaret(caret)`. */
  | {do: 'logNormalizeCaret'; caret: CaretSpec}
  /** Logs `['selectionText', $getSelection().getTextContent()]`. */
  | {do: 'logSelectionText'}
  /** `$setState(node, STATES[state], value)`. */
  | {do: 'setState'; key: string; state: string; value: Json}
  /** `$setState(node, STATES[state], UPDATERS[updater])`. */
  | {do: 'updateState'; key: string; state: string; updater: string}
  /** Logs `['state', key, state, $getState(node, STATES[state])]`. */
  | {do: 'logState'; key: string; state: string}
  /**
   * Logs `['stateChange', key, state, change]`: `$getStateChange` of the
   * node's latest version against its version in the committed editor state,
   * or `'new'` when that state has no node with the key.
   */
  | {do: 'logStateChange'; key: string; state: string}
  /**
   * `$setBlocksType($getSelection(), $create)` (@lexical/selection), with its
   * default `$afterCreateElement` (`$copyBlockFormatIndent`), where `$create`
   * is `$createParagraphNode`, `$createQuoteNode` or `$createHeadingNode(tag)`.
   */
  | {do: 'setBlocksType'; block: 'paragraph' | 'quote' | HeadingTagType}
  | {do: 'dispatch'; command: string; payload?: Json};

/**
 * A command handler: logs, runs its actions, then returns `result`. Like
 * `dispatch`, `command` names one of Lexical's own commands (CORE_COMMANDS),
 * a command of a package the script registers (PACKAGE_COMMANDS), or a
 * command the script makes.
 */
export type HandlerSpec = {
  id: string;
  command: string;
  priority: 0 | 1 | 2 | 3 | 4;
  result: boolean;
  actions?: Action[];
};

/**
 * A step: one discrete update, or a command dispatched outside an update.
 * `now` sets the session's clock (in ms, what `Date.now()` returns while
 * steps run) before the step; without it the clock keeps its last value,
 * which starts at 0.
 */
export type Step = (
  | {update: Action[]; tag?: string[]}
  | {dispatch: string; payload?: Json}
) & {now?: number};

export type Script = {
  name: string;
  /** Names from TRANSFORMS, registered in order. */
  transforms?: string[];
  handlers?: HandlerSpec[];
  /** Log update, mutation and text content listener calls. */
  listen?: boolean;
  /**
   * `registerHistory(editor, createEmptyHistoryState(), delay)` (@lexical/history),
   * registered after the handlers and listeners, on the session's clock.
   */
  history?: {delay: number};
  /**
   * Register `@lexical/rich-text`'s nodes (`HeadingNode`, `QuoteNode`) and
   * `registerRichText(editor)`, before the transforms and handlers, as
   * `RichTextExtension` does.
   */
  richText?: boolean;
  /** `registerPlainText(editor)`, where it registers `registerRichText`. */
  plainText?: boolean;
  /**
   * Register `@lexical/list`'s nodes (`ListNode`, `ListItemNode`),
   * `registerList(editor)` and `registerCheckList(editor)`, after rich and
   * plain text, as `CheckListExtension` does with its `ListExtension`
   * dependency (with their default options).
   */
  list?: boolean;
  /**
   * Register `@lexical/link`'s nodes (`LinkNode`, `AutoLinkNode`) and
   * `registerLink(editor)` with `LinkExtension`'s default config, after the
   * list. `TOGGLE_LINK_COMMAND` takes `null`, a URL or
   * `{url, target, rel, title}`.
   */
  link?: boolean;
  /**
   * With `link`, `registerAutoLink(editor, config)` after `registerLink`, as
   * `AutoLinkExtension` does with its `LinkExtension` dependency. The config
   * is `AutoLinkExtension`'s default (no matchers, `/[.,;\s]/` separators)
   * with the matchers `@lexical/link` exports, `autoLinkUrlMatcher` then
   * `autoLinkEmailMatcher`, and a change handler that logs
   * `['autoLink', url, prevUrl]`.
   */
  autoLink?: boolean;
  /**
   * Register `@lexical/code-core`'s nodes (`CodeNode`, `CodeHighlightNode`),
   * `CodeExtension`'s `register` (its TabNode transform and KEY_ENTER_COMMAND
   * handler) and `registerCodeIndentation(editor)`, after the link, as
   * `CodeIndentExtension` does with its `CodeExtension` dependency (with
   * their default config). The editor is made with `createEditor`, so
   * `CodeNode.insertNewAfter` takes its path for an editor without
   * `CodeExtension` and exits the block itself.
   */
  code?: boolean;
  /**
   * Register `@lexical/overflow`'s `OverflowNode`, as `OverflowExtension`
   * does. It registers nothing else: its `$transform` comes with the class.
   */
  overflow?: boolean;
  /**
   * Register `@lexical/hashtag`'s `HashtagNode` and
   * `registerLexicalHashtag(editor)`, after the code, as `HashtagExtension`
   * does (with its default config).
   */
  hashtag?: boolean;
  /**
   * Register `@lexical/extension`'s decorators, `HorizontalRuleNode` (a
   * block decorator) and `DecoratorTextNode` (an inline one), as
   * `HorizontalRuleExtension` and `DecoratorTextExtension` do. Their other
   * registrations (inserting a rule by command, clicks) are left out.
   */
  extension?: boolean;
  steps: Step[];
  /** Record `state` after every step (default), or after the last only. */
  snapshot?: 'every' | 'final';
};

export type StepResult = {
  /** `JSON.stringify(editorState.toJSON())`. */
  state?: string;
  /** `JSON.stringify(editorState.toJSON(true))`. */
  compact?: string;
  selection: Json;
  text: string;
  log: Json[];
  /** The step threw (an invariant); the editor kept its previous state. */
  error?: true;
};

export type ScriptResult = Script & {results: StepResult[]};

type Context = {
  editor: LexicalEditor;
  log: Json[];
  commands: Map<string, LexicalCommand<Json>>;
};

function $node(key: string): LexicalNode {
  const node = $getNodeByKey(key);
  if (node === null) {
    throw new Error(`no node ${key}`);
  }
  return node;
}

function $parse(nodes: SerializedLexicalNode[]): LexicalNode[] {
  return nodes.map(n => $parseSerializedNode(n));
}

function arg(args: Json[] | undefined, i: number): Json {
  return args === undefined || i >= args.length ? null : args[i];
}

function optional<T>(value: Json): T | undefined {
  return value === null ? undefined : (value as T);
}

/** A selection snapshot: `null`, `['range', anchor, focus, format, style]` or
 * `['node', keys]`. */
export function $selectionSnapshot(): Json {
  const selection = $getSelection();
  if ($isRangeSelection(selection)) {
    const {anchor, focus} = selection;
    return [
      'range',
      [anchor.key, anchor.offset, anchor.type],
      [focus.key, focus.offset, focus.type],
      selection.format,
      selection.style,
    ];
  }
  if ($isNodeSelection(selection)) {
    return ['node', [...selection._nodes]];
  }
  return selection === null ? null : ['other'];
}

function caretSnapshot(caret: PointCaret | NodeCaret | null): Json {
  if (caret === null) {
    return null;
  }
  return caret.type === 'text'
    ? ['text', caret.origin.getKey(), caret.direction, caret.offset]
    : [caret.type, caret.origin.getKey(), caret.direction];
}

function $caret(spec: CaretSpec): PointCaret {
  const [key, direction, kind] = spec;
  const node = $node(key);
  if (kind === 'text') {
    if (!$isTextNode(node)) {
      throw new Error(`${key} is not a text node`);
    }
    return $getTextPointCaret(node, direction, spec[3]);
  }
  if (kind === 'child') {
    if (!$isElementNode(node)) {
      throw new Error(`${key} is not an element`);
    }
    return $getChildCaret(node, direction);
  }
  return $getSiblingCaret(node, direction);
}

/**
 * The selection methods a script may call, by name. Arguments are JSON;
 * serialized nodes are parsed.
 */
const SELECTION_METHODS: Record<string, (args: Json[] | undefined) => void> = {
  deleteCharacter: args => $sel().deleteCharacter(arg(args, 0) as boolean),
  deleteLine: args => $sel().deleteLine(arg(args, 0) as boolean),
  deleteWord: args => $sel().deleteWord(arg(args, 0) as boolean),
  formatText: args =>
    $sel().formatText(
      arg(args, 0) as never,
      optional<number>(arg(args, 1)) as never,
    ),
  insertLineBreak: args =>
    $sel().insertLineBreak(optional<boolean>(arg(args, 0))),
  insertNodes: args =>
    $anySel().insertNodes($parse(arg(args, 0) as SerializedLexicalNode[])),
  insertParagraph: () => {
    $sel().insertParagraph();
  },
  insertRawText: args => $anySel().insertRawText(arg(args, 0) as string),
  insertText: args => $anySel().insertText(arg(args, 0) as string),
  // `modify(alter, isBackward, granularity)`: a headless editor has no
  // `Selection.modify`, so only the moves the model resolves (around
  // decorators and blocks) succeed; the rest throw.
  modify: args =>
    $sel().modify(
      arg(args, 0) as never,
      arg(args, 1) as boolean,
      arg(args, 2) as never,
    ),
  removeText: () => $sel().removeText(),
  setFormat: args => $sel().setFormat(arg(args, 0) as number),
  setStyle: args => $sel().setStyle(arg(args, 0) as string),
  toggleFormat: args => $sel().toggleFormat(arg(args, 0) as never),
};

function $anySel() {
  const selection = $getSelection();
  if (selection === null) {
    throw new Error('no selection');
  }
  return selection;
}

function $sel() {
  const selection = $getSelection();
  if (!$isRangeSelection(selection)) {
    throw new Error('not a range selection');
  }
  return selection;
}

function text(node: LexicalNode): TextNode {
  if (!$isTextNode(node)) {
    throw new Error(`${node.getKey()} is not a text node`);
  }
  return node;
}

function element(node: LexicalNode): ElementNode {
  if (!$isElementNode(node)) {
    throw new Error(`${node.getKey()} is not an element`);
  }
  return node;
}

function listItem(node: LexicalNode): ListItemNode {
  if (!$isListItemNode(node)) {
    throw new Error(`${node.getKey()} is not a list item`);
  }
  return node;
}

/**
 * The node methods a script may call, by name. A method that exists on both
 * text and element nodes (`select`, `setFormat`, `setStyle`) calls the one of
 * the node's kind. The return value is logged only for `splitText`.
 */
const NODE_METHODS: Record<
  string,
  (node: LexicalNode, args: Json[] | undefined, ctx: Context) => void
> = {
  append: (n, args) =>
    element(n).append(...$parse(arg(args, 0) as SerializedLexicalNode[])),
  clear: n => element(n).clear(),
  insertAfter: (n, args) =>
    n.insertAfter(
      $parse([arg(args, 0) as SerializedLexicalNode])[0],
      optional<boolean>(arg(args, 1)),
    ),
  insertBefore: (n, args) =>
    n.insertBefore(
      $parse([arg(args, 0) as SerializedLexicalNode])[0],
      optional<boolean>(arg(args, 1)),
    ),
  markDirty: n => n.markDirty(),
  mergeWithSibling: (n, args) =>
    text(n).mergeWithSibling(text($node(arg(args, 0) as string))),
  remove: (n, args) => n.remove(optional<boolean>(arg(args, 0))),
  replace: (n, args) =>
    n.replace(
      $parse([arg(args, 0) as SerializedLexicalNode])[0],
      optional<boolean>(arg(args, 1)),
    ),
  select: (n, args) => {
    const a = optional<number>(arg(args, 0));
    const f = optional<number>(arg(args, 1));
    if ($isTextNode(n)) {
      n.select(a, f);
    } else {
      element(n).select(a, f);
    }
  },
  selectEnd: n => n.selectEnd(),
  selectNext: (n, args) =>
    n.selectNext(
      optional<number>(arg(args, 0)),
      optional<number>(arg(args, 1)),
    ),
  selectPrevious: (n, args) =>
    n.selectPrevious(
      optional<number>(arg(args, 0)),
      optional<number>(arg(args, 1)),
    ),
  selectStart: n => n.selectStart(),
  setDetail: (n, args) => text(n).setDetail(arg(args, 0) as number),
  setDirection: (n, args) => element(n).setDirection(arg(args, 0) as never),
  setFormat: (n, args) => {
    if ($isTextNode(n)) {
      n.setFormat(arg(args, 0) as never);
    } else {
      element(n).setFormat(arg(args, 0) as never);
    }
  },
  setIndent: (n, args) => element(n).setIndent(arg(args, 0) as number),
  setMode: (n, args) => text(n).setMode(arg(args, 0) as never),
  setStyle: (n, args) => {
    if ($isTextNode(n)) {
      n.setStyle(arg(args, 0) as string);
    } else {
      element(n).setStyle(arg(args, 0) as string);
    }
  },
  setTextContent: (n, args) => text(n).setTextContent(arg(args, 0) as string),
  setTextFormat: (n, args) => element(n).setTextFormat(arg(args, 0) as number),
  setTextStyle: (n, args) => element(n).setTextStyle(arg(args, 0) as string),
  splice: (n, args) =>
    element(n).splice(
      arg(args, 0) as number,
      arg(args, 1) as number,
      $parse(arg(args, 2) as SerializedLexicalNode[]),
    ),
  spliceText: (n, args) =>
    text(n).spliceText(
      arg(args, 0) as number,
      arg(args, 1) as number,
      arg(args, 2) as string,
      optional<boolean>(arg(args, 3)),
    ),
  splitText: (n, args, ctx) => {
    const parts = text(n).splitText(...((args ?? []) as number[]));
    ctx.log.push(['splitText', parts.map(p => p.getKey())]);
  },
  toggleChecked: n => listItem(n).toggleChecked(),
  toggleDirectionless: n => text(n).toggleDirectionless(),
  toggleFormat: (n, args) => text(n).toggleFormat(arg(args, 0) as never),
  toggleUnmergeable: n => text(n).toggleUnmergeable(),
};

/**
 * The NodeState configs a script names in `setState`, `updateState`,
 * `logState` and `logStateChange`. Every session shares them, as an app
 * shares its module-scope configs between editors.
 */
const STATES: Record<string, AnyStateConfig> = {
  // A number, 1 by default.
  count: createState('count', {
    parse: v => (typeof v === 'number' ? v : 1),
  }),
  // A flag that a copy of its node ($copyNode) does not keep.
  fresh: createState('fresh', {
    parse: v => v === true,
    resetOnCopyNode: true,
  }),
  // A string, '' by default.
  label: createState('label', {
    parse: v => (typeof v === 'string' ? v : ''),
  }),
  // Another config with the key of `label`, which collides with it on a
  // node type that uses `label`.
  otherLabel: createState('label', {
    parse: v => (typeof v === 'string' ? v : ''),
  }),
  // An array of words, written as one string and compared by its words.
  words: createState('words', {
    isEqual: (a: readonly string[], b: readonly string[]) =>
      a.length === b.length && a.every((word, i) => word === b[i]),
    parse: (v): readonly string[] =>
      typeof v === 'string' && v !== '' ? v.split(' ') : [],
    unparse: v => v.join(' '),
  }),
};

/** The updater functions a script names in `updateState`. */
const UPDATERS: Record<string, (value: never) => unknown> = {
  // A new number.
  'count:increment': (v: number) => v + 1,
  // The same number, so the node is not written.
  'count:same': (v: number) => v,
  // A new string.
  'label:exclaim': (v: string) => `${v}!`,
  // A new array with the same words, equal by `isEqual`.
  'words:copy': (v: readonly string[]) => [...v],
  // A new array with one more word.
  'words:push-x': (v: readonly string[]) => [...v, 'x'],
};

function stateConfig(name: string): AnyStateConfig {
  const config = STATES[name];
  if (config === undefined) {
    throw new Error(`unknown state ${name}`);
  }
  return config;
}

function $runAction(action: Action, ctx: Context): void {
  switch (action.do) {
    case 'append':
      element($node(action.key)).append(...$parse(action.nodes));
      return;
    case 'select': {
      const selection = $createRangeSelection();
      const focus = action.focus ?? action.anchor;
      selection.anchor.set(...action.anchor);
      selection.focus.set(...focus);
      $setSelection(selection);
      return;
    }
    case 'selectNodes': {
      const selection = $createNodeSelection();
      for (const key of action.keys) {
        selection.add(key);
      }
      $setSelection(selection);
      return;
    }
    case 'clearSelection':
      $setSelection(null);
      return;
    case 'selectAll':
      $selectAll();
      return;
    case 'insertNodes':
      $insertNodes($parse(action.nodes));
      return;
    case 'selection': {
      const method = SELECTION_METHODS[action.method];
      if (method === undefined) {
        throw new Error(`unknown selection method ${action.method}`);
      }
      method(action.args);
      return;
    }
    case 'node': {
      const method = NODE_METHODS[action.method];
      if (method === undefined) {
        throw new Error(`unknown node method ${action.method}`);
      }
      method($node(action.key), action.args, ctx);
      return;
    }
    case 'logCaretRange': {
      const range =
        action.anchor !== undefined && action.focus !== undefined
          ? $getCaretRange($caret(action.anchor), $caret(action.focus))
          : $caretRangeFromSelection($sel());
      const [anchorSlice, focusSlice] = range.getTextSlices();
      const slice = (s: typeof anchorSlice): Json =>
        s === null
          ? null
          : [s.caret.origin.getKey(), s.caret.offset, s.distance];
      ctx.log.push([
        'caretRange',
        caretSnapshot(range.anchor),
        caretSnapshot(range.focus),
        [...range].map(caretSnapshot),
        [slice(anchorSlice), slice(focusSlice)],
      ]);
      return;
    }
    case 'removeTextFromCaretRange': {
      const range = $removeTextFromCaretRange($caretRangeFromSelection($sel()));
      ctx.log.push([
        'removeTextFromCaretRange',
        caretSnapshot(range.anchor),
        caretSnapshot(range.focus),
      ]);
      $setSelectionFromCaretRange(range);
      return;
    }
    case 'logNormalizeCaret':
      ctx.log.push([
        'normalizeCaret',
        caretSnapshot($normalizeCaret($caret(action.caret))),
      ]);
      return;
    case 'setState':
      $setState($node(action.key), stateConfig(action.state), action.value);
      return;
    case 'updateState': {
      const updater = UPDATERS[action.updater];
      if (updater === undefined) {
        throw new Error(`unknown updater ${action.updater}`);
      }
      // Each updater takes the value of the config it is named after.
      $setState(
        $node(action.key),
        stateConfig(action.state),
        updater as (prevValue: unknown) => unknown,
      );
      return;
    }
    case 'logState':
      ctx.log.push([
        'state',
        action.key,
        action.state,
        $getState($node(action.key), stateConfig(action.state)) as Json,
      ]);
      return;
    case 'logStateChange': {
      const prevNode = ctx.editor.getEditorState()._nodeMap.get(action.key);
      ctx.log.push([
        'stateChange',
        action.key,
        action.state,
        prevNode === undefined
          ? 'new'
          : ($getStateChange(
              $node(action.key),
              prevNode,
              stateConfig(action.state),
            ) as Json),
      ]);
      return;
    }
    case 'setBlocksType': {
      const {block} = action;
      $setBlocksType($getSelection(), (): ElementNode => {
        switch (block) {
          case 'paragraph':
            return $createParagraphNode();
          case 'quote':
            return $createQuoteNode();
          case 'h1':
          case 'h2':
          case 'h3':
          case 'h4':
          case 'h5':
          case 'h6':
            return $createHeadingNode(block);
        }
        throw new Error(`unknown block ${block}`);
      });
      return;
    }
    case 'logSelectionText':
      ctx.log.push(['selectionText', $anySel().getTextContent()]);
      return;
    case 'dispatch':
      ctx.log.push([
        'dispatched',
        action.command,
        ctx.editor.dispatchCommand(
          command(ctx, action.command),
          action.payload ?? null,
        ),
      ]);
      return;
  }
}

/**
 * Lexical's own commands, which `dispatch` and handlers name as JS exports
 * them. Any other name is a command the script makes.
 */
const CORE_COMMANDS = new Map<string, AnyLexicalCommand>([
  ['CAN_REDO_COMMAND', CAN_REDO_COMMAND],
  ['CAN_UNDO_COMMAND', CAN_UNDO_COMMAND],
  ['CLEAR_EDITOR_COMMAND', CLEAR_EDITOR_COMMAND],
  ['CLEAR_HISTORY_COMMAND', CLEAR_HISTORY_COMMAND],
  ['CONTROLLED_TEXT_INSERTION_COMMAND', CONTROLLED_TEXT_INSERTION_COMMAND],
  ['DELETE_CHARACTER_COMMAND', DELETE_CHARACTER_COMMAND],
  ['DELETE_LINE_COMMAND', DELETE_LINE_COMMAND],
  ['DELETE_WORD_COMMAND', DELETE_WORD_COMMAND],
  ['FORMAT_ELEMENT_COMMAND', FORMAT_ELEMENT_COMMAND],
  ['FORMAT_TEXT_COMMAND', FORMAT_TEXT_COMMAND],
  ['INDENT_CONTENT_COMMAND', INDENT_CONTENT_COMMAND],
  ['INSERT_LINE_BREAK_COMMAND', INSERT_LINE_BREAK_COMMAND],
  ['INSERT_PARAGRAPH_COMMAND', INSERT_PARAGRAPH_COMMAND],
  ['INSERT_TAB_COMMAND', INSERT_TAB_COMMAND],
  ['OUTDENT_CONTENT_COMMAND', OUTDENT_CONTENT_COMMAND],
  ['REDO_COMMAND', REDO_COMMAND],
  ['REMOVE_TEXT_COMMAND', REMOVE_TEXT_COMMAND],
  ['SELECT_ALL_COMMAND', SELECT_ALL_COMMAND],
  ['SET_TEXT_FORMAT_COMMAND', SET_TEXT_FORMAT_COMMAND],
  ['UNDO_COMMAND', UNDO_COMMAND],
]);

/**
 * The commands of the packages a script registers (`list`, `link`), named as
 * their package exports them.
 */
const PACKAGE_COMMANDS = new Map<string, AnyLexicalCommand>([
  ['INSERT_CHECK_LIST_COMMAND', INSERT_CHECK_LIST_COMMAND],
  ['INSERT_ORDERED_LIST_COMMAND', INSERT_ORDERED_LIST_COMMAND],
  ['INSERT_UNORDERED_LIST_COMMAND', INSERT_UNORDERED_LIST_COMMAND],
  ['REMOVE_LIST_COMMAND', REMOVE_LIST_COMMAND],
  ['UPDATE_LIST_START_COMMAND', UPDATE_LIST_START_COMMAND],
  ['TOGGLE_LINK_COMMAND', TOGGLE_LINK_COMMAND],
]);

function command(ctx: Context, name: string): LexicalCommand<Json> {
  const known = CORE_COMMANDS.get(name) ?? PACKAGE_COMMANDS.get(name);
  if (known !== undefined) {
    return known;
  }
  let cmd = ctx.commands.get(name);
  if (cmd === undefined) {
    cmd = createCommand<Json>(name);
    ctx.commands.set(name, cmd);
  }
  return cmd;
}

/**
 * Node transforms a script may register, by name. Each logs
 * `['transform', name, key]` when called, and each converges.
 */
const TRANSFORMS: Record<string, (ctx: Context) => () => void> = {
  // Records every paragraph transform without changing anything.
  'paragraph:log': ctx =>
    ctx.editor.registerNodeTransform(ParagraphNode, node => {
      ctx.log.push(['transform', 'paragraph:log', node.getKey()]);
    }),

  // Removes an empty paragraph that has a next sibling.
  'paragraph:remove-empty': ctx =>
    ctx.editor.registerNodeTransform(ParagraphNode, node => {
      ctx.log.push(['transform', 'paragraph:remove-empty', node.getKey()]);
      if (node.isEmpty() && node.getNextSibling() !== null) {
        node.remove();
      }
    }),

  // Records every root transform without changing anything.
  'root:log': ctx =>
    ctx.editor.registerNodeTransform(RootNode, node => {
      ctx.log.push(['transform', 'root:log', node.getKey()]);
    }),

  // Bolds a simple text node that starts with "#".
  'text:bold-hash': ctx =>
    ctx.editor.registerNodeTransform(TextNode, node => {
      ctx.log.push(['transform', 'text:bold-hash', node.getKey()]);
      if (
        node.isSimpleText() &&
        node.getTextContent().startsWith('#') &&
        !node.hasFormat('bold')
      ) {
        node.toggleFormat('bold');
      }
    }),

  // Splits a simple text node before its first "#", and makes the "#" part
  // unmergeable so the halves stay apart.
  'text:split-hash': ctx =>
    ctx.editor.registerNodeTransform(TextNode, node => {
      ctx.log.push(['transform', 'text:split-hash', node.getKey()]);
      const index = node.getTextContent().indexOf('#');
      if (node.isSimpleText() && index > 0) {
        const [, hash] = node.splitText(index);
        hash.toggleUnmergeable();
      }
    }),

  // Capitalizes every "a".
  'text:upper-a': ctx =>
    ctx.editor.registerNodeTransform(TextNode, node => {
      ctx.log.push(['transform', 'text:upper-a', node.getKey()]);
      const content = node.getTextContent();
      if (content.includes('a')) {
        node.setTextContent(content.replaceAll('a', 'A'));
      }
    }),
};

/** A script's editor, set up with its transforms, handlers and listeners,
 * that runs steps one at a time. */
export type Session = {
  editor: LexicalEditor;
  /** Runs `step` and records it; `snapshot` records the state too. */
  run(step: Step, snapshot: boolean): StepResult;
};

/** Starts `script` (without running its steps) on a new headless editor. */
export function startSession(script: Omit<Script, 'steps'>): Session {
  resetRandomKey();
  const editor = createEditor({
    namespace: '',
    nodes: [
      ...(script.richText ? [HeadingNode, QuoteNode] : []),
      ...(script.list ? [ListNode, ListItemNode] : []),
      ...(script.link ? [LinkNode, AutoLinkNode] : []),
      ...(script.code ? [CodeNode, CodeHighlightNode] : []),
      ...(script.overflow ? [OverflowNode] : []),
      ...(script.hashtag ? [HashtagNode] : []),
      ...(script.extension ? [HorizontalRuleNode, DecoratorTextNode] : []),
    ],
    onError: e => {
      throw e;
    },
  });
  const ctx: Context = {commands: new Map(), editor, log: []};
  if (script.richText) {
    registerRichText(editor);
  }
  if (script.plainText) {
    registerPlainText(editor);
  }
  if (script.list) {
    registerList(editor);
    registerCheckList(editor);
  }
  if (script.link) {
    registerLink(editor, namedSignals(LinkExtension.config!));
    if (script.autoLink) {
      registerAutoLink(editor, {
        changeHandlers: [
          (url, prevUrl) => {
            ctx.log.push(['autoLink', url, prevUrl]);
          },
        ],
        excludeParents: [],
        matchers: [autoLinkUrlMatcher, autoLinkEmailMatcher],
      });
    }
  }
  if (script.code) {
    // `CodeExtension.register` reads only the editor.
    (CodeExtension.register as (e: LexicalEditor) => () => void)(editor);
    registerCodeIndentation(editor);
  }
  if (script.hashtag) {
    registerLexicalHashtag(editor);
  }
  for (const name of script.transforms ?? []) {
    const transform = TRANSFORMS[name];
    if (transform === undefined) {
      throw new Error(`unknown transform ${name}`);
    }
    transform(ctx);
  }
  for (const handler of script.handlers ?? []) {
    editor.registerCommand(
      command(ctx, handler.command),
      payload => {
        ctx.log.push(['handler', handler.id, payload]);
        for (const action of handler.actions ?? []) {
          $runAction(action, ctx);
        }
        return handler.result;
      },
      handler.priority ?? COMMAND_PRIORITY_EDITOR,
    );
  }
  if (script.listen) {
    editor.registerUpdateListener(({dirtyElements, dirtyLeaves, tags}) => {
      ctx.log.push([
        'update',
        [...dirtyElements],
        [...dirtyLeaves],
        [...tags].sort(),
      ]);
    });
    for (const klass of [TextNode, ParagraphNode]) {
      editor.registerMutationListener(
        klass,
        mutations => {
          ctx.log.push(['mutation', klass.getType(), [...mutations]]);
        },
        {skipInitialization: true},
      );
    }
    editor.registerTextContentListener(content => {
      ctx.log.push(['textContent', content]);
    });
  }
  // The session's clock: `Date.now()` returns it while a step runs,
  // including the read below that commits a dispatch's update, so the
  // history listener sees it.
  let clock = 0;
  const atClock = <T>(f: () => T): T => {
    const dateNow = Date.now;
    Date.now = () => clock;
    try {
      return f();
    } finally {
      Date.now = dateNow;
    }
  };
  if (script.history !== undefined) {
    const {delay} = script.history;
    atClock(() =>
      // `registerHistory`'s default clock would keep the `Date.now` of the
      // moment it registers; this one reads it at every update.
      registerHistory(editor, createEmptyHistoryState(), delay, () =>
        Date.now(),
      ),
    );
  }
  const run = (step: Step, snapshot: boolean): StepResult => {
    if (step.now !== undefined) {
      clock = step.now;
    }
    return atClock(() => runAtClock(step, snapshot));
  };
  const runAtClock = (step: Step, snapshot: boolean): StepResult => {
    ctx.log = [];
    let error = false;
    try {
      if ('update' in step) {
        editor.update(
          () => {
            for (const action of step.update) {
              $runAction(action, ctx);
            }
          },
          {discrete: true, tag: step.tag},
        );
      } else {
        ctx.log.push([
          'dispatched',
          step.dispatch,
          editor.dispatchCommand(
            command(ctx, step.dispatch),
            step.payload ?? null,
          ),
        ]);
      }
    } catch {
      error = true;
    }
    // A dispatch outside an update commits in a microtask; `editor.read`
    // commits it first.
    const result: StepResult = editor.read(() => ({
      log: ctx.log,
      selection: $selectionSnapshot(),
      text: $getRoot().getTextContent(),
    }));
    if (snapshot) {
      const state = editor.getEditorState();
      result.state = JSON.stringify(state.toJSON());
      result.compact = JSON.stringify(state.toJSON(true));
    }
    if (error) {
      result.error = true;
    }
    return result;
  };
  return {editor, run};
}

/** Runs `script` on a new headless editor and records every step. */
export function runScript(script: Script): ScriptResult {
  const session = startSession(script);
  const results = script.steps.map((step, i) =>
    session.run(
      step,
      script.snapshot !== 'final' || i === script.steps.length - 1,
    ),
  );
  return {...script, results};
}
