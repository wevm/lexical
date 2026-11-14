/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

import {createEmptyHistoryState, registerHistory} from '@lexical/history';
import {registerPlainText} from '@lexical/plain-text';
import {HeadingNode, QuoteNode, registerRichText} from '@lexical/rich-text';
import {
  $caretRangeFromSelection,
  $createNodeSelection,
  $createRangeSelection,
  $getCaretRange,
  $getChildCaret,
  $getNodeByKey,
  $getRoot,
  $getSelection,
  $getSiblingCaret,
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
  type AnyLexicalCommand,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  type CaretDirection,
  CLEAR_EDITOR_COMMAND,
  CLEAR_HISTORY_COMMAND,
  COMMAND_PRIORITY_EDITOR,
  CONTROLLED_TEXT_INSERTION_COMMAND,
  createCommand,
  createEditor,
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
  | {do: 'dispatch'; command: string; payload?: Json};

/**
 * A command handler: logs, runs its actions, then returns `result`. Like
 * `dispatch`, `command` names one of Lexical's own commands (CORE_COMMANDS)
 * or a command the script makes.
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
  toggleDirectionless: n => text(n).toggleDirectionless(),
  toggleFormat: (n, args) => text(n).toggleFormat(arg(args, 0) as never),
  toggleUnmergeable: n => text(n).toggleUnmergeable(),
};

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

function command(ctx: Context, name: string): LexicalCommand<Json> {
  const core = CORE_COMMANDS.get(name);
  if (core !== undefined) {
    return core;
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
    nodes: script.richText ? [HeadingNode, QuoteNode] : [],
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
