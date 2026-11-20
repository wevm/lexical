/**
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 *
 */

import type {
  MenuTextMatch,
  TriggerFn,
} from '@lexical/react/LexicalTypeaheadMenuPlugin';

import {
  AriaLiveRegionExtension,
  EditorModeAnnounceExtension,
  HistoryAnnounceExtension,
} from '@lexical/a11y';
import {
  $generateJSONFromSelectedNodes,
  $generateNodesFromSerializedNodes,
  $getLexicalContent,
  $insertDataTransferForPlainText,
  $insertDataTransferForRichText,
  $insertGeneratedNodes,
  type LexicalClipboardData,
  setLexicalClipboardDataTransfer,
} from '@lexical/clipboard';
import {
  $isCodeNode,
  $plainifyCodeContent,
  CodeExtension,
  CodeHighlightNode,
  CodeIndentExtension,
  CodeNode,
  DEFAULT_CODE_LANGUAGE,
  registerCodeIndentation,
} from '@lexical/code-core';
import * as CodePrism from '@lexical/code-prism';
import {
  PrismTokenizer,
  registerCodeHighlighting,
  type Tokenizer,
} from '@lexical/code-prism';
import {
  $defaultIsIdentifiedBlock,
  $getBlockById,
  $getBlockHandle,
  $getBlockHandles,
  $getBlockId,
  $getExtensionOutput,
  $isRootChildBlock,
  $resolveBlockHandle,
  AutoFocusExtension,
  batch,
  BlockIdExtension,
  blockIdState,
  buildEditorFromExtensions,
  ClearEditorExtension,
  ClickAfterLastBlockExtension,
  computed,
  DecoratorTextExtension,
  DecoratorTextNode,
  EditorStateExtension,
  effect,
  formatKeyboardShortcut,
  getExtensionDependencyFromEditor,
  getKnownTypesAndNodes,
  getPeerDependencyFromEditor,
  HorizontalRuleExtension,
  HorizontalRuleNode,
  IMEExtension,
  INSERT_HORIZONTAL_RULE_COMMAND,
  KeyboardShortcutsExtension,
  LexicalBuilder,
  namedSignals,
  type NamedSignalsOutput,
  NestedEditorExtension,
  NodeSelectionExtension,
  NodeStructureExtension,
  NormalizeInlineElementsExtension,
  NormalizeTripleClickSelectionExtension,
  registerNodeStructure,
  SelectBlockExtension,
  TabIndentationExtension,
  untracked,
} from '@lexical/extension';
import {
  HashtagExtension,
  HashtagNode,
  registerLexicalHashtag,
} from '@lexical/hashtag';
import {createHeadlessEditor, HeadlessExtension} from '@lexical/headless';
import {
  createEmptyHistoryState,
  HistoryExtension,
  type HistoryState,
  registerHistory,
  SharedHistoryExtension,
} from '@lexical/history';
import {
  AutoLinkAnnounceExtension,
  autoLinkEmailMatcher,
  AutoLinkExtension,
  AutoLinkNode,
  autoLinkUrlMatcher,
  ClickableLinkExtension,
  LinkExtension,
  LinkNode,
  registerAutoLink,
  registerLink,
  TOGGLE_LINK_COMMAND,
} from '@lexical/link';
import {
  $isListItemNode,
  $isListNode,
  CheckListExtension,
  INSERT_CHECK_LIST_COMMAND,
  INSERT_ORDERED_LIST_COMMAND,
  INSERT_UNORDERED_LIST_COMMAND,
  ListExtension,
  ListItemNode,
  ListNode,
  type ListType,
  registerCheckList,
  registerList,
  REMOVE_LIST_COMMAND,
  UPDATE_LIST_START_COMMAND,
} from '@lexical/list';
import {
  $addMarkID,
  $createMarkNode,
  $getMarkIDs,
  $getMarkIDsAtSelection,
  $getMarkIDsByFlavor,
  $getNodeMarkIDs,
  $isMarkNode,
  $migrateMarkNodesToMarkState,
  $removeMarkID,
  $unwrapMarkNode,
  $wrapSelectionInMarkNode,
  type AnyMarkFlavorExtension,
  DEFAULT_MARKABLE_NODES,
  defineMarkExtension,
  MarkBaseExtension,
  MarkExtension,
  MarkNode,
  MarkStateExtension,
  registerMarkIDsListener,
} from '@lexical/mark';
import {registerMarkdownShortcuts, TRANSFORMERS} from '@lexical/markdown';
import {
  $isOverflowNode,
  OverflowExtension,
  OverflowNode,
} from '@lexical/overflow';
import {PlainTextExtension, registerPlainText} from '@lexical/plain-text';
import {
  INSERT_EMBED_COMMAND,
  URL_MATCHER,
} from '@lexical/react/LexicalAutoEmbedPluginUtils';
import * as CharacterLimitPlugin from '@lexical/react/LexicalCharacterLimitPlugin';
import {
  $isDecoratorBlockNode,
  DecoratorBlockNode,
} from '@lexical/react/LexicalDecoratorBlockNode';
import {
  type TableOfContentsEntry,
  TableOfContentsPlugin,
} from '@lexical/react/LexicalTableOfContentsPlugin';
import * as TypeaheadMenuPlugin from '@lexical/react/LexicalTypeaheadMenuPlugin';
import {
  PUNCTUATION,
  useBasicTypeaheadTriggerMatch,
} from '@lexical/react/LexicalTypeaheadMenuPluginUtils';
import {
  $createHeadingNode,
  $createQuoteNode,
  $isHeadingNode,
  DRAG_DROP_PASTE,
  HeadingAnnounceExtension,
  HeadingNode,
  type HeadingTagType,
  QuoteNode,
  registerRichText,
  RichTextExtension,
} from '@lexical/rich-text';
import {
  $addNodeStyle,
  $copyBlockFormatIndent,
  $ensureForwardRangeSelection,
  $forEachSelectedTextNode,
  $getSelectionStyleValueForProperty,
  $isAtNodeEnd,
  $isParentElementRTL,
  $moveCharacter,
  $patchStyleText,
  $setBlocksType,
  $shouldOverrideDefaultCharacterSelection,
  $sliceSelectedTextNodeContent,
  $trimTextContentFromAnchor,
  $wrapNodes,
  getCSSFromStyleObject,
} from '@lexical/selection';
import {
  $computeTableMapSkipCellCheck,
  $createTableSelectionFrom,
  $deleteTableColumnAtSelection,
  $deleteTableRowAtSelection,
  $findCellNode,
  $findTableNode,
  $getTableCellNodeRect,
  $getTableColumnIndexFromTableCellNode,
  $getTableRowIndexFromTableCellNode,
  $insertTableColumnAtNode,
  $insertTableColumnAtSelection,
  $insertTableRowAtNode,
  $insertTableRowAtSelection,
  $isSimpleTable,
  $isTableCellNode,
  $isTableNode,
  $isTableSelection,
  $mergeCells,
  $moveTableColumn,
  $moveTableRow,
  $padTableRows,
  $setTableColumnIsHeader,
  $setTableRowIsHeader,
  $unmergeCell,
  INSERT_TABLE_COMMAND,
  type TableCellNode,
  TableExtension,
  type TableNode,
} from '@lexical/table';
import {
  $canShowPlaceholder,
  $canShowPlaceholderCurry,
  $findTextIntersectionFromCharacters,
  $isRootTextContentEmpty,
  $isRootTextContentEmptyCurry,
  $rootTextContent,
} from '@lexical/text';
import {
  $descendantsMatching,
  $dfs,
  $dfsWithSlots,
  $firstToLastIterator,
  $getDepth,
  $getNearestBlockElementAncestorOrThrow,
  $getNextRightPreorderNode,
  $getNextSiblingOrParentSibling,
  $insertFirst,
  $insertNodeIntoLeaf,
  $insertNodeToNearestRoot,
  $isBlockFullySelected,
  $isEditorIsNestedEditor,
  $lastToFirstIterator,
  $onEscapeDown,
  $onEscapeUp,
  $restoreEditorState,
  $reverseDfs,
  $reverseDfsWithSlots,
  $splitNode,
  $unwrapAndFilterDescendants,
  $unwrapNode,
  $wrapNodeInElement,
  registerNestedElementResolver,
} from '@lexical/utils';
import * as LexicalEvents from 'lexical';
import {
  $addUpdateTag,
  $caretRangeFromSelection,
  $cloneWithProperties,
  $create,
  $createNodeSelection,
  $createParagraphNode,
  $createRangeSelection,
  $createTextNode,
  $getAdjacentNode,
  $getCaretRange,
  $getChildCaret,
  $getDocument,
  $getEditor,
  $getNodeByKey,
  $getRoot,
  $getSelection,
  $getSelectionSlotFrame,
  $getSiblingCaret,
  $getSlotFrame,
  $getSlotHost,
  $getSlotNames,
  $getSlotNameWithinHost,
  $getState,
  $getStateChange,
  $getTextPointCaret,
  $hasUpdateTag,
  $insertNodes,
  $isBlockContainer,
  $isDecoratorNode,
  $isElementNode,
  $isNodeSelection,
  $isRangeSelection,
  $isSlotHost,
  $isTextNode,
  $nodesOfType,
  $normalizeCaret,
  $onUpdate,
  $parseSerializedNode,
  $removeSlot,
  $removeTextFromCaretRange,
  $selectAll,
  $setSelection,
  $setSelectionFromCaretRange,
  $setSlot,
  $setState,
  aliasedValue,
  type AnyLexicalCommand,
  type AnyLexicalExtension,
  type AnyLexicalExtensionArgument,
  type AnyStateConfig,
  arraySetValue,
  arrayValue,
  type BaseSelection,
  BEFORE_INPUT_COMMAND,
  CAN_REDO_COMMAND,
  CAN_UNDO_COMMAND,
  type CaretDirection,
  CLEAR_EDITOR_COMMAND,
  CLEAR_HISTORY_COMMAND,
  CLICK_COMMAND,
  COMMAND_PRIORITY_EDITOR,
  COMPOSITION_END_COMMAND,
  COMPOSITION_START_COMMAND,
  configExtension,
  CONTROL_OR_OTHER_KEY,
  CONTROLLED_TEXT_INSERTION_COMMAND,
  COPY_COMMAND,
  createCommand,
  createEditor,
  createState,
  CUT_COMMAND,
  declarePeerDependency,
  DecoratorNode,
  DELETE_CHARACTER_COMMAND,
  DELETE_LINE_COMMAND,
  DELETE_WORD_COMMAND,
  DRAGEND_COMMAND,
  DRAGOVER_COMMAND,
  DRAGSTART_COMMAND,
  DROP_COMMAND,
  type EditorState,
  ElementNode,
  enumValue,
  FORMAT_ELEMENT_COMMAND,
  FORMAT_TEXT_COMMAND,
  getDeclaredNodeStructure,
  getStyleObjectFromCSS,
  INDENT_CONTENT_COMMAND,
  INSERT_LINE_BREAK_COMMAND,
  INSERT_PARAGRAPH_COMMAND,
  INSERT_TAB_COMMAND,
  INTERNAL_$normalizeNodeStructure,
  INTERNAL_isNodeStructureEnforced,
  KEY_ARROW_DOWN_COMMAND,
  KEY_ARROW_LEFT_COMMAND,
  KEY_ARROW_RIGHT_COMMAND,
  KEY_ARROW_UP_COMMAND,
  KEY_BACKSPACE_COMMAND,
  KEY_DELETE_COMMAND,
  KEY_DOWN_COMMAND,
  KEY_ENTER_COMMAND,
  KEY_ESCAPE_COMMAND,
  KEY_MODIFIER_COMMAND,
  KEY_SPACE_COMMAND,
  KEY_TAB_COMMAND,
  type KeyboardEventModifierMask,
  type KeyboardShortcut,
  type Klass,
  type LexicalCommand,
  type LexicalEditor,
  type LexicalEditorWithDispose,
  type LexicalExtension,
  type LexicalNode,
  type LexicalNodeReplacement,
  MOVE_TO_END,
  MOVE_TO_START,
  type NodeCaret,
  type NodeChildrenConfig,
  nodeSchema,
  nullable,
  numberValue,
  optional as optionalValue,
  OUTDENT_CONTENT_COMMAND,
  ParagraphNode,
  PASTE_COMMAND,
  type PointCaret,
  type RangeSelection,
  REDO_COMMAND,
  REMOVE_TEXT_COMMAND,
  resetRandomKey,
  RootNode,
  SELECT_ALL_COMMAND,
  SELECTION_CHANGE_COMMAND,
  type SerializedEditorState,
  type SerializedLexicalNode,
  SET_TEXT_FORMAT_COMMAND,
  shallowMergeConfig,
  stringValue,
  TextNode,
  UNDO_COMMAND,
  withAccessors,
  withField,
} from 'lexical';
import {createElement} from 'react';
import {renderToStaticMarkup} from 'react-dom/server';

import {browserWindow} from './browserWindow';
import {mountHooks} from './reactHost';

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
  /**
   * `onSelectionChange(domSelection, editor, true)` (LexicalEvents.ts) for a
   * DOM selection at these points, as a browser's `selectionchange` runs it:
   * the selection gets the points (keeping the last one's format and style,
   * as `$internalCreateRangeSelection` does), a caret the format and style
   * of where it is, a range the format its text shares, and the change is
   * announced with SELECTION_CHANGE_COMMAND. Takes a root element
   * (`setRootElement`), whose DOM the points resolve to. In Node Lexical
   * reads no DOM selection, so the update's selection is given the points
   * first, as it would read them; the Rust and Swift ports take the points
   * from the app's view. The focus defaults to the anchor.
   */
  | {do: 'selectionChange'; anchor: PointSpec; focus?: PointSpec}
  | {do: 'selectNodes'; keys: string[]}
  | {do: 'clearSelection'}
  /**
   * Logs `['formatKeyboardShortcut', segments]`:
   * `formatKeyboardShortcut(shortcut, {isApple})` (@lexical/extension) of
   * the shortcut's `key`, `modifiers` and `unshiftedKey`; without
   * `isApple`, the platform's (`IS_APPLE`, the script's `isApple`).
   */
  | {do: 'formatKeyboardShortcut'; shortcut: ShortcutJson; isApple?: boolean}
  /**
   * `$fixFocusOverselection` of NormalizeTripleClickSelectionExtension's
   * output (`getExtensionDependencyFromEditor($getEditor(), ...)
   * .output.$fixFocusOverselection.peek()()`), which a triple click runs.
   */
  | {do: 'fixFocusOverselection'}
  /** `$selectAll()`, or with `scoped`, `$selectAll($getSelection())`. */
  | {do: 'selectAll'; scoped?: boolean}
  /** `$insertNodes` with the parsed nodes. */
  | {do: 'insertNodes'; nodes: SerializedLexicalNode[]}
  /**
   * `$createTableSelectionFrom(table, anchorCell, focusCell)`
   * (@lexical/table), set as the selection.
   */
  | {do: 'selectTable'; table: string; anchor: string; focus: string}
  /**
   * A function of @lexical/table, see TABLE_FUNCTIONS; logs
   * `['table', fn, result]` when it returns something.
   */
  | {do: 'table'; fn: string; args?: Json[]}
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
  /** Logs `['rootTextContent', $rootTextContent()]` (@lexical/text). */
  | {do: 'logRootTextContent'}
  /**
   * Logs `['rootTextContentEmpty', isComposing, trim, direct, curried]`:
   * `$isRootTextContentEmpty(isComposing, trim)` and what
   * `$isRootTextContentEmptyCurry(isComposing, trim)` returns, called
   * (@lexical/text). `trim` is left out (`undefined`) when absent, and
   * logged as `null` then.
   */
  | {do: 'logRootTextContentEmpty'; isComposing: boolean; trim?: boolean}
  /**
   * Logs `['canShowPlaceholder', isComposing, direct, curried]`:
   * `$canShowPlaceholder(isComposing)` and what
   * `$canShowPlaceholderCurry(isComposing)` returns, called (@lexical/text).
   */
  | {do: 'logCanShowPlaceholder'; isComposing: boolean}
  /**
   * Logs `['textIntersection', characters, [key, offset] | null]`:
   * `$findTextIntersectionFromCharacters($getRoot(), characters)`
   * (@lexical/text).
   */
  | {do: 'logTextIntersection'; characters: number}
  /**
   * `$wrapSelectionInMarkNode($getSelection(), isBackward, id)`
   * (@lexical/mark), with the default `$createMarkNode`.
   */
  | {do: 'wrapSelectionInMarkNode'; isBackward: boolean; id: string}
  /** `$unwrapMarkNode(node)`. */
  | {do: 'unwrapMarkNode'; key: string}
  /** Logs `['markIDs', key, offset, $getMarkIDs(node, offset)]`. */
  | {do: 'logMarkIDs'; key: string; offset: number}
  /**
   * `$addMarkID(id, $getSelection())` (MarkStateExtension.ts; without the
   * extension when the editor isn't built with it), logging
   * `['addMarkID', id, keys]` of the nodes it marked. With `flavor` (a mark
   * flavor of REAL_EXTENSIONS, by its key), the flavor-first form
   * `$addMarkID(flavor, id, selection)`; the other MarkState actions take
   * `flavor` the same way.
   */
  | {do: 'addMarkID'; id: string; flavor?: string}
  /**
   * `$removeMarkID(id, $getSelection())` with `inSelection`, otherwise
   * `$removeMarkID(id)`, which walks the document (or, with the extension,
   * reads its `markIDNodeKeys` output).
   */
  | {do: 'removeMarkID'; id: string; inSelection?: boolean; flavor?: string}
  /** Logs `['nodeMarkIDs', key, $getNodeMarkIDs(node)]`. */
  | {do: 'logNodeMarkIDs'; key: string; flavor?: string}
  /**
   * Logs `['markIDsAtSelection', mode, ids]`: `$getMarkIDsAtSelection()` of
   * the current selection, with `mode` when it is given (`null` otherwise).
   */
  | {do: 'logMarkIDsAtSelection'; mode?: 'any' | 'all'; flavor?: string}
  /**
   * `$migrateMarkNodesToMarkState(node)` of the node `key`, or of the root
   * when there is none, logging `['migrateMarkNodesToMarkState', count]`.
   */
  | {do: 'migrateMarkNodesToMarkState'; key?: string; flavor?: string}
  /**
   * Logs `['markIDsByFlavor', key, [[stateKey, ids]...]]`:
   * `$getMarkIDsByFlavor(node)`, its entries sorted by `stateKey`.
   */
  | {do: 'logMarkIDsByFlavor'; key: string}
  /**
   * Copies the selection (@lexical/clipboard): the data
   * `$getClipboardDataFromSelection` gives with its default MIME types but
   * `text/html` (`text/plain` from `getTextContent()`, then
   * `application/x-lexical-editor` from `$getLexicalContent`, left out when
   * that is `null`), written by `setLexicalClipboardDataTransfer` to a new
   * `DataTransfer`. Logs `['copy', [[type, data]...]]` and keeps the
   * `DataTransfer` for `pasteClipboard`.
   */
  | {do: 'copy'}
  /**
   * `$insertDataTransferForRichText` (or, with `plain`,
   * `$insertDataTransferForPlainText`) of the last `copy`'s `DataTransfer` at
   * `$getSelection()`.
   */
  | {do: 'pasteClipboard'; plain?: boolean}
  /**
   * `$insertDataTransferForRichText` of a `DataTransfer` whose
   * `application/x-lexical-editor` is `JSON.stringify({namespace, nodes})`,
   * in the editor's namespace, at `$getSelection()`.
   */
  | {do: 'pasteLexical'; nodes: SerializedLexicalNode[]}
  /**
   * `$insertDataTransferForPlainText` of a `DataTransfer` whose `text/plain`
   * is `text` at `$getSelection()`.
   */
  | {do: 'pastePlainText'; text: string}
  /**
   * `$insertDataTransferForRichText` (or, with `plain`,
   * `$insertDataTransferForPlainText`) of a `DataTransfer` with `data` set
   * in key order, at `$getSelection()`.
   */
  | {do: 'pasteData'; data: Record<string, string>; plain?: boolean}
  /**
   * Logs `['generateJSON', $generateJSONFromSelectedNodes(editor, selection)]`
   * of `$getSelection()`, or of `null` (the whole document) with `whole`.
   */
  | {do: 'generateJSON'; whole?: boolean}
  /**
   * `$insertGeneratedNodes(editor, $generateNodesFromSerializedNodes(nodes),
   * $getSelection())`.
   */
  | {do: 'insertGeneratedNodes'; nodes: SerializedLexicalNode[]}
  /**
   * `$patchStyleText($getSelection(), patch)` (@lexical/selection). A value
   * `{fn: 'describe'}` is the function `(prev, target) => prev@kind`, where
   * `prev` is the current value (`null` when unset) and `kind` is
   * `selection`, `text` or `element`.
   */
  | {do: 'patchStyleText'; patch: {[key: string]: PatchValue}}
  /**
   * Logs `['styleValue', property, value]`:
   * `$getSelectionStyleValueForProperty($getSelection(), property,
   * defaultValue)`, with JS's default `''` when `defaultValue` is absent.
   */
  | {do: 'logSelectionStyleValue'; property: string; defaultValue?: string}
  /**
   * Logs `['styleObject', css, entries, getCSSFromStyleObject(object)]` of
   * `object = getStyleObjectFromCSS(css)`.
   */
  | {do: 'logStyleObject'; css: string}
  /** Logs `['atNodeEnd', anchor, focus]`: `$isAtNodeEnd` of each point. */
  | {do: 'logAtNodeEnd'}
  /** `$trimTextContentFromAnchor(editor, selection.anchor, count)`. */
  | {do: 'trimTextContentFromAnchor'; count: number}
  /**
   * `$forEachSelectedTextNode(fn)`, logging
   * `['forEachSelectedTextNode', [[key, text]...]]` of the nodes `fn` got.
   */
  | {do: 'forEachSelectedTextNode'}
  /** `$ensureForwardRangeSelection($getSelection())`. */
  | {do: 'ensureForwardRangeSelection'}
  /**
   * `$wrapNodes($getSelection(), $create(block), wrapping)`, where
   * `wrapping` is `$create(wrapping)` when given and `null` otherwise.
   */
  | {do: 'wrapNodes'; block: BlockType; wrapping?: BlockType}
  /**
   * Logs `['shouldOverride', isBackward, result, rtl]`:
   * `$shouldOverrideDefaultCharacterSelection(selection, isBackward)` and
   * `$isParentElementRTL(selection)`.
   */
  | {do: 'logShouldOverride'; isBackward: boolean}
  /** `$moveCharacter($getSelection(), shift, isBackward)`. */
  | {do: 'moveCharacter'; shift: boolean; isBackward: boolean}
  /**
   * Logs `['slice', key, text]`: the text of
   * `$sliceSelectedTextNodeContent($getSelection(), node, 'clone')`.
   */
  | {do: 'logSlice'; key: string}
  /** `$copyBlockFormatIndent(from, to)`. */
  | {do: 'copyBlockFormatIndent'; from: string; to: string}
  /**
   * Logs `['clone', key, json]`: the `exportJSON()` of
   * `$cloneWithProperties(node)` after `$addNodeStyle(node)`, a no-op.
   */
  | {do: 'logCloneWithProperties'; key: string}
  /**
   * Logs `['dfs', reverse, [[key, depth]...]]`: `$dfs(start, end)`, or
   * `$reverseDfs(start, end)` with `reverse` (@lexical/utils), where an
   * absent key is `undefined`.
   */
  | {do: 'logDfs'; start?: string; end?: string; reverse?: boolean}
  /** Logs `['depth', key, $getDepth(node)]`. */
  | {do: 'logDepth'; key: string}
  /**
   * Logs `['nextSiblingOrParentSibling', key, [key, depthDiff] | null]`:
   * `$getNextSiblingOrParentSibling(node)`.
   */
  | {do: 'logNextSiblingOrParentSibling'; key: string}
  /**
   * Logs `['nextRightPreorderNode', key, key | null]`:
   * `$getNextRightPreorderNode(node)`.
   */
  | {do: 'logNextRightPreorderNode'; key: string}
  /**
   * Logs `['nearestBlockElementAncestor', key, key]`:
   * `$getNearestBlockElementAncestorOrThrow(node)`.
   */
  | {do: 'logNearestBlockElementAncestor'; key: string}
  /**
   * Logs `['children', key, reverse, keys]`: `$firstToLastIterator(node)`,
   * or `$lastToFirstIterator(node)` with `reverse`.
   */
  | {do: 'logChildren'; key: string; reverse?: boolean}
  /**
   * `$unwrapAndFilterDescendants(node, n => keep.includes(n.getType()))`,
   * logging `['unwrapAndFilterDescendants', key, didMutate]`.
   */
  | {do: 'unwrapAndFilterDescendants'; key: string; keep: string[]}
  /**
   * Logs `['descendantsMatching', key, keys]`: `$descendantsMatching` of the
   * node's children with `n => types.includes(n.getType())`.
   */
  | {do: 'logDescendantsMatching'; key: string; types: string[]}
  /** `$insertFirst(node, parsed)`. */
  | {do: 'insertFirst'; key: string; node: SerializedLexicalNode}
  /**
   * `$insertNodeToNearestRoot(parsed)`, logging
   * `['insertNodeToNearestRoot', key]` of what it returns.
   */
  | {do: 'insertNodeToNearestRoot'; node: SerializedLexicalNode}
  /** `$insertNodeIntoLeaf(parsed)`. */
  | {do: 'insertNodeIntoLeaf'; node: SerializedLexicalNode}
  /** `$splitNode(node, offset)`, logging `['splitNode', left, right]`. */
  | {do: 'splitNode'; key: string; offset: number}
  /**
   * `$wrapNodeInElement(node, $create(block))`, logging
   * `['wrapNodeInElement', key]` of the new element.
   */
  | {do: 'wrapNodeInElement'; key: string; block: BlockType}
  /** `$unwrapNode(node)`. */
  | {do: 'unwrapNode'; key: string}
  /** Keeps `editor.getEditorState()` (the committed state) as `name`. */
  | {do: 'saveEditorState'; name: string}
  /** `$restoreEditorState(editor, state)` of the state kept as `name`. */
  | {do: 'restoreEditorState'; name: string}
  /**
   * `$onEscapeUp(isContainer)` or `$onEscapeDown(isContainer)`, where
   * `isContainer` accepts an element of one of `types`, logging
   * `['escape', direction, result]`.
   */
  | {do: 'escape'; direction: 'up' | 'down'; types: string[]}
  /**
   * Logs `['blockFullySelected', key, result]`:
   * `$isBlockFullySelected(node, $getSelection())`.
   */
  | {do: 'logBlockFullySelected'; key: string}
  /**
   * `$setSlot(host, name, value)` of the node `key` (an element or a
   * decorator), where `value` is the parsed `node`, or the existing node
   * `valueKey` (which moves).
   */
  | {
      do: 'setSlot';
      key: string;
      name: string;
      node?: SerializedLexicalNode;
      valueKey?: string;
    }
  /** `$removeSlot(host, name)` of the node `key`. */
  | {do: 'removeSlot'; key: string; name: string}
  /**
   * Logs `['slots', key, names, host, nameWithinHost, frame]`:
   * `$getSlotNames(node)`, and the keys of `$getSlotHost(node)` and
   * `$getSlotFrame(node)` and `$getSlotNameWithinHost(node)`, `null` when
   * there is none.
   */
  | {do: 'logSlots'; key: string}
  /**
   * Logs `['selectionSlotFrame', key]`: the key of
   * `$getSelectionSlotFrame($getSelection())`, or `null`.
   */
  | {do: 'logSelectionSlotFrame'}
  /**
   * Logs `['dfsWithSlots', reverse, [[key, depth]...]]`:
   * `$dfsWithSlots(start, end)`, or `$reverseDfsWithSlots(start, end)` with
   * `reverse` (@lexical/utils), where an absent key is `undefined`.
   */
  | {do: 'logDfsWithSlots'; start?: string; end?: string; reverse?: boolean}
  /**
   * Logs `['nodesExist', [[key, exists]...]]`: whether `$getNodeByKey(key)`
   * finds each node, which shows what the last commit collected.
   */
  | {do: 'logNodesExist'; keys: string[]}
  /**
   * `$setState(node, blockIdState, id)` of the node `key` (@lexical/extension),
   * with `null` as `undefined`: the value is stored as it is, so an invalid
   * id reads as none.
   */
  | {do: 'setBlockId'; key: string; id: string | null}
  /**
   * Logs `['blockIds', [[key, id]...]]`: `$getBlockId` of each node, for the
   * keys given.
   */
  | {do: 'logBlockIds'; keys: string[]}
  /**
   * Logs `['nodeStructure', type, structure]` for each type:
   * `getDeclaredNodeStructure` of the class the editor registers under it,
   * as STRUCTURE_JSON writes it.
   */
  | {do: 'logNodeStructure'; types: string[]}
  /** Logs `['blockContainer', key, $isBlockContainer(node)]` for each key. */
  | {do: 'logBlockContainer'; keys: string[]}
  /**
   * Logs `['structureEnforced', INTERNAL_isNodeStructureEnforced(editor)]`.
   */
  | {do: 'logStructureEnforced'}
  /** `INTERNAL_$normalizeNodeStructure(node)` of the node `key`. */
  | {do: 'normalizeNodeStructure'; key: string}
  /**
   * `$create(klass)` of the class the editor registers under `type`,
   * appended to the element `key` (the test classes of scripts/node-structure
   * have no `$create...` function of their own).
   */
  | {do: 'appendNew'; key: string; type: string}
  /**
   * Logs `['blockById', id, key]`: the key of `$getBlockById(id)`, or
   * `null`.
   */
  | {do: 'logBlockById'; id: string}
  /**
   * Logs `['identified', key, configured, default, rootChild]`: whether the
   * node `key` is identified by `$getExtensionOutput(BlockIdExtension)`'s
   * `$isIdentifiedBlock`, by `$defaultIsIdentifiedBlock` and by
   * `$isRootChildBlock`.
   */
  | {do: 'logIdentified'; key: string}
  /**
   * Logs `['blockHandles', [[id, handle]...]]`: `$getBlockHandles()`, in its
   * order; with `name`, also keeps it as `name` for `resolveBlockHandle`.
   */
  | {do: 'logBlockHandles'; name?: string}
  /** Logs `['blockHandle', key, handle]`: `$getBlockHandle(node)`. */
  | {do: 'logBlockHandle'; key: string}
  /**
   * Logs `['resolved', handle, status, detail]` for
   * `$resolveBlockHandle(handle, {read})`, where `read` is the handles
   * `logBlockHandles` kept as `read`: `detail` is the found node's key, the
   * ambiguous candidates, the invalid reason, or `null` when not found.
   */
  | {do: 'resolveBlockHandle'; handle: string; read?: string}
  /**
   * What `LexicalMenu`'s `selectOptionAndCleanUp` runs in its update for the
   * typeahead menu (@lexical/react), whose resolution is the match of the
   * text before the caret: `$splitNodeContainingQuery(match)`, logged as
   * `['splitNodeContainingQuery', key, text]` (both `null` without a node),
   * then an `onSelectOption` that replaces that node with a text node of
   * `replacement` and selects its end. Without a match it logs `null`s and
   * does nothing else.
   */
  | {do: 'selectTypeaheadOption'; typeahead: TypeaheadSpec; replacement: string}
  /**
   * Logs `['fullMatchOffset', documentText, entryText, offset, result]`:
   * `getFullMatchOffset(documentText, entryText, offset)` (LexicalMenu.tsx).
   */
  | {
      do: 'logFullMatchOffset';
      documentText: string;
      entryText: string;
      offset: number;
    }
  /**
   * Logs `['decoratorBlock', key, is, isInline, format, canIndent]` of the
   * node `key`: `$isDecoratorBlockNode(node)`, `node.isInline()`, and for a
   * `DecoratorBlockNode` its `getFormat()` and `canIndent()` (`null`s
   * otherwise).
   */
  | {do: 'logDecoratorBlock'; key: string}
  /**
   * Logs `['getDecorators', [[key, decoration]...]]`: the entries of
   * `editor.getDecorators()`, in `Object.entries` order.
   */
  | {do: 'logDecorators'}
  /**
   * Logs `['urlMatcher', text, match]`: `URL_MATCHER.exec(text)`
   * (LexicalAutoEmbedPluginUtils.ts) as `[index, match[0]]`, or `null`.
   */
  | {do: 'logUrlMatcher'; text: string}
  /**
   * Logs `['singleTokenPaste', text, matches]`: whether
   * `LexicalAutoEmbedPlugin`'s `/^\S+$/` matches the pasted `text/plain`
   * (`isSingleTokenPaste`), the literal of the plugin's `PASTE_COMMAND`
   * handler, which it does not export.
   */
  | {do: 'logSingleTokenPaste'; text: string}
  /**
   * Logs `['findOffset', text, strlen, offsets]`: `findOffset(text,
   * maxCharacters, strlen)` (@lexical/react's shared/useCharacterLimit.ts,
   * which segments with `Intl.Segmenter`) for each of `maxCharacters`, where
   * `strlen` is `'utf16'` (the hook's default) unless it is `'utf8'`
   * (STRLEN).
   */
  | {
      do: 'findOffset';
      text: string;
      maxCharacters: number[];
      strlen?: StrlenName;
    }
  /** `$wrapOverflowedNodes(offset)` (shared/useCharacterLimit.ts). */
  | {do: 'wrapOverflowedNodes'; offset: number}
  /** `$mergePrevious(node)` of the overflow node `key`. */
  | {do: 'mergePrevious'; key: string}
  /** `$addUpdateTag(tag)`. */
  | {do: 'addUpdateTag'; tag: string}
  /** Logs `['hasUpdateTag', tag, $hasUpdateTag(tag)]`. */
  | {do: 'logHasUpdateTag'; tag: string}
  /** `$onUpdate(fn)`, `fn` logging `['$onUpdate']`. */
  | {do: 'onUpdate'}
  /**
   * Logs `['hasNodes', [[type, editor.hasNode(klass)]...],
   * editor.hasNodes(klasses)]` of the active editor (`$getEditor()`), the
   * classes of KNOWN_CLASSES.
   */
  | {do: 'logHasNodes'; types: string[]}
  /**
   * Logs `['nodesOfType', type, keys]`: `$nodesOfType(klass)` of the class
   * the editor registers under the type.
   */
  | {do: 'logNodesOfType'; type: string}
  /**
   * Logs `['adjacentNode', isBackward, key]`: `$getAdjacentNode` of the
   * range selection's focus, `null` at a boundary.
   */
  | {do: 'logAdjacentNode'; isBackward: boolean}
  /** Logs `['getters', key, entries]` of the node `key` ($getters). */
  | {do: 'logGetters'; key: string}
  /** Appends `$createTextNode(text)` to the element `key`. */
  | {do: 'appendText'; key: string; text: string}
  /** Logs `['editable', $getEditor().isEditable()]`. */
  | {do: 'logEditable'}
  | {do: 'dispatch'; command: string; payload?: Json};

/**
 * How `findOffset` and `useCharacterLimit` measure text: `'utf16'` is the
 * hook's default (`input.length`), `'utf8'` the UTF-8 byte length, as
 * `CharacterLimitPlugin` measures with `charset: 'UTF-8'`.
 */
export type StrlenName = 'utf16' | 'utf8';

const STRLEN: Record<StrlenName, (input: string) => number> = {
  utf16: input => input.length,
  utf8: input => new TextEncoder().encode(input).length,
};

/** The blocks the `$create` of an action makes. */
export type BlockType = 'paragraph' | 'quote' | HeadingTagType;

/** A value of a `patchStyleText` patch. */
export type PatchValue = string | null | {fn: 'describe'};

/**
 * `useBasicTypeaheadTriggerMatch(trigger, options)`'s arguments
 * (@lexical/react's LexicalTypeaheadMenuPluginUtils.ts): the options left
 * out take its defaults (`PUNCTUATION` for `punctuation`).
 */
export type TypeaheadSpec = {
  trigger: string;
  minLength?: number;
  maxLength?: number;
  punctuation?: string;
  allowWhitespace?: boolean;
};

/**
 * A command handler: logs, runs its actions, then returns `result`. Like
 * `dispatch`, `command` names one of Lexical's own commands (CORE_COMMANDS),
 * a command of a package the script registers (PACKAGE_COMMANDS), or a
 * command the script makes.
 */
export type HandlerSpec = {
  id: string;
  command: string;
  /** A `COMMAND_PRIORITY_*`, `-8` to `-4` the `COMMAND_PRIORITY_BEFORE_*` ones. */
  priority: 0 | 1 | 2 | 3 | 4 | -8 | -7 | -6 | -5 | -4;
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
  | {
      update: Action[];
      tag?: string[];
      /** The update's `onUpdate` logs `['onUpdate']`. */
      onUpdate?: true;
    }
  /**
   * `editor.read(() => editor.dispatchCommand(command, payload))`, logging
   * `['dispatched', command, handled]`: a command dispatched in a read.
   */
  | {readDispatch: string; payload?: Json}
  /**
   * `editor.focus(fn, options)`, `fn` logging `['focused']`. In Node the
   * update finds no selection with a root element (see `setRootElement`).
   */
  | {focus: {defaultSelection?: 'rootStart' | 'rootEnd'}}
  /**
   * `editor.dispatchCommand(command, payload)`; a command whose payload is
   * a DOM event (EVENT_COMMANDS) gets the event its JSON describes.
   */
  | {dispatch: string; payload?: Json}
  /** `editor.dispose()` of an editor built from `extensions`. */
  | {dispose: true}
  /**
   * In one `batch`, `output[signal].value = value` for each
   * `[extension, signal, value]`, where `output` is
   * `getExtensionDependencyFromEditor(editor, extension).output` of an
   * extension of `extensionDefs` with `signals` (its `namedSignals`).
   */
  | {setSignals: [string, string, Json][]}
  /**
   * Logs `['dependency', key, config]`:
   * `getExtensionDependencyFromEditor(editor, extension).config` of an
   * extension of `extensionDefs`.
   */
  | {logDependency: string}
  /**
   * Logs `['peer', name, config]`: the config of
   * `getPeerDependencyFromEditor(editor, name)`, or `null` without one.
   */
  | {logPeer: string}
  /**
   * Logs `['hasExtension', name, has]` for each name:
   * `LexicalBuilder.fromEditor(editor).hasExtensionByName(name)`.
   */
  | {hasExtensions: string[]}
  /**
   * Logs `['history', canUndo, canRedo, undo, redo]`: the `canUndo` and
   * `canRedo` signals of `HistoryExtension`'s output, and the lengths of its
   * history state's undo and redo stacks.
   */
  | {logHistory: true}
  /**
   * `useCharacterLimit(editor, maxCharacters, {remainingCharacters,
   * strlen})` (@lexical/react's shared/useCharacterLimit.ts), mounted as a
   * first render runs it (src/reactHost.ts): its effects run at once.
   * `remainingCharacters` logs `['remainingCharacters', n]`; `strlen` is
   * left to the hook's default unless it is `'utf8'` (STRLEN).
   */
  | {characterLimit: {maxCharacters: number; strlen?: StrlenName}}
  /**
   * `TableOfContentsPlugin` (@lexical/react), mounted as `characterLimit`
   * is: each time it sets its state, logs `['tableOfContents', entries]`,
   * the `[key, text, tag]` of each entry.
   */
  | {tableOfContents: true}
  /** Unmounts what `characterLimit` or `tableOfContents` mounted. */
  | {unmount: 'characterLimit' | 'tableOfContents'}
  /**
   * `editor.setRootElement(root)` with the session's root element (a `div`
   * of a jsdom document of the session's own, made on first use, not in
   * the document), or `setRootElement(null)`. With a root, the reconciler
   * runs and mutation listeners hear from it. In Node, Lexical has no DOM
   * selection (`CAN_USE_DOM` is false), so an update of an editor with a
   * root starts with no selection: the scripts select in each update.
   * With `viewSelection`, the session's window has a current event that no
   * handler reads (`window.event`), so an update that no DOM event drives
   * keeps the editor state's range selection
   * (`$internalCreateRangeSelection`), as it does in a browser whose DOM
   * selection is where Lexical put it: a session recorded from an app's
   * view, whose selection the editor keeps between updates.
   */
  | {setRootElement: boolean; viewSelection?: true}
  /**
   * Stubs the computed styles of the session's window (`getComputedStyle`,
   * which `@lexical/selection`'s `$getComputedStyleForElement` reads) for
   * the elements the reconciler made for these node keys: `direction`
   * (`ltr` when left out) and `writingMode` (`horizontal-tb`). Any other
   * element gets jsdom's. Each such step replaces the last one's stubs; it
   * takes a root element (`setRootElement`) for the nodes to have elements.
   */
  | {
      computedStyles: {
        [key: string]: {direction?: string; writingMode?: string};
      };
    }
  /** `editor.setEditable(editable)`. */
  | {setEditable: boolean}
  /**
   * `getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension)
   * .output.announce(text)` (@lexical/a11y).
   */
  | {announce: string}
  /**
   * What `LexicalTypeaheadMenuPlugin`'s update listener reads in
   * `editor.getEditorState().read()`: `getTextUpToAnchor` of a range
   * selection (`null` otherwise), the trigger function's match of it, and
   * `isSelectionOnEntityBoundary(editor, match.leadOffset)`, logged as
   * `['typeahead', text, match, onEntityBoundary]` (`null`s past a `null`).
   */
  | {typeahead: TypeaheadSpec}
  /**
   * Calls what `registerCodeHighlighting` returned (`codeHighlight`), which
   * unregisters the highlighting transforms and the indent handlers.
   */
  | {unregisterCodeHighlighting: true}
  /**
   * `editor.setEditorState(state)` with the state `saveEditorState` kept
   * under this name (committed, so read-only).
   */
  | {setEditorState: string}
  /**
   * `registerNodeStructure(editor)` (@lexical/extension), its disposer kept
   * under this name.
   */
  | {registerNodeStructure: string}
  /** Calls the disposer `registerNodeStructure` kept under this name. */
  | {disposeNodeStructure: string}
  /**
   * `getExtensionDependencyFromEditor(editor, NodeSelectionExtension)
   * .output.watchNodeKey(key)`, and an `effect` that logs
   * `['nodeSelected', key, signal.value]` now and each time the signal
   * changes, kept under the key.
   */
  | {watchNodeKey: string}
  /** Disposes the effect `watchNodeKey` kept under this key. */
  | {unwatchNodeKey: string}
  /**
   * A DOM event dispatched at the element the reconciler made for the node
   * `targetKey` (`editor.getElementByKey`, `root` the root element; the
   * root element when left out), as a browser would dispatch it, for the
   * extensions that listen to the root element rather than to a command:
   * a bubbling `click` or `auxclick` `MouseEvent` of the mouse fields
   * `eventPayload` reads, or a `compositionend` `CompositionEvent` of
   * `data`. Logs `['domEvent', type, event.defaultPrevented]`. The Rust and
   * Swift runners dispatch CLICK_COMMAND (of the event's type, with
   * `targetKey`) or COMPOSITION_END_COMMAND instead, which their views
   * send for these events.
   */
  | {domEvent: DomEventJson}
  /**
   * Logs `['ime', compositionKey, composingTextNode]` of IMEExtension's
   * output, the text node as its key (or `null`).
   */
  | {logIME: true}
  /**
   * Logs `['editor', namespace, editable, hasRootElement]`:
   * `editor._config.namespace`, `editor.isEditable()` and whether
   * `editor.getRootElement()` is not `null`.
   */
  | {logEditor: true}
  /**
   * `registerMarkIDsListener(editor, flavor, listener)` (@lexical/mark) of
   * the mark flavor `flavor` (MARK_FLAVORS), its disposer kept under
   * `name`. Each call logs `['markIDsChange', name, added, removed, ids,
   * tags]`, each sorted.
   */
  | {registerMarkIDsListener: {name: string; flavor: string}}
  /** Calls the disposer `registerMarkIDsListener` kept under this name. */
  | {unregisterMarkIDsListener: string}
  /**
   * Logs `['markIDNodeKeys', flavor, [[id, keys]...]]`: the `markIDNodeKeys`
   * output of the mark flavor `flavor`, by id, each id's keys sorted.
   */
  | {logMarkIDNodeKeys: string}
) & {
  now?: number;
  /**
   * Run the step on the script's `parent` editor instead of the session's
   * (which the step's result still records).
   */
  inParent?: true;
};

/**
 * A token of a `Tokenizer` as JSON: a string, or Prism's `Token` with its
 * `type`, its `content` (a string or tokens) and its `alias` when that is
 * a string (the highlighter ignores any other).
 */
export type TokenJson =
  | string
  | {type: string; content: string | TokenJson[]; alias?: string};

/**
 * The tokenizer of `codeHighlight`. `'conformance'` is CONFORMANCE_TOKENIZE
 * (every runner implements it). `'recorded'` returns `tokens`, which the
 * script recorded from Prism (`PrismTokenizer.tokenize`), for each
 * `[language, code]` it is asked for, and throws on any other. `'prism'`
 * is `PrismTokenizer`'s `tokenize`, which only JS runs: it adds what it
 * returns to `tokens`, for a script to record them. The tokenizer has only
 * `languages`, each of which Prism has loaded, since the loop asks Prism's
 * `isCodeLanguageLoaded` and the ports ask the tokenizer.
 */
export type CodeHighlightSpec = {
  tokenizer: 'conformance' | 'recorded' | 'prism';
  /**
   * The languages the tokenizer has (`isCodeLanguageLoaded`), each one
   * Prism has loaded; any other is unsupported.
   */
  languages: string[];
  /** `defaultLanguage`; `DEFAULT_CODE_LANGUAGE` when absent. */
  defaultLanguage?: string | null;
  /** `[language, code, tokens]`, for `'recorded'`. */
  tokens?: [string, string, TokenJson[]][];
};

/** A config of an extension of `extensionDefs`, or overrides of one. */
export type ExtensionConfig = {[key: string]: Json};

/**
 * An extension, by its key in `extensionDefs` or its name in
 * REAL_EXTENSIONS, alone or with config overrides
 * (`configExtension(extension, ...overrides)`). The overrides of a real
 * extension name its functions by CONFIG_PRESETS.
 */
export type ExtensionArg = string | [string, ...ExtensionConfig[]];

/** The phases an extension of `extensionDefs` defines. */
export type ExtensionPhase =
  | 'init'
  | 'build'
  | 'register'
  | 'afterRegistration';

/**
 * A test extension, made by `testExtension` from JSON so that every runner
 * can make the same one. Each phase it defines (all four by default) logs
 * its name and merged config: `['init', name, config]`,
 * `['build', name, config]`, `['register', name, config, text]` and
 * `['afterRegistration', name, config, text]`, where `text` is the root
 * text of `editor.getEditorState()`. `init` returns `{init: name}`. The
 * clean-ups that `register` and `afterRegistration` return log
 * `['unregister', name]` and `['afterUnregister', name]`. Unless it has
 * `signals`, `register` also handles `EXTENSION_COMMAND` (a command the
 * script makes), logging `['handled', name, payload]` and returning false.
 */
export type ExtensionSpec = {
  name: string;
  /** `namespace`, of the editor it builds. */
  namespace?: string;
  /** `editable`, of the editor it builds. */
  editable?: boolean;
  /** The default config; without it, the builder's default is `{}`. */
  config?: ExtensionConfig;
  dependencies?: ExtensionArg[];
  /** `declarePeerDependency(name, config?)` each. */
  peerDependencies?: [string, ExtensionConfig?][];
  conflictsWith?: string[];
  /** Node types, of `@lexical/extension`'s decorators (NODES). */
  nodes?: string[];
  /** `theme`: an `EditorThemeClasses`, as JSON. */
  theme?: {[key: string]: Json};
  /**
   * `$initialEditorState`: `null`, an editor state serialized with
   * `JSON.stringify` (`json`), or a function that appends a paragraph
   * with `text` to the root.
   */
  initialEditorState?: null | {json: SerializedEditorState} | {text: string};
  /**
   * `mergeConfig` that merges with `shallowMergeConfig`, then concatenates
   * every array of the overrides to the config's array of the same key.
   */
  mergeConfig?: 'concat';
  phases?: ExtensionPhase[];
  /**
   * `init` logs `['initPeer', name, peer, config]` for `state.getPeer(peer)`
   * of each, `null` when it is absent.
   */
  initPeers?: string[];
  /** `build` logs `['buildPeer', name, peer, config]`, as `initPeers`. */
  buildPeers?: string[];
  /**
   * `init` logs `['initDependency', name, key, config]` and `build`
   * `['buildDependency', name, key, config]` for
   * `state.getDependency(extension)` of each key of `extensionDefs`.
   */
  lookups?: string[];
  /** `register` logs `['initResult', name, state.getInitResult()]`. */
  initResult?: boolean;
  /**
   * `init` logs `['knownTypes', name, types]` of
   * `getKnownTypesAndNodes(editorConfig)`, `['dependents', name, names]` of
   * `state.getDirectDependentNames()` and `['peerNames', name, names]` of
   * `state.getPeerNameSet()`.
   */
  inspect?: boolean;
  /** `register` logs `['aborted', name]` when `state.getSignal()` aborts. */
  abort?: boolean;
  /**
   * `register` registers an update listener that logs
   * `['update', name, tags]` (sorted).
   */
  listen?: boolean;
  /**
   * `build` returns `namedSignals(config)`, of a config with a boolean
   * `disabled` and a string `label`; `register` runs two effects. One logs
   * `['summary', name, summary]`, where `summary` is a `computed` of
   * `'off'` when disabled and `on:<label>` otherwise. The other, unless
   * disabled, logs `['enable', name, label]` with the label read
   * `untracked`, and handles `EXTENSION_COMMAND`, logging
   * `['handled', name, label, payload]`; its clean-up logs
   * `['disable', name]` and unregisters the handler.
   */
  signals?: boolean;
  /**
   * `register` runs an effect that logs `['editorState', name, text]`, the
   * root text of the output of `EditorStateExtension`, which the extension
   * has as a dependency.
   */
  editorState?: boolean;
};

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
   * With `code`, `registerCodeHighlighting(editor, tokenizer)`
   * (@lexical/code-prism) in place of `registerCodeIndentation`: the
   * highlighting transforms, then the same indent handlers. The step
   * `unregisterCodeHighlighting` unregisters both. After every step, the
   * log ends with `['codeBlocks', [[key, language, supported, childKeys]...]]`
   * (`codeBlocksEntry`).
   */
  codeHighlight?: CodeHighlightSpec;
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
  /**
   * Build the editor with `buildEditorFromExtensions(...)` of these
   * extensions instead of `createEditor`; the package flags above don't
   * apply. The transforms, handlers and history are registered on the
   * built editor. The build is recorded in `build`, and when it throws,
   * the script has no steps.
   */
  extensions?: ExtensionArg[];
  /** The test extensions `extensions` names, by key. */
  extensionDefs?: {[key: string]: ExtensionSpec};
  /**
   * With `extensions`, a parent editor: `buildEditorFromExtensions(...)` of
   * these (from `extensionDefs` too), built first, and the session's
   * editor is built in its `read` (so `NestedEditorExtension`'s default
   * `$getParentEditor` finds it), unless `parentRead` is false. After the
   * build and every step, the log ends with
   * `['nested', isNested, parentIsParent, sameNamespace, theme, editable,
   * parentEditable, parentText, parentSelection]` (`nestedEntry`).
   */
  parent?: ExtensionArg[];
  /** Build the session's editor outside the parent's `read`. */
  parentRead?: false;
  /**
   * Register `@lexical/mark`'s `MarkNode`, as `MarkExtension` does. The
   * MarkState functions (`$addMarkID`...) run without `MarkStateExtension`,
   * with its default `markableNodes`.
   */
  mark?: boolean;
  /**
   * Register `TestDecoratorBlockNode` (`test-decorator-block`), a
   * `DecoratorBlockNode` (@lexical/react) that adds nothing, as an app's
   * block embed would extend it.
   */
  decoratorBlock?: boolean;
  /**
   * Register `TestDecoratorNode` (`test-decorator`) and
   * `TestSharedDecoratorNode` (`test-shared-decorator`), inline decorators
   * whose `decorate()` returns a new object on every call, and one object
   * for every node and call.
   */
  testDecorators?: boolean;
  /**
   * `editor.registerDecoratorListener`, registered after the handlers,
   * logging `['decorators', [[key, decoration]...]]` (the entries in
   * `Object.entries` order).
   */
  decorators?: boolean;
  /**
   * After every step, log `['textFormats', [[key, textFormat, textStyle]...]]`:
   * `getTextFormat()` and `getTextStyle()` of these elements in the
   * editor's state, `null`s for a key that isn't an element there. The
   * JSON leaves them out of an element with text (#7968).
   */
  textFormats?: string[];
  /**
   * Mutation listeners (`skipInitialization: true`) for the registered
   * classes of these node types, registered after the handlers, each
   * logging `['mutation', type, [[key, mutation]...], tags]` (the update
   * tags sorted). They only hear from the reconciler, so from an editor
   * with a root element (`setRootElement`).
   */
  mutations?: string[];
  /**
   * After the build and every step, log `['liveRegion', regions]`: the
   * `[aria-live, textContent]` of each `role="status"` element in the body
   * of the session's document, in document order, which
   * `AriaLiveRegionExtension` (@lexical/a11y) mounts there for the root
   * element.
   */
  liveRegion?: boolean;
  /**
   * `registerMarkdownShortcuts(editor, TRANSFORMERS)` (@lexical/markdown),
   * after the hashtag, as `MarkdownShortcutPlugin` does. `TRANSFORMERS`
   * needs the nodes of `richText`, `list`, `link` and `code`.
   */
  markdown?: boolean;
  /**
   * The platform `IS_APPLE` names, which decides the keyboard shortcuts
   * (`CONTROL_OR_META`, the keydown bindings); `false` when left out, as in
   * Node. JS can't change it while running, so the test that records a
   * script with `isApple: true` mocks `lexical/src/environment` and checks
   * that the mock took; the Rust and Swift runners set it for the script.
   */
  isApple?: boolean;
  /** Register `TestTextNode` (`test-text`). */
  testText?: boolean;
  /**
   * The session's root element logs `['rootFocus']` when it is focused
   * (`rootElement.focus()`, which AutoFocusExtension calls), in place of
   * jsdom's focus of an element that isn't in its document.
   */
  rootFocus?: boolean;
  /**
   * Register `TestTextNode` and TEXT_REPLACEMENT with its `withKlass`, so
   * `$create(TextNode)` and `$createTextNode` make `TestTextNode`s.
   */
  replaceText?: boolean;
  /** `createHeadlessEditor` (@lexical/headless) in place of `createEditor`. */
  headless?: boolean;
  /** `createEditor`'s `editable`. */
  editable?: boolean;
  /**
   * Log each `console.warn` as `['warn', message]`, in the build's log and
   * the steps' (`withWarnings`).
   */
  warnings?: boolean;
  /**
   * An update listener, registered after `listen`'s, that logs
   * `['mutatedNodes', null | [[type, [[key, mutation]...]]...]]`, the
   * payload's `mutatedNodes` by class type.
   */
  mutatedNodes?: boolean;
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

/**
 * What `buildEditorFromExtensions` logged, then either the state, selection
 * and text of the built editor, as a step records them, or the message it
 * threw.
 */
export type BuildResult = {
  log: Json[];
  state?: string;
  compact?: string;
  selection?: Json;
  text?: string;
  error?: string;
};

export type ScriptResult = Script & {
  build?: BuildResult;
  results: StepResult[];
};

type Context = {
  editor: LexicalEditor;
  log: Json[];
  commands: Map<string, LexicalCommand<Json>>;
  /** The test extensions made so far, by key. */
  extensions: Map<string, AnyLexicalExtension>;
  defs: {[key: string]: ExtensionSpec};
  /** The `DataTransfer` of the last `copy`. */
  clipboard: ScriptDataTransfer | null;
  /** The editor states `saveEditorState` kept, by name. */
  saved: Map<string, EditorState>;
  /** The block handles `logBlockHandles` kept, by name. */
  handles: Map<string, Map<string, string>>;
  /** The session's jsdom window and root element (`setRootElement`). */
  dom: {window: ReturnType<typeof browserWindow>; root: HTMLElement} | null;
  /** What `characterLimit` and `tableOfContents` mounted, to unmount. */
  mounted: Map<string, () => void>;
  /** The disposers `registerNodeStructure` returned, by name. */
  structures: Map<string, () => void>;
  /** The script's `parent` editor. */
  parent: LexicalEditorWithDispose | null;
  /** The effects `watchNodeKey` made, by key. */
  watchers: Map<string, () => void>;
  /** The disposers `registerMarkIDsListener` returned, by name. */
  markIDsListeners: Map<string, () => void>;
};

/**
 * The module-private functions of `shared/useCharacterLimit.ts` that
 * `vitest.config.mts` re-exports, unchanged, from
 * `LexicalCharacterLimitPlugin.tsx`.
 */
type CharacterLimitInternals = {
  INTERNAL_useCharacterLimit: (
    editor: LexicalEditor,
    maxCharacters: number,
    options?: {
      remainingCharacters?: (characters: number) => void;
      strlen?: (input: string) => number;
    },
  ) => void;
  INTERNAL_findOffset: (
    source: string,
    maxCharacters: number,
    strlen: (input: string) => number,
  ) => number;
  INTERNAL_$wrapOverflowedNodes: (offset: number) => void;
  INTERNAL_$mergePrevious: (overflowNode: OverflowNode) => void;
};

const CHARACTER_LIMIT =
  CharacterLimitPlugin as unknown as CharacterLimitInternals;

/** LexicalEvents.ts' `onSelectionChange`, re-exported by vitest.config.mts. */
type EventsInternals = {
  INTERNAL_onSelectionChange: (
    domSelection: Selection,
    editor: LexicalEditor,
    isActive: boolean,
  ) => void;
};

const EVENTS_INTERNALS = LexicalEvents as unknown as EventsInternals;

/**
 * `DecoratorBlockNode` (@lexical/react) as an app's block embed extends it,
 * with nothing of its own: a block decorator with a `format`.
 */
export class TestDecoratorBlockNode extends DecoratorBlockNode {
  $config() {
    return this.config('test-decorator-block', {extends: DecoratorBlockNode});
  }

  decorate(): never {
    return null as never;
  }
}

/**
 * The session whose `TestTextNode`s log their `selectionTransform` (JS
 * calls the hook with no context of the session's).
 */
let selectionTransformCtx: Context | null = null;

/** `[anchorKey, anchorOffset, focusKey, focusOffset]` of a range selection. */
function rangeJson(selection: BaseSelection | null): Json {
  return $isRangeSelection(selection)
    ? [
        selection.anchor.key,
        selection.anchor.offset,
        selection.focus.key,
        selection.focus.offset,
      ]
    : null;
}

/**
 * `TestTextNode` (`test-text`): a `TextNode` whose `selectionTransform`
 * logs `['selectionTransform', key, prev, next]`, the previous and next
 * selections as `rangeJson`.
 */
export class TestTextNode extends TextNode {
  $config() {
    return this.config('test-text', {extends: TextNode});
  }

  selectionTransform(
    prevSelection: null | BaseSelection,
    nextSelection: RangeSelection,
  ): void {
    if (selectionTransformCtx !== null) {
      selectionTransformCtx.log.push([
        'selectionTransform',
        this.getKey(),
        rangeJson(prevSelection),
        rangeJson(nextSelection),
      ]);
    }
  }
}

/**
 * `{replace: TextNode, with, withKlass: TestTextNode}`, `with` making a
 * `TestTextNode` with the node's text; without `withKlass` when `legacy`.
 */
function textReplacement(legacy: boolean): LexicalNodeReplacement {
  const replacement: LexicalNodeReplacement = {
    replace: TextNode,
    with: node => new TestTextNode((node as TextNode).getTextContent()),
  };
  if (!legacy) {
    replacement.withKlass = TestTextNode;
  }
  return replacement;
}

/** The classes `logHasNodes` asks about, by type. */
const KNOWN_CLASSES = new Map<string, Klass<LexicalNode>>([
  ['heading', HeadingNode],
  ['link', LinkNode],
  ['paragraph', ParagraphNode],
  ['quote', QuoteNode],
  ['test-text', TestTextNode],
  ['text', TextNode],
]);

/** `f()`, or `'<throws>'` when it throws. */
function attempt(f: () => Json): Json {
  try {
    return f();
  } catch {
    return '<throws>';
  }
}

/**
 * The getters of a node, as `[name, value]` entries: for every node
 * `getTopLevelElement` and `getTopLevelElementOrThrow` (keys) and
 * `getParentKeys`; for an element `getChildrenKeys`, `isLastChild`,
 * `getFirstChildOrThrow`, `getLastChildOrThrow` (keys), `hasTextFormat`
 * and `getFormatFlags` of `bold`, and `canReplaceWith` and
 * `canInsertAfter` of itself; for a text node `getFormatFlags` of `bold`
 * and of `italic` aligned with format 2, and `isDirectionless`.
 */
function $getters(node: LexicalNode): Json {
  const entries: Json[] = [
    [
      'topLevelElement',
      attempt(() => {
        const top = node.getTopLevelElement();
        return top === null ? null : top.getKey();
      }),
    ],
    [
      'topLevelElementOrThrow',
      attempt(() => node.getTopLevelElementOrThrow().getKey()),
    ],
    ['parentKeys', node.getParentKeys()],
  ];
  if ($isElementNode(node)) {
    entries.push(
      ['childrenKeys', node.getChildrenKeys()],
      ['isLastChild', attempt(() => node.isLastChild())],
      [
        'firstChildOrThrow',
        attempt(() => node.getFirstChildOrThrow().getKey()),
      ],
      ['lastChildOrThrow', attempt(() => node.getLastChildOrThrow().getKey())],
      ['hasTextFormat', node.hasTextFormat('bold')],
      ['formatFlags', node.getFormatFlags('bold', null)],
      ['canReplaceWith', node.canReplaceWith(node)],
      ['canInsertAfter', node.canInsertAfter(node)],
    );
  }
  if ($isTextNode(node)) {
    entries.push(
      ['formatFlags', node.getFormatFlags('bold', null)],
      ['alignedFormatFlags', node.getFormatFlags('italic', 2)],
      ['isDirectionless', node.isDirectionless()],
    );
  }
  return entries;
}

/**
 * Runs `f` with each `console.warn` logged as `['warn', message]` in the
 * session's log, when the script asks for it.
 */
function withWarnings<T>(ctx: Context, on: boolean, f: () => T): T {
  if (!on) {
    return f();
  }
  const warn = console.warn;
  console.warn = (...args: unknown[]) => {
    ctx.log.push(['warn', args.map(String).join(' ')]);
  };
  try {
    return f();
  } finally {
    console.warn = warn;
  }
}

/**
 * `test-decorator`: an inline decorator whose `decorate()` returns a new
 * object on every call, `{decorator: 'fresh', key}`, as a React decorator
 * returns a new element.
 */
export class TestDecoratorNode extends DecoratorNode<Json> {
  $config() {
    return this.config('test-decorator', {extends: DecoratorNode});
  }

  createDOM(): HTMLElement {
    return $getDocument().createElement('span');
  }

  updateDOM(): false {
    return false;
  }

  decorate(): Json {
    return {decorator: 'fresh', key: this.__key};
  }
}

/** What every `TestSharedDecoratorNode` decorates to. */
const SHARED_DECORATION: Json = {decorator: 'shared'};

/**
 * `test-shared-decorator`: an inline decorator whose `decorate()` returns
 * one object, `{decorator: 'shared'}`, for every node and call.
 */
export class TestSharedDecoratorNode extends DecoratorNode<Json> {
  $config() {
    return this.config('test-shared-decorator', {extends: DecoratorNode});
  }

  createDOM(): HTMLElement {
    return $getDocument().createElement('span');
  }

  updateDOM(): false {
    return false;
  }

  decorate(): Json {
    return SHARED_DECORATION;
  }
}

/** The entries of a decorator map, in `Object.entries` order. */
function decoratorEntries(decorators: Record<string, unknown>): Json {
  return Object.entries(decorators).map(([key, value]) => [key, value as Json]);
}

/** `flags`: a set NodeState of `TestSchemaNode`, kept in canonical order. */
const testSchemaFlagsState = createState('flags', {
  parse: arraySetValue(stringValue()),
});

const testSchemaSchema = nodeSchema<TestSchemaNode>()({
  count: withField(
    numberValue(1, {clamp: true, integer: true, max: 10, min: 0}),
    {field: '__count'},
  ),
  kind: withField(
    aliasedValue(enumValue(['a', 'b']), {alpha: 'a', beta: 'b'}),
    {field: '__kind'},
  ),
  label: withAccessors(stringValue(), {setter: null}),
  note: withField(optionalValue(stringValue()), {field: '__note'}),
  shown: withField(stringValue(), {field: '__shown', when: 'isShown'}),
  tags: withField(arrayValue(stringValue()), {field: '__tags'}),
  width: withAccessors(nullable(numberValue()), {}),
});

/**
 * `test-schema`: a block element whose `$config` declares a `json` schema of
 * every kind of property, for serialization/schema-node: fields with clamped,
 * aliased, optional and array values, a field written only `when` a
 * predicate holds, accessor properties (a nullable `width`, an export-only
 * `label`), and an `arraySetValue` NodeState.
 */
export class TestSchemaNode extends ElementNode {
  __count: number = 1;
  __kind: 'a' | 'b' = 'a';
  __note: string | undefined = undefined;
  __shown: string = '';
  __width: number | null = null;
  __tags: readonly string[] = [];
  $config() {
    return this.config('test-schema', {
      extends: ElementNode,
      json: testSchemaSchema,
      stateConfigs: [testSchemaFlagsState],
    });
  }
  /**
   * `width` is declared through accessors, which `$config` does not carry
   * across a clone, so the class copies it; and a class's own
   * `afterCloneFrom` replaces the one `$config` derives for its `withField`
   * properties, so it copies those too.
   */
  afterCloneFrom(prevNode: this): void {
    super.afterCloneFrom(prevNode);
    this.__count = prevNode.__count;
    this.__kind = prevNode.__kind;
    this.__note = prevNode.__note;
    this.__shown = prevNode.__shown;
    this.__tags = prevNode.__tags;
    this.__width = prevNode.__width;
  }
  /** Whether `shown` is written: only for a count above 1. */
  isShown(): boolean {
    return this.__count > 1;
  }
  /** `label`, computed from `kind`: written, never read back. */
  getLabel(): string {
    return this.getLatest().__kind.toUpperCase();
  }
  getWidth(): number | null {
    return this.getLatest().__width;
  }
  setWidth(width: number | null): this {
    const self = this.getWritable();
    self.__width = width;
    return self;
  }
}

// The node classes of scripts/node-structure, for the structure rules the
// packages' own nodes don't declare: a block container (`children:
// 'block'`), an inline element merged by a schema field and a NodeState,
// derived values, and subclasses that inherit those declarations.

/** `color`: a NodeState that `TestTagNode`'s `mergeAdjacent` names. */
const testTagColorState = createState('color', {
  parse: (v: unknown) => (typeof v === 'string' ? v : ''),
});

/** `count`: a derived NodeState of `TestTagNode`. */
const testTagCountState = createState('count', {
  derived: true,
  parse: (v: unknown) => (typeof v === 'number' ? v : 0),
});

const testTagSchema = nodeSchema<TestTagNode>()({
  label: withField(stringValue(), {derived: true, field: '__label'}),
  tag: withField(stringValue(), {field: '__tag'}),
});

/** `test-box`: a block that holds blocks (`children: 'block'`), a callout. */
export class TestBoxNode extends ElementNode {
  $config() {
    return this.config('test-box', {children: 'block', extends: ElementNode});
  }
}

/** `test-sub-box`: a `TestBoxNode` that declares nothing of its own. */
export class TestSubBoxNode extends TestBoxNode {
  $config() {
    return this.config('test-sub-box', {extends: TestBoxNode});
  }
}

/**
 * `test-tag`: an inline element holding inline content, which cannot be
 * empty; equal adjacent ones (same `tag`, same `color` state) merge. Its
 * `label` field and `count` state are derived.
 */
export class TestTagNode extends ElementNode {
  __tag: string = '';
  __label: string = '';
  $config() {
    return this.config('test-tag', {
      children: 'inline',
      extends: ElementNode,
      inline: true,
      json: testTagSchema,
      mergeAdjacent: {by: ['tag', testTagColorState]},
      stateConfigs: [testTagColorState, testTagCountState],
    });
  }
  canBeEmpty(): false {
    return false;
  }
}

/**
 * `test-sub-tag`: a `TestTagNode` that inherits its declarations, so the
 * normalization never merges two of them (a subclass merges by its own code).
 */
export class TestSubTagNode extends TestTagNode {
  $config() {
    return this.config('test-sub-tag', {extends: TestTagNode});
  }
}

/**
 * `getDeclaredNodeStructure(klass)` as `logNodeStructure` logs it: `inline`,
 * `children` (`'inline'`, `'block'`, or the list with each class as its
 * type), `mergeAdjacent` (its `by`, a NodeState as `{state: key}`), each when
 * declared, then `derived` (sorted) and `declaredBy` (each class as its type).
 */
function STRUCTURE_JSON(klass: Klass<LexicalNode>): Json {
  const declared = getDeclaredNodeStructure(klass);
  const children = (c: NodeChildrenConfig): Json =>
    typeof c === 'string' ? c : c.map(e => (e === 'inline' ? e : e.getType()));
  const out: {[key: string]: Json} = {};
  if (declared.inline !== undefined) {
    out.inline = declared.inline;
  }
  if (declared.children !== undefined) {
    out.children = children(declared.children);
  }
  if (declared.mergeAdjacent !== undefined) {
    out.mergeAdjacent = declared.mergeAdjacent.by.map(e =>
      typeof e === 'string' ? e : {state: e.key},
    );
  }
  out.derived = [...declared.derived].sort();
  const by: {[key: string]: Json} = {};
  for (const key of ['inline', 'children', 'mergeAdjacent'] as const) {
    const k = declared.declaredBy[key];
    if (k !== undefined) {
      by[key] = k.getType();
    }
  }
  out.declaredBy = by;
  return out;
}

/** The class `editor` registers under `type`. */
function $registeredClass(type: string): Klass<LexicalNode> {
  const registered = $getEditor()._nodes.get(type);
  if (registered === undefined) {
    throw new Error(`unknown node type ${type}`);
  }
  return registered.klass;
}

/**
 * The functions of @lexical/react's menu modules that they don't export,
 * which `vitest.config.mts` re-exports as they are from
 * `LexicalTypeaheadMenuPlugin.tsx`.
 */
type ReactInternals = {
  INTERNAL_getTextUpToAnchor: (selection: RangeSelection) => string | null;
  INTERNAL_isSelectionOnEntityBoundary: (
    editor: LexicalEditor,
    offset: number,
  ) => boolean;
  INTERNAL_getFullMatchOffset: (
    documentText: string,
    entryText: string,
    offset: number,
  ) => number;
  INTERNAL_$splitNodeContainingQuery: (match: MenuTextMatch) => TextNode | null;
};

const REACT_INTERNALS = TypeaheadMenuPlugin as unknown as ReactInternals;

/**
 * The trigger function `useBasicTypeaheadTriggerMatch` returns for `spec`,
 * from a render of a component that calls the hook.
 */
function typeaheadTrigger(spec: TypeaheadSpec): TriggerFn {
  const {trigger, ...options} = spec;
  let triggerFn: TriggerFn | null = null;
  renderToStaticMarkup(
    createElement(() => {
      triggerFn = useBasicTypeaheadTriggerMatch(trigger, {
        punctuation: PUNCTUATION,
        ...options,
      });
      return null;
    }),
  );
  if (triggerFn === null) {
    throw new Error('useBasicTypeaheadTriggerMatch did not run');
  }
  return triggerFn;
}

/** The text before the caret and its match (`null`s past a `null`). */
function $typeaheadMatch(
  spec: TypeaheadSpec,
  editor: LexicalEditor,
): [string | null, MenuTextMatch | null] {
  const selection = $getSelection();
  const textBefore = $isRangeSelection(selection)
    ? REACT_INTERNALS.INTERNAL_getTextUpToAnchor(selection)
    : null;
  return [
    textBefore,
    textBefore === null ? null : typeaheadTrigger(spec)(textBefore, editor),
  ];
}

/** The jsdom window and root element of the session, made on first use. */
function sessionDOM(ctx: Context): NonNullable<Context['dom']> {
  if (ctx.dom === null) {
    const win = browserWindow();
    ctx.dom = {root: win.document.createElement('div'), window: win};
    // As in a browser, the listeners the packages add to the session's DOM
    // (TableObserver's, with an abort signal) take the window's own
    // AbortSignal; runScript puts Node's back.
    globalThis.AbortController = win.AbortController;
  }
  return ctx.dom;
}

/** jsdom's own `getComputedStyle` of each window `computedStyles` stubbed. */
const jsdomGetComputedStyle = new WeakMap<
  object,
  (el: Element, pseudo?: string | null) => CSSStyleDeclaration
>();

/** The `computedStyles` step: stubs `win.getComputedStyle`. */
function stubComputedStyles(
  win: ReturnType<typeof browserWindow>,
  editor: LexicalEditor,
  styles: {[key: string]: {direction?: string; writingMode?: string}},
): void {
  let jsdom = jsdomGetComputedStyle.get(win);
  if (jsdom === undefined) {
    jsdom = win.getComputedStyle.bind(win);
    jsdomGetComputedStyle.set(win, jsdom);
  }
  const fallback = jsdom;
  win.getComputedStyle = (el: Element, pseudo?: string | null) => {
    for (const [key, style] of Object.entries(styles)) {
      if (editor.getElementByKey(key) === el) {
        return {
          direction: style.direction ?? 'ltr',
          writingMode: style.writingMode ?? 'horizontal-tb',
        } as CSSStyleDeclaration;
      }
    }
    return fallback(el, pseudo);
  };
}

/**
 * The jsdom globals the reconciler reads (`Node`, `MutationObserver`...),
 * which Node lacks, installed while `f` runs once the session has a DOM.
 */
const DOM_GLOBALS = [
  'Node',
  'Element',
  'HTMLElement',
  'Text',
  'DocumentFragment',
  'MutationObserver',
] as const;

function withDOMGlobals<T>(ctx: Context, f: () => T): T {
  if (ctx.dom === null) {
    return f();
  }
  const globals = globalThis as Record<string, unknown>;
  const win = ctx.dom.window as unknown as Record<string, unknown>;
  const saved = DOM_GLOBALS.map(name => globals[name]);
  for (const name of DOM_GLOBALS) {
    globals[name] = win[name];
  }
  try {
    return f();
  } finally {
    DOM_GLOBALS.forEach((name, i) => {
      globals[name] = saved[i];
    });
  }
}

/** `['liveRegion', regions]` of `Script['liveRegion']`. */
function liveRegionEntry(ctx: Context): Json {
  const regions =
    ctx.dom === null
      ? []
      : [...ctx.dom.window.document.body.querySelectorAll('[role="status"]')];
  return [
    'liveRegion',
    regions.map(el => [el.getAttribute('aria-live'), el.textContent]),
  ];
}

/** Mounts `characterLimit` or `tableOfContents` (see `Step`). */
function mountStep(
  step: Extract<Step, {characterLimit: unknown} | {tableOfContents: true}>,
  ctx: Context,
): void {
  const {editor} = ctx;
  if ('characterLimit' in step) {
    const {maxCharacters, strlen} = step.characterLimit;
    const remainingCharacters = (characters: number) => {
      ctx.log.push(['remainingCharacters', characters]);
    };
    ctx.mounted.set(
      'characterLimit',
      mountHooks(
        editor,
        () => {},
        () =>
          CHARACTER_LIMIT.INTERNAL_useCharacterLimit(
            editor,
            maxCharacters,
            strlen === undefined
              ? {remainingCharacters}
              : {remainingCharacters, strlen: STRLEN[strlen]},
          ),
      ),
    );
  } else {
    ctx.mounted.set(
      'tableOfContents',
      mountHooks(
        editor,
        value => {
          ctx.log.push([
            'tableOfContents',
            (value as TableOfContentsEntry[]).map(([key, content, tag]) => [
              key,
              content,
              tag,
            ]),
          ]);
        },
        () => TableOfContentsPlugin({children: () => null as never}),
      ),
    );
  }
}

/**
 * A headless `DataTransfer` (Node has none): the data by format, in the
 * order it was first set, with the format lowercased and `text` and `url`
 * read as `text/plain` and `text/uri-list`, as browsers do. The Swift and
 * Rust ports have the same one.
 */
class ScriptDataTransfer {
  entries: [string, string][] = [];
  /** `files`, which `types` reports as `Files`, last. */
  fileList: File[] = [];

  static format(format: string): string {
    const lower = format.toLowerCase();
    return lower === 'text'
      ? 'text/plain'
      : lower === 'url'
        ? 'text/uri-list'
        : lower;
  }

  get types(): string[] {
    const types = this.entries.map(([type]) => type);
    return this.fileList.length > 0 ? [...types, 'Files'] : types;
  }

  get files(): File[] {
    return this.fileList;
  }

  getData(format: string): string {
    const type = ScriptDataTransfer.format(format);
    const entry = this.entries.find(([t]) => t === type);
    return entry === undefined ? '' : entry[1];
  }

  setData(format: string, data: string): void {
    const type = ScriptDataTransfer.format(format);
    const entry = this.entries.find(([t]) => t === type);
    if (entry === undefined) {
      this.entries.push([type, data]);
    } else {
      entry[1] = data;
    }
  }

  static of(data: Record<string, string>): ScriptDataTransfer {
    const dataTransfer = new ScriptDataTransfer();
    for (const [type, value] of Object.entries(data)) {
      dataTransfer.setData(type, value);
    }
    return dataTransfer;
  }

  asDataTransfer(): DataTransfer {
    return this as unknown as DataTransfer;
  }
}

/**
 * `$getClipboardDataFromSelection(selection)` with its default MIME types but
 * `text/html`, whose `$getHtmlContent` needs a DOM: the same steps
 * (`$getClipboardDataWithConfigFromSelection` over
 * `DEFAULT_EXPORT_MIME_TYPE`), in its key order.
 */
function $clipboardDataWithoutHtml(
  editor: LexicalEditor,
  selection: ReturnType<typeof $getSelection>,
): LexicalClipboardData {
  const clipboardData: LexicalClipboardData = {'text/plain': ''};
  if (selection !== null) {
    const lexical = $getLexicalContent(editor, selection);
    if (lexical !== null) {
      clipboardData['application/x-lexical-editor'] = lexical;
    }
    clipboardData['text/plain'] = selection.getTextContent();
  }
  return clipboardData;
}

function $node(key: string): LexicalNode {
  const node = $getNodeByKey(key);
  if (node === null) {
    throw new Error(`no node ${key}`);
  }
  return node;
}

/** The node `key` when `value` is a string, else `value` parsed. */
function $nodeArg(value: Json): LexicalNode {
  return typeof value === 'string'
    ? $node(value)
    : $parseSerializedNode(value as unknown as SerializedLexicalNode);
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
  if ($isTableSelection(selection)) {
    const {anchor, focus} = selection;
    return [
      'table',
      selection.tableKey,
      [anchor.key, anchor.offset, anchor.type],
      [focus.key, focus.offset, focus.type],
    ];
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

/** `$create(block)`: a new paragraph, quote or heading. */
function $createBlock(block: BlockType): ElementNode {
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
}

/** The mark flavor of REAL_EXTENSIONS named `key`, if any (MARK_FLAVORS). */
function markFlavor(
  key: string | undefined,
): AnyMarkFlavorExtension | undefined {
  if (key === undefined) {
    return undefined;
  }
  const flavor = MARK_FLAVORS.get(key);
  if (flavor === undefined) {
    throw new Error(`unknown mark flavor ${key}`);
  }
  return flavor;
}

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

function markNode(node: LexicalNode): MarkNode {
  if (!$isMarkNode(node)) {
    throw new Error(`${node.getKey()} is not a mark node`);
  }
  return node;
}

function listItem(node: LexicalNode): ListItemNode {
  if (!$isListItemNode(node)) {
    throw new Error(`${node.getKey()} is not a list item`);
  }
  return node;
}

function codeNode(node: LexicalNode): CodeNode {
  if (!$isCodeNode(node)) {
    throw new Error(`${node.getKey()} is not a code node`);
  }
  return node;
}

/**
 * The function of `@lexical/code-prism`'s `FacadePrism.ts` that it doesn't
 * export, which `vitest.config.mts` re-exports from its `index.ts`.
 */
type PrismInternals = {
  INTERNAL_$mapTokensToLexicalStructure: (
    tokens: (string | TokenJson)[],
    type?: string,
  ) => LexicalNode[];
};

const PRISM_INTERNALS = CodePrism as unknown as PrismInternals;

const CONFORMANCE_KEYWORDS = new Set(['fn', 'if', 'let', 'return']);

function isConformanceSpace(c: string): boolean {
  return c === ' ' || c === '\t' || c === '\n' || c === '\r';
}

/** An ASCII letter, digit or `_`, or any code point past ASCII. */
function isConformanceWord(c: string): boolean {
  return /^[A-Za-z0-9_]$/.test(c) || c.codePointAt(0)! >= 0x80;
}

/**
 * CONFORMANCE_TOKENIZE's grammar of every language but `diff`, over code
 * points. A run of spaces, tabs, `\n` and `\r` is a string. A run of word
 * code points is a `keyword` (CONFORMANCE_KEYWORDS) or a `number` (digits
 * only) with string content, or a `word` whose content is `[run]`. `"`
 * starts a `string` up to the next `"` or the end, whose content is `'"'`,
 * the text between escapes, each `\` with the code point after it as an
 * `escape`, and the closing `'"'`. `#` starts a `comment` up to the end of
 * the line. Any other code point is a `punctuation`.
 */
function conformanceWords(code: string): TokenJson[] {
  const chars = [...code];
  const out: TokenJson[] = [];
  let i = 0;
  const runOf = (test: (c: string) => boolean): string => {
    let j = i;
    while (j < chars.length && test(chars[j])) {
      j++;
    }
    const run = chars.slice(i, j).join('');
    i = j;
    return run;
  };
  while (i < chars.length) {
    const c = chars[i];
    if (isConformanceSpace(c)) {
      out.push(runOf(isConformanceSpace));
    } else if (isConformanceWord(c)) {
      const run = runOf(isConformanceWord);
      if (CONFORMANCE_KEYWORDS.has(run)) {
        out.push({content: run, type: 'keyword'});
      } else if (/^[0-9]+$/.test(run)) {
        out.push({content: run, type: 'number'});
      } else {
        out.push({content: [run], type: 'word'});
      }
    } else if (c === '"') {
      const content: TokenJson[] = ['"'];
      i++;
      let run = '';
      while (i < chars.length && chars[i] !== '"') {
        if (chars[i] === '\\' && i + 1 < chars.length) {
          if (run !== '') {
            content.push(run);
            run = '';
          }
          content.push({content: '\\' + chars[i + 1], type: 'escape'});
          i += 2;
        } else {
          run += chars[i++];
        }
      }
      if (run !== '') {
        content.push(run);
      }
      if (i < chars.length) {
        content.push('"');
        i++;
      }
      out.push({content, type: 'string'});
    } else if (c === '#') {
      out.push({content: runOf(d => d !== '\n'), type: 'comment'});
    } else {
      out.push({content: c, type: 'punctuation'});
      i++;
    }
  }
  return out;
}

/**
 * The tokens of `code` in `language` by the conformance tokenizer, which
 * every runner implements. `diff` is line by line, with `\n` between lines
 * as a string, as Prism's grammar makes it: a line starting with `+`, `-`
 * or a space is an `inserted`, `deleted` or `unchanged` token whose
 * content is a `prefix` token of that code point (with string content,
 * aliased to the line's type) and the words of the rest; with `!`, a
 * `changed` token of a `prefix` without an alias and the words of the
 * rest; and any other line is its words. Every other language is
 * `conformanceWords`.
 */
export function CONFORMANCE_TOKENIZE(
  code: string,
  language: string,
): TokenJson[] {
  if (language !== 'diff') {
    return conformanceWords(code);
  }
  const types: Record<string, string> = {
    ' ': 'unchanged',
    '!': 'changed',
    '+': 'inserted',
    '-': 'deleted',
  };
  const out: TokenJson[] = [];
  code.split('\n').forEach((line, i) => {
    if (i > 0) {
      out.push('\n');
    }
    const type = line === '' ? undefined : types[line[0]];
    if (type === undefined) {
      out.push(...conformanceWords(line));
      return;
    }
    const prefix: TokenJson =
      type === 'changed'
        ? {content: line[0], type: 'prefix'}
        : {alias: type, content: line[0], type: 'prefix'};
    out.push({content: [prefix, ...conformanceWords(line.slice(1))], type});
  });
  return out;
}

/** A Prism token as JSON (`TokenJson`). */
function prismTokenJson(token: string | PrismToken): TokenJson {
  if (typeof token === 'string') {
    return token;
  }
  const {content, alias, type} = token;
  if (typeof content !== 'string' && !Array.isArray(content)) {
    throw new Error('a Prism token with a token as its content');
  }
  return {
    ...(typeof alias === 'string' ? {alias} : {}),
    content:
      typeof content === 'string'
        ? content
        : (content as (string | PrismToken)[]).map(prismTokenJson),
    type,
  };
}

type PrismToken = {type: string; content: unknown; alias: unknown};

/**
 * The `Tokenizer` of a `codeHighlight` spec, whose `$tokenize` is
 * `PrismTokenizer.$tokenize` with `$getHighlightNodes(codeNode, lang)` in
 * its general form: `$mapTokensToLexicalStructure` of `this.tokenize` of the
 * block's text (without the `diff-<language>` grammars of Prism).
 */
function scriptTokenizer(spec: CodeHighlightSpec): Tokenizer {
  const languages = new Set(spec.languages);
  for (const language of languages) {
    if (!CodePrism.isCodeLanguageLoaded(language)) {
      throw new Error(`Prism has no ${language}`);
    }
  }
  const tokenize = (code: string, language: string): TokenJson[] => {
    if (!languages.has(language)) {
      throw new Error(`the tokenizer has no ${language}`);
    }
    switch (spec.tokenizer) {
      case 'conformance':
        return CONFORMANCE_TOKENIZE(code, language);
      case 'recorded': {
        const entry = (spec.tokens ?? []).find(
          ([l, c]) => l === language && c === code,
        );
        if (entry === undefined) {
          throw new Error(`no tokens recorded for ${language}: ${code}`);
        }
        return entry[2];
      }
      case 'prism': {
        const tokens = PrismTokenizer.tokenize(code, language).map(token =>
          prismTokenJson(token as string | PrismToken),
        );
        spec.tokens ??= [];
        if (!spec.tokens.some(([l, c]) => l === language && c === code)) {
          spec.tokens.push([language, code, tokens]);
        }
        return tokens;
      }
    }
  };
  return {
    $tokenize(this: Tokenizer, node: CodeNode, language?: string) {
      const lang = language || this.defaultLanguage;
      return lang === null
        ? $plainifyCodeContent(node.getTextContent())
        : PRISM_INTERNALS.INTERNAL_$mapTokensToLexicalStructure(
            this.tokenize(node.getTextContent(), lang) as TokenJson[],
          );
    },
    defaultLanguage:
      spec.defaultLanguage === undefined
        ? DEFAULT_CODE_LANGUAGE
        : spec.defaultLanguage,
    tokenize: (code: string, language?: string) =>
      tokenize(code, language ?? '') as never,
  };
}

/**
 * The node methods a script may call, by name. A method that exists on both
 * text and element nodes (`select`, `setFormat`, `setStyle`) calls the one of
 * the node's kind. The return value is logged only for `splitText` and the
 * methods that read (`hasID`, `getIDs`, `extractWithChild`,
 * `excludeFromCopy`).
 */
const NODE_METHODS: Record<
  string,
  (node: LexicalNode, args: Json[] | undefined, ctx: Context) => void
> = {
  addID: (n, args) => markNode(n).addID(arg(args, 0) as string),
  append: (n, args) =>
    element(n).append(...$parse(arg(args, 0) as SerializedLexicalNode[])),
  clear: n => element(n).clear(),
  deleteID: (n, args) => markNode(n).deleteID(arg(args, 0) as string),
  // `excludeFromCopy(destination)`, logged.
  excludeFromCopy: (n, args, ctx) => {
    const destination = arg(args, 0) as 'clone' | 'html';
    ctx.log.push([
      'excludeFromCopy',
      n.getKey(),
      destination,
      element(n).excludeFromCopy(destination),
    ]);
  },
  // `extractWithChild(child, $getSelection(), destination)`, logged.
  extractWithChild: (n, args, ctx) => {
    const child = arg(args, 0) as string;
    const destination = arg(args, 1) as 'clone' | 'html';
    ctx.log.push([
      'extractWithChild',
      n.getKey(),
      child,
      destination,
      element(n).extractWithChild($node(child), $anySel(), destination),
    ]);
  },
  getIDs: (n, _args, ctx) => {
    ctx.log.push(['getIDs', n.getKey(), markNode(n).getIDs()]);
  },
  hasID: (n, args, ctx) => {
    const id = arg(args, 0) as string;
    ctx.log.push(['hasID', n.getKey(), id, markNode(n).hasID(id)]);
  },
  // A string names an existing node, which moves.
  insertAfter: (n, args) =>
    n.insertAfter($nodeArg(arg(args, 0)), optional<boolean>(arg(args, 1))),
  insertBefore: (n, args) =>
    n.insertBefore($nodeArg(arg(args, 0)), optional<boolean>(arg(args, 1))),
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
    } else if ($isDecoratorBlockNode(n)) {
      n.setFormat(arg(args, 0) as never);
    } else {
      element(n).setFormat(arg(args, 0) as never);
    }
  },
  setIDs: (n, args) => markNode(n).setIDs(arg(args, 0) as string[]),
  setIndent: (n, args) => element(n).setIndent(arg(args, 0) as number),
  // `CodeNode.setIsSyntaxHighlightSupported(isSupported)`.
  setIsSyntaxHighlightSupported: (n, args) =>
    codeNode(n).setIsSyntaxHighlightSupported(arg(args, 0) as boolean),
  // `CodeNode.setLanguage(language)`; `null` is `undefined`.
  setLanguage: (n, args) =>
    codeNode(n).setLanguage(optional<string>(arg(args, 0))),
  // `ListNode.setListType(type)`.
  setListType: (n, args) => {
    if (!$isListNode(n)) {
      throw new Error(`${n.getKey()} is not a list`);
    }
    n.setListType(arg(args, 0) as ListType);
  },
  setMode: (n, args) => text(n).setMode(arg(args, 0) as never),
  setStyle: (n, args) => {
    if ($isTextNode(n)) {
      n.setStyle(arg(args, 0) as string);
    } else {
      element(n).setStyle(arg(args, 0) as string);
    }
  },
  setTag: (n, args) => {
    if (!$isHeadingNode(n)) {
      throw new Error(`${n.getKey()} is not a heading`);
    }
    n.setTag(arg(args, 0) as HeadingTagType);
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

function tableNode(node: LexicalNode): TableNode {
  if (!$isTableNode(node)) {
    throw new Error(`${node.getKey()} is not a table`);
  }
  return node;
}

function tableCell(node: LexicalNode): TableCellNode {
  if (!$isTableCellNode(node)) {
    throw new Error(`${node.getKey()} is not a table cell`);
  }
  return node;
}

const keyOf = (node: LexicalNode | null): Json =>
  node === null ? null : node.getKey();

/**
 * @lexical/table's `$` functions, by name without the `$`, with node keys
 * for their nodes. Each returns what is logged: a node as its key, a table
 * map as rows of `[cellKey, startRow, startColumn]`.
 */
const TABLE_FUNCTIONS: Record<string, (args: Json[]) => Json | void> = {
  computeTableMap: ([table]) =>
    $computeTableMapSkipCellCheck(
      tableNode($node(table as string)),
      null,
      null,
    )[0].map(row =>
      row.map(value =>
        value === undefined || value === null
          ? null
          : [value.cell.getKey(), value.startRow, value.startColumn],
      ),
    ),
  deleteTableColumnAtSelection: () => $deleteTableColumnAtSelection(),
  deleteTableRowAtSelection: () => $deleteTableRowAtSelection(),
  findCellNode: ([key]) => keyOf($findCellNode($node(key as string))),
  findTableNode: ([key]) => keyOf($findTableNode($node(key as string))),
  getTableCellNodeRect: ([key]) => {
    const rect = $getTableCellNodeRect(tableCell($node(key as string)));
    return rect && {...rect};
  },
  getTableColumnIndexFromTableCellNode: ([key]) =>
    $getTableColumnIndexFromTableCellNode(tableCell($node(key as string))),
  getTableRowIndexFromTableCellNode: ([key]) =>
    $getTableRowIndexFromTableCellNode(tableCell($node(key as string))),
  insertTableColumnAtNode: ([key, insertAfter]) =>
    keyOf(
      $insertTableColumnAtNode(
        tableCell($node(key as string)),
        insertAfter as boolean,
      ),
    ),
  insertTableColumnAtSelection: ([insertAfter]) =>
    keyOf($insertTableColumnAtSelection(insertAfter as boolean)),
  insertTableRowAtNode: ([key, insertAfter]) =>
    keyOf(
      $insertTableRowAtNode(
        tableCell($node(key as string)),
        insertAfter as boolean,
      ),
    ),
  insertTableRowAtSelection: ([insertAfter]) =>
    keyOf($insertTableRowAtSelection(insertAfter as boolean)),
  isSimpleTable: ([key]) => $isSimpleTable(tableNode($node(key as string))),
  mergeCells: ([keys]) =>
    keyOf($mergeCells((keys as string[]).map(k => tableCell($node(k))))),
  moveTableColumn: ([key, from, to]) =>
    $moveTableColumn(
      tableNode($node(key as string)),
      from as number,
      to as number,
    ),
  moveTableRow: ([key, from, to]) =>
    $moveTableRow(
      tableNode($node(key as string)),
      from as number,
      to as number,
    ),
  padTableRows: ([key]) => $padTableRows(tableNode($node(key as string))),
  selectionShape: () => {
    const selection = $getSelection();
    if (!$isTableSelection(selection)) {
      throw new Error('not a table selection');
    }
    return {...selection.getShape()};
  },
  setTableColumnIsHeader: ([key, index, isHeader]) =>
    $setTableColumnIsHeader(
      tableNode($node(key as string)),
      index as number,
      isHeader as boolean,
    ),
  setTableRowIsHeader: ([key, index, isHeader]) =>
    $setTableRowIsHeader(
      tableNode($node(key as string)),
      index as number,
      isHeader as boolean,
    ),
  unmergeCell: () => $unmergeCell(),
};

function $runAction(action: Action, ctx: Context): void {
  switch (action.do) {
    case 'formatKeyboardShortcut':
      ctx.log.push([
        'formatKeyboardShortcut',
        formatKeyboardShortcut(
          scriptShortcutMatch(action.shortcut),
          action.isApple === undefined ? {} : {isApple: action.isApple},
        ),
      ]);
      return;
    case 'fixFocusOverselection':
      getExtensionDependencyFromEditor(
        $getEditor(),
        NormalizeTripleClickSelectionExtension,
      ).output.$fixFocusOverselection.peek()();
      return;
    case 'append':
      element($node(action.key)).append(...$parse(action.nodes));
      return;
    case 'selectionChange': {
      const editor = $getEditor();
      const focusSpec = action.focus ?? action.anchor;
      // What $internalCreateRangeSelection resolves from the DOM: a new
      // selection at the points with the format and style of the editor
      // state's (the last committed) selection.
      const last = editor.getEditorState()._selection;
      const selection = $createRangeSelection();
      if ($isRangeSelection(last)) {
        selection.format = last.format;
        selection.style = last.style;
      }
      selection.anchor.set(...action.anchor);
      selection.focus.set(...focusSpec);
      $setSelection(selection);
      selection.dirty = false;
      // The DOM boundary points of the model points: a text point in its
      // text node, an element point in its element.
      const domPoint = ([key, offset, type]: PointSpec): [Node, number] => {
        const dom = editor.getElementByKey(key);
        if (dom === null) {
          throw new Error(`no element for ${key}`);
        }
        return [type === 'text' ? (dom.firstChild ?? dom) : dom, offset];
      };
      const [anchorNode, anchorOffset] = domPoint(action.anchor);
      const [focusNode, focusOffset] = domPoint(focusSpec);
      const domSelection = {
        anchorNode,
        anchorOffset,
        focusNode,
        focusOffset,
        rangeCount: 0,
        type: selection.isCollapsed() ? 'Caret' : 'Range',
      } as unknown as Selection;
      EVENTS_INTERNALS.INTERNAL_onSelectionChange(domSelection, editor, true);
      return;
    }
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
    case 'selectTable':
      $setSelection(
        $createTableSelectionFrom(
          tableNode($node(action.table)),
          tableCell($node(action.anchor)),
          tableCell($node(action.focus)),
        ),
      );
      return;
    case 'table': {
      const fn = TABLE_FUNCTIONS[action.fn];
      if (fn === undefined) {
        throw new Error(`unknown table function ${action.fn}`);
      }
      const result = fn(action.args ?? []);
      if (result !== undefined) {
        ctx.log.push(['table', action.fn, result]);
      }
      return;
    }
    case 'selectAll':
      if (action.scoped) {
        $selectAll($sel());
      } else {
        $selectAll();
      }
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
      $setBlocksType($getSelection(), () => $createBlock(block));
      return;
    }
    case 'logSelectionText':
      ctx.log.push(['selectionText', $anySel().getTextContent()]);
      return;
    case 'logRootTextContent':
      ctx.log.push(['rootTextContent', $rootTextContent()]);
      return;
    case 'logRootTextContentEmpty': {
      const {isComposing, trim} = action;
      ctx.log.push([
        'rootTextContentEmpty',
        isComposing,
        trim ?? null,
        $isRootTextContentEmpty(isComposing, trim),
        $isRootTextContentEmptyCurry(isComposing, trim)(),
      ]);
      return;
    }
    case 'logCanShowPlaceholder': {
      const {isComposing} = action;
      ctx.log.push([
        'canShowPlaceholder',
        isComposing,
        $canShowPlaceholder(isComposing),
        $canShowPlaceholderCurry(isComposing)(),
      ]);
      return;
    }
    case 'logTextIntersection': {
      const found = $findTextIntersectionFromCharacters(
        $getRoot(),
        action.characters,
      );
      ctx.log.push([
        'textIntersection',
        action.characters,
        found === null ? null : [found.node.getKey(), found.offset],
      ]);
      return;
    }
    case 'wrapSelectionInMarkNode':
      $wrapSelectionInMarkNode($sel(), action.isBackward, action.id);
      return;
    case 'unwrapMarkNode':
      $unwrapMarkNode(markNode($node(action.key)));
      return;
    case 'logMarkIDs':
      ctx.log.push([
        'markIDs',
        action.key,
        action.offset,
        $getMarkIDs(text($node(action.key)), action.offset),
      ]);
      return;
    case 'addMarkID': {
      const flavor = markFlavor(action.flavor);
      const nodes =
        flavor === undefined
          ? $addMarkID(action.id, $anySel())
          : $addMarkID(flavor, action.id, $anySel());
      ctx.log.push(['addMarkID', action.id, nodes.map(n => n.getKey())]);
      return;
    }
    case 'removeMarkID': {
      const flavor = markFlavor(action.flavor);
      const selection = action.inSelection ? $anySel() : undefined;
      if (flavor === undefined) {
        $removeMarkID(action.id, selection);
      } else {
        $removeMarkID(flavor, action.id, selection);
      }
      return;
    }
    case 'logNodeMarkIDs': {
      const flavor = markFlavor(action.flavor);
      const n = $node(action.key);
      ctx.log.push([
        'nodeMarkIDs',
        action.key,
        flavor === undefined ? $getNodeMarkIDs(n) : $getNodeMarkIDs(flavor, n),
      ]);
      return;
    }
    case 'logMarkIDsAtSelection': {
      const flavor = markFlavor(action.flavor);
      ctx.log.push([
        'markIDsAtSelection',
        action.mode ?? null,
        flavor !== undefined
          ? $getMarkIDsAtSelection(flavor, undefined, action.mode)
          : action.mode === undefined
            ? $getMarkIDsAtSelection()
            : $getMarkIDsAtSelection(undefined, action.mode),
      ]);
      return;
    }
    case 'migrateMarkNodesToMarkState': {
      const flavor = markFlavor(action.flavor);
      const n = action.key === undefined ? undefined : $node(action.key);
      ctx.log.push([
        'migrateMarkNodesToMarkState',
        flavor === undefined
          ? $migrateMarkNodesToMarkState(n)
          : $migrateMarkNodesToMarkState(flavor, n),
      ]);
      return;
    }
    case 'logMarkIDsByFlavor':
      ctx.log.push([
        'markIDsByFlavor',
        action.key,
        Object.entries($getMarkIDsByFlavor($node(action.key))).sort(
          ([a], [b]) => (a < b ? -1 : a > b ? 1 : 0),
        ),
      ]);
      return;
    case 'copy': {
      const dataTransfer = new ScriptDataTransfer();
      setLexicalClipboardDataTransfer(
        dataTransfer.asDataTransfer(),
        $clipboardDataWithoutHtml(ctx.editor, $getSelection()),
      );
      ctx.clipboard = dataTransfer;
      ctx.log.push(['copy', dataTransfer.entries]);
      return;
    }
    case 'pasteClipboard': {
      if (ctx.clipboard === null) {
        throw new Error('nothing copied');
      }
      $insertDataTransfer(ctx.clipboard, action.plain === true);
      return;
    }
    case 'pasteLexical':
      $insertDataTransfer(
        ScriptDataTransfer.of({
          'application/x-lexical-editor': JSON.stringify({
            namespace: ctx.editor._config.namespace,
            nodes: action.nodes,
          }),
        }),
        false,
      );
      return;
    case 'pastePlainText':
      $insertDataTransfer(
        ScriptDataTransfer.of({'text/plain': action.text}),
        true,
      );
      return;
    case 'pasteData':
      $insertDataTransfer(
        ScriptDataTransfer.of(action.data),
        action.plain === true,
      );
      return;
    case 'generateJSON':
      ctx.log.push([
        'generateJSON',
        $generateJSONFromSelectedNodes(
          ctx.editor,
          action.whole === true ? null : $getSelection(),
        ) as unknown as Json,
      ]);
      return;
    case 'insertGeneratedNodes':
      $insertGeneratedNodes(
        ctx.editor,
        $generateNodesFromSerializedNodes(action.nodes),
        $anySel(),
      );
      return;
    case 'patchStyleText': {
      const patch: Record<
        string,
        | string
        | null
        | ((
            prev: string | null,
            target: TextNode | ElementNode | BaseSelection,
          ) => string)
      > = {};
      for (const [key, value] of Object.entries(action.patch)) {
        patch[key] =
          value !== null && typeof value === 'object'
            ? (prev, target) =>
                `${prev ?? 'null'}@${
                  $isRangeSelection(target)
                    ? 'selection'
                    : target instanceof TextNode
                      ? 'text'
                      : 'element'
                }`
            : value;
      }
      $patchStyleText($anySel(), patch as never);
      return;
    }
    case 'logSelectionStyleValue':
      ctx.log.push([
        'styleValue',
        action.property,
        action.defaultValue === undefined
          ? $getSelectionStyleValueForProperty($anySel(), action.property)
          : $getSelectionStyleValueForProperty(
              $anySel(),
              action.property,
              action.defaultValue,
            ),
      ]);
      return;
    case 'logStyleObject': {
      const object = getStyleObjectFromCSS(action.css);
      ctx.log.push([
        'styleObject',
        action.css,
        Object.entries(object),
        getCSSFromStyleObject(object),
      ]);
      return;
    }
    case 'logAtNodeEnd': {
      const selection = $sel();
      ctx.log.push([
        'atNodeEnd',
        $isAtNodeEnd(selection.anchor),
        $isAtNodeEnd(selection.focus),
      ]);
      return;
    }
    case 'trimTextContentFromAnchor':
      $trimTextContentFromAnchor(ctx.editor, $sel().anchor, action.count);
      return;
    case 'forEachSelectedTextNode': {
      const visited: Json[] = [];
      $forEachSelectedTextNode(textNode => {
        visited.push([textNode.getKey(), textNode.getTextContent()]);
      });
      ctx.log.push(['forEachSelectedTextNode', visited]);
      return;
    }
    case 'ensureForwardRangeSelection':
      $ensureForwardRangeSelection($sel());
      return;
    case 'wrapNodes': {
      const {block, wrapping} = action;
      $wrapNodes(
        $anySel(),
        () => $createBlock(block),
        wrapping === undefined ? null : $createBlock(wrapping),
      );
      return;
    }
    case 'logShouldOverride': {
      const selection = $sel();
      ctx.log.push([
        'shouldOverride',
        action.isBackward,
        $shouldOverrideDefaultCharacterSelection(selection, action.isBackward),
        $isParentElementRTL(selection),
      ]);
      return;
    }
    case 'moveCharacter':
      $moveCharacter($sel(), action.shift, action.isBackward);
      return;
    case 'logSlice':
      ctx.log.push([
        'slice',
        action.key,
        $sliceSelectedTextNodeContent(
          $anySel(),
          text($node(action.key)),
          'clone',
        ).getTextContent(),
      ]);
      return;
    case 'copyBlockFormatIndent':
      $copyBlockFormatIndent(
        element($node(action.from)),
        element($node(action.to)),
      );
      return;
    case 'logCloneWithProperties': {
      const n = $node(action.key);
      if ($isTextNode(n)) {
        $addNodeStyle(n);
      }
      ctx.log.push([
        'clone',
        action.key,
        $cloneWithProperties(n).exportJSON() as unknown as Json,
      ]);
      return;
    }
    case 'logDfs': {
      const start =
        action.start === undefined ? undefined : $node(action.start);
      const end = action.end === undefined ? undefined : $node(action.end);
      const nodes = action.reverse ? $reverseDfs(start, end) : $dfs(start, end);
      ctx.log.push([
        'dfs',
        action.reverse ?? false,
        nodes.map(({depth, node: n}) => [n.getKey(), depth]),
      ]);
      return;
    }
    case 'logDepth':
      ctx.log.push(['depth', action.key, $getDepth($node(action.key))]);
      return;
    case 'logNextSiblingOrParentSibling': {
      const found = $getNextSiblingOrParentSibling($node(action.key));
      ctx.log.push([
        'nextSiblingOrParentSibling',
        action.key,
        found === null ? null : [found[0].getKey(), found[1]],
      ]);
      return;
    }
    case 'logNextRightPreorderNode': {
      const found = $getNextRightPreorderNode($node(action.key));
      ctx.log.push([
        'nextRightPreorderNode',
        action.key,
        found === null ? null : found.getKey(),
      ]);
      return;
    }
    case 'logNearestBlockElementAncestor':
      ctx.log.push([
        'nearestBlockElementAncestor',
        action.key,
        $getNearestBlockElementAncestorOrThrow($node(action.key)).getKey(),
      ]);
      return;
    case 'logChildren': {
      const parent = element($node(action.key));
      const iterator = action.reverse
        ? $lastToFirstIterator(parent)
        : $firstToLastIterator(parent);
      ctx.log.push([
        'children',
        action.key,
        action.reverse ?? false,
        Array.from(iterator, n => n.getKey()),
      ]);
      return;
    }
    case 'unwrapAndFilterDescendants': {
      const {keep} = action;
      ctx.log.push([
        'unwrapAndFilterDescendants',
        action.key,
        $unwrapAndFilterDescendants(element($node(action.key)), n =>
          keep.includes(n.getType()),
        ),
      ]);
      return;
    }
    case 'logDescendantsMatching': {
      const {types} = action;
      ctx.log.push([
        'descendantsMatching',
        action.key,
        $descendantsMatching(
          element($node(action.key)).getChildren(),
          (n): n is LexicalNode => types.includes(n.getType()),
        ).map(n => n.getKey()),
      ]);
      return;
    }
    case 'insertFirst':
      $insertFirst(element($node(action.key)), $parse([action.node])[0]);
      return;
    case 'insertNodeToNearestRoot':
      ctx.log.push([
        'insertNodeToNearestRoot',
        $insertNodeToNearestRoot($parse([action.node])[0]).getKey(),
      ]);
      return;
    case 'insertNodeIntoLeaf':
      $insertNodeIntoLeaf($parse([action.node])[0]);
      return;
    case 'splitNode': {
      const [left, right] = $splitNode(
        element($node(action.key)),
        action.offset,
      );
      ctx.log.push([
        'splitNode',
        left === null ? null : left.getKey(),
        right.getKey(),
      ]);
      return;
    }
    case 'wrapNodeInElement': {
      const {block} = action;
      ctx.log.push([
        'wrapNodeInElement',
        $wrapNodeInElement($node(action.key), () =>
          $createBlock(block),
        ).getKey(),
      ]);
      return;
    }
    case 'unwrapNode':
      $unwrapNode(element($node(action.key)));
      return;
    case 'saveEditorState':
      ctx.saved.set(action.name, ctx.editor.getEditorState());
      return;
    case 'restoreEditorState': {
      const state = ctx.saved.get(action.name);
      if (state === undefined) {
        throw new Error(`no saved state ${action.name}`);
      }
      $restoreEditorState(ctx.editor, state);
      return;
    }
    case 'escape': {
      const {types} = action;
      const isContainer = (n?: LexicalNode | null): n is ElementNode =>
        $isElementNode(n) && types.includes(n.getType());
      ctx.log.push([
        'escape',
        action.direction,
        action.direction === 'up'
          ? $onEscapeUp(isContainer)
          : $onEscapeDown(isContainer),
      ]);
      return;
    }
    case 'logBlockFullySelected':
      ctx.log.push([
        'blockFullySelected',
        action.key,
        $isBlockFullySelected(element($node(action.key)), $sel()),
      ]);
      return;
    case 'setSlot': {
      const host = $node(action.key);
      if (!$isSlotHost(host)) {
        throw new Error(`${action.key} is not a slot host`);
      }
      const value =
        action.valueKey !== undefined
          ? $node(action.valueKey)
          : $parse([action.node as SerializedLexicalNode])[0];
      $setSlot(host, action.name, value);
      return;
    }
    case 'removeSlot': {
      const host = $node(action.key);
      if (!$isSlotHost(host)) {
        throw new Error(`${action.key} is not a slot host`);
      }
      $removeSlot(host, action.name);
      return;
    }
    case 'logSlots': {
      const n = $node(action.key);
      const host = $getSlotHost(n);
      const frame = $getSlotFrame(n);
      ctx.log.push([
        'slots',
        action.key,
        $getSlotNames(n),
        host === null ? null : host.getKey(),
        $getSlotNameWithinHost(n),
        frame === null ? null : frame.getKey(),
      ]);
      return;
    }
    case 'logSelectionSlotFrame': {
      const frame = $getSelectionSlotFrame($getSelection());
      ctx.log.push([
        'selectionSlotFrame',
        frame === null ? null : frame.getKey(),
      ]);
      return;
    }
    case 'logDfsWithSlots': {
      const start =
        action.start === undefined ? undefined : $node(action.start);
      const end = action.end === undefined ? undefined : $node(action.end);
      const nodes = action.reverse
        ? $reverseDfsWithSlots(start, end)
        : $dfsWithSlots(start, end);
      ctx.log.push([
        'dfsWithSlots',
        action.reverse ?? false,
        nodes.map(({depth, node: n}) => [n.getKey(), depth]),
      ]);
      return;
    }
    case 'logNodesExist':
      ctx.log.push([
        'nodesExist',
        action.keys.map(key => [key, $getNodeByKey(key) !== null]),
      ]);
      return;
    case 'setBlockId':
      $setState($node(action.key), blockIdState, action.id ?? undefined);
      return;
    case 'logNodeStructure':
      for (const type of action.types) {
        ctx.log.push([
          'nodeStructure',
          type,
          STRUCTURE_JSON($registeredClass(type)),
        ]);
      }
      return;
    case 'logBlockContainer':
      for (const key of action.keys) {
        ctx.log.push(['blockContainer', key, $isBlockContainer($node(key))]);
      }
      return;
    case 'logStructureEnforced':
      ctx.log.push([
        'structureEnforced',
        INTERNAL_isNodeStructureEnforced(ctx.editor),
      ]);
      return;
    case 'normalizeNodeStructure':
      INTERNAL_$normalizeNodeStructure($node(action.key));
      return;
    case 'appendNew':
      element($node(action.key)).append($create($registeredClass(action.type)));
      return;
    case 'logBlockIds':
      ctx.log.push([
        'blockIds',
        action.keys.map(key => [key, $getBlockId($node(key))]),
      ]);
      return;
    case 'logBlockById': {
      const found = $getBlockById(action.id);
      ctx.log.push([
        'blockById',
        action.id,
        found === null ? null : found.getKey(),
      ]);
      return;
    }
    case 'logIdentified': {
      const target = $node(action.key);
      ctx.log.push([
        'identified',
        action.key,
        $getExtensionOutput(BlockIdExtension).$isIdentifiedBlock(target),
        $defaultIsIdentifiedBlock(target),
        $isRootChildBlock(target),
      ]);
      return;
    }
    case 'logBlockHandles': {
      const handles = $getBlockHandles();
      if (action.name !== undefined) {
        ctx.handles.set(action.name, handles);
      }
      ctx.log.push(['blockHandles', [...handles]]);
      return;
    }
    case 'logBlockHandle':
      ctx.log.push([
        'blockHandle',
        action.key,
        $getBlockHandle($node(action.key)),
      ]);
      return;
    case 'resolveBlockHandle': {
      let read: Map<string, string> | undefined;
      if (action.read !== undefined) {
        read = ctx.handles.get(action.read);
        if (read === undefined) {
          throw new Error(`no handles kept as ${action.read}`);
        }
      }
      const result = $resolveBlockHandle(
        action.handle,
        read === undefined ? undefined : {read},
      );
      ctx.log.push([
        'resolved',
        action.handle,
        result.status,
        result.status === 'found'
          ? result.node.getKey()
          : result.status === 'ambiguous'
            ? [...result.candidates]
            : result.status === 'invalid'
              ? result.reason
              : null,
      ]);
      return;
    }
    case 'selectTypeaheadOption': {
      const [, match] = $typeaheadMatch(action.typeahead, ctx.editor);
      const node =
        match === null
          ? null
          : REACT_INTERNALS.INTERNAL_$splitNodeContainingQuery(match);
      ctx.log.push([
        'splitNodeContainingQuery',
        node === null ? null : node.getKey(),
        node === null ? null : node.getTextContent(),
      ]);
      if (node !== null) {
        const replacement = $createTextNode(action.replacement);
        node.replace(replacement);
        replacement.select();
      }
      return;
    }
    case 'logFullMatchOffset':
      ctx.log.push([
        'fullMatchOffset',
        action.documentText,
        action.entryText,
        action.offset,
        REACT_INTERNALS.INTERNAL_getFullMatchOffset(
          action.documentText,
          action.entryText,
          action.offset,
        ),
      ]);
      return;
    case 'logDecorators':
      ctx.log.push([
        'getDecorators',
        decoratorEntries(ctx.editor.getDecorators()),
      ]);
      return;
    case 'logDecoratorBlock': {
      const n = $node(action.key);
      const block = $isDecoratorBlockNode(n) ? n : null;
      ctx.log.push([
        'decoratorBlock',
        action.key,
        block !== null,
        n.isInline(),
        block === null ? null : block.getFormat(),
        block === null ? null : block.canIndent(),
      ]);
      return;
    }
    case 'logUrlMatcher': {
      const match = URL_MATCHER.exec(action.text);
      ctx.log.push([
        'urlMatcher',
        action.text,
        match === null ? null : [match.index, match[0]],
      ]);
      return;
    }
    case 'logSingleTokenPaste':
      ctx.log.push([
        'singleTokenPaste',
        action.text,
        /^\S+$/.test(action.text),
      ]);
      return;
    case 'findOffset': {
      const strlen = action.strlen ?? 'utf16';
      ctx.log.push([
        'findOffset',
        action.text,
        strlen,
        action.maxCharacters.map(max =>
          CHARACTER_LIMIT.INTERNAL_findOffset(action.text, max, STRLEN[strlen]),
        ),
      ]);
      return;
    }
    case 'wrapOverflowedNodes':
      CHARACTER_LIMIT.INTERNAL_$wrapOverflowedNodes(action.offset);
      return;
    case 'mergePrevious': {
      const overflowNode = $node(action.key);
      if (!$isOverflowNode(overflowNode)) {
        throw new Error(`${action.key} is not an overflow node`);
      }
      CHARACTER_LIMIT.INTERNAL_$mergePrevious(overflowNode);
      return;
    }
    case 'addUpdateTag':
      $addUpdateTag(action.tag);
      return;
    case 'logHasUpdateTag':
      ctx.log.push(['hasUpdateTag', action.tag, $hasUpdateTag(action.tag)]);
      return;
    case 'onUpdate':
      $onUpdate(() => {
        ctx.log.push(['$onUpdate']);
      });
      return;
    case 'logHasNodes': {
      const editor = $getEditor();
      const klasses = action.types.map(type => {
        const klass = KNOWN_CLASSES.get(type);
        if (klass === undefined) {
          throw new Error(`unknown class ${type}`);
        }
        return klass;
      });
      ctx.log.push([
        'hasNodes',
        action.types.map((type, i) => [type, editor.hasNode(klasses[i])]),
        editor.hasNodes(klasses),
      ]);
      return;
    }
    case 'logNodesOfType':
      ctx.log.push([
        'nodesOfType',
        action.type,
        $nodesOfType($registeredClass(action.type)).map(n => n.getKey()),
      ]);
      return;
    case 'logAdjacentNode': {
      const adjacent = $getAdjacentNode($sel().focus, action.isBackward);
      ctx.log.push([
        'adjacentNode',
        action.isBackward,
        adjacent === null ? null : adjacent.getKey(),
      ]);
      return;
    }
    case 'logGetters':
      ctx.log.push(['getters', action.key, $getters($node(action.key))]);
      return;
    case 'appendText':
      element($node(action.key)).append($createTextNode(action.text));
      return;
    case 'logEditable':
      ctx.log.push(['editable', $getEditor().isEditable()]);
      return;
    case 'dispatch':
      dispatchLogged(ctx, action.command, action.payload ?? null);
      return;
  }
}

/**
 * `$insertDataTransferForRichText` (or, with `plain`,
 * `$insertDataTransferForPlainText`) of `dataTransfer` at the selection.
 */
function $insertDataTransfer(
  dataTransfer: ScriptDataTransfer,
  plain: boolean,
): void {
  if (plain) {
    $insertDataTransferForPlainText(dataTransfer.asDataTransfer(), $anySel());
  } else {
    $insertDataTransferForRichText(dataTransfer.asDataTransfer(), $anySel());
  }
}

/**
 * Lexical's own commands, which `dispatch` and handlers name as JS exports
 * them. Any other name is a command the script makes.
 */
const CORE_COMMANDS = new Map<string, AnyLexicalCommand>([
  ['BEFORE_INPUT_COMMAND', BEFORE_INPUT_COMMAND],
  ['CAN_REDO_COMMAND', CAN_REDO_COMMAND],
  ['CAN_UNDO_COMMAND', CAN_UNDO_COMMAND],
  ['CLEAR_EDITOR_COMMAND', CLEAR_EDITOR_COMMAND],
  ['CLEAR_HISTORY_COMMAND', CLEAR_HISTORY_COMMAND],
  ['CLICK_COMMAND', CLICK_COMMAND],
  ['COMPOSITION_END_COMMAND', COMPOSITION_END_COMMAND],
  ['COMPOSITION_START_COMMAND', COMPOSITION_START_COMMAND],
  ['CONTROLLED_TEXT_INSERTION_COMMAND', CONTROLLED_TEXT_INSERTION_COMMAND],
  ['COPY_COMMAND', COPY_COMMAND],
  ['CUT_COMMAND', CUT_COMMAND],
  ['DELETE_CHARACTER_COMMAND', DELETE_CHARACTER_COMMAND],
  ['DELETE_LINE_COMMAND', DELETE_LINE_COMMAND],
  ['DELETE_WORD_COMMAND', DELETE_WORD_COMMAND],
  ['DRAGEND_COMMAND', DRAGEND_COMMAND],
  ['DRAGOVER_COMMAND', DRAGOVER_COMMAND],
  ['DRAGSTART_COMMAND', DRAGSTART_COMMAND],
  ['DROP_COMMAND', DROP_COMMAND],
  ['FORMAT_ELEMENT_COMMAND', FORMAT_ELEMENT_COMMAND],
  ['FORMAT_TEXT_COMMAND', FORMAT_TEXT_COMMAND],
  ['INDENT_CONTENT_COMMAND', INDENT_CONTENT_COMMAND],
  ['INSERT_LINE_BREAK_COMMAND', INSERT_LINE_BREAK_COMMAND],
  ['INSERT_PARAGRAPH_COMMAND', INSERT_PARAGRAPH_COMMAND],
  ['INSERT_TAB_COMMAND', INSERT_TAB_COMMAND],
  ['KEY_ARROW_DOWN_COMMAND', KEY_ARROW_DOWN_COMMAND],
  ['KEY_ARROW_LEFT_COMMAND', KEY_ARROW_LEFT_COMMAND],
  ['KEY_ARROW_RIGHT_COMMAND', KEY_ARROW_RIGHT_COMMAND],
  ['KEY_ARROW_UP_COMMAND', KEY_ARROW_UP_COMMAND],
  ['KEY_BACKSPACE_COMMAND', KEY_BACKSPACE_COMMAND],
  ['KEY_DELETE_COMMAND', KEY_DELETE_COMMAND],
  ['KEY_DOWN_COMMAND', KEY_DOWN_COMMAND],
  ['KEY_ENTER_COMMAND', KEY_ENTER_COMMAND],
  ['KEY_ESCAPE_COMMAND', KEY_ESCAPE_COMMAND],
  ['KEY_MODIFIER_COMMAND', KEY_MODIFIER_COMMAND],
  ['KEY_SPACE_COMMAND', KEY_SPACE_COMMAND],
  ['KEY_TAB_COMMAND', KEY_TAB_COMMAND],
  ['MOVE_TO_END', MOVE_TO_END],
  ['MOVE_TO_START', MOVE_TO_START],
  ['OUTDENT_CONTENT_COMMAND', OUTDENT_CONTENT_COMMAND],
  ['PASTE_COMMAND', PASTE_COMMAND],
  ['REDO_COMMAND', REDO_COMMAND],
  ['REMOVE_TEXT_COMMAND', REMOVE_TEXT_COMMAND],
  ['SELECT_ALL_COMMAND', SELECT_ALL_COMMAND],
  ['SELECTION_CHANGE_COMMAND', SELECTION_CHANGE_COMMAND],
  ['SET_TEXT_FORMAT_COMMAND', SET_TEXT_FORMAT_COMMAND],
  ['UNDO_COMMAND', UNDO_COMMAND],
]);

/**
 * The commands of the packages a script registers (`list`, `link`, TableExtension,
 * `richText` and `HorizontalRuleExtension`'s), named as their package
 * exports them. `DRAG_DROP_PASTE`'s payload, the dropped or pasted files, is
 * logged as `[{name, type}...]`.
 */
const PACKAGE_COMMANDS = new Map<string, AnyLexicalCommand>([
  ['INSERT_CHECK_LIST_COMMAND', INSERT_CHECK_LIST_COMMAND],
  ['INSERT_ORDERED_LIST_COMMAND', INSERT_ORDERED_LIST_COMMAND],
  ['INSERT_UNORDERED_LIST_COMMAND', INSERT_UNORDERED_LIST_COMMAND],
  ['REMOVE_LIST_COMMAND', REMOVE_LIST_COMMAND],
  ['UPDATE_LIST_START_COMMAND', UPDATE_LIST_START_COMMAND],
  ['TOGGLE_LINK_COMMAND', TOGGLE_LINK_COMMAND],
  ['INSERT_EMBED_COMMAND', INSERT_EMBED_COMMAND],
  ['INSERT_HORIZONTAL_RULE_COMMAND', INSERT_HORIZONTAL_RULE_COMMAND],
  ['INSERT_TABLE_COMMAND', INSERT_TABLE_COMMAND],
  ['DRAG_DROP_PASTE', DRAG_DROP_PASTE],
]);

/**
 * The commands whose payload is a DOM event, by the event a JSON payload
 * makes for them (`eventPayload`): `keydown` (a `KeyboardEvent` of `key`,
 * `code` and the modifier flags), `beforeinput` (an `InputEvent` of
 * `inputType` and `data`), a composition event (a `CompositionEvent` of
 * `data`), `click` (a `MouseEvent` of `clientX`, `clientY`, `button`,
 * `detail` and the modifier flags whose `target` is
 * `editor.getElementByKey(targetKey)`, the DOM the reconciler made for the
 * node `targetKey`, or `null` when `targetKey` is absent or `null`), a drag
 * event (a `DragEvent` of `clientX`, `clientY` and `dataTransfer`) or
 * `paste` (a `ClipboardEvent` of `clipboardData`). A data transfer is
 * `{data: {type: data...}, files: [{name, type, text}...]}`, or `null`. A
 * `null` payload stays `null`, as `KEY_ENTER_COMMAND` and
 * `SELECT_ALL_COMMAND` take it. `COPY_COMMAND` and
 * `CUT_COMMAND` are only listened to, as the keydown and beforeinput
 * handlers dispatch them with their event.
 */
const EVENT_COMMANDS = new Map<string, string>([
  ['BEFORE_INPUT_COMMAND', 'beforeinput'],
  ['CLICK_COMMAND', 'click'],
  ['COMPOSITION_END_COMMAND', 'compositionend'],
  ['COMPOSITION_START_COMMAND', 'compositionstart'],
  ['DRAGEND_COMMAND', 'dragend'],
  ['DRAGOVER_COMMAND', 'dragover'],
  ['DRAGSTART_COMMAND', 'dragstart'],
  ['DROP_COMMAND', 'drop'],
  ['PASTE_COMMAND', 'paste'],
  ...[
    'KEY_ARROW_DOWN_COMMAND',
    'KEY_ARROW_LEFT_COMMAND',
    'KEY_ARROW_RIGHT_COMMAND',
    'KEY_ARROW_UP_COMMAND',
    'KEY_BACKSPACE_COMMAND',
    'KEY_DELETE_COMMAND',
    'KEY_DOWN_COMMAND',
    'KEY_ENTER_COMMAND',
    'KEY_ESCAPE_COMMAND',
    'KEY_MODIFIER_COMMAND',
    'KEY_SPACE_COMMAND',
    'KEY_TAB_COMMAND',
    'MOVE_TO_END',
    'MOVE_TO_START',
    'SELECT_ALL_COMMAND',
  ].map(name => [name, 'keydown'] as [string, string]),
]);

const DRAG_EVENTS = new Set(['dragend', 'dragover', 'dragstart', 'drop']);

/**
 * The event classes the packages' handlers name, which Node lacks: their
 * handlers tell a paste's or a drag's event by its class's name
 * (`objectKlassEquals(event, ClipboardEvent)` in `eventFiles` and
 * `onPasteForRichText`), as a browser's globals let them. jsdom has no
 * `ClipboardEvent` or `DragEvent`, so these are made on its `Event` and
 * `MouseEvent`; the others are jsdom's. They are put on `globalThis` the
 * first time a script makes such an event.
 */
type EventClasses = {
  ClipboardEvent: new (
    type: string,
    init: EventInit & {clipboardData: ScriptDataTransfer | null},
  ) => Event & {clipboardData: ScriptDataTransfer | null};
  DragEvent: new (
    type: string,
    init: MouseEventInit & {dataTransfer: ScriptDataTransfer | null},
  ) => MouseEvent & {dataTransfer: ScriptDataTransfer | null};
};

let eventClasses: EventClasses | null = null;

function scriptEventClasses(
  win: ReturnType<typeof browserWindow>,
): EventClasses {
  if (eventClasses !== null) {
    return eventClasses;
  }
  class ClipboardEvent extends win.Event {
    clipboardData: ScriptDataTransfer | null;
    constructor(
      type: string,
      init: EventInit & {clipboardData: ScriptDataTransfer | null},
    ) {
      super(type, init);
      this.clipboardData = init.clipboardData;
    }
  }
  class DragEvent extends win.MouseEvent {
    dataTransfer: ScriptDataTransfer | null;
    constructor(
      type: string,
      init: MouseEventInit & {dataTransfer: ScriptDataTransfer | null},
    ) {
      super(type, init);
      this.dataTransfer = init.dataTransfer;
    }
  }
  eventClasses = {ClipboardEvent, DragEvent} as unknown as EventClasses;
  const globals = globalThis as Record<string, unknown>;
  for (const [name, klass] of [
    ['ClipboardEvent', ClipboardEvent],
    ['DragEvent', DragEvent],
    ['InputEvent', win.InputEvent],
    ['KeyboardEvent', win.KeyboardEvent],
    ['MouseEvent', win.MouseEvent],
  ] as const) {
    globals[name] ??= klass;
  }
  return eventClasses;
}

/** A data transfer's JSON (`eventPayload`) as a `ScriptDataTransfer`. */
function scriptDataTransfer(json: Json | undefined): ScriptDataTransfer | null {
  if (json === null || json === undefined || typeof json !== 'object') {
    return null;
  }
  const {data, files} = json as {
    data?: Record<string, string>;
    files?: {name: string; type: string; text?: string}[];
  };
  const dataTransfer = ScriptDataTransfer.of(data ?? {});
  dataTransfer.fileList = (files ?? []).map(
    file => new File([file.text ?? ''], file.name, {type: file.type}),
  );
  return dataTransfer;
}

/** The jsdom window the events of `eventPayload` come from. */
let eventWindow: ReturnType<typeof browserWindow> | null = null;

/**
 * A `KeyboardEventModifierMask` as JSON: each modifier `true`, `false` or
 * `'any'` (left out is `false`), and `controlOrOther`, the
 * `[CONTROL_OR_OTHER_KEY]` that `CONTROL_OR_META` (`'metaKey'`) and
 * `CONTROL_OR_ALT` (`'altKey'`) carry.
 */
export type ModifiersJson = {
  altKey?: boolean | 'any';
  ctrlKey?: boolean | 'any';
  metaKey?: boolean | 'any';
  shiftKey?: boolean | 'any';
  controlOrOther?: 'metaKey' | 'altKey';
};

/**
 * A `KeyboardShortcut` as JSON: its `key`, `modifiers` and `unshiftedKey`,
 * and, in KeyboardShortcutsExtension's `shortcuts`, the `command` it
 * dispatches (by name, `command(ctx, name)`), its `description`,
 * `bubbleFromNestedEditors`, and its functions by name: `$disabled:
 * 'always'` skips it, `'notRange'` skips it unless the selection is a
 * range selection; `$dispatch: 'log'` logs `['shortcutDispatch', key,
 * fromEditorIsEditor]` and calls `$next`, `'unhandled'` logs the same and
 * returns `false` without dispatching.
 */
export type ShortcutJson = {
  key: string;
  modifiers?: ModifiersJson;
  unshiftedKey?: string;
  command?: string;
  description?: string;
  bubbleFromNestedEditors?: boolean;
  $disabled?: 'always' | 'notRange';
  $dispatch?: 'log' | 'unhandled';
};

/** `NamedKeyboardShortcuts` as JSON. */
export type ShortcutsJson = {
  [name: string]: ShortcutJson | ShortcutJson[] | null;
};

/** The mask of `ModifiersJson`. */
function scriptModifiers(json: ModifiersJson): KeyboardEventModifierMask {
  const {controlOrOther, ...mask} = json;
  return controlOrOther === undefined
    ? mask
    : ({...mask, [CONTROL_OR_OTHER_KEY]: controlOrOther} as never);
}

/** The `KeyboardShortcutMatch` of a `ShortcutJson`. */
function scriptShortcutMatch(json: ShortcutJson): {
  key: string;
  modifiers?: KeyboardEventModifierMask;
  unshiftedKey?: string;
} {
  return {
    key: json.key,
    ...(json.modifiers === undefined
      ? {}
      : {modifiers: scriptModifiers(json.modifiers)}),
    ...(json.unshiftedKey === undefined
      ? {}
      : {unshiftedKey: json.unshiftedKey}),
  };
}

/** The `NamedKeyboardShortcuts` of a `ShortcutsJson`. */
function scriptShortcuts(
  ctx: Context,
  json: ShortcutsJson,
): Record<string, KeyboardShortcut | KeyboardShortcut[] | null> {
  const shortcut = (s: ShortcutJson): KeyboardShortcut => {
    const out: KeyboardShortcut = {
      ...scriptShortcutMatch(s),
      command: command(ctx, s.command ?? '') as never,
    };
    if (s.description !== undefined) {
      out.description = s.description;
    }
    if (s.bubbleFromNestedEditors !== undefined) {
      out.bubbleFromNestedEditors = s.bubbleFromNestedEditors;
    }
    if (s.$disabled === 'always') {
      out.$disabled = () => true;
    } else if (s.$disabled === 'notRange') {
      out.$disabled = selection => !$isRangeSelection(selection);
    }
    if (s.$dispatch !== undefined) {
      const handled = s.$dispatch === 'log';
      out.$dispatch = (_command, event, $next, fromEditor) => {
        ctx.log.push([
          'shortcutDispatch',
          event.key,
          fromEditor === ctx.editor,
        ]);
        return handled ? $next() : false;
      };
    }
    return out;
  };
  const out: Record<string, KeyboardShortcut | KeyboardShortcut[] | null> = {};
  for (const [name, value] of Object.entries(json)) {
    out[name] =
      value === null
        ? null
        : Array.isArray(value)
          ? value.map(shortcut)
          : shortcut(value);
  }
  return out;
}

/** A DOM event of the `domEvent` step. */
export type DomEventJson = {
  type: 'click' | 'auxclick' | 'compositionend';
  targetKey?: string;
  clientX?: number;
  clientY?: number;
  button?: number;
  detail?: number;
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
  data?: string;
};

/** The `domEvent` step: dispatches the event and logs it. */
function dispatchDomEvent(
  ctx: Context,
  editor: LexicalEditor,
  json: DomEventJson,
): void {
  const win = sessionDOM(ctx).window;
  const target =
    json.targetKey === undefined
      ? editor.getRootElement()
      : editor.getElementByKey(json.targetKey);
  if (target === null) {
    throw new Error(`no element for ${json.targetKey ?? 'the root'}`);
  }
  const event =
    json.type === 'compositionend'
      ? new win.CompositionEvent(json.type, {
          bubbles: true,
          cancelable: true,
          data: json.data ?? '',
        })
      : new win.MouseEvent(json.type, {
          altKey: json.altKey === true,
          bubbles: true,
          button: json.button ?? 0,
          cancelable: true,
          clientX: json.clientX ?? 0,
          clientY: json.clientY ?? 0,
          ctrlKey: json.ctrlKey === true,
          detail: json.detail ?? 1,
          metaKey: json.metaKey === true,
          shiftKey: json.shiftKey === true,
        });
  target.dispatchEvent(event);
  ctx.log.push(['domEvent', json.type, event.defaultPrevented]);
}

/**
 * `payload` as the command `name` takes it: for a command of
 * EVENT_COMMANDS, a JSON object becomes the event it describes.
 */
function eventPayload(
  name: string,
  payload: Json,
  editor: LexicalEditor,
): unknown {
  const type = EVENT_COMMANDS.get(name);
  if (type === undefined || payload === null || typeof payload !== 'object') {
    return payload;
  }
  const p = payload as {[key: string]: Json};
  const win = (eventWindow ??= browserWindow());
  if (type === 'keydown') {
    return new win.KeyboardEvent(type, {
      altKey: p.altKey === true,
      cancelable: true,
      code: typeof p.code === 'string' ? p.code : '',
      ctrlKey: p.ctrlKey === true,
      key: typeof p.key === 'string' ? p.key : '',
      metaKey: p.metaKey === true,
      shiftKey: p.shiftKey === true,
    });
  } else if (type === 'beforeinput') {
    return new win.InputEvent(type, {
      cancelable: true,
      data: typeof p.data === 'string' ? p.data : null,
      inputType: String(p.inputType),
    });
  }
  const mouse = {
    altKey: p.altKey === true,
    button: typeof p.button === 'number' ? p.button : 0,
    cancelable: true,
    clientX: typeof p.clientX === 'number' ? p.clientX : 0,
    clientY: typeof p.clientY === 'number' ? p.clientY : 0,
    ctrlKey: p.ctrlKey === true,
    detail: typeof p.detail === 'number' ? p.detail : 1,
    metaKey: p.metaKey === true,
    shiftKey: p.shiftKey === true,
  };
  if (type === 'click') {
    scriptEventClasses(win);
    const event = new win.MouseEvent(type, mouse);
    // As if the browser dispatched it on that element: a handler reads only
    // `event.target`.
    Object.defineProperty(event, 'target', {
      value:
        typeof p.targetKey === 'string'
          ? editor.getElementByKey(p.targetKey)
          : null,
    });
    return event;
  } else if (DRAG_EVENTS.has(type)) {
    return new (scriptEventClasses(win).DragEvent)(type, {
      ...mouse,
      dataTransfer: scriptDataTransfer(p.dataTransfer),
    });
  } else if (type === 'paste') {
    return new (scriptEventClasses(win).ClipboardEvent)(type, {
      cancelable: true,
      clipboardData: scriptDataTransfer(p.clipboardData),
    });
  }
  return new win.CompositionEvent(type, {
    data: typeof p.data === 'string' ? p.data : '',
  });
}

/**
 * A command's payload as a handler logs it: an event as the fields Lexical
 * reads of it (`{type, key, code, altKey, ctrlKey, metaKey, shiftKey}` for a
 * `KeyboardEvent`, `{type, inputType, data}` for an `InputEvent`, `{type,
 * data}` for a `CompositionEvent`), files as `[{name, type}...]`, anything
 * else as it is.
 */
function payloadJson(payload: unknown): Json {
  if (Array.isArray(payload) && payload.every(item => item instanceof File)) {
    return payload.map(({name, type}) => ({name, type}));
  }
  if (eventWindow === null || !(payload instanceof eventWindow.Event)) {
    return payload as Json;
  }
  if (payload instanceof eventWindow.KeyboardEvent) {
    const {altKey, code, ctrlKey, key, metaKey, shiftKey, type} = payload;
    return {altKey, code, ctrlKey, key, metaKey, shiftKey, type};
  } else if (payload instanceof eventWindow.InputEvent) {
    return {
      data: payload.data,
      inputType: payload.inputType,
      type: payload.type,
    };
  } else if (payload instanceof eventWindow.CompositionEvent) {
    return {data: payload.data, type: payload.type};
  }
  throw new Error(`no JSON for a ${payload.type} event`);
}

/**
 * `editor.dispatchCommand(command(ctx, name), payload)`, logged as
 * `['dispatched', name, handled]`, then, when the payload is an event,
 * `['defaultPrevented', event.defaultPrevented]`, and for a drag event with
 * a data transfer, `['dataTransfer', [[type, data]...]]` of what it carries
 * then. The data of `application/x-lexical-drag`, the marker of the editor
 * the drag started in, is `'<this editor>'` when its `editorKey` is this
 * editor's, whose key is random.
 */
function dispatchLogged(
  ctx: Context,
  name: string,
  payload: Json,
  target: LexicalEditor = ctx.editor,
): void {
  const event = eventPayload(name, payload, target);
  ctx.log.push([
    'dispatched',
    name,
    target.dispatchCommand(command(ctx, name), event as Json),
  ]);
  if (event !== payload) {
    ctx.log.push(['defaultPrevented', (event as Event).defaultPrevented]);
    const dataTransfer = DRAG_EVENTS.has((event as Event).type)
      ? (event as {dataTransfer: ScriptDataTransfer | null}).dataTransfer
      : null;
    if (dataTransfer !== null) {
      ctx.log.push([
        'dataTransfer',
        dataTransfer.entries.map(([type, data]) => [
          type,
          type === 'application/x-lexical-drag' &&
          (JSON.parse(data) as {editorKey?: string}).editorKey ===
            target.getKey()
            ? '<this editor>'
            : data,
        ]),
      ]);
    }
  }
}

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
  // `registerNestedElementResolver` (@lexical/utils) for MarkNode: a mark
  // directly in another moves out of it, after logging
  // `['nestedOverlap', parent, node]` and giving the parent's ids to the
  // node; the parent's children after it go into a new mark with its ids.
  'mark:nested': ctx =>
    registerNestedElementResolver<MarkNode>(
      ctx.editor,
      MarkNode,
      from => $createMarkNode(from.getIDs()),
      (from, to) => {
        ctx.log.push(['nestedOverlap', from.getKey(), to.getKey()]);
        for (const id of from.getIDs()) {
          to.addID(id);
        }
      },
    ),

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

/**
 * The mark flavors (`@lexical/mark`'s `defineMarkExtension`) an
 * `ExtensionArg` or a MarkState action's `flavor` may name. `ClashingMarks`
 * shares CommentMarks' `stateKey`, and `AttributeClashMarks` its attribute,
 * which the build of an editor with both rejects.
 */
export const MARK_FLAVORS = new Map<string, AnyMarkFlavorExtension>([
  ['MarkStateExtension', MarkStateExtension],
  [
    'CommentMarks',
    defineMarkExtension({
      className: 'comment',
      name: '@conformance/CommentMarks',
      overlapClassName: 'comment-overlap',
      stateKey: 'commentIds',
    }),
  ],
  [
    'SuggestionMarks',
    defineMarkExtension({
      attribute: 'data-Suggestion',
      name: '@conformance/SuggestionMarks',
      stateKey: 'suggestionIds',
    }),
  ],
  [
    'ClashingMarks',
    defineMarkExtension({
      name: '@conformance/ClashingMarks',
      stateKey: 'commentIds',
    }),
  ],
  [
    'AttributeClashMarks',
    defineMarkExtension({
      attribute: 'data-lexical-comment-ids',
      name: '@conformance/AttributeClashMarks',
      stateKey: 'otherIds',
    }),
  ],
]);

/**
 * The extensions of `@lexical/extension` and the other packages an
 * `ExtensionArg` may name, as the packages export them. The first five
 * take no config overrides (their configs aren't JSON); the others take
 * the JSON fields of theirs, and their functions by CONFIG_PRESETS.
 */
const REAL_EXTENSIONS = new Map<string, AnyLexicalExtension>([
  ['ClearEditorExtension', ClearEditorExtension],
  ['DecoratorTextExtension', DecoratorTextExtension],
  ['EditorStateExtension', EditorStateExtension],
  ['HorizontalRuleExtension', HorizontalRuleExtension],
  ['NodeSelectionExtension', NodeSelectionExtension],
  ['AutoLinkExtension', AutoLinkExtension],
  ['BlockIdExtension', BlockIdExtension],
  ['CheckListExtension', CheckListExtension],
  ['CodeExtension', CodeExtension],
  ['CodeIndentExtension', CodeIndentExtension],
  ['HashtagExtension', HashtagExtension],
  ['HistoryExtension', HistoryExtension],
  ['LinkExtension', LinkExtension],
  ['ListExtension', ListExtension],
  ['NestedEditorExtension', NestedEditorExtension],
  ['NodeStructureExtension', NodeStructureExtension],
  ['NormalizeInlineElementsExtension', NormalizeInlineElementsExtension],
  ['OverflowExtension', OverflowExtension],
  ['PlainTextExtension', PlainTextExtension],
  ['RichTextExtension', RichTextExtension],
  ['SelectBlockExtension', SelectBlockExtension],
  ['SharedHistoryExtension', SharedHistoryExtension],
  ['TabIndentationExtension', TabIndentationExtension],
  ['AriaLiveRegionExtension', AriaLiveRegionExtension],
  ['AutoLinkAnnounceExtension', AutoLinkAnnounceExtension],
  ['EditorModeAnnounceExtension', EditorModeAnnounceExtension],
  ['HeadingAnnounceExtension', HeadingAnnounceExtension],
  ['HistoryAnnounceExtension', HistoryAnnounceExtension],
  ['MarkExtension', MarkExtension],
  ['MarkBaseExtension', MarkBaseExtension],
  ['HeadlessExtension', HeadlessExtension],
  ['KeyboardShortcutsExtension', KeyboardShortcutsExtension],
  ['ClickAfterLastBlockExtension', ClickAfterLastBlockExtension],
  [
    'NormalizeTripleClickSelectionExtension',
    NormalizeTripleClickSelectionExtension,
  ],
  ['IMEExtension', IMEExtension],
  ['AutoFocusExtension', AutoFocusExtension],
  ['ClickableLinkExtension', ClickableLinkExtension],
  ['TableExtension', TableExtension],
  ...MARK_FLAVORS,
]);

/**
 * The editor state `createInitialHistoryState: 'prefilled'` puts on the
 * undo stack: a paragraph with the text `prefilled`.
 */
export const PREFILLED_HISTORY_STATE =
  '{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"prefilled","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}';

/**
 * The functions a real extension's config overrides name, by field and
 * name: `validateUrl: 'https'` (LinkExtension) accepts only `https://`
 * URLs; `matchers: ['url', 'email']` (AutoLinkExtension) are
 * `autoLinkUrlMatcher` and `autoLinkEmailMatcher`, and `changeHandlers:
 * ['log']` logs `['autoLink', url, prevUrl]`; `$canIndent: 'never'`
 * (TabIndentationExtension) indents nothing; `createInitialHistoryState:
 * 'prefilled'` (HistoryExtension) starts with PREFILLED_HISTORY_STATE on the
 * undo stack; `shouldHandlePasteAsFiles: 'always'` (RichTextExtension)
 * pastes the files of a paste that has any, with text or not. BlockIdExtension's `generateId: 'counter'` returns `id1`,
 * `id2`..., `'stutter'` returns each of those twice in a row, and
 * `'invalid'` always `not valid`; its `$isIdentifiedBlock: 'rootChild'` is
 * `$isRootChildBlock`. NestedEditorExtension's `$getParentEditor: 'parent'`
 * returns the script's `parent` editor. A mark flavor's `markableNodes:
 * 'text'` is `[TextNode]` (a class), and `'decorators'` is
 * `[...DEFAULT_MARKABLE_NODES, $isDecoratorNode]` (a guard, which block
 * decorators pass too).
 */
function configPreset(ctx: Context, field: string, name: string): unknown {
  switch (`${field}:${name}`) {
    case 'validateUrl:https':
      return (url: string) => url.startsWith('https://');
    case 'matchers:url':
      return autoLinkUrlMatcher;
    case 'matchers:email':
      return autoLinkEmailMatcher;
    case 'changeHandlers:log':
      return (url: string | null, prevUrl: string | null) => {
        ctx.log.push(['autoLink', url, prevUrl]);
      };
    case '$canIndent:never':
      return () => false;
    case 'shouldHandlePasteAsFiles:always':
      return (files: File[]) => files.length > 0;
    case 'generateId:counter': {
      let n = 0;
      return () => `id${++n}`;
    }
    case 'generateId:stutter': {
      let n = 0;
      return () => `id${(n++ >> 1) + 1}`;
    }
    case 'generateId:invalid':
      return () => 'not valid';
    case '$isIdentifiedBlock:rootChild':
      return $isRootChildBlock;
    case '$getParentEditor:parent':
      return () => {
        if (ctx.parent === null) {
          throw new Error('the script has no parent editor');
        }
        return ctx.parent;
      };
    case '$shouldInsertAfter:always':
      return () => true;
    case 'getWindow:log':
      return () => ({
        open: (url: string, target: string, features?: string) => {
          ctx.log.push(['open', url, target, features ?? null]);
          return null;
        },
      });
    case 'getWindow:none':
      return () => null;
    case 'markableNodes:text':
      return [TextNode];
    case 'markableNodes:decorators':
      return [...DEFAULT_MARKABLE_NODES, $isDecoratorNode];
    case 'createInitialHistoryState:prefilled':
      return (editor: LexicalEditor): HistoryState => ({
        current: null,
        redoStack: [],
        undoStack: [
          {
            editor,
            editorState: editor.parseEditorState(PREFILLED_HISTORY_STATE),
          },
        ],
      });
    default:
      throw new Error(`unknown config preset ${field}: ${name}`);
  }
}

/** The config fields whose string value names a preset (`configPreset`). */
const PRESET_FIELDS = new Set([
  'shouldHandlePasteAsFiles',
  'validateUrl',
  '$canIndent',
  'createInitialHistoryState',
  'generateId',
  '$isIdentifiedBlock',
  '$getParentEditor',
  'markableNodes',
  '$shouldInsertAfter',
  'getWindow',
]);

/** A real extension's config overrides, with their presets resolved. */
function realConfig(ctx: Context, config: ExtensionConfig): ExtensionConfig {
  const out: {[key: string]: unknown} = {};
  for (const [field, value] of Object.entries(config)) {
    if (typeof value === 'string' && PRESET_FIELDS.has(field)) {
      out[field] = configPreset(ctx, field, value);
    } else if (field === 'shortcuts' && value !== null) {
      out[field] = scriptShortcuts(ctx, value as ShortcutsJson);
    } else if (
      Array.isArray(value) &&
      (field === 'matchers' || field === 'changeHandlers')
    ) {
      out[field] = value.map(name => configPreset(ctx, field, name as string));
    } else {
      out[field] = value;
    }
  }
  return out as ExtensionConfig;
}

/** The node classes an `ExtensionSpec`'s `nodes` names, by type. */
const NODES = new Map<string, Klass<LexicalNode>>([
  ['autolink', AutoLinkNode],
  ['decorator-text', DecoratorTextNode],
  ['horizontalrule', HorizontalRuleNode],
  ['mark', MarkNode],
  ['overflow', OverflowNode],
  ['test-box', TestBoxNode],
  ['test-sub-box', TestSubBoxNode],
  ['test-tag', TestTagNode],
  ['test-sub-tag', TestSubTagNode],
  ['test-text', TestTextNode],
]);

const EXTENSION_PHASES: ExtensionPhase[] = [
  'init',
  'build',
  'register',
  'afterRegistration',
];

function extensionOf(ctx: Context, key: string): AnyLexicalExtension {
  const real = REAL_EXTENSIONS.get(key);
  if (real !== undefined) {
    return real;
  }
  let made = ctx.extensions.get(key);
  if (made === undefined) {
    const spec = ctx.defs[key];
    if (spec === undefined) {
      throw new Error(`unknown extension ${key}`);
    }
    made = testExtension(ctx, spec);
    ctx.extensions.set(key, made);
  }
  return made;
}

function extensionArg(
  ctx: Context,
  extensionArgument: ExtensionArg,
): AnyLexicalExtensionArgument {
  if (typeof extensionArgument === 'string') {
    return extensionOf(ctx, extensionArgument);
  }
  const [key, ...configs] = extensionArgument;
  if (REAL_EXTENSIONS.has(key)) {
    return configExtension(
      extensionOf(ctx, key),
      ...configs.map(config => realConfig(ctx, config)),
    );
  }
  return configExtension(extensionOf(ctx, key), ...configs);
}

/** The test extension `spec` describes (see `ExtensionSpec`). */
function testExtension(ctx: Context, spec: ExtensionSpec): AnyLexicalExtension {
  const {name} = spec;
  const log = (...entry: Json[]) => {
    ctx.log.push(entry);
  };
  const phases = new Set(spec.phases ?? EXTENSION_PHASES);
  const rootText = (editor: LexicalEditor) =>
    editor.getEditorState().read(() => $getRoot().getTextContent());
  type Signals = NamedSignalsOutput<{disabled: boolean; label: string}>;
  const extension: LexicalExtension<ExtensionConfig, string, unknown, unknown> =
    {name};
  if (spec.config !== undefined) {
    extension.config = spec.config;
  }
  if (spec.namespace !== undefined) {
    extension.namespace = spec.namespace;
  }
  if (spec.editable !== undefined) {
    extension.editable = spec.editable;
  }
  if (spec.conflictsWith !== undefined) {
    extension.conflictsWith = spec.conflictsWith;
  }
  if (spec.dependencies !== undefined) {
    extension.dependencies = spec.dependencies.map(d => extensionArg(ctx, d));
  }
  if (spec.peerDependencies !== undefined) {
    extension.peerDependencies = spec.peerDependencies.map(([peer, config]) =>
      config === undefined
        ? declarePeerDependency(peer)
        : declarePeerDependency(peer, config),
    );
  }
  if (spec.theme !== undefined) {
    // A copy: the builder's `deepThemeMergeInPlace` merges later themes
    // into the objects of the first one it takes, which would change the
    // spec that every session of a vector file shares.
    extension.theme = JSON.parse(JSON.stringify(spec.theme));
  }
  if (spec.nodes !== undefined) {
    const nodes = spec.nodes;
    // `replace-text` and `replace-text-legacy` are textReplacement's.
    extension.nodes = () =>
      nodes.map(type => {
        if (type === 'replace-text' || type === 'replace-text-legacy') {
          return textReplacement(type === 'replace-text-legacy');
        }
        const klass = NODES.get(type);
        if (klass === undefined) {
          throw new Error(`unknown node type ${type}`);
        }
        return klass;
      });
  }
  if (spec.initialEditorState !== undefined) {
    const initial = spec.initialEditorState;
    if (initial === null) {
      extension.$initialEditorState = null;
    } else if ('json' in initial) {
      extension.$initialEditorState = JSON.stringify(initial.json);
    } else {
      extension.$initialEditorState = () => {
        $getRoot().append(
          $createParagraphNode().append($createTextNode(initial.text)),
        );
      };
    }
  }
  if (spec.mergeConfig === 'concat') {
    extension.mergeConfig = (config, overrides) => {
      const merged = shallowMergeConfig(config, overrides);
      for (const k in overrides) {
        const [a, b] = [config[k], overrides[k]];
        if (Array.isArray(a) && Array.isArray(b)) {
          merged[k] = [...a, ...b];
        }
      }
      return merged;
    };
  }
  if (phases.has('init')) {
    extension.init = (editorConfig, config, state) => {
      log('init', name, config);
      if (spec.inspect) {
        log('knownTypes', name, [...getKnownTypesAndNodes(editorConfig).types]);
        log('dependents', name, [...state.getDirectDependentNames()]);
        log('peerNames', name, [...state.getPeerNameSet()]);
      }
      for (const peer of spec.initPeers ?? []) {
        const dep = state.getPeer<AnyLexicalExtension>(peer);
        log('initPeer', name, peer, dep === undefined ? null : dep.config);
      }
      for (const key of spec.lookups ?? []) {
        const {config: depConfig} = state.getDependency(extensionOf(ctx, key));
        log('initDependency', name, key, depConfig);
      }
      return {init: name};
    };
  }
  if (phases.has('build')) {
    extension.build = (editor, config, state) => {
      log('build', name, config);
      for (const peer of spec.buildPeers ?? []) {
        const dep = state.getPeer<AnyLexicalExtension>(peer);
        log('buildPeer', name, peer, dep === undefined ? null : dep.config);
      }
      for (const key of spec.lookups ?? []) {
        const {config: depConfig} = state.getDependency(extensionOf(ctx, key));
        log('buildDependency', name, key, depConfig);
      }
      return spec.signals ? namedSignals(config) : undefined;
    };
  }
  if (phases.has('register')) {
    extension.register = (editor, config, state) => {
      log('register', name, config, rootText(editor));
      if (spec.initResult) {
        log('initResult', name, state.getInitResult() as Json);
      }
      const cleanups: (() => void)[] = [];
      if (spec.abort) {
        state.getSignal().addEventListener('abort', () => log('aborted', name));
      }
      if (spec.listen) {
        cleanups.push(
          editor.registerUpdateListener(({tags}) =>
            log('update', name, [...tags].sort()),
          ),
        );
      }
      const extensionCommand = command(ctx, 'EXTENSION_COMMAND');
      if (spec.signals) {
        const {disabled, label} = state.getOutput() as Signals;
        const summary = computed(() =>
          disabled.value ? 'off' : `on:${label.value}`,
        );
        cleanups.push(
          effect(() => {
            log('summary', name, summary.value);
          }),
          effect(() => {
            if (disabled.value) {
              return;
            }
            const current = untracked(() => label.value);
            log('enable', name, current);
            const unregister = editor.registerCommand(
              extensionCommand,
              payload => {
                log('handled', name, current, payload);
                return false;
              },
              COMMAND_PRIORITY_EDITOR,
            );
            return () => {
              log('disable', name);
              unregister();
            };
          }),
        );
      } else {
        cleanups.push(
          editor.registerCommand(
            extensionCommand,
            payload => {
              log('handled', name, payload);
              return false;
            },
            COMMAND_PRIORITY_EDITOR,
          ),
        );
      }
      if (spec.editorState) {
        const editorState = state.getDependency(EditorStateExtension).output;
        cleanups.push(
          effect(() => {
            log(
              'editorState',
              name,
              editorState.value.read(() => $getRoot().getTextContent()),
            );
          }),
        );
      }
      return () => {
        log('unregister', name);
        for (const cleanup of cleanups.reverse()) {
          cleanup();
        }
      };
    };
  }
  if (phases.has('afterRegistration')) {
    extension.afterRegistration = (editor, config) => {
      log('afterRegistration', name, config, rootText(editor));
      return () => log('afterUnregister', name);
    };
  }
  return extension;
}

/**
 * Every session's editor namespace, which a clipboard payload names. An
 * empty one would make `createEditor` pick one (`createUID`), from a counter
 * that every editor of the process shares.
 */
export const NAMESPACE = 'conformance';

/** A script's editor, set up with its transforms, handlers and listeners,
 * that runs steps one at a time. */
export type Session = {
  editor: LexicalEditor;
  /** What building the editor from `extensions` recorded. */
  build?: BuildResult;
  /** Runs `step` and records it; `snapshot` records the state too. */
  run(step: Step, snapshot: boolean): StepResult;
};

/** The editor's state, selection and text, as a step records them. */
function snapshotOf(
  editor: LexicalEditor,
  log: Json[],
  snapshot: boolean,
): StepResult {
  // A dispatch outside an update commits in a microtask; `editor.read`
  // commits it first.
  const result: StepResult = editor.read(() => ({
    log,
    selection: $selectionSnapshot(),
    text: $getRoot().getTextContent(),
  }));
  if (snapshot) {
    const state = editor.getEditorState();
    result.state = JSON.stringify(state.toJSON());
    result.compact = JSON.stringify(state.toJSON(true));
  }
  return result;
}

/** Starts `script` (without running its steps) on a new headless editor. */
export function startSession(script: Omit<Script, 'steps'>): Session {
  resetRandomKey();
  const ctx: Context = {
    clipboard: null,
    commands: new Map(),
    defs: script.extensionDefs ?? {},
    dom: null,
    editor: undefined as never,
    extensions: new Map(),
    handles: new Map(),
    log: [],
    markIDsListeners: new Map(),
    mounted: new Map(),
    parent: null,
    saved: new Map(),
    structures: new Map(),
    watchers: new Map(),
  };
  let build: BuildResult | undefined;
  if (script.extensions !== undefined) {
    for (const flag of [
      'richText',
      'plainText',
      'list',
      'link',
      'code',
      'overflow',
      'hashtag',
      'extension',
      'mark',
      'decoratorBlock',
    ] as const) {
      if (script[flag]) {
        throw new Error(`extensions and ${flag} don't mix`);
      }
    }
    let built: LexicalEditorWithDispose;
    try {
      const parent =
        script.parent === undefined
          ? null
          : buildEditorFromExtensions(
              ...script.parent.map(e => extensionArg(ctx, e)),
            );
      ctx.parent = parent;
      const extensions = script.extensions.map(e => extensionArg(ctx, e));
      built = withWarnings(ctx, script.warnings === true, () =>
        parent === null || script.parentRead === false
          ? buildEditorFromExtensions(...extensions)
          : parent.read(() => buildEditorFromExtensions(...extensions)),
      );
    } catch (e) {
      build = {error: (e as Error).message, log: ctx.log};
      return {
        build,
        editor: undefined as never,
        run: () => {
          throw new Error('the editor failed to build');
        },
      };
    }
    ctx.editor = built;
    if (script.liveRegion) {
      ctx.log.push(liveRegionEntry(ctx));
    }
    if (ctx.parent !== null) {
      ctx.log.push(nestedEntry(built, ctx.parent));
    }
    build = snapshotOf(built, ctx.log, true) as BuildResult;
  } else {
    ctx.editor = (script.headless ? createHeadlessEditor : createEditor)({
      ...(script.editable === undefined ? {} : {editable: script.editable}),
      namespace: NAMESPACE,
      nodes: [
        ...(script.richText ? [HeadingNode, QuoteNode] : []),
        ...(script.list ? [ListNode, ListItemNode] : []),
        ...(script.link ? [LinkNode, AutoLinkNode] : []),
        ...(script.code ? [CodeNode, CodeHighlightNode] : []),
        ...(script.overflow ? [OverflowNode] : []),
        ...(script.hashtag ? [HashtagNode] : []),
        ...(script.extension ? [HorizontalRuleNode, DecoratorTextNode] : []),
        ...(script.mark ? [MarkNode] : []),
        ...(script.decoratorBlock ? [TestDecoratorBlockNode] : []),
        ...(script.testText || script.replaceText ? [TestTextNode] : []),
        ...(script.replaceText ? [textReplacement(false)] : []),
        ...(script.testDecorators
          ? [TestDecoratorNode, TestSharedDecoratorNode]
          : []),
      ],
      onError: e => {
        throw e;
      },
    });
  }
  const {editor} = ctx;
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
  let unregisterCodeHighlighting: (() => void) | undefined;
  if (script.code) {
    // `CodeExtension.register` reads only the editor.
    (CodeExtension.register as (e: LexicalEditor) => () => void)(editor);
    if (script.codeHighlight === undefined) {
      registerCodeIndentation(editor);
    } else {
      unregisterCodeHighlighting = registerCodeHighlighting(
        editor,
        scriptTokenizer(script.codeHighlight),
      );
    }
  }
  if (script.hashtag) {
    registerLexicalHashtag(editor);
  }
  if (script.markdown) {
    registerMarkdownShortcuts(editor, TRANSFORMERS);
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
        ctx.log.push(['handler', handler.id, payloadJson(payload)]);
        for (const action of handler.actions ?? []) {
          $runAction(action, ctx);
        }
        return handler.result;
      },
      handler.priority ?? COMMAND_PRIORITY_EDITOR,
    );
  }
  for (const type of script.mutations ?? []) {
    const registered = editor._nodes.get(type);
    if (registered === undefined) {
      throw new Error(`unknown node type ${type}`);
    }
    editor.registerMutationListener(
      registered.klass,
      (mutations, {updateTags}) => {
        ctx.log.push([
          'mutation',
          type,
          [...mutations],
          [...updateTags].sort(),
        ]);
      },
      {skipInitialization: true},
    );
  }
  if (script.decorators) {
    editor.registerDecoratorListener(decorators => {
      ctx.log.push(['decorators', decoratorEntries(decorators)]);
    });
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
  if (script.mutatedNodes) {
    editor.registerUpdateListener(({mutatedNodes}) => {
      ctx.log.push([
        'mutatedNodes',
        mutatedNodes === null
          ? null
          : [...mutatedNodes].map(([klass, mutations]) => [
              klass.getType(),
              [...mutations],
            ]),
      ]);
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
    if ('setRootElement' in step) {
      const {root} = sessionDOM(ctx);
      if (script.rootFocus) {
        root.focus = () => {
          ctx.log.push(['rootFocus']);
        };
      }
    }
    selectionTransformCtx = ctx;
    return atClock(() =>
      withDOMGlobals(ctx, () =>
        withWarnings(ctx, script.warnings === true, () =>
          runAtClock(step, snapshot),
        ),
      ),
    );
  };
  const runAtClock = (step: Step, snapshot: boolean): StepResult => {
    ctx.log = [];
    let error = false;
    try {
      if (step.inParent) {
        if (ctx.parent === null) {
          throw new Error('the script has no parent editor');
        }
        ctx.editor = ctx.parent;
        try {
          runStepOn(ctx.parent, step);
        } finally {
          ctx.editor = editor;
        }
      } else {
        runStepOn(editor, step);
      }
    } catch {
      error = true;
    }
    if (script.liveRegion) {
      ctx.log.push(liveRegionEntry(ctx));
    }
    if (script.codeHighlight !== undefined) {
      ctx.log.push(codeBlocksEntry(editor));
    }
    if (script.textFormats !== undefined) {
      ctx.log.push(textFormatsEntry(editor, script.textFormats));
    }
    if (ctx.parent !== null) {
      ctx.log.push(nestedEntry(editor, ctx.parent));
    }
    const result = snapshotOf(editor, ctx.log, snapshot);
    if (error) {
      result.error = true;
    }
    return result;
  };
  /** Runs `step` on `target`, the session's editor or (`inParent`) the parent. */
  const runStepOn = (target: LexicalEditor, step: Step): void => {
    if ('update' in step) {
      target.update(
        () => {
          for (const action of step.update) {
            $runAction(action, ctx);
          }
        },
        {
          discrete: true,
          onUpdate: step.onUpdate
            ? () => {
                ctx.log.push(['onUpdate']);
              }
            : undefined,
          tag: step.tag,
        },
      );
    } else if ('readDispatch' in step) {
      const name = step.readDispatch;
      const payload = step.payload ?? null;
      ctx.log.push([
        'dispatched',
        name,
        target.read(() => target.dispatchCommand(command(ctx, name), payload)),
      ]);
    } else if ('focus' in step) {
      target.focus(() => {
        ctx.log.push(['focused']);
      }, step.focus);
    } else if ('dispatch' in step) {
      dispatchLogged(ctx, step.dispatch, step.payload ?? null, target);
    } else if ('setRootElement' in step) {
      const root = step.setRootElement ? sessionDOM(ctx).root : null;
      if (step.viewSelection) {
        const win = sessionDOM(ctx).window;
        const event = new win.Event('lexical-conformance-view');
        Object.defineProperty(win, 'event', {
          configurable: true,
          get: () => event,
        });
      }
      target.setRootElement(root);
    } else if ('computedStyles' in step) {
      stubComputedStyles(sessionDOM(ctx).window, target, step.computedStyles);
    } else if ('setEditable' in step) {
      target.setEditable(step.setEditable);
    } else if ('domEvent' in step) {
      dispatchDomEvent(ctx, target, step.domEvent);
    } else if ('logEditor' in step) {
      ctx.log.push([
        'editor',
        target._config.namespace,
        target.isEditable(),
        target.getRootElement() !== null,
      ]);
    } else if ('announce' in step) {
      getExtensionDependencyFromEditor(
        target,
        AriaLiveRegionExtension,
      ).output.announce(step.announce);
    } else if ('unregisterCodeHighlighting' in step) {
      if (unregisterCodeHighlighting === undefined) {
        throw new Error('no code highlighting to unregister');
      }
      unregisterCodeHighlighting();
    } else if ('typeahead' in step) {
      const spec = step.typeahead;
      target.getEditorState().read(() => {
        const [textBefore, match] = $typeaheadMatch(spec, target);
        ctx.log.push([
          'typeahead',
          textBefore,
          match,
          match === null
            ? null
            : REACT_INTERNALS.INTERNAL_isSelectionOnEntityBoundary(
                target,
                match.leadOffset,
              ),
        ]);
      });
    } else if ('setEditorState' in step) {
      const state = ctx.saved.get(step.setEditorState);
      if (state === undefined) {
        throw new Error(`no saved state ${step.setEditorState}`);
      }
      target.setEditorState(state);
    } else if ('characterLimit' in step || 'tableOfContents' in step) {
      mountStep(step, ctx);
    } else if ('registerNodeStructure' in step) {
      ctx.structures.set(
        step.registerNodeStructure,
        registerNodeStructure(target),
      );
    } else if ('disposeNodeStructure' in step) {
      const dispose = ctx.structures.get(step.disposeNodeStructure);
      if (dispose === undefined) {
        throw new Error(`no registration ${step.disposeNodeStructure}`);
      }
      dispose();
    } else if ('unmount' in step) {
      const unmount = ctx.mounted.get(step.unmount);
      if (unmount === undefined) {
        throw new Error(`nothing mounted as ${step.unmount}`);
      }
      ctx.mounted.delete(step.unmount);
      unmount();
    } else {
      runExtensionStep(step, ctx);
    }
  };
  return build === undefined ? {editor, run} : {build, editor, run};
}

/**
 * `['codeBlocks', [[key, language, isSyntaxHighlightSupported, childKeys]...]]`
 * of the code blocks among the root's children: what the JSON doesn't show
 * (`language` is `null` when it is `undefined`), and the keys of the
 * children, which show which ones the highlighter kept.
 */
function codeBlocksEntry(editor: LexicalEditor): Json {
  return editor.read(() => [
    'codeBlocks',
    $getRoot()
      .getChildren()
      .filter($isCodeNode)
      .map(n => [
        n.getKey(),
        n.getLanguage() ?? null,
        n.getIsSyntaxHighlightSupported(),
        n.getChildrenKeys(),
      ]),
  ]);
}

/** `['textFormats', [[key, textFormat, textStyle]...]]` (`textFormats`). */
function textFormatsEntry(editor: LexicalEditor, keys: string[]): Json {
  return editor.getEditorState().read(() => [
    'textFormats',
    keys.map(key => {
      const n = $getNodeByKey(key);
      return $isElementNode(n)
        ? [key, n.getTextFormat(), n.getTextStyle()]
        : [key, null, null];
    }),
  ]);
}

/**
 * `['nested', isNested, parentIsParent, sameNamespace, theme, editable,
 * parentEditable, parentText, parentSelection]`: `$isEditorIsNestedEditor`
 * of the session's editor, whether its `_parentEditor` is the script's
 * `parent`, whether the two share a namespace, the `[key, value]` entries
 * of its `_config.theme` sorted by key, whether each is editable, and the
 * parent's text and selection (read with `parent.read`, which commits it).
 */
function nestedEntry(editor: LexicalEditor, parent: LexicalEditor): Json {
  const theme = editor._config.theme as {[key: string]: Json};
  const [parentText, parentSelection] = parent.read(() => [
    $getRoot().getTextContent(),
    $selectionSnapshot(),
  ]);
  return [
    'nested',
    $isEditorIsNestedEditor(editor),
    editor._parentEditor === parent,
    editor._config.namespace === parent._config.namespace,
    Object.keys(theme)
      .sort()
      .map(k => [k, theme[k]]),
    editor.isEditable(),
    parent.isEditable(),
    parentText,
    parentSelection,
  ];
}

/** Runs `script` on a new headless editor and records every step. */
export function runScript(script: Script): ScriptResult {
  const {AbortController} = globalThis;
  const session = startSession(script);
  let results: StepResult[];
  try {
    results = script.steps.map((step, i) =>
      session.run(
        step,
        script.snapshot !== 'final' || i === script.steps.length - 1,
      ),
    );
  } finally {
    globalThis.AbortController = AbortController;
  }
  return session.build === undefined
    ? {...script, results}
    : {...script, build: session.build, results};
}

/** A step of an editor built from extensions. */
function runExtensionStep(
  step: Exclude<
    Step,
    | {update: Action[]}
    | {readDispatch: string}
    | {focus: {defaultSelection?: 'rootStart' | 'rootEnd'}}
    | {dispatch: string}
    | {setRootElement: boolean}
    | {computedStyles: unknown}
    | {setEditable: boolean}
    | {domEvent: DomEventJson}
    | {logEditor: true}
    | {announce: string}
    | {typeahead: TypeaheadSpec}
    | {characterLimit: unknown}
    | {tableOfContents: true}
    | {unmount: string}
    | {unregisterCodeHighlighting: true}
    | {setEditorState: string}
    | {registerNodeStructure: string}
    | {disposeNodeStructure: string}
  >,
  ctx: Context,
): void {
  const editor = ctx.editor;
  if ('dispose' in step) {
    (editor as LexicalEditorWithDispose).dispose();
  } else if ('setSignals' in step) {
    batch(() => {
      for (const [key, name, value] of step.setSignals) {
        const output = getExtensionDependencyFromEditor(
          editor,
          extensionOf(ctx, key),
        ).output as NamedSignalsOutput<ExtensionConfig>;
        output[name].value = value;
      }
    });
  } else if ('hasExtensions' in step) {
    const builder = LexicalBuilder.fromEditor(editor);
    for (const name of step.hasExtensions) {
      ctx.log.push(['hasExtension', name, builder.hasExtensionByName(name)]);
    }
  } else if ('logHistory' in step) {
    const {output} = getExtensionDependencyFromEditor(editor, HistoryExtension);
    const {redoStack, undoStack} = output.historyState.value;
    ctx.log.push([
      'history',
      output.canUndo.value,
      output.canRedo.value,
      undoStack.length,
      redoStack.length,
    ]);
  } else if ('watchNodeKey' in step) {
    const key = step.watchNodeKey;
    if (ctx.watchers.has(key)) {
      throw new Error(`already watching ${key}`);
    }
    const selected = getExtensionDependencyFromEditor(
      editor,
      NodeSelectionExtension,
    ).output.watchNodeKey(key);
    ctx.watchers.set(
      key,
      effect(() => {
        ctx.log.push(['nodeSelected', key, selected.value]);
      }),
    );
  } else if ('registerMarkIDsListener' in step) {
    const {name, flavor} = step.registerMarkIDsListener;
    if (ctx.markIDsListeners.has(name)) {
      throw new Error(`already listening as ${name}`);
    }
    const sorted = (set: ReadonlySet<string>) => Array.from(set).sort();
    ctx.markIDsListeners.set(
      name,
      registerMarkIDsListener(
        editor,
        markFlavor(flavor)!,
        ({added, ids, removed, tags}) => {
          ctx.log.push([
            'markIDsChange',
            name,
            sorted(added),
            sorted(removed),
            sorted(ids),
            sorted(tags),
          ]);
        },
      ),
    );
  } else if ('unregisterMarkIDsListener' in step) {
    const dispose = ctx.markIDsListeners.get(step.unregisterMarkIDsListener);
    if (dispose === undefined) {
      throw new Error(`no listener ${step.unregisterMarkIDsListener}`);
    }
    ctx.markIDsListeners.delete(step.unregisterMarkIDsListener);
    dispose();
  } else if ('logMarkIDNodeKeys' in step) {
    const keys = getExtensionDependencyFromEditor(
      editor,
      markFlavor(step.logMarkIDNodeKeys)!,
    ).output.markIDNodeKeys.peek();
    ctx.log.push([
      'markIDNodeKeys',
      step.logMarkIDNodeKeys,
      Array.from(keys.keys())
        .sort()
        .map(id => [id, [...keys.get(id)!].sort()]),
    ]);
  } else if ('unwatchNodeKey' in step) {
    const dispose = ctx.watchers.get(step.unwatchNodeKey);
    if (dispose === undefined) {
      throw new Error(`not watching ${step.unwatchNodeKey}`);
    }
    ctx.watchers.delete(step.unwatchNodeKey);
    dispose();
  } else if ('logIME' in step) {
    const {compositionKey, composingTextNode} =
      getExtensionDependencyFromEditor(editor, IMEExtension).output;
    const node = composingTextNode.peek();
    ctx.log.push([
      'ime',
      compositionKey.peek(),
      node === null ? null : node.getKey(),
    ]);
  } else if ('logDependency' in step) {
    const {config} = getExtensionDependencyFromEditor(
      editor,
      extensionOf(ctx, step.logDependency),
    );
    ctx.log.push(['dependency', step.logDependency, config]);
  } else {
    const dep = getPeerDependencyFromEditor<AnyLexicalExtension>(
      editor,
      step.logPeer,
    );
    ctx.log.push([
      'peer',
      step.logPeer,
      dep === undefined ? null : (dep.config as Json),
    ]);
  }
}
