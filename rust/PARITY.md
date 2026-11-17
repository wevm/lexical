# Parity with JS Lexical

What the Rust port (`lexical-ui`) has ported from the upstream packages,
how it is checked, and where it deliberately differs. The conformance runner
(`cargo run -p lexical-conformance`) prints the live numbers; this file is
updated with each milestone.

## Ported

| Area | JS source | Rust | Vectors |
| --- | --- | --- | --- |
| JSON parse and stringify | `JSON.parse`, `JSON.stringify`, property order, `Number#toString` | `json` | every vector file is read with it; unit tests against Node's output |
| Constants | `LexicalConstants.ts` (flags and name tables) | `constants` | through the nodes |
| Export form | `LexicalSerializedExport.ts` | `serialized_export` | `serialization/core` |
| Generated JSON of the core nodes | `LexicalGeneratedJSON.ts` | `generated_json` | `serialization/core`, `property/documents` |
| Node schemas | `LexicalSchema.ts`: `SerializationSchemaMeta`, `NumberValueOptions`, `isSchemaEqual`, `isSchemaDefault`, `declaredAccepts`, the builders (`stringValue`, `numberValue` with integer bounds rounded inward and clamping, `booleanValue`, `enumValue` with SameValueZero, `nullable` with `defaultAsNull`, `optional` with `omitDefault`, `unionValue` with `$bestUnionMember`, `$fitOf`, `$metaFitOf` and the catch-all rule, `aliasedValue`, `transformValue`, `rawValue`, `arrayValue`, `arraySetValue` in UTF-16 order without duplicates, `objectValue`), their refusals with JS's messages (`undeclared`, `notNestedSet`, the `Object.prototype` field names), `nodeSchema`, `withField` (with `when` and `derived`), `withAccessors`; and from `LexicalUtils.ts` the composed schema (`getComposedSchemaFields`, flat states first), `$exportNodeJSONOnce`'s walk (most derived first, `when`, the compact form's defaults and import-derived properties left out), `$applyJSONSetters`'s walk (flat states, then setters base first); the `$config` `json` schemas of `ElementNode`, `TextNode`, `TabNode`, `HeadingNode`, `LinkNode`, `AutoLinkNode`, `ListNode`, `ListItemNode`, `CodeNode`, `CodeHighlightNode`, `MarkNode` and `DecoratorBlockNode`; the structure's `derived` values read from them (`derivedNamesOf`) | `schema`, `LexicalNode::json_schema_chain`, `export_json`, `update_from_json` | `serialization/schema`, `serialization/schema-node`; every `serialization/*` and `property/*` case checks each node's generated JSON against the walk; a unit test does for hostile values |
| Core nodes | `nodes/LexicalRootNode.ts`, `LexicalParagraphNode.ts`, `LexicalTextNode.ts`, `LexicalTabNode.ts`, `LexicalLineBreakNode.ts`, `LexicalElementNode.ts` (fields, `getTextContent`, `shouldSerializeTextStyles`) | `nodes::*` | `serialization/core`, `property/documents` |
| Node methods | `LexicalNode.ts` (`getLatest`, `getWritable`, `isAttached`, `getIndexWithinParent`, `remove`/`$removeNode`, `replace`, `insertBefore`, `insertAfter`, `select*`, `markDirty`, `getTopLevelElement`, `getTopLevelElementOrThrow`, `getParentKeys`); of `ElementNode`, `getChildrenKeys`, `isLastChild`, `getFirstChildOrThrow`, `getLastChildOrThrow`, `hasTextFormat`, `getFormatFlags` and the deprecated `canReplaceWith` and `canInsertAfter` (node hooks); of `TextNode`, `getFormatFlags` and `isDirectionless` | `node`, `utils` | `scripts/updates`, `scripts/transforms`, `scripts/core-api` |
| Element methods | `LexicalElementNode.ts` (`splice` with selection repair, `append`, `clear`, `select`, children and descendants, the setters), `RootNode.splice` and its other overrides | `nodes::element`, `node` | `scripts/updates` |
| Text methods | `LexicalTextNode.ts` (`splitText`, `mergeWithSibling`, `spliceText`, `setTextContent`, `select`, the format, detail, mode and style setters and toggles), `toggleTextFormatType`, `TabNode`'s overrides | `nodes::text` | `scripts/updates`, `scripts/listeners`, `scripts/transforms` |
| Tree surgery | `caret/LexicalCaretTree.ts` (`$insertNodeBetween`, `$detachNode`, `$detachSiblingRange`, `$detachNodeWithSelection`, `$insertSibling`, `$selectAdjacentNode`) | `caret_tree` | `scripts/updates` |
| Selection model | `LexicalSelection.ts`: points, `RangeSelection` and `NodeSelection` fields, `$getSelection`, `$setSelection`, `$createRangeSelection`, and the helpers node methods use to keep a selection valid | `selection` | `scripts/updates` |
| Selection reading | `LexicalSelection.ts`: `Point.isBefore`, `isBackward` across nodes, `RangeSelection.getNodes` (`$getNodesFromCaretRangeCompat`) and `getTextContent`, `NodeSelection.getNodes` and `getTextContent`, the `_cachedNodes` cache | `selection` | `scripts/caret`; a unit test against JS's output |
| Carets | `caret/LexicalCaret.ts`: sibling, child and text point carets in both directions and their methods (`getAdjacentCaret`, `getNodeAtCaret`, `getFlipped`, `getParentCaret`, `getChildCaret`, `insert`, `remove`, `replaceOrInsert`, `splice`, iteration), `$getSiblingCaret`, `$getChildCaret`, `$getTextPointCaret`, `$getTextNodeOffset`, text slices, caret ranges (`$getCaretRange`, `iterNodeCarets`, `getTextSlices`, `isCollapsed`), `$getAdjacentChildCaret`, `$getCaretInDirection`, `$extendCaretToRange`, `$comparePointCaretNext`, `$getCommonAncestor`, `makeStepwiseIterator` | `caret` | `scripts/caret` |
| Caret utilities | `caret/LexicalCaretUtils.ts`: `$caretFromPoint`, `$setPointFromCaret`, `$caretRangeFromSelection`, `$setSelectionFromCaretRange`, `$updateRangeSelectionFromCaretRange`, `$rewindSiblingCaret`, `$removeTextFromCaretRange` (block merges, empty-container repair), `$normalizeCaret`, `$isExtendableTextPointCaret`, `$getCaretRangeInDirection`, `$getChildCaretAtIndex`, `$getAdjacentSiblingOrParentSiblingCaret`, `$getTextPointCaretSliceForNode`; with `$getNodeByKeyOrThrow`, `$removeFromParent`, `$restoreEmptyContainerParagraph`, `$isShadowRootNode` and `INTERNAL_$isBlock` from `LexicalUtils.ts` | `caret_utils`, `utils` | `scripts/caret` |
| Selection editing | `LexicalSelection.ts`: `insertText` (`$insertText`, `$insertTextAtPoint`), `insertRawText` (`tokenizeRawText`, `$generateNodesFromRawText`), `insertNodes` for both selection kinds (`$removeTextAndSplitBlock`, `$splitNodeAtPoint`, `$wrapInlineNodes`), `insertParagraph`, `insertLineBreak`, `removeText`, `formatText` (`$updateTextFormat`, `$setTextFormat`, `$getCharacterOffsets`), `toggleFormat`, `setFormat`, `setStyle`, `deleteCharacter`, `deleteWord`, `deleteLine` and `modify` (`forwardDeletion`, `$extendSelectionForDeletion`, `$modifySelectionAroundDecoratorsAndBlocks`, `$collapseAtStart`, `$removeSegment`, `$updateCaretSelectionForUnicodeCharacter`, `$stopLineDeletionAtLineBreak`, `$shrinkSelectionToRoot`, and the model side of `applyDOMRange`), with an editor's `SelectionMovement` standing in for `Selection.modify`, `$insertNodes`, `$selectAll`; `$splitTextPointCaretSlice` and `$isBlockFullySelected`; `ParagraphNode.insertNewAfter`, `canInsertTextBefore`/`After` | `selection_editing`, `selection_insert_text`, `selection_insert_nodes`, `selection_format`, `selection_deletion` | `scripts/selection`, `fuzz/core`, `property/sequences`; unit tests against JS's output for paths no vector covers |
| Update cycle | `LexicalUpdates.ts` (`$beginUpdate`, nested and queued updates, `$applyAllTransforms`, `$commitPendingUpdates`, `$discardPendingUpdate`, listeners, `triggerCommandListeners`, `updateEditorSync`, the update cascade cut short through `onWarn`, the selection notification: `$notifyPendingSelectionChange` dispatching `SELECTION_CHANGE_COMMAND` in the pending update before each commit while the selection differs from `_lastNotifiedSelection`, up to 100 times, a range selection only with a connected root element (`RootElement::is_connected`)), `LexicalSelection.ts`'s `$applySelectionTransforms` with `TextNode.selectionTransform` as a node hook, `LexicalNormalization.ts` (`$normalizeTextNode`), `LexicalGC.ts` (nodes) | `updates`, `normalization`, `gc`, `lexical_selection` | `scripts/updates`, `scripts/listeners`, `scripts/transforms`, `scripts/commands`, `scripts/core-api` |
| Editor state | `LexicalEditorState.ts` (`toJSON`, `read`, `isEmpty`, `_parsed`) | `editor_state` | `serialization/core` |
| Parsing | `parseEditorState`, `$parseSerializedNode` (`LexicalUpdates.ts`) | `updates` | `serialization/core`, `property/documents` |
| NodeState | `LexicalNodeState.ts`: `createState` (`StateConfig` with `parse`, `unparse`, `isEqual`, `defaultValue` and `resetOnCopyNode`), `$getState` (latest and direct), `$setState` with a value or an updater, `$getStateChange`, `$getWritableNodeState`, `NodeState` (`getValue`, `toJSON`, `getWritable` with `parseAndPruneNextUnknownState`, `updateFromKnown`, `updateFromUnknown`, `updateFromJSON`, `resetOnCopyNode`, the size count), `$getSharedNodeState` and `createSharedNodeState` (a class's `stateConfigs`, flat keys, key collisions), `$updateStateFromJSON` (`Object.entries` of a `"$"` that is not an object, too), `$applyFlatStates`, `$cloneNodeState`, `$copyNodeState`, `$carryNodeState`, `nodeStatesAreEquivalent`; their call sites: the `"$"` key and flat states in `exportJSON` and `$parseSerializedNode`, `$copyNode`, `splitText`, the merge of `$normalizeTextNode`, history's JSON, a segmented node made plain text (`$insertText`, `$removeTextFromCaretRange`); `QuoteNode`'s `quoteShadowRootState` as a flat state | `node_state`, `node`, `nodes::text`, `normalization`, `updates`, `rich_text`, `caret_utils`, `selection_insert_text` | `scripts/node-state`, the node state case of `serialization/core` |
| DecoratorNode | `nodes/LexicalDecoratorNode.ts` (`isInline`, `isIsolated`, `isKeyboardSelectable`, `decorate` as a hook returning an app's value, `$isDecoratorNode`) as `LexicalNode` hooks (`is_decorator`, `decorate`, `is_isolated`, `is_keyboard_selectable`); with a root element, `editor._decorators` and `_pendingDecorators` filled by the reconciler's `reconcileDecorator` (`cloneDecorators`; a value is an `Rc<dyn Any>` compared by `Rc::ptr_eq`, as JS compares with `===`), `getDecorators()` (`Decorators`, in `Object.entries` order), `registerDecoratorListener`, `$garbageCollectDetachedDecorators` and the commit's `'decorator'` listeners after the mutation listeners; where the model treats decorators: `INTERNAL_$isBlock`, `$isLeafNode` (`formatList.ts`), `$isInlineElementOrDecoratorNode`, `$needsBlockCursorBeside`, `RootNode.splice` and `errorOnInsertTextNodeOnRoot`, `RangeSelection.getTextContent`, `insertNodes` (CASE 2's `notInline`, `$extractInlineFromBlocks`, `$isInlineRunNode`), `deleteCharacter`'s decorator branch (an isolated decorator stays; one beside an empty block is selected as a node, else removed), `$modifySelectionAroundDecoratorsAndBlocks` (stepping over, stopping at an isolated one, a node selection of a keyboard-selectable one); `$isInlineFormattable` and `InlineFormattableNode` (`is_inline_formattable`, `get_inline_format`, `set_inline_format`) in `$updateTextFormat` | `node`, `lexical_utils`, `caret_tree`, `nodes::element`, `lexical_selection`, `selection_insert_nodes`, `selection_deletion`, `selection_format`, `list`, `reconciler`, `editor`, `updates` | `scripts/decorator`, `scripts/reconciler` |
| Editor | `createEditor` (node registration, `editorState`, `editable`, `onWarn`), `update` (`onUpdate`), `read`, `getEditorState`, `setEditorState`, `registerNodeTransform`, `registerUpdateListener` (with `mutatedNodes`), `registerTextContentListener`, `registerCommand` with the `COMMAND_PRIORITY_*` and `COMMAND_PRIORITY_BEFORE_*` priorities (a before-priority listener goes to the front of its priority's queue), `dispatchCommand`, `createCommand`, `hasNode`, `hasNodes`, `focus` with `EditorFocusOptions` (the selection it restores or makes, `FOCUS_TAG`, the callback through `$onUpdate`; `RootElement::focus` and `has_focus` for the view); `$getEditor`, `$addUpdateTag` and the update tags of `LexicalUpdateTags.ts` (`SKIP_COLLAB_TAG`, `SKIP_SCROLL_INTO_VIEW_TAG`, `SKIP_DOM_SELECTION_TAG`, `FOCUS_TAG` beside those ported before); `$nodesOfType`, `$getAdjacentNode`, `$cloneWithPropertiesEphemeral` (the same clone as `$cloneWithProperties`, which is already outside every editor state); `LexicalRefCountedRegistry.ts` (`createRefCountedRegistry`) | `editor`, `lexical_utils`, `ref_counted_registry` | `scripts/*`, `scripts/core-api`; unit tests for `hasNodes`, `$getEditor`, `editorState`, `focus` without a root element, `onWarn` and the ref-counted registry |
| Node replacements | `LexicalNodeReplacement` (`{replace, with, withKlass}`, a `CreateEditorArgs::nodes` entry through `From`), `createEditor`'s check that `withKlass` extends the replaced class and its warning for a replacement without one, `resolveRegisteredNodeAfterReplacements`, `$create` and `importJSON` resolving a replacement, `$applyNodeReplacement` with its invariants (called by the `$create*` functions JS wraps in it: text, paragraph, tab, line break, heading, quote, list, list item, link, autolink, mark, overflow, hashtag, code highlight), transforms and mutation listeners of the replaced class reaching `withKlass`'s nodes; the extension API's replacements in `nodes` and `LexicalBuilder`'s conflict invariant | `editor`, `lexical_utils`, `extension::lexical_builder` | `scripts/core-api` |
| Developer warnings | `console.warn` where the ported code calls it, through a per-thread handler (`with_warning_handler`, as Swift's `withWarningHandler`; standard error without one): a replacement without `withKlass`, `editor.read()` while commit listeners run with an update pending, an update started from a read (`updateEditorSync`), `NormalizeInlineElementsExtension`'s `canBeEmpty` check, `BlockIdExtension`'s repair update that left blocks without a unique id, a duplicate DOM import rule name, and `warnOnlyOnce`'s `$addNodeStyle` and `CodeNode` without `CodeExtension` (once per thread) | `warnings` | `scripts/core-api` (warnings logged through the hook); a unit test of the handler |
| `@lexical/headless` | `createHeadlessEditor` (`_headless`, refusing any root element) and `HeadlessExtension` (refusing a root element but allowing none), as `create_headless_editor` and `headless::headless_extension`; a headless editor never reconciles, and a nested editor of a headless one is headless; `getRootElement` and `registerRootListener`, which JS makes throw, can't fail in Rust (an `Option` and an `Unregister`), so they behave as on an editor with no root, as in Swift | `headless`, `editor` | `scripts/core-api` |
| History (the `history` feature) | `packages/lexical-history/src/index.ts`: `registerHistory` (with `dateNow`, `onHistoryStateChange` and `maxDepth`), `createEmptyHistoryState`, `createMergeActionGetter`, `getChangeType`, `getDirtyNodes`, `isTextNodeUnchanged`, `areNodesUnchanged`, `getInverseEntry`, `getUndoEntry`, `undo`, `redo`, `clearHistory`; the commands it handles and dispatches from `LexicalCommands.ts` (`UNDO_COMMAND`, `REDO_COMMAND`, `CAN_UNDO_COMMAND`, `CAN_REDO_COMMAND`, `CLEAR_HISTORY_COMMAND`, `CLEAR_EDITOR_COMMAND`) and its tags from `LexicalUpdateTags.ts` | `history`, `commands`, `editor` | `scripts/history`, `property/sequences`; unit tests for a history shared by two editors (upstream's #8623 test) and `maxDepth` |
| Rich text nodes (the `rich-text` feature) | `packages/lexical-rich-text/src/index.ts`: `HeadingNode` (`getTag`, `setTag`, `insertNewAfter`, `collapseAtStart`, `extractWithChild`), `QuoteNode` (`insertNewAfter`, `collapseAtStart`, `canMergeWhenEmpty`, the opt-in shadow root), `$createHeadingNode`, `$isHeadingNode`, `$createQuoteNode`, `$isQuoteNode`; their generated JSON (`LexicalRichTextGeneratedJSON.ts`); `ElementNode.collapseAtStart` as a node hook, and `ElementNode`'s own `$transform` (`$normalizeShadowRootChildren`); from `@lexical/selection` and `@lexical/utils` only `$isAtEdgeOfElement`, `$isAtStartOfNode`, `$isAtEndOfNode`, `$findMatchingParent` and `$handleIndentAndOutdent` | `rich_text`, `selection`, `utils`, `updates` | `scripts/rich-text`, the heading and quote cases of `property/documents` |
| `registerRichText` (the `rich-text` feature) | `packages/lexical-rich-text/src/index.ts` `registerRichText`: the handlers of `DELETE_CHARACTER_COMMAND` (with `NodeSelection.deleteNodes`), `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `FORMAT_TEXT_COMMAND`, `SET_TEXT_FORMAT_COMMAND`, `FORMAT_ELEMENT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND`, `OUTDENT_CONTENT_COMMAND`, `SELECT_ALL_COMMAND`, and `KEY_ENTER_COMMAND` (with `$escapeFormatsForTrigger` on `DEFAULT_ESCAPE_FORMAT_TRIGGERS`); its handlers of the key and pointer commands: `CLICK_COMMAND` (a node selection cleared unless the click's `target_key` is in it, `escapeFormatTriggers`' `click`), `KEY_ARROW_UP_COMMAND`, `KEY_ARROW_DOWN_COMMAND`, `KEY_ARROW_LEFT_COMMAND` and `KEY_ARROW_RIGHT_COMMAND` (`$exitNodeSelectionToward`, `$convertContiguousNodeSelection`, `$promoteNodeSelectionToBlockEdge`, the block cursor at the root's edges and beside shadow roots, `$tryDecoratorLineNavigation` through the editor's `SelectionMovement`, `$shouldOverrideDefaultCharacterSelection` and `$moveCharacter`, the `arrow` trigger with `onlyAtBoundary`), `KEY_BACKSPACE_COMMAND` (outdenting an indented block at its start), `KEY_DELETE_COMMAND`, `KEY_ESCAPE_COMMAND` (`editor.blur()`, `RootElement::blur`), `KEY_SPACE_COMMAND`, `KEY_TAB_COMMAND`, `MOVE_TO_START` and `MOVE_TO_END` (past an inline decorator at a block's start, a node selection collapsed to its block's edge), `DROP_COMMAND`, `DRAGSTART_COMMAND` and `DRAGOVER_COMMAND` (with the clipboard row's drag and drop), and `PASTE_COMMAND`'s files (`eventFiles`, `isMimeType`, `shouldHandlePasteAsFiles`, `DRAG_DROP_PASTE`); `RichTextConfig` (`escapeFormatTriggers` with `TriggerConfig`, `shouldHandlePasteAsFiles` with `defaultShouldHandlePasteAsFiles`), as `register_rich_text_with_config`; those commands from `LexicalCommands.ts`, with the events of the event commands row; `ElementNode.canIndent`; `editor.getKey()` and `editor.blur()` | `rich_text::register_rich_text`, `commands`, `selection_deletion`, `utils`, `clipboard`, `editor` | `scripts/rich-text-commands`, `scripts/key-handlers`, `property/sequences` |
| `registerPlainText` (the `plain-text` feature) | `packages/lexical-plain-text/src/index.ts` `registerPlainText`: the handlers of `DELETE_CHARACTER_COMMAND`, `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND` (a line break), `KEY_ENTER_COMMAND`, `SELECT_ALL_COMMAND`, `KEY_ARROW_LEFT_COMMAND` and `KEY_ARROW_RIGHT_COMMAND` (`$shouldOverrideDefaultCharacterSelection`, `$moveCharacter`), `KEY_BACKSPACE_COMMAND`, `KEY_DELETE_COMMAND`, `DROP_COMMAND` (`$handlePlainTextDrop`) and `DRAGSTART_COMMAND` (the drag marker) | `plain_text` | `scripts/plain-text`, `scripts/key-handlers` |
| Event commands and payloads | `LexicalCommands.ts`'s DOM event commands (`SELECTION_CHANGE_COMMAND`, `CLICK_COMMAND`, `BEFORE_INPUT_COMMAND`, `INPUT_COMMAND`, `COMPOSITION_START_COMMAND`, `COMPOSITION_END_COMMAND`, `KEY_DOWN_COMMAND` (named `KEYDOWN_COMMAND`, as in JS), `KEY_ARROW_RIGHT_COMMAND`, `MOVE_TO_END`, `KEY_ARROW_LEFT_COMMAND`, `MOVE_TO_START`, `KEY_ARROW_UP_COMMAND`, `KEY_ARROW_DOWN_COMMAND`, `KEY_SPACE_COMMAND`, `KEY_BACKSPACE_COMMAND`, `KEY_ESCAPE_COMMAND`, `KEY_DELETE_COMMAND`, `KEY_TAB_COMMAND`, `KEY_MODIFIER_COMMAND`, `DROP_COMMAND`, `DRAGSTART_COMMAND`, `DRAGOVER_COMMAND`, `DRAGEND_COMMAND`, `FOCUS_COMMAND`, `BLUR_COMMAND`) and the payloads of `PASTE_COMMAND` (`PasteCommandType`), `COPY_COMMAND` and `CUT_COMMAND` (`ClipboardOrKeyboardEvent`), `SELECT_ALL_COMMAND` and `KEY_ENTER_COMMAND`; the DOM's events as Swift models them (`Event` with `preventDefault`, `defaultPrevented` and `stopPropagation`, `KeyboardEvent`, `MouseEvent` with the app's `target_key`, `DragEvent`, `FocusEvent`, `InputEvent`, `CompositionEvent`, `ClipboardEvent`), shared as `Rc`s so a handler's `preventDefault` reaches the dispatcher; `DataTransfer`'s files (`DataTransferFile`, read lazily, `"Files"` in `types`), `clearData`, `dropEffect` and `effectAllowed` | `commands`, `events` | `scripts/events`, `scripts/events-apple` |
| Keyboard shortcuts | `LexicalKeyboardShortcuts.ts` (`KeyboardShortcutMatch`, `KeyboardShortcut`, `CONTROL_OR_META` and `CONTROL_OR_ALT` as functions of the platform, `compileKeyboardShortcuts` with `add`, `matches` and `match`: by `key` case-insensitively, by `code` for a non-ASCII key, by the modifier mask); `LexicalUtils.ts`' `isModifierMatch`, `isExactShortcutMatch`, `KeyboardEventModifierMask` (`true`, `false` and `'any'` as `ModifierExpectation`), `keyboardEventMaskForPlatform`, `isModifier`, `isBackspace`, `isEscape`, `isDelete`, `isSelectAll`; `environment.ts`'s `IS_APPLE` as `is_apple()`, `true` on Apple targets, which `with_is_apple` overrides on a thread as Swift's `withIsApple` does | `keyboard_shortcuts`, `lexical_utils`, `environment` | `scripts/events` (off Apple platforms, as Node is), `scripts/events-apple` (recorded with `IS_APPLE` mocked to `true`) |
| The core's event handlers | `LexicalEvents.ts`, the part that needs no DOM: `registerDefaultCommandHandlers` (which `create_editor` calls) with `$handleKeyDown` (the compiled `buildKeyDownShortcuts` table, kept per platform, then `KEY_MODIFIER_COMMAND`), `$handleBeforeInput` in controlled mode (the input types JS handles, `deleteContentBackward` with no selection restoring the previous one, Shift+Enter making the next `insertParagraph` a line break, `$maybeMoveSelectionPastTrailingAcceptanceBoundary`, `$canRemoveText`), `$handleCompositionStart`, `$handleCompositionEnd` (`$onCompositionEndImpl`, `$cleanupComposedSubclass`), and `onKeyDown`'s, `onBeforeInput`'s and the composition listeners' model half as entry points an app calls with its own events (`on_key_down`, `on_before_input`, `on_composition_start`, `on_composition_end`); composition through the core: `_compositionKey`, `$setCompositionKey`, `$getCompositionKey`, `isComposing` on the editor and on a `TextNode`, the key following a split, merge, replace or point move, transforms skipping the composed node, `DEFER_COMPOSITION_MERGE_TAG`, the garbage collector clearing it, `insertText`'s composition paths, and `editor.isComposing()` in `@lexical/history`, the markdown shortcuts, the character limit and the tree view's `compositionKey` line; `SKIP_SELECTION_FOCUS_TAG`; `RootNode.__cachedText`'s effect on a commit with a root element (a stale cache marks the root dirty) | `events`, `editor`, `lexical_utils`, `lexical_selection`, `nodes::text`, `node`, `normalization`, `updates`, `gc`, `selection_insert_text`, `history`, `markdown`, `overflow`, `devtools` | `scripts/events` |
| List nodes (the `list` feature) | `packages/lexical-list/src/LexicalListNode.ts` (`ListNode`: `getListType`, `setListType`, `getTag`, `getStart`, `setStart`, `canBeEmpty`, `canIndent`, `splice`, its `$transform`), `LexicalListItemNode.ts` (`ListItemNode`: `append`, `replace`, `insertAfter`, `remove`, `resetOnCopyNodeFrom`, `insertNewAfter`, `collapseAtStart`, `getValue`, `setValue`, `getChecked`, `setChecked`, `toggleChecked`, `getIndent`, `setIndent`, `canMergeWith`, `isParentRequired`, `createParentElementNode`, `canMergeWhenEmpty`, its `$transform`), `$createListNode`, `$isListNode`, `$createListItemNode`, `$isListItemNode`; their generated JSON (`LexicalListGeneratedJSON.ts`); `utils.ts`; the `LexicalNode` hooks they override (`append`, `splice`, `replace`, `insertAfter`, `remove`, `getIndent`, `setIndent`, `resetOnCopyNodeFrom`, a class's own `$transform`); from the core `$copyNode`, `getPreviousSiblings`, `getNextSiblings`, `$splitTextPointCaret`, `$splitAtPointCaretNext`, `$insertNodeToNearestRootAtCaret`; from `@lexical/utils` `$getNearestNodeOfType` | `list`, `lexical_utils`, `caret_utils`, `node`, `nodes::element`, `utils` | `serialization/list`, the list cases of `property/documents`, `scripts/list` |
| `formatList.ts`, `registerList` and `registerCheckList` (the `list` feature) | `formatList.ts` (`$insertList`, `$createListOrMerge`, `mergeLists`, `$removeList`, `updateChildrenListItemValue`, `mergeNextSiblingListIfSameType`, `$handleIndent`, `$handleOutdent`, `$outdentFromItemWithContent`, `$handleListInsertParagraph`); `registerList.ts` `registerList` (`INSERT_ORDERED_LIST_COMMAND`, `INSERT_UNORDERED_LIST_COMMAND`, `REMOVE_LIST_COMMAND`, `UPDATE_LIST_START_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, the `KEY_BACKSPACE_COMMAND` handler at `COMMAND_PRIORITY_BEFORE_EDITOR` (an item's `collapseAtStart` when the caret is at its start), and the `ListItemNode` and `TextNode` transforms), with `RegisterListOptions`; `checkList.ts` `INSERT_CHECK_LIST_COMMAND` and its handler | `list::format_list`, `list::register_list` | `scripts/list`, `scripts/key-handlers`, `property/sequences` |
| Link nodes (the `link` feature) | `packages/lexical-link/src/LexicalLinkNode.ts` (`LinkNode`: `getURL`, `setURL`, `getTarget`, `setTarget`, `getRel`, `setRel`, `getTitle`, `setTitle`, `insertNewAfter`, `canInsertTextBefore`, `canInsertTextAfter`, `canBeEmpty`, `isInline`, `extractWithChild`, `isEmailURI`, `isWebSiteURI`, `shouldMergeAdjacentLink`; `AutoLinkNode`: `getIsUnlinked`, `setIsUnlinked`, its `shouldMergeAdjacentLink`), `$createLinkNode`, `$isLinkNode`, `$createAutoLinkNode`, `$isAutoLinkNode`, `$linkNodeTransform`, `formatUrl`; their generated JSON (`LexicalLinkGeneratedJSON.ts`, with `afterCloneLinkNode` and `afterCloneAutoLinkNode` as `Clone`); `ElementNode.extractWithChild` as a node hook and `isParentOf`; from the core `RangeSelection.extract` and `NodeSelection.extract` | `link`, `node`, `selection_editing` | `serialization/link`, the link cases of `property/documents`, `scripts/link` |
| `$toggleLink` and `registerLink` (the `link` feature) | `LexicalLinkNode.ts` (`TOGGLE_LINK_COMMAND`, `$toggleLink`, `$withSelectedNodes`, `$getPointNode`, `$splitLinkAtSelection`); `LexicalLinkExtension.ts` `registerLink` (the `LinkNode` transform, the `TOGGLE_LINK_COMMAND` handler, and with `validateUrl` the `PASTE_COMMAND` handler that links the selected text to a pasted URL, as `register_link_paste`; `validateUrl` and `attributes` from `LinkConfig`) | `link::register_link`, `link::toggle_link` | `scripts/link`, `scripts/key-handlers`, `property/sequences` |
| `registerAutoLink` (the `autolink` feature) | `packages/lexical-link/src/LexicalAutoLinkExtension.ts`: `registerAutoLink` (the `TextNode` transform, `$handleLinkCreation`, `handleLinkEdit`, `handleBadNeighbors`, `$createAutoLinkNode_`, `extractMatchingNodes`, `isContentAroundIsValid`, `isPreviousNodeValid`, `isNextNodeValid`, `startsWithTLD`, the separator checks, `getTextNodesToMatch`, `replaceWithChildren`, and the `TOGGLE_LINK_COMMAND` handler that flips `isUnlinked`), `AutoLinkConfig` and its defaults (`changeHandlers`, `excludeParents`, `matchers`, `separatorRegex`), `ChangeHandler`, `LinkMatcher`, `LinkMatcherResult`, `createLinkMatcherWithRegExp`, `autoLinkUrlMatcher` (with its parenthesis balancing) and `autoLinkEmailMatcher`, on the `regex` crate | `link::register_auto_link` | `scripts/autolink`, `entities/autolink-match` |
| Code nodes (the `code` feature) | `packages/lexical-code-core/src/CodeNode.ts` (`CodeNode`: `getLanguage`, `setLanguage`, `getTheme`, `setTheme`, `getIsSyntaxHighlightSupported`, `setIsSyntaxHighlightSupported`, `insertNewAfter` (the path of an editor without `CodeExtension`), `canIndent`, `collapseAtStart`; `$createCodeNode`, `$isCodeNode`, `DEFAULT_CODE_LANGUAGE`, `getDefaultCodeLanguage`, `$exitCodeNodeOnEnter`), `CodeHighlightNode.ts` (`CodeHighlightNode`: `getHighlightType`, `setHighlightType`, `setFormat` as a no-op, `isParentRequired`, `createParentElementNode`; `$createCodeHighlightNode`, `$isCodeHighlightNode`); their generated JSON (`LexicalCodeCoreGeneratedJSON.ts`, with `afterCloneCodeNode` and `afterCloneCodeHighlightNode` as `Clone`); `TextNode.setFormat` as a node hook; from the core `getTextDirection` and `RTL_REGEX`/`LTR_REGEX` (`LexicalUtils.ts`, `LexicalConstants.ts`) | `code`, `node`, `nodes::text`, `lexical_utils`, `constants` | `serialization/code`, the code cases of `property/documents`, `scripts/code` |
| `FlatStructureUtils.ts` (the `code` feature) | `$getFirstCodeNodeOfLine`, `$getLastCodeNodeOfLine`, `$getCodeLineDirection`, `$getStartOfCodeInLine` (with `findNextNonBlankInLine`), `$getEndOfCodeInLine`, `$plainifyCodeContent`, `$outdentLeadingSpaces` | `code::flat_structure_utils` | `scripts/code`; a unit test against JS's output for each function |
| `CodeExtension`'s register and `registerCodeIndentation` (the `code` feature) | `CodeExtension.ts` `register` (the `TabNode` transform and the `KEY_ENTER_COMMAND` handler, `$exitCodeNodeOnEnter`), as `register_code`; `CodeIndentation.ts` `registerCodeIndentation`'s handlers of `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND` and `OUTDENT_CONTENT_COMMAND` (`$isSelectionInCode`, `$getCodeLines`, `$outdentLineAtCaret`, `$handleMultilineIndent`), `KEY_TAB_COMMAND` (`$handleTab`), the arrow keys (`$handleShiftLines` with Alt, the guards at a code block's first and last line, `escapeWithArrows`) and `MOVE_TO_START` and `MOVE_TO_END` (`$handleMoveTo`), with `tabSize` and `escapeWithArrows` | `code::register_code`, `code::register_code_indentation` | `scripts/code`, `scripts/key-handlers` |
| Code highlighting (the `code-highlight` feature) | `packages/lexical-code-prism/src/CodeHighlighterPrism.ts`: `registerCodeHighlighting` (with `registerCodeIndentation`) and `registerHighlightingOnly`, without `registerCodeGutter` (ported apart, as `register_code_gutter`: see `@lexical/code-core` under Not ported yet); `$codeNodeTransform` (`defaultLanguage`, `isSyntaxHighlightSupported` from the tokenizer's languages, the `nodesCurrentlyHighlighting` guard cleared by `$onUpdate`), `$textNodeTransform` with `$copyNodeState`, `$updateAndRetainSelection`, `getDiffRange`, `isEqual`, the `Tokenizer` interface (`defaultLanguage`, `tokenize`, `$tokenize`) with Prism's `Token` shape; `FacadePrism.ts`'s `$mapTokensToLexicalStructure` and `$getHighlightNodes` without the `diff-<language>` grammars; `CodeHighlighterShiki.ts`'s lazy languages (`loadCodeLanguage`'s update tagged `history-merge`), as Swift's `CodeLanguageState`; with `extension`, `CodePrismExtension` (`disabled`, `tokenizer`) as Swift's `CodeHighlighterExtension` | `code::register_code_highlighting`, `code::register_highlighting_only`, `code::Tokenizer`, `code::map_tokens_to_lexical_structure`, `code::code_highlighter_extension` | `scripts/code-highlight`, over `ops.ts`'s conformance tokenizer and over tokens recorded from Prism |
| Mark node (the `mark` feature) | `packages/lexical-mark/src/MarkNode.ts` (`MarkNode`: `hasID`, `getIDs`, `setIDs`, `addID`, `deleteID`, `insertNewAfter`, `canInsertTextBefore`, `canInsertTextAfter`, `canBeEmpty`, `isInline`, `extractWithChild`, `excludeFromCopy`; `$createMarkNode`, `$isMarkNode`), its generated JSON (`LexicalMarkGeneratedJSON.ts`, with `afterCloneMarkNode` as `Clone`); `index.ts` (`$unwrapMarkNode`, `$wrapSelectionInMarkNode` with `createNode`, `$getMarkIDs`) | `mark` | `serialization/mark`, `scripts/mark` |
| Mark ids as NodeState (the `mark` feature) | `packages/lexical-mark/src/MarkStateExtension.ts`, the functions, which work without an extension on `MarkStateExtension`'s ids with the default `markableNodes`: `MARK_STATE_KEY`, `isValidMarkID`, the `markIds` state (`createMarkIDsState`, `arraySetValue(markIDValue())`'s parse, and `$setState`'s `canonicalValue` for it), `$isInlineDecoratorNode`, `$isMarkableNode`, `$getNodeMarkIDs`, `$addMarkID`, `$removeMarkID` (by selection, or walking the document without the extension), `$getMarkIDsAtSelection` (with `$getPointMarkIDs` and `$getAdjacentLeaf`), `$migrateMarkNodesToMarkState`, and their helpers (`$readNodeMarkIDs`, `$getMarkableIDs`, `$addNodeMarkIDs`, `$removeNodeMarkIDs`, `assertValidMarkID`, `$markableNodesInSelection`, `$collectMarkableNodes`, `$markableNodesIn`) | `mark::mark_state_extension` | `scripts/mark`, `serialization/mark` |
| `MarkExtension` and the mark flavors (the `mark` and `extension` features) | `packages/lexical-mark/src/index.ts`'s `MarkExtension` (`mark_nodes()`); `MarkStateExtension.ts`'s `MarkFlavor` (`createMarkIDsState` per `stateKey`), `MarkStateExtension`, `MarkBaseExtension` (the `flavors` registry, the stateKey and attribute clash invariants, the shared `SplitOrigins`), `defineMarkExtension` (`MarkStateKeyCheck`'s rules as its invariant, `defaultMarkAttribute`, the lowercased attribute), `createMarkFlavorExtension`, the flavor-first forms of `$getNodeMarkIDs`, `$addMarkID`, `$removeMarkID`, `$getMarkIDsAtSelection` and `$migrateMarkNodesToMarkState` (`*_for`), `$getMarkIDsByFlavor`, `DEFAULT_MARKABLE_NODES` with `compileMarkableNodes` and the `markableNodes` config, `disabled`, `registerMarkIDsState`, the `markIDNodeKeys` output and `$getMarkIDCandidates`, `registerMarkIDsListener` and `reportMarkIDs`, `registerMarkState` (`setKey`, `$visit`, `$rescan`, `rescanChanged`, `publish`, the root transform, the `COMPOSITION_START_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` and `PASTE_COMMAND` handlers, the collaboration taint), and the inserted-text rule: `PendingEdits` with `forgetEdits` and `forgetSoon`, `$getBlock`, `$getReplacedRange`, `$getCompositionReplacedRange` (`COMPOSITION_PLACEHOLDERS`, `$childrenText`, `$offsetInBlock`), `$collectSegments`, `sameEnd`, `segmentEnds`, `flattenSide`, `intersect`, `without`, `union`, `matchUnits` with `MATCH_BUDGET`, `$hasMarkIDsState`, `$splitsIntoOwnClass`, `$setUnitIDs`, `getInsertedRange`, `getSplitOrigin` and `$applyInsertedTextMarkIDs`; with `html` too, `exportIDsToHTML` (the `$exportDOM` override) and `createMarkIDsImportRule` | `mark` (`mark::mark_state_extension`, `mark::mark_state_inserted_text`), `node_state` (`has_state_value`) | `scripts/mark-extension`, `html/mark-state` |
| Overflow node (the `overflow` feature) | `packages/lexical-overflow/src/index.ts` (`OverflowNode`: `insertNewAfter` (its parent's), `excludeFromCopy`, the `$transform` of its `$config` that removes it once empty; `$createOverflowNode`, `$isOverflowNode`), with `ElementNode`'s generated JSON; `ElementNode.excludeFromCopy` as a node hook | `overflow`, `node` | `serialization/overflow`, the overflow cases of `property/documents`, `scripts/overflow` |
| The character limit (the `overflow` feature) | `packages/lexical-react/src/shared/useCharacterLimit.ts`, its logic without React's hook plumbing: `useCharacterLimit`'s effects as `register_character_limit` (the `OverflowNode` invariant, the count at registration, the text content and update listeners, `history-merge` updates, the low-priority `DELETE_CHARACTER_COMMAND` handler; `strlen` and `remainingCharacters`), `findOffset` (its `Intl.Segmenter` path), `$wrapOverflowedNodes` (with `$dfsWithSlots`, the `\n\n` between blocks and slot value leaves), `$wrapNode`, `$mergePrevious`; `Intl.Segmenter`'s extended grapheme clusters as UAX #29's rules on tables that `rust/scripts/generate-grapheme-tables.mjs` makes from the Unicode data of Node's ICU (Unicode 17.0.0), checked against every line of GraphemeBreakTest.txt by `Intl.Segmenter` and by a unit test | `overflow::character_limit`, `overflow::grapheme`, `overflow::grapheme_tables` | `scripts/character-limit`; a unit test of GraphemeBreakTest.txt |
| Table of contents (the `rich-text` feature) | `packages/lexical-react/src/LexicalTableOfContentsPlugin.tsx`, its logic without React: `TableOfContentsEntry` (key, text, tag), `toEntry`, `$insertHeadingIntoTableOfContents`, `$deleteHeadingFromTableOfContents`, `$updateHeadingInTableOfContents`, `$updateHeadingPosition`, `$getPreviousHeading`, and `TableOfContentsPlugin`'s effect as `register_table_of_contents` (the initial entries, the root update listener, the `HeadingNode` and `TextNode` mutation listeners with `skipInitialization`), reporting each `setTableOfContents` to a closure, on the core's root element and mutation listeners (the root element and mutation listeners row) | `rich_text::table_of_contents` | `scripts/table-of-contents` |
| Hashtag node and matcher (the `hashtag` feature) | `packages/lexical-hashtag/src/LexicalHashtagNode.ts` (`HashtagNode`: `canInsertTextBefore`, `isTextEntity`; `$createHashtagNode`, `$isHashtagNode`), with `TextNode`'s generated JSON; `LexicalHashtagExtension.ts` (`getHashtagRegexStringChars`, `getHashtagRegexString` and `getHashtagMatch` as the default matcher, `HashtagConfig`, `registerLexicalHashtag`), the character classes as range tables generated from the JS source by `rust/scripts/generate-hashtag-tables.mjs`; `TextNode.isTextEntity` as a node hook | `hashtag`, `node` | `entities/hashtag-match`, `entities/hashtag-documents`, `entities/hashtag-typing`, the hashtag cases of `property/documents` |
| `@lexical/extension`'s decorators (the `extension` feature) | `packages/lexical-extension/src/HorizontalRuleExtension.ts` (`HorizontalRuleNode`: a block decorator whose text is `\n`; `$createHorizontalRuleNode`, `$isHorizontalRuleNode`), `DecoratorTextExtension.ts` (`DecoratorTextNode`: inline, `__isInlineFormattable`, `getFormat`, `hasFormat`, `setFormat`, `toggleFormat`, its `format` as a flat NodeState; `$isDecoratorTextNode`), with `LexicalNode`'s generated JSON | `extension` | `scripts/decorator` |
| `NodeSelectionExtension` and `HorizontalRuleExtension` (the `extension` feature) | `packages/lexical-extension/src/NodeSelectionExtension.ts` (`watchNodeKey`: a `computed` per call over a `watchedSignal` of the selected keys, which follows `EditorStateExtension`'s store while watched, `node.isSelected()` of each watched key, keys without watchers dropped, the selected set replaced only when it changed); `HorizontalRuleExtension.ts` (`INSERT_HORIZONTAL_RULE_COMMAND` at `COMMAND_PRIORITY_EDITOR`, which handles a range selection with `$insertNodeToNearestRoot`; the `CLICK_COMMAND` handler at `COMMAND_PRIORITY_LOW` with `$toggleNodeSelection` and Shift; the extension's `nodes` and dependencies) | `extension::node_selection_extension`, `extension::horizontal_rule_extension` | `scripts/horizontal-rule` |
| `@lexical/extension`'s input extensions (the `extension` feature) | `KeyboardShortcutsExtension.ts` (`formatKeyboardShortcut` with `FormatKeyboardShortcutOptions`, the Apple and universal key names and the unshifted key; `NamedKeyboardShortcuts`, merged by name with the overrides first, `null` disabling a name; `KeyboardShortcutsExtension`'s one compiled `KEY_DOWN_COMMAND` listener at the config's `priority`, with `$disabled`, `$dispatch` and `bubbleFromNestedEditors`, its config as signals); `ClickAfterLastBlockExtension.ts` (`$defaultShouldInsertAfter`, `$shouldInsertAfter`, the paragraph inserted after the last block and selected); `NormalizeTripleClickSelectionExtension.ts` (`$fixFocusOverselection`); `IMEExtension.ts` (`compositionKey` and `composingTextNode`, set at `COMPOSITION_START_COMMAND` and the update tagged `COMPOSITION_START_TAG`, cleared at the end of the composition and when the root element goes); `AutoFocusExtension.ts` (`Editor::focus` with `defaultSelection` when a root element is set, then the root's `focus()` unless it `has_focus()`) | `extension::keyboard_shortcuts_extension`, `extension::click_after_last_block_extension`, `extension::normalize_triple_click_selection_extension`, `extension::ime_extension`, `extension::auto_focus_extension` | `scripts/input-extensions` |
| `ClickableLinkExtension` (the `link` and `extension` features) | `packages/lexical-link/src/ClickableLinkExtension.ts`: `registerClickableLink` (a click or middle `auxclick` on a `LinkNode` opens its sanitized URL, in a new tab with `noopener` for `newTab`, the middle button, Command or Control, or a `_blank` target; an unlinked autolink, an empty URL and a click that ends a non-collapsed selection are left alone, the last with its default prevented), `ClickableLinkConfig` (`newTab`, `disabled`, `getWindow`) as signals, `ClickableLinkExtension` and its `LinkExtension` dependency | `link::clickable_link_extension` | `scripts/input-extensions` |
| `BlockWithAlignableContents` (the `decorator-block` and `extension` features) | `packages/lexical-react/src/LexicalBlockWithAlignableContents.tsx`'s two handlers as `block_with_alignable_contents_extension` (Swift's `BlockWithAlignableContentsExtension`): `FORMAT_ELEMENT_COMMAND` aligning a selected block (and, under a range, the nearest block element of every other node), and `CLICK_COMMAND` on a block selecting it, deselecting it, or with Shift toggling it in the node selection, both at `COMMAND_PRIORITY_LOW` | `decorator_block` | unit tests (`decorator_block::tests`) against the output of JS's component in a React test, which no vector records (the component is React) |
| Extension core (the `extension` feature) | `packages/lexical/src/extension-core/`: `defineExtension.ts` (`defineExtension`, `configExtension`, `declarePeerDependency`), `safeCast.ts`, `shallowMergeConfig.ts`, `types.ts` (`LexicalExtension` with `name`, `conflictsWith`, `dependencies`, `peerDependencies`, `config`, `mergeConfig`, `nodes`, `$initialEditorState`, `init`, `build`, `register`, `afterRegistration`; `AnyLexicalExtension`, `AnyLexicalExtensionArgument`, `NormalizedPeerDependency`, `ExtensionConfigBase`, `ExtensionInitState`, `ExtensionBuildState`, `ExtensionRegisterState`, `LexicalExtensionDependency`, `InitialEditorConfig`, `InitialEditorStateType`, `LexicalEditorWithDispose`); `AbortController`/`AbortSignal` as `getSignal` uses them; `utils/mergeRegister.ts` | `extension_core` (re-exported at the crate root), `merge_register` | `scripts/extensions` |
| Extension builder (the `extension` feature) | `packages/lexical-extension/src/LexicalBuilder.ts` (`buildEditorFromExtensions`, `LexicalBuilder`: `fromExtensions`, `maybeFromEditor`, `fromEditor`, `constructEditor`, `buildEditor` with `dispose`, `hasExtensionByName`, `getExtensionRep`, `addEdge`, `addExtension` with the duplicate-name and conflict invariants, `sortedExtensionReps`' depth-first sort, cycle invariant and config order, `registerEditor`, `buildCreateEditorArgs` for nodes and `$initialEditorState`), `internal/ExtensionRep.ts` (the state ids, `mergeConfigs`, `init`, `build`, `register` with its rollback, `afterRegistration`, `getSignal`, `getInitResult`, `getInitPeer`, `getExtensionInitDependency`, `getPeer`, `getInitDependency`, `getDependency`, `getDirectDependentNames`, `getPeerNameSet`, `getExtensionDependency`), `config.ts` (`getKnownTypesAndNodes`, `getNodeConfig`), `getExtensionDependencyFromEditor.ts`, `getPeerDependencyFromEditor.ts` (with `OrThrow`), `getExtensionDependency.ts` (`$getExtensionDependency`, `$getExtensionOutput`, `$getPeerDependency`) | `extension::lexical_builder`, `extension::extension_rep`, `extension::config`, `extension::get_*` | `scripts/extensions` |
| Signals (the `extension` feature) | `signals.ts`, a port of `@preact/signals-core` 1.14.4 (`signal` with `watched`/`unwatched`, `computed`, `effect` with clean-ups, `batch` with its snapshots, `untracked`, `peek`, `subscribe`); `namedSignals.ts`; `watchedSignal.ts` | `extension::signals`, `extension::named_signals`, `extension::watched_signal` | `scripts/extensions` (`namedSignals toggle a registration`, `a signal disabled by config`, `EditorStateExtension as a signal`) |
| Model-level extensions (the `extension` feature) | `InitialStateExtension.ts` (always first; its config and every form of `$initialEditorState`), `ClearEditorExtension.ts` (`registerClearEditor`, `$defaultOnClear`, `$onClear` as a signal), `EditorStateExtension.ts`, `DecoratorTextExtension.ts`'s extension | `extension::initial_state_extension`, `extension::clear_editor_extension`, `extension::editor_state_extension`, `extension::decorator_text_extension` | `scripts/extensions` |
| `NormalizeInlineElementsExtension` (the `extension` feature) | `packages/lexical-extension/src/NormalizeInlineElementsExtension.ts` (`deleteEmptyInline`, and the `register` that adds it to the transforms of every registered `ElementNode` class with an `isInline` of its own, without marking their nodes dirty, as `register_normalize_inline_elements`; the extension with its `disabled` signal); `RichTextExtension` and `PlainTextExtension` depend on it | `extension` | `property/sequences`, `scripts/package-extensions` |
| `@lexical/extension`'s model-level extensions (the `extension` feature) | `TabIndentationExtension.ts` (`registerTabIndentation`'s `KEY_TAB_COMMAND` handler with `$indentOverTab` and its `INDENT_CONTENT_COMMAND` handler with `canIndentPredicate` and `maxIndent`, as `register_tab_indentation`; the extension with its `disabled`, `maxIndent` and `$canIndent` signals); `SelectBlockExtension.ts` (the `SELECT_ALL_COMMAND` handler that selects the block, then the document, with `disabled` and `cascadeSelection`, which selects all of the editor when the command comes from a nested editor whose content is fully selected, read with `read('pending')`); `PreventSelectAllExtension.ts`'s config and `disabled` signal, which `SelectBlockExtension`'s drives | `extension::tab_indentation_extension`, `extension::select_block_extension`, `extension::prevent_select_all_extension` | `scripts/package-extensions`, `scripts/nested-editor`, `scripts/key-handlers` |
| Nested editors (the `extension` feature, `$isEditorIsNestedEditor` with `utils`) | From the core: `createEditor`'s `parentEditor` (`_parentEditor`, `Editor::parent_editor`) and the parent's namespace when none is given, `getEditorsToPropagate`, and `triggerCommandListeners` running each priority's listeners in the editor, then its parent editors, every listener getting the editor the command was dispatched in, with a parent that is updating skipped and the command dispatched again from it in an update of its own (the legacy delegation); `read(mode, fn)` with `EditorReadMode` (`'force-commit'`, `'latest'`, `'pending'`) as `Editor::read_with_mode`. `InitialEditorConfig`'s `parentEditor` and `theme`, and `buildCreateEditorArgs` taking an extension's `parentEditor` and merging the themes with `deepThemeMergeInPlace` (without the unsafe keys). `packages/lexical-extension/src/NestedEditorExtension.ts`: `NestedEditorConfig` (`$getParentEditor`, by default `$defaultGetParentEditor`, the active editor built with `LexicalBuilder`; `inheritEditableFromParent`), its `init` (the parent editor, and its theme unless the extensions give one), its output signal, and the effect that follows the parent's editable listener. `@lexical/utils`' `$isEditorIsNestedEditor`. `SharedHistoryExtension`'s `parentEditor` falling back to `editor._parentEditor` | `editor`, `updates`, `lexical_utils`, `extension_core`, `extension::lexical_builder`, `extension::nested_editor_extension`, `utils`, `history::shared_history_extension` | `scripts/nested-editor`; unit tests of the command delegation while the parent updates, which no vector records (JS delegates in an update whose commit a step can't separate) |
| The packages' extensions (each behind its package's feature and `extension`) | `LexicalRichTextExtension.ts` (`RichTextExtension`: its nodes, `conflictsWith`, the `NormalizeInlineElementsExtension` and `NormalizeTripleClickSelectionExtension` dependencies (`PlainTextExtension`'s too), `RichTextConfig` with `mergeRichTextConfig`'s format-by-format merge of `escapeFormatTriggers`, `registerRichText` reading the config's signals); `PlainTextExtension` (`@lexical/plain-text`); `@lexical/history`'s `HistoryExtension` (`HistoryConfig` with `delay`, `createInitialHistoryState`, `disabled`, `maxDepth` and `now`, the init's `canUndo`/`canRedo` signals, the output's config signals, `registerHistory` reading `delay` and `maxDepth` as signals) and `SharedHistoryExtension` (its own `HistoryExtension`, disabled, takes the `delay`, `historyState`, `now`, `maxDepth` and `disabled` of the parent editor's peer `HistoryExtension`, the parent being its `parentEditor` or the editor's own parent editor); `LexicalListExtension.ts` (`ListExtension` with `hasStrictIndent`, which registers `registerListStrictIndentTransform`, and `shouldPreserveNumbering`; `CheckListExtension`'s `registerCheckList`); `LexicalLinkExtension.ts` (`LinkExtension` with its `mergeConfig` of `attributes`, and `registerLink` reading `validateUrl` and `attributes` as signals, its paste handler among them); `LexicalAutoLinkExtension.ts` (`AutoLinkExtension` with its concatenating `mergeConfig`); `LexicalHashtagExtension.ts` (`HashtagExtension`); `OverflowExtension`; `CodeExtension.ts` (`CodeExtension`, and `CodeNode.insertNewAfter`'s path for an editor built with it) and `CodeIndentExtension` (`tabSize`, `disabled`, `escapeWithArrows`); `Editor` equality by identity, as JS's `===` | `rich_text::rich_text_extension` (`RichTextConfigOverrides`), `plain_text::plain_text_extension`, `history::history_extension`, `history::shared_history_extension`, `list::list_extension`, `list::check_list_extension`, `link::link_extension`, `link::auto_link_extension`, `hashtag::hashtag_extension`, `overflow::overflow_extension`, `code::code_extension`, `code::code_indent_extension` | `scripts/package-extensions`; `property/sequences` and the hashtag `entities` suites build their editors with them |
| `registerLexicalTextEntity` (the `text` feature) | `packages/lexical-text/src/registerLexicalTextEntity.ts`: `EntityMatch`, `registerLexicalTextEntity` (the `TextNode` transform and `$reverseNodeTransform`, `$replaceWithSimpleText`, `$wasTurnedIntoText`, `getFirstMatchAfter`, `isNextTextEntity`, `isTextAfterOutsideSource`, `isEntityAfter`, `$checkNextSibling`, `$moveSelectionIntoEntity`, `$wouldMerge` (with `haveEquivalentNodeState` and `readStateValue`), `MAX_MATCHES_PASSED`, `MAX_TURNED_INTO_TEXT`, the per-update `markedInUpdate` and `turnedIntoTextInUpdate`); from the core `INTERNAL_getActiveEditorState` | `text`, `updates`, `node_state` | `entities/hashtag-documents`, `entities/hashtag-typing`, `property/sequences`; a unit test of `$wouldMerge`'s NodeState rule against JS's output |
| `@lexical/text`'s root helpers (the `text` feature) | `packages/lexical-text/src/rootTextContent.ts` (`$rootTextContent`), `isRootTextContentEmpty.ts` (`$isRootTextContentEmpty`, with `String#trim`'s whitespace, and `$isRootTextContentEmptyCurry`), `canShowPlaceholder.ts` (`$canShowPlaceholder`, `$canShowPlaceholderCurry`), `findTextIntersectionFromCharacters.ts` (`$findTextIntersectionFromCharacters`, deprecated upstream, with UTF-16 counts), `index.ts`'s `TextNodeWithOffset` | `text` | `scripts/text` |
| `@lexical/file` (the `file` feature) | `packages/lexical-file/src/fileImportExport.ts`: `SerializedDocument`, `serializedDocumentFromEditorState` (an empty `source` is `"Lexical"`, a `lastSaved` of 0 or NaN is `Date.now()`, the version is the upstream release), `editorStateFromSerializedDocument` (from a string or a document, either JSON form), `importFile` and `exportFile` (with `toISOString` for the default file name), with the app reading and saving the file where JS's `readTextFileFromSystem` and `exportBlob` use the DOM | `file::file_import_export` | `file/export`, `file/import`; unit tests for `toISOString` against Node and an export-then-import round trip |
| `@lexical/devtools-core`'s tree view (the `devtools` feature) | `packages/lexical-devtools-core/src/generateContent.ts`: `generateContent` (the text form, and with `html` the `exportDOM` form: `$generateHtmlFromNodes`, `printPrettyHTML`, `prettifyHTML`), `$printRangeSelection` (with the anchor's slot), `$resolveSlotContext`, `$slotValueMetaSuffix`, `printNodeSelection`, `$visitTree` (slot values first, with their names), `normalize` (with `obfuscateText`), `printNode` (text, link with `link`, mark with `mark`, paragraph), `printTextFormatProperties`, `printAllTextNodeProperties`, `printAllLinkNodeProperties`, `printDetailProperties`, `printModeProperties`, `printFormatProperties`, `printTargetProperties`, `printRelProperties`, `printTitleProperties`, `printStateProperties`, `$printSelectedCharsLine`, `$getSelectionStartEnd`, `CustomPrintNodeFn`, `LexicalCommandLog`'s entries; from the core `LexicalNode.isSelected` (`$isKeyInSelectionNodes` and the inline decorator rule), `isBefore`, `LexicalEditor.version` (the port's own, `lexical-rust@0.53.0`, as Swift prints `lexical-swift@...`), `_config.namespace` (`createUID` when none is given), `isEditable` and `setEditable` | `devtools`, `node`, `editor`, `lexical_utils` | `devtools/tree` (keys mapped to the port's and the version line to the port's, as Swift's runner does; marks, named slots, a selection in a slot, and the `exportDOM` form of each document) |
| `$setBlocksType` (the `selection` feature) | `packages/lexical-selection/src/range-selection.ts` (`$setBlocksType`, with its default `$afterCreateElement`, `$copyBlockFormatIndent`, and `$isPointAtBlockEdge`), through `LexicalNode.replace` and `ListItemNode.replace` | `selection` | `scripts/set-blocks-type`, the block-decorator case of `scripts/decorator`, `property/sequences` |
| The DOM (the `html` feature) | what a browser or jsdom gives JS: `Node` (`nodeType`, `nodeName`, `parentNode`, `childNodes`, siblings, `textContent`, `insertBefore`, `appendChild`, `append`, `prepend`, `removeChild`, `replaceChild`, `remove`, `replaceWith`, `replaceChildren`, `contains`, `cloneNode`), `Element` (attributes in order, `id`, `dir`, `className`, `classList`, `style`, `innerHTML`, `outerHTML`, `getElementsByTagName`), `Text`, `Comment`, `DocumentFragment`, `Document` (`createElement` and the other factories, `head`, `body`), `DOMTokenList`, `CSSStyleDeclaration` (`setProperty`, `getPropertyValue`, `getPropertyPriority`, `removeProperty`, `cssText`, `item`) with jsdom's normalization, `DOMParser.parseFromString`, the HTML fragment serialization algorithm; ported from the Swift port's `DOM/` | `dom` | `html/parse`, `html/entities`, `css/properties`, `css/values` |
| Core DOM hooks (the `html` feature) | `LexicalNode.createDOM`, `exportDOM` and the static `importDOM` as node hooks (`create_dom`, `export_dom`, `import_dom`), `ElementNode.exportDOM` (indent as `padding-inline-start` and `data-lexical-indent`, `dir`); `DOMExportOutput`, `DOMConversionOutput`, `DOMChildConversion`, `DOMConversion`, `DOMConversionFn`, `DOMConversionMap`; `createEditor`'s `theme` and `html` (`export` overrides, `import` conversions), `editor._config`, `initializeConversionCache`; `DEFAULT_EDITOR_DOM_CONFIG` (`$createDOM`, `$exportDOM`, `$extractWithChild`, `$shouldExclude`, `$shouldInclude`); `LexicalNode.isSelected`; `TextNode` (`createDOM` with `getElementOuterTag`, `getElementInnerTag`, `$createTextInnerDOM` and `setTextThemeClassNames`; `exportDOM`; `importDOM` with `$convertTextDOMNode`, `findParentPreDOMNode`, `findTextInLine`, `convertSpanElement`, `convertBringAttentionToElement`, `convertTextFormatElement`, `applyTextFormatFromStyle`), `TabNode.createDOM`, `LineBreakNode` (`createDOM`, `importDOM`), `ParagraphNode` (`createDOM`, `exportDOM`, `importDOM` with `$convertParagraphElement`, `extractWithChild`), `ArtificialNode__DO_NOT_USE`; from `LexicalUtils.ts` `isInlineDomNode`, `isBlockDomNode`, `getCachedClassNameArray`, `removeEmptyDOMAttribute`, `setNodeIndentFromDOM`, `$setDirectionFromDOM`, `$setFormatFromDOM`, `isOnlyChildInBlockNode`, `isLastChildInBlockNode`; `utils/classNames.ts` | `lexical_dom`, `node`, `nodes::*`, `editor` | `html/export`, `html/import` |
| The node packages' DOM parts (the `html` feature with each package's) | `createDOM`, `exportDOM` and `importDOM` of `HeadingNode` (with the Google Docs title), `QuoteNode` (`$convertHeadingElement`, `$convertBlockquoteElement`), `ListNode` (`$setListThemeClassNames`, `$convertListNode`, `$normalizeChildren`, `isDomChecklist`, `HTMLOListElement.start`), `ListItemNode` (`updateListItemDOM` with `updateListItemChecked`, `$setListItemThemeClassNames` and `applyMarkerStyles`, the nested item's `after`, `$convertListItemElement`, `$convertCheckboxInput`, `setFormatFromChildren`), `LinkNode` (`updateLinkDOM` for a new element, `sanitizeUrl`, `$convertAnchorElement`), `AutoLinkNode`, `CodeNode` (`$convertPreElement`, `$convertDivElement`, the GitHub table converters), `CodeHighlightNode`, `OverflowNode`, `HashtagNode`, `HorizontalRuleNode` and `DecoratorTextNode`, with `applyFormatFromStyle` and `applyFormatToDom`; `MarkNode.createDOM` (the `mark` and `markOverlap` theme classes), with `excludeFromCopy('html')` leaving marks out of the HTML export | `rich_text`, `list`, `link`, `code`, `overflow`, `hashtag`, `extension`, `mark` | the HTML of `property/documents`, `html/import-nodes`, `html/export-mark` |
| `@lexical/html` (the `html` feature) | `packages/lexical-html/src/index.ts`: `$generateHtmlFromNodes`, `$generateDOMFromNodes`, `$generateDOMFromRoot`, `$appendNodeToHTML` (`$appendNodesToHTML` with `$appendTerminatingLineBreak`, and `@lexical/selection`'s `$sliceSelectedTextNodeContent` for a selection), `$generateNodesFromDOM` (`IGNORE_TAGS`, `getConversionFunction`, `$createNodesFromDOM`, `wrapContinuousInlines`, `$unwrapArtificialNodes`, `isDomNodeBetweenTwoInlineNodes`) | `html` | `html/export`, `html/import` |
| `DOMImportExtension` (the `html` and `extension` features) | `packages/lexical-html/src/import/`: `DOMImportExtension.ts` (`DOMImportConfig` and its `mergeConfig`, `$runPreprocessStack`, `DefaultHoistRule`, `$generateNodesFromDOMViaExtension`), `CoreImportExtension.ts`, `HorizontalRuleImportExtension.ts`, `coreImportRules.ts` (the inline format, `#text` with `collapseWhitespace` and `findAdjacentTextOnLine`, script and style, `<br>`, `<p>`, `<hr>` and transparent block rules, `isAlignmentValue`), `defineImportRule.ts`, `compileImportRules.ts`, `defineOverlayRules.ts`, `ImportContext.ts` (`createImportState`, `ImportSource`, `ImportTextFormat`, `ImportTextStyle`, `ImportWhitespaceConfig` with `defaultPreservesWhitespace` and `defaultIsInline`, `ImportOverlays`, `ImportSessionImpl`, `$getImportContextValue`, `$withImportContext`), `runImport.ts`, `schemas.ts` (`$isBlockLevel`, `$distributeInlineWrapper`, `$applySchema`, `$propagateTextAlignToBlockChildren`, `BlockSchema`, `InlineSchema`, `NestedBlockSchema`, `RootSchema`), `sel.ts` (with `RegExp` matchers as functions), `parseCss.ts`, `inlineStylesFromStyleSheets.ts`; `internal/ContextRecord.ts`'s import half; `CSSStyleSheet.cssRules` and `querySelectorAll` in the DOM | `html::import`, `html::internal`, `dom` | `html/import-extension`; unit tests of the style sheet parser and the selectors against jsdom |
| `DOMRenderExtension` (the `html` and `extension` features) | `packages/lexical-html/src/internal/`: `DOMRenderExtension.ts` (its `init`, `build`, `mergeConfig` and the `RootNode` export), `domOverride.ts`, `compileDOMRenderConfigOverrides.ts` (`sortedOverrides`, `getPredicate`, the merge of each export hook), `DOMRenderRuntime.ts` (`createEditorContextRecord`, `filterEditorInstalled`, `setContextValue`, `getSessionConfig` with its cache), `RenderContext.ts` (`createRenderState`, `RenderContextRoot`, `RenderContextExport`, `$getRenderContextValue`, `$setRenderContextValue`, `$updateRenderContextValue`, `$getSessionDOMRenderConfig`, `$withRenderContext`), `types.ts`'s render half, `ContextRecord.ts`'s render half; from the core `EditorDOMRenderConfig`'s export hooks, `$getEditorDOMRenderConfig` and `createEditor`'s `dom`; from `@lexical/extension` `buildCreateEditorArgs`'s `html` merge; `$generateDOMFromNodes` and `$generateDOMFromRoot` with the render context and the session's render config | `html`, `html::internal`, `lexical_dom`, `extension` | `html/export-render` |
| The packages' import rules (the `html` and `extension` features with each package's) | `packages/lexical-rich-text/src/RichTextImportExtension.ts` (`RichTextImportRules`, the heading, Google Docs title and blockquote rules, and the shadow root quote rule), `packages/lexical-list/src/ListImportExtension.ts` (`ListImportRules`, `isDomChecklist`, the list schema with its flatten step), `packages/lexical-list/src/WordListImportExtension.ts` (the `mso-list` paragraph preprocess and rules), `packages/lexical-link/src/LinkImportExtension.ts` (`LinkImportRules`), `packages/lexical-code-core/src/CodeImportExtension.ts` (`CodeImportRules` and the VS Code paste overlay); each package extension depends on `CoreImportExtension` and its rules, as in JS | `rich_text::rich_text_import_extension`, `list::list_import_extension`, `list::word_list_import_extension`, `link::link_import_extension`, `code::code_import_extension` | `html/import-packages` |
| `BlockIdHTMLExtension` (the `html` and `extension` features) | `packages/lexical-html/src/BlockIdHTMLExtension.ts`: `BLOCK_ID_ATTRIBUTE`, `BLOCK_HANDLE_ATTRIBUTE`, `BLOCK_WRAPPER_ID_ATTRIBUTE`, `BlockIdHTMLConfig` (`export`, `handles`, `import`) and its output signals, the `'*'` export override (an identified block's id or handle on its element, and the wrapper id on the list of an item that only wraps one, skipped where `disabledForSession` is set), the import rule, the wrapper id rule and its resolve rule (`pendingWrapperIds`), installed as overlays when `import` is on, and the root transform that clears the ids `$isIdentifiedBlock` rejects on imported blocks | `html::block_id_html_extension` | `html/block-id` |
| Inline CSS text | `utils/setDOMStyle.ts` (`getStyleObjectFromCSS`, with JS's property order; with the `html` feature `setDOMStyleObject` and `setDOMStyleFromCSS`) | `set_dom_style` | through `css/values`; unit tests against JS's output |
| Clipboard, JSON and plain text (the `clipboard` feature) | `packages/lexical-clipboard/src/clipboard.ts`: `$generateJSONFromSelectedNodes` (`$appendNodesToJSON`: `isSelected`, `excludeFromCopy('clone')`, `extractWithChild`, the text sliced at the selection's edges, empty slices left out, a node selection's element taking its subtree), `$generateNodesFromSerializedNodes`, `$insertGeneratedNodes` (`SELECTION_INSERT_CLIPBOARD_NODES_COMMAND`, `$updateSelectionOnInsert`), `$getLexicalContent`, `$insertDataTransferForPlainText`, `$insertDataTransferForRichText` (the default stack of `ClipboardImportExtension.ts`: `$runImport` with `$defaultLexicalEditorImporter` and `$defaultPlainTextImporter` for `text/plain` and `text/uri-list`, by `DEFAULT_IMPORT_MIME_TYPE_PRIORITY`), `$getClipboardDataFromSelection` (the default `ExportMimeTypeConfig` but `text/html`), `setLexicalClipboardDataTransfer`; `$sliceSelectedTextNodeContent` (`@lexical/selection`, `'clone'`); from the core `LexicalNode.isSelected`, `createEditor`'s `namespace` and `getNamespace` (as the devtools row has them), `insertNodes`' CASE 1 (inside a code block), `PASTE_COMMAND`, `COPY_COMMAND`, `CUT_COMMAND` and `SELECTION_INSERT_CLIPBOARD_NODES_COMMAND`, and `DataTransfer` (`getData`, `setData`, `types`, as Swift's in `LexicalEvents.swift`); the `extractWithChild` of `ParagraphNode`, `ListNode` and `ListItemNode` | `clipboard`, `events`, `selection`, `node`, `editor`, `commands`, `selection_insert_nodes`, `nodes::paragraph`, `list` | `scripts/clipboard` |
| Clipboard, HTML and the extensions (the `clipboard` feature with `html`, `extension` or both) | `packages/lexical-clipboard/src/clipboard.ts`: `$getHtmlContent`, the `text/html` entry of `DEFAULT_EXPORT_MIME_TYPE`, `$getExportConfig`, `callExportMimeTypeFunctionStack`, `$exportMimeTypeFromSelection`, `GetClipboardDataExtension` (`GetClipboardDataConfig`, `ExportMimeTypeFunction`, its `mergeConfig`); `ClipboardImportExtension.ts`: `$defaultHtmlImporter`, `DEFAULT_IMPORT_MIME_TYPE`, `DEFAULT_IMPORT_MIME_TYPE_PRIORITY`, `$callImportMimeTypeFunctionStack` (with `$next`'s data and selection), `orderedMimeTypes`, `$runImport` (with the iOS Safari skip of HTML equal to the plain text), `$getImportOutput`, `ClipboardImportExtension`, `ClipboardDOMImportExtension`; `registerPlainText`'s copy writing `text/html`; `@lexical/html`'s `ImportSourceDataTransfer`; from the core, `insertNodes`' CASE 3a (`$insertNodeToNearestRootAtCaret` for blocks with no block ancestor) | `clipboard`, `plain_text`, `html::import`, `selection_insert_nodes` | `html/clipboard` |
| Copy, cut and paste handlers (the `rich-text` and `plain-text` features) | `registerRichText`'s `COPY_COMMAND` (`copyToClipboard` with an event, `$copyToClipboardEvent`), `CUT_COMMAND` (`onCutForRichText`, with `INTERNAL_$expandSelectionToWholeDocument`) and `PASTE_COMMAND` (`onPasteForRichText`) handlers; `registerPlainText`'s (`onCopyForPlainText`, `onCutForPlainText`, `onPasteForPlainText`), with JS's `paste` and `cut` tags; drag and drop: `$handleRichTextDrop`, `$handlePlainTextDrop`, `$doDrop` (`normalizeDropBoundary`, `isDropCaretInsideSelection`, a same-editor move), `$writeDragSourceToDataTransfer` and the drag marker, with `caretFromPoint` as the root element's hit test (`RootElement::caret_from_point`, `caret_from_event`) | `rich_text`, `plain_text`, `clipboard` | `scripts/key-handlers` (a paste, the drag start, drag over and drop handlers where no caret resolves, as in jsdom); unit tests for copy and cut (headless JS has no copy to record) |
| The rest of `@lexical/selection`'s model (the `selection` feature) | `packages/lexical-selection/src/lexical-node.ts` (`$sliceSelectedTextNodeContent`, its `'clone'` mode only, `$isAtNodeEnd`, `$trimTextContentFromAnchor`, `$addNodeStyle` as the deprecated no-op it is, `$patchStyleText` with values, `null` and functions of the previous value, `$forEachSelectedTextNode`, `$ensureForwardRangeSelection`), `range-selection.ts` (`$wrapNodes` with `$wrapNodesImpl`, `$shouldOverrideDefaultCharacterSelection`, `$isParentElementRTL`, `$moveCaretSelection`, `$moveCharacter`, `$getSelectionStyleValueForProperty`, `$isEditorVerticalOrientation`, `$getComputedStyle`), `utils.ts` (`getCSSFromStyleObject`, `getStyleObjectFromRawCSS`, `$getComputedStyleForElement`, `$getComputedStyleForParent` and `$isParentRTL` on the root element's `RootElement::computed_style`, and `createRectsFromDOMRange`'s filtering of a range's rects as `filter_selection_rects`), and its re-exports of `getStyleObjectFromCSS`, `$cloneWithProperties` and `$selectAll` (with a selection to scope it, `select_all_in`); from the core `getStyleObjectFromCSS` (`StyleObject`, in a JS object's key order), `$cloneWithProperties`, `$splitNode`, `ElementNode.getDescendantByIndex`, `TextNode.canHaveFormat` | `selection`, `set_dom_style`, `lexical_utils`, `selection_insert_nodes`, `editor` | `scripts/selection-utils`, the right-to-left and vertical arrows of `scripts/key-handlers` (a stubbed `getComputedStyle`); unit tests of `filter_selection_rects` against JS's output |
| `@lexical/utils`'s model (the `utils` feature) | `packages/lexical-utils/src/index.ts`: `DFSNode`, `$dfs`, `$reverseDfs`, `$dfsIterator`, `$reverseDfsIterator` (Rust iterators), `$getAdjacentCaret`, `$getNextSiblingOrParentSibling`, `$getDepth`, `$getNextRightPreorderNode`, `$getNearestNodeOfType`, `$getNearestBlockElementAncestorOrThrow`, `registerNestedElementResolver` (by node type), `$restoreEditorState`, `$insertNodeToNearestRoot`, `$insertNodeIntoLeaf`, `$wrapNodeInElement`, `$filter`, `$handleIndentAndOutdent`, `$insertFirst`, `$unwrapAndFilterDescendants`, `$descendantsMatching`, `$firstToLastIterator`, `$lastToFirstIterator`, `$unwrapNode`, `StateConfigWrapper`, `makeStateWrapper`, `$onEscapeUp`, `$onEscapeDown`, `$isAtStartOfNode`, `$isAtEndOfNode`; its re-exports of `$findMatchingParent`, `$getAdjacentSiblingOrParentSiblingCaret`, `$insertNodeToNearestRootAtCaret`, `$isBlockFullySelected`, `$isRootOrShadowRoot`, `$splitNode` and `mergeRegister` (no longer behind the `extension` feature) | `utils`, `lexical_utils`, `caret_utils`, `selection_deletion`, `merge_register` | `scripts/selection-utils` |
| Slots | `LexicalSlot.ts` (`$isSlotHost`, `$isSlotChild`, `$getSlotHost`, `$getSlotNameWithinHost`, `$getSlotFrame`, `$getSelectionSlotFrame`, `$getSlotNames` in canonical order, `$getSlot`, `getDeclaredSlots` as `LexicalNode::declared_slots`, `$setSlot` with its refusals and moves, `$removeSlot`, the cycle check, `_slotsUsed`); their call sites in `LexicalNode.ts` (`__slotHost` and `__slots` on elements and decorators, `remove`, `replace`, `isAttached`, `getTextContent` and its size, `selectNext`/`selectPrevious` through the host), `LexicalElementNode.ts` (`isEmpty`, `getAllTextNodes`, text content with slots first), `LexicalEditorState.ts` and `LexicalUpdates.ts` (`"$slots"` in both JSON forms, written after `children`, and parsed through `$setSlot`), `LexicalGC.ts` (a removed host's slot subtrees), `LexicalSelection.ts` (`$clampRangeSelectionToSlotFrame` in `$setSelection` and at commit, `$getPointSlotFrame`, insertion and deletion stopping at a slot boundary, `$selectAll` scoped to the slot frame), `caret/LexicalCaret.ts` (`$filterByMode` stopping at slotted nodes, `$getCommonAncestorAcrossSlots`, `$comparePointCaretNextAcrossSlots`), `caret/LexicalCaretTree.ts`, `caret/LexicalCaretUtils.ts`; in the packages, `SELECT_ALL_COMMAND`'s scoping (`rich-text`, `plain-text`), `$setBlocksType`, `$wrapNodes`, `$insertList` and `$removeList` skipping slot values, `SelectBlockExtension`'s slot frames, `@lexical/clipboard`'s copy of a host's `$slots` from the selection's slot frame, `@lexical/mark`'s `$forEachSlot` walks, and `@lexical/utils`' `$dfsWithSlots`, `$reverseDfsWithSlots`, their iterators and `$getDepth` across hosts | `slot`, `node`, `nodes::element`, `editor_state`, `updates`, `gc`, `lexical_selection`, `selection_deletion`, `selection_insert_nodes`, `caret`, `caret_tree`, `caret_utils`, `rich_text`, `plain_text`, `selection`, `extension`, `clipboard`, `mark`, `utils` | `scripts/slots`, the named slots case of `serialization/core` |
| Node structure (`@experimental node-structure`) | `LexicalStructure.ts`: the `$config()` keys `inline`, `children` (`'inline'`, `'block'` or a list of `'inline'` and classes), `mergeAdjacent` (`{by: [...]}` of schema fields and NodeStates, or `false`) and the values declared `derived`, read along the class's config chain (`iterStaticNodeConfigChain`, the nearest declaration winning; a key the class only inherits and contradicts reads as undeclared), `getDeclaredNodeStructure`, `validateNodeStructure` (when an editor is created, with JS's messages), the synthesized `isInline()`, `INTERNAL_$isInlineContent`, `$isBlockContainer`, `INTERNAL_registerNodeStructure` (counted registrations, the dirty marking of every element and decorator, a full normalization on registering), `INTERNAL_isNodeStructureEnforced`, `INTERNAL_$normalizeNodeStructure`, and the normalization of each dirty subtree: lifting blocks out of holders of inline content and splitting them (the anchor saved across), paragraphs joined as lines, inline runs wrapped in a block container's paragraph, a stray child wrapped in a list's first class, empty inline elements removed (not an `OverflowNode`), equal adjacent siblings merged with the seam merged in turn, and the class hooks `INTERNAL_$afterStructureMerge`, `INTERNAL_$afterStructureSplit` and `INTERNAL_$liftsFromStructure`; its call sites in the core (`ElementNode`'s shadow-root transform, `insertNodes`, `$setBlocksType`, `$wrapNodes`, `$insertNodeToNearestRoot`, the `is_inline` and `declares_inline_children` defaults); the packages' declarations (`ParagraphNode`, `HeadingNode`, `QuoteNode`, `CodeNode`, `MarkNode`, `LinkNode`, `AutoLinkNode`'s `mergeAdjacent: false`, `ListNode`, `ListItemNode`) and their structure code (`$structureMerges`, `ListNode`'s merge and split hooks, `ListItemNode`'s lift hook and `$pointsAround`, `formatList.ts`'s `$keptTops`, `$listTarget`, `$isListItemContent` and the `onlyChild` seam merge, `$linkNodeTransform` leaving lifts and merges to the normalization); `NodeStructureExtension.ts` (`registerNodeStructure`, `NodeStructureExtension` with its `disabled` signal) | `structure` (`LexicalNode::static_node_config_chain` and the `internal_after_structure_merge`, `internal_after_structure_split` and `internal_lifts_from_structure` hooks), `NodeClass::get_declared_node_structure`, `create`, `extension::node_structure_extension`, `link`, `list` | `scripts/node-structure` |
| `BlockIdExtension` (the `extension` feature) | `packages/lexical-extension/src/BlockIdExtension.ts`: `blockIdState`, `$getBlockId`, `$getBlockById`, `$getBlockHandle`, `$getBlockHandles`, `$resolveBlockHandle` with `BlockHandleOptions.read`, `BlockHandleResult`, `$isRootChildBlock`, `$defaultIsIdentifiedBlock` (with `LexicalNode::declares_inline_children` for a class whose `$config` declares `children: 'inline'`), `generateBlockId`, `BlockIdConfig` (`disabled`, `$isIdentifiedBlock`, `generateId`, `repairTransforms`), `BlockIdOutput`, and the extension: the index, the transforms of every registered class and the root transform (`$assignBlockIds`: lineage by children and by position, slots included, uniqueness, provisional ids across transform passes), the update listener (`$syncIndex`, `$nameUnnamed`), the repair update, document order (`$compareDocumentOrder`), the handle views; `internal/positionalBlockId.ts` (SHA-256, `canonicalJSON`, `$canonicalDocument`, `$documentKey`, `derivePositionalId`, `deriveDuplicateId`); `internal/blockHandles.ts` (`MIN_BLOCK_HANDLE_LENGTH`, `handleKey`, `buildHandleTable`, `handleInTable`, `idsWithPrefix`); from the core `$hasUpdateTag` and `COLLABORATION_TAG` | `extension::block_id_extension`, `extension::positional_block_id`, `extension::block_handles`, `editor` | `scripts/block-id`; a unit test of SHA-256 against FIPS 180-4's digests |
| Root element and mutation listeners | `LexicalEditor.ts`'s `setRootElement` (its model half: `resetEditor` keeping the pending state and the queue, a full reconcile tagged `history-merge`, then the root listeners), `getRootElement`, `registerRootListener`, `registerMutationListener` (by node type, with `skipInitialization` and `initializeMutationListener`), `registerEditableListener` and `setEditable`'s notification; `LexicalReconciler.ts`'s walk as far as it writes back to the model (`$reconcileRoot`, `$reconcileNode`, `$createNode` with a cross-parent or cross-slot-host move reconciled, `$destroyNode` with a host's slot subtrees, `$reconcileChildren`, `$reconcileNodeChildren`, the slot walks `$mountSlotChildren` and `$reconcileSlotChildren` with their containers by identity, `reconcileDecorator`; the first-text capture `$beginCaptureGuard`, `$endCaptureGuard` and `$bubbleChildFirstText` over `__lexicalFirstTextKey`, and `$reconcileChildrenWithDirection`'s `reconcileTextFormat` and `reconcileTextStyle`, which set a reconciled element's `textFormat` and `textStyle` from its first text, not on the root or a shadow root, not in a read-only state, and in a full reconcile only for an element the update cloned or marked), with `editor._keyToDOMMap` as `ReconciledNode` records and `setMutatedNode` (a destroy then a create is an update) and each node's `updateDOM` as a model hook (`LexicalNode::update_dom`: text by its tags, `HeadingNode` by tag, `ListNode` by type, `AutoLinkNode` by `isUnlinked`); `triggerMutationListeners` before the text content and update listeners; `$onUpdate` and the deferred callbacks; the root element is the app's (`RootElement`, compared by identity); `RootElementExtension.ts` | `editor`, `reconciler`, `updates`, `node`, `extension::root_element_extension` | `scripts/a11y` and `scripts/auto-embed` (the `mutations` logs), `scripts/reconciler` |
| `@lexical/a11y`'s announcements (the `a11y` feature) | `packages/lexical-a11y/src/index.ts`: `AriaPoliteness`, the live region `createLiveRegion` makes (role, `aria-atomic`, `aria-live`, `textContent`) as `LiveRegion`, mounted on the app's `LiveRegionHost` (the configured `owner`, or what the root element names, where JS uses its document's body), `AriaLiveRegionExtension` (`announce` with U+200B on a repeat, its three effects: mounting per root or `owner`, the politeness, the message without replaying it into a new region), `HistoryAnnounceExtension`, `EditorModeAnnounceExtension`; `packages/lexical-rich-text/src/HeadingAnnounceExtension.ts` (with `rich-text`) and `packages/lexical-link/src/AutoLinkAnnounceExtension.ts` (with `link`), `%s` replaced once as JS's `replace` does | `a11y`, `rich_text::heading_announce_extension`, `link::auto_link_announce_extension` | `scripts/a11y` |
| `DecoratorBlockNode` (the `decorator-block` feature) | `packages/lexical-react/src/LexicalDecoratorBlockNode.ts`: `DecoratorBlockNode` as data a node returns from `LexicalNode::decorator_block` (a block decorator, `isInline` and `canIndent` false), `getFormat`, `setFormat`, `decoratorBlockNodeSchema`'s `format` (`enumValue`, left out of the compact form when `''`), `$isDecoratorBlockNode` | `decorator_block`, `node` | `scripts/decorator-block`, `serialization/decorator-block` |
| The typeahead menu's text matching (the `menu` feature) | `packages/lexical-react/src/LexicalTypeaheadMenuPluginUtils.ts` (`PUNCTUATION`, `useBasicTypeaheadTriggerMatch` as `basic_typeahead_trigger_match`, its `RegExp` matched by hand over UTF-16 code units, with the trigger and punctuation read as character class bodies); `LexicalMenuOption.ts` (`MenuOption`); `shared/LexicalMenu.tsx` (`MenuTextMatch`, `MenuResolution` anchored at a model range, `getFullMatchOffset`, `$splitNodeContainingQuery`, `selectOptionAndCleanUp`, the highlighted index with `preselectFirstItem`); `LexicalTypeaheadMenuPlugin.tsx` (`getTextUpToAnchor`, `isSelectionOnEntityBoundary`, the update listener with `tryToPositionRange`'s model half, `closeTypeahead`/`openTypeahead` with an `onClose` that returns a promise as `on_close_async` and its `MenuCloseDone`, `closeUnlessComposing`, the editable listener, `commandPriority`); `LexicalNodeMenuPlugin.tsx`'s `positionOrCloseMenu`; `LexicalMenu`'s keyboard commands while a menu is open, at its `commandPriority` (`KEY_ARROW_DOWN_COMMAND` and `KEY_ARROW_UP_COMMAND` wrapping the highlight and dispatching `SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND`, `KEY_ESCAPE_COMMAND`, `KEY_TAB_COMMAND`, `KEY_ENTER_COMMAND` but Shift+Enter; an empty menu the default renderer draws nothing for lets them go on) | `menu` | `scripts/menu`; unit tests for the keys, the close while composing and `on_close_async` (a React hook, which no vector records; Swift's `LexicalMenuTests` check the same) |
| Auto-embed (the `auto-embed` feature) | `packages/lexical-react/src/LexicalAutoEmbedPluginUtils.ts` (`EmbedMatchResult`, `EmbedConfig`, `URL_MATCHER` matched by hand as `url_matcher`, `INSERT_EMBED_COMMAND`, `AutoEmbedOption`); `LexicalAutoEmbedPlugin.tsx`'s logic as `register_auto_embed`: the `PASTE_COMMAND` handler at `COMMAND_PRIORITY_BEFORE_EDITOR` with `/^\S+$/` (`is_single_token`) and `$onUpdate`, the `LinkNode` and `AutoLinkNode` mutation listeners, `checkIfLinkNodeIsEmbeddable` (the last config wins, `latestCheck`, `$getCurrentLink`, `$disableMenuFor`), `embedLinkViaActiveEmbedConfig`, `getMenuOptions` and the `INSERT_EMBED_COMMAND` handler, driving a `NodeMenu` | `auto_embed` | `scripts/auto-embed` (`URL_MATCHER`, the single-token test, the command and the mutations a paste makes) |
| Markdown (the `markdown` feature) | `packages/lexical-markdown/src`: `MarkdownTransformers.ts` (the four transformer types; `HEADING`, `QUOTE`, `CODE`, `UNORDERED_LIST`, `ORDERED_LIST`, `CHECK_LIST`, the text formats, `LINK`, `TRANSFORMERS` and the groups by type; the list marker, code fence, code meta and hard line break NodeStates; `normalizeMarkdown`, `isTableRowDivider`), `MarkdownImport.ts` (`createMarkdownImport`, `$importMultiline`, `$importBlocks`), `importTextTransformers.ts`, `importTextFormatTransformer.ts`, `importTextMatchTransformer.ts`, `MarkdownExport.ts` (`createMarkdownExport`, with selection), `utils.ts`; `$convertFromMarkdownString`, `$generateNodesFromMarkdownString` (into an `ArtificialNode__DO_NOT_USE`), `$convertToMarkdownString` and `$convertSelectionToMarkdownString` (`index.ts`); `MarkdownShortcuts.ts` (`registerMarkdownShortcuts`) | `markdown` | `markdown/import-export` (its `importNodes`), `property/documents`, `scripts/markdown-shortcuts` |
| UTF-16 offsets | `String#length`, `String#slice` | `utf16` | through the text methods; unit tests against Node |
| Upstream version | `meta.json` | `UPSTREAM_VERSION` | `meta` |

## Not ported yet

Everything else; [DESIGN.md](DESIGN.md) has the order. Within what M0
touches:

- Of the schema facility: `SchemaFieldBase.method` and
  `resolveSchemaField`, the prototype lookup that abandons a field access
  for an accessor a subclass overrides (a Rust class has no methods to look
  up; it gives its base's schema with `NodeSchema::with_setter` instead, as
  `CodeHighlightNode` does for `format`); `getterTable` and `setterTable`
  as data, and `getterTableOf`, `setterTableOf`, `aliasTableOf` and
  `setterDefaultOf` (a field's conversion is its `SchemaValue` or its
  accessor closures, as for `TextNode`'s `mode` and `ElementNode`'s
  `format`); the conventional `get<Prop>`/`set<Prop>` names a plain schema
  stands for, and the registration checks of them (every property names its
  accessors); `@lexical/compiler`'s `SchemaJsonCodegen.ts` and
  `verifyCompiledParse` (the generated code is ported by hand and checked
  against the walk); the freezing of derived defaults (`deepFreeze`,
  `markCallerOwned`), which a Rust value that is cloned on every read does
  not need; and `validateOwnFields`. Containers compare by content, as in
  Swift, where a JS array or object without `isEqual` compares by
  identity; no vector tells them apart, since JS parses a fresh container
  each time. The schemas of the packages not ported yet (`@lexical/table`,
  the playground's nodes) come with them.
- `LexicalGenMap.ts` (`GenMap`, `cloneMap`), JS's generational node map,
  whose clone is O(1) with a copy on the first write. An editor state's
  node map here is a `HashMap` of `Rc` nodes (`editor_state::NodeMap`),
  which an update's clone copies as pointers, the nodes staying shared until
  `getWritable`, as Swift's `JSMap` does (swift/DESIGN.md, GenMap); a GenMap
  would change the cost of a clone, not what any editor state holds.
- Of slots: the `'deletion'` mode of `$setPointAtSlotBoundary`, where JS
  lets `Selection.modify` carry a deletion into another slot frame and
  pulls it back to the boundary; a headless editor's `SelectionMovement`
  moves within text only, as Swift's does. `LexicalMutations.ts`' slot
  handling and the slot containers' DOM (`$createSlotDOM`,
  `$applySlotEditable`, `$applySlotTarget`) are DOM-only.
- Of `DecoratorNode`: the parts of `$extendSelectionForDeletion` and `applyDOMRange` that read a decorator's
  DOM (a line boundary landing in its private text, a DOM range inside
  one) have no model counterpart. `isSelected()`, with its rule for an
  inline decorator at the end of its parent, is ported (for the tree view,
  the `devtools` feature, and `@lexical/clipboard`'s copy), though no vector reaches that rule,
  and so is `getTopLevelElement` (see the node methods row). Of `@lexical/extension`'s
  `HorizontalRuleExtension`, the selected class isn't ported (see the
  extension API below), and `applyFormatFromStyle` reads CSS.
- Of the extension API (the `extension` feature): `InitialEditorConfig`'s
  `disableEvents`, as the app dispatches the events it has (`namespace`,
  the last extension's that sets one winning, `theme` and `parentEditor`
  are ported, see the nested editors row, and `html` and `dom` come with
  the `html` feature; JS's `deepThemeMergeInPlace` also merges an object
  into an array, which a theme's JSON never holds, and Rust replaces it;
  `editable` and `onWarn` are ported, `onWarn` getting the editor as JS's
  does); `onError`, as an update's error is the `Result` that
  `Editor::update` returns (`CreateEditorArgs` has no `onError` either);
  `Symbol.dispose` (dropping a `LexicalEditorWithDispose` doesn't dispose
  it; `dispose` removes the root element first, then unregisters, as JS's
  `mergeRegister` does, and `scripts/input-extensions` records it);
  `builderSymbol`'s version
  and copy checks in `maybeFromEditor`, which one crate can't break; a
  dependency cycle, which JS builds by mutating a defined extension's
  `dependencies` and Rust can't, as a defined extension is an immutable
  `Rc` (the builder keeps the cycle invariant, unreachable); the
  `nodes` thunk form (`nodes` is a list); `HMRExtension`. Configs are Rust
  types that name their overrides' type (`ExtensionConfigBase::Partial`);
  a JSON `Object` merges as JS does. Signal values compare with `==`
  (`SignalValue`; an `EditorState` by identity) where JS's `!==` compares
  objects by identity, and effect and computed bodies can't throw, so
  preact's error paths have no counterpart and a cycle panics. `named_signals`
  takes a JSON object; a typed config makes its signals field by field.
  The input extensions (the row above) listen to the commands an app's
  view sends where JS listens to the root element, as Swift's do:
  `ClickAfterLastBlockExtension` takes a `CLICK_COMMAND` whose
  `target_key` is `root` (the view decides that the click is below the
  last block, which JS measures with `getBoundingClientRect`) at
  `COMMAND_PRIORITY_CRITICAL`, running `$shouldInsertAfter` once, in that
  command's update, where JS runs it in a read and again in its update;
  `NormalizeTripleClickSelectionExtension` runs `$fixFocusOverselection`
  on a `CLICK_COMMAND` whose `detail` is 3 or more at
  `COMMAND_PRIORITY_BEFORE_CRITICAL`, where JS arms on `mousedown` and
  trims the next selection change that matches the DOM, and its
  deprecated `thresholdMsec` and `dateNow` aren't ported;
  `IMEExtension` clears the composition at `COMPOSITION_END_COMMAND`
  (`COMMAND_PRIORITY_BEFORE_EDITOR`) where JS listens to `compositionend`,
  and `composingTextNode` compares nodes by key; `AutoFocusExtension`
  asks the app's `RootElement` (`has_focus`, `focus`) where JS asks the
  DOM. `FormatKeyboardShortcutOptions.separator` is kept but unread, as
  in JS. `NamedKeyboardShortcuts` compares by identity, as JS's objects
  do. The scripts' composition starts on a text point: one on an
  element point inserts JS's placeholder for the browser (the empty
  space heuristic, see the events rows), which the port leaves out. The
  rest of `@lexical/extension`'s extensions (`NodeSelectionDataSelected`,
  `SelectionAlwaysOnDisplay`, `WatchEditable`) read the DOM, a
  selection's DOM or focus. Of the model-level
  ones: `NestedEditorExtension` is ported (see the nested
  editors row); `createEditor()` called without a config, which JS nests
  in the active editor, has no counterpart, as `create_editor` always
  takes `CreateEditorArgs` (as Swift's does); `_headless` is copied from
  the parent, as in JS. Of JS's other `_parentEditor` uses,
  `LexicalEvents.ts`' `activeNestedEditorsMap` (which nested editor has
  DOM focus) and `HMRExtension`'s keys come with the DOM and HMR.
  `NodeStructureExtension` is ported (see the rows above), so
  `BlockIdExtension`'s peer dependency on `@lexical/NodeStructure` resolves
  where an editor has both, and the builder registers it first.
  `HorizontalRuleExtension`'s `CLICK_COMMAND` handler finds the rule by
  the event's `target_key`, the node the app hit, where JS calls
  `$getNodeFromDOMNode(event.target)` (as Swift's `targetKey`). Its
  selected class isn't ported: JS keeps the theme's `hrSelected`
  (`selected` by default) on each rule's `<hr>` through a mutation listener
  that reads `editor.getElementByKey`, and a Rust editor has no DOM of its
  nodes, so an app draws the selected state from the rule's `watchNodeKey`
  signal. `NodeSelectionExtension`'s watched keys are a list in the order
  first watched, as JS's `Map` iterates.
  Of `BlockIdExtension`: `generateBlockId` hashes a counter and the clock
  with the standard library's randomly keyed `RandomState` where JS uses
  `crypto.getRandomValues` (no dependencies: it is unpredictable but not
  a cryptographic generator, and the config takes a `generate_id` of an
  app's own); and `frozenLookup`'s per-state cache, which only
  saves repeated lookups and changes no result. `$getBlockHandles()`
  returns its `Map`'s entries in order, and `BlockHandleOptions.read`
  takes them so. `PreventSelectAllExtension`'s
  `register` is a capturing `keydown` listener on the root element, so
  only its config and signal are ported. Of the
  packages' extensions, the DOM, click and focus ones (`DragonExtension`,
  `CheckListExtension`'s pointer, focus and key handlers) aren't ported (`ClickableLinkExtension` is, on
  `CLICK_COMMAND`, see its row and the `@lexical/link` item below)
  (the HTML import ones come with `html`). `HistoryExtension`'s `disabled` defaults to `false`,
  as Swift's does, where JS's is `typeof window === 'undefined'` (true in a
  headless JS editor); `SharedHistoryExtension`'s HMR re-link
  (`hmrRestoreCount`) isn't ported. A config's functions
  (`validateUrl`, `createInitialHistoryState`, `$canIndent`, the matchers)
  are `Rc` closures, which compare by identity as JS's do.
- Of NodeState: `$getStateChange`'s use in `updateDOM`, which comes with
  the DOM; and the check that a flat key is not an `Object.prototype` property, which has no
  counterpart in the port's own JSON object. JS tells the version of a node
  that owns a NodeState by object identity; a node version here is its key
  and the editor state it was made writable in. `StateConfig` values are
  Rust types (`StateValue` for numbers, booleans, strings and `Option`, or
  any `V` with `create_state_with`'s `StateValueConfig`); JS's unknown
  values compare by identity (`===`), which the port keeps by holding each
  one in an `Rc`.
- Deletion and `modify` past the model: JS's `deleteCharacter`,
  `deleteWord`, `deleteLine` and `modify` measure moves within text with
  the DOM's `Selection.modify`. A headless JS editor has no window and
  throws (`window object not found`), and so does this port unless the
  editor has a `SelectionMovement` (`Editor::set_selection_movement`),
  which an app implements from its text layout; the rest follows JS from
  where it lands (a unit test checks that against JS with a stubbed
  `Selection.modify`). The vectors record the headless behavior, except
  `property/sequences`, whose editor has a window that gives no DOM
  selection (jsdom's, installed after Lexical loaded, so `CAN_USE_DOM` is
  false): there `$extendSelectionForDeletion` and `modify` stop after their
  model step, which a movement whose `has_dom_selection` is false does. The DOM
  details around the measurement (block cursor element, shadow DOM ranges,
  Firefox's line-affinity retry) have no counterpart.
- Of `@lexical/selection` and `@lexical/utils`, what needs the DOM, a
  browser or pieces the port doesn't have: `createDOMRange`,
  `createRectsFromDOMRange`'s measuring of the range and the root (its
  filtering is `filter_selection_rects`, and `dedupeSelectionRects` is
  ported: they are geometry), `positionNodeOnRange`, `markSelection`,
  `selectionAlwaysOnDisplay` (DOM ranges and rects);
  `addClassNamesToElement`, `removeClassNamesFromElement`, `isHTMLElement`,
  `isHTMLAnchorElement`, `isBlockDomNode`, `isInlineDomNode`,
  `getScrollParent`, `calculateZoomLevel` (DOM elements); `mediaFileReader`
  (a `FileReader`; a `DRAG_DROP_PASTE` handler reads each
  `DataTransferFile` itself, as Swift's does); `eventFiles` takes the event
  as `FilesEvent` (a drag or a paste), where JS tells them apart by class;
  the `CAN_USE_*` and `IS_*` environment flags (a
  browser's user agent). The computed styles
  (`$getComputedStyleForElement`) are what the root element reports
  (`RootElement::computed_style`, `direction` and `writingMode` only, as
  Swift's `ComputedStyleProvider`), where JS calls `getComputedStyle` on
  the node's DOM; without one they are `None`, as for a headless JS editor,
  so the arrows' right-to-left and vertical swaps don't apply.
  `$sliceSelectedTextNodeContent`'s default `'self'`
  mode (it writes the node's text without `getWritable`, and a node's
  committed value can't change in place here);
  `objectKlassEquals` (JS constructors, which a node type string stands for
  here); and `trimTextContentFromAnchor`, JS's deprecated other name for
  `$trimTextContentFromAnchor`.
- Of `@lexical/list`'s DOM parts, `updateDOM` (the reconciler's);
  `registerCheckList`'s pointer, focus, arrow, Space and Escape handlers:
  the pointer ones hit-test the DOM, and the key ones act on the check
  list item that has DOM focus (`getActiveCheckListItem` reads
  `document.activeElement`), which a model has no counterpart for; Swift
  leaves them out too, and a toggle is `toggle_checked`. What `@lexical/list` does where `NodeStructureExtension` enforces
  node structure is ported (see the node structure row), with
  `ListImportRules`' flatten step, which then puts no line break beside a
  block decorator (`html/import-packages`' `nodeStructure` cases). `ListNode` reads its tag off its list type rather
  than keeping `__tag` apart: JS sets both together everywhere. Backspace
  at the start of a document's first item, with nothing before it, needs
  `Selection.modify` in headless JS, which throws, as `scripts/list`
  records.
- Of `@lexical/link`'s DOM parts, `updateDOM`. `sanitizeUrl` asks `new URL` whether a URL
  parses and for its protocol; as in Swift, that is a model of the WHATWG
  parser's scheme and host rules without IDNA or IPv4 parsing, so a host
  only those reject (`https://1.2.3.4.5`) is taken as parsing.
  `ClickableLinkExtension` handles the app's `CLICK_COMMAND` at
  `COMMAND_PRIORITY_LOW`, the link found from the event's `target_key`
  (an `auxclick`'s type and `button` with it), and opens it in the app's
  `LinkWindow` (`getWindow` returns none by default, as Swift's, where JS
  returns the global `window`); it opens a plain `<a>` that isn't a
  `LinkNode` (`findMatchingDOM`) in JS, which a model has no counterpart
  of. `sanitizeUrl` is exported without `html`
  (`AutoLinkAnnounceExtension` is ported, with `a11y`, and is a dependency of `AutoLinkExtension` there as in JS). Where `NodeStructureExtension` enforces node structure,
  `$linkNodeTransform` leaves lifting blocks and merging equal adjacent
  links to its normalization, as JS does (see the node structure row). `extractWithChild` is a node hook that `@lexical/html`'s
  export (the `html` feature) and `@lexical/clipboard`'s copy (the
  `clipboard` feature) call; `HeadingNode`'s, `ParagraphNode`'s,
  `ListNode`'s, `ListItemNode`'s and `LinkNode`'s overrides are ported. In a headless editor,
  `LinkNode.canInsertTextBefore` and `canInsertTextAfter` change nothing a
  vector can see: `insertText` at the edge of an inline parent's text puts
  the text outside the parent either way, and
  `$resolveSelectionPointOnBoundary`, which also reads them, runs only
  after `Selection.modify`.
- Of `@lexical/code-core`'s DOM parts, `updateDOM`.
  `registerCodeGutter` (`CodeGutter.ts`) is `register_code_gutter`, a
  `CodeNode` mutation listener that gives the app each block's line
  numbers (`"1\n2\n3"`) where JS writes them to the block's
  `data-gutter` attribute, again only when its number of children
  changes, and nothing in a headless editor; as no DOM is kept, the app
  draws them. JS's `registerCodeGutter` is `@internal` and registered by
  `registerCodeHighlighting`; the port's `register_code_highlighting` leaves
  it out, as it has no attribute to write, and an app that draws line
  numbers registers it. `register_code_indentation` takes `escapeWithArrows` as a third
  argument (JS's `registerCodeIndentation(editor, tabSize,
  escapeWithArrows)`). In an editor without
  `CodeExtension` (one made with `create_editor`, or built without it),
  `CodeNode.insertNewAfter` takes the path JS takes there (exiting the
  block after two line breaks itself, as `scripts/code` records with
  `createEditor`), with its deprecation warning (`warnOnlyOnce`, once per
  thread); in one built with it,
  `INSERT_PARAGRAPH_COMMAND` adds a line even after two line breaks, as
  in JS, where exiting is the `KEY_ENTER_COMMAND` handler's. JS's `CodeHighlightNode`
  constructor keeps a `null` highlight type it is given; nothing but a
  direct `new` makes one, and Rust has none. The highlighting loop of `@lexical/code-prism` is ported
  (`code-highlight`), but not a tokenizer: Prism's grammars
  (`FacadePrism.ts`'s `PrismTokenizer`, the language maps and
  `tokenizeDiffHighlight`), Shiki's and Swift's tree-sitter tokenizer would
  each be a third-party dependency, so an app supplies a `Tokenizer`, and
  the default one (`PlainTokenizer`) knows no language. Without Prism the
  loop asks the tokenizer whether a language is loaded, where JS asks
  Prism's `isCodeLanguageLoaded`; `scripts/code-highlight` gives its
  tokenizers only languages Prism has, so the two agree, and replays
  Prism's tokens recorded in the vector. Of the 25 `property/documents` cases with code nodes, 4
  also hold hashtag or overflow nodes; they waited on those packages,
  which are now ported.
- `@lexical/mark`'s `updateDOM` (the reconciler's; `createDOM` is
  ported, but the HTML export leaves marks out, so only a unit test sees
  it), and the renderer's half of `MarkStateExtension.ts`:
  `$decorateMarkIDs` with `$classTokensOfOtherFlavors` (the `$decorateDOM`
  override, which sets the ids attribute and the theme's `mark` and
  `markOverlap` classes, or a flavor's `className` and `overlapClassName`,
  on the reconciled element), the `renderDisabled` render state it is gated
  by, and `isMarkIDShown` with `registerMarkIDShownRerender` (a full
  reconcile when it changes). The port has no reconciled DOM for them to
  write to (DOMRenderExtension's reconciler hooks are not ported either);
  Swift's are on its DOM views, which no vector records. So the
  `isMarkIDShown` config and output are left out, and a flavor's
  `className` and `overlapClassName` are kept in
  `MarkFlavorDefinition` without effect. The HTML export of the ids
  (`exportIDsToHTML`, through the `$exportDOM` override) and the ids
  attribute's import rule (`createMarkIDsImportRule`) are ported, with
  `html` and `extension` (`html/mark-state`).
  `MarkableNodeMatch` is a guard: JS also takes a class, matched with
  `instanceof`, which the port spells as the class's `is_…` guard
  (`NodeRef::is_text` for `TextNode`). The flavor-first overloads of the
  `$` functions are `*_for` functions (`add_mark_id_for(flavor, ...)`),
  and `$getMarkIDsByFlavor`'s record is a list of `(stateKey, ids)` in the
  order the flavors were built. JS forgets the inserted-text rule's
  pending edits (the selection an insertion or paste replaces, the ids
  `$removeMarkID` removed) at the next microtask (`forgetSoon`); with no
  event loop, the port keeps the pending editor state they were recorded
  in and forgets them once the editor has another (or none), and at each
  commit, as JS does; within one command dispatch (whose priority levels
  each run in an update of their own when dispatched outside an update) it
  keeps them for the next level's pending state, as JS's microtask runs
  only after that update commits. `$isSplitOffEntity`'s identity of the ids array
  `splitText` copied is the identity of the stored value (`Rc::ptr_eq`),
  as Swift compares array storage. `scripts/mark-extension` composes over
  a selection with the selection collapsed by the input layer first, as
  `$handleCompositionStart`'s placeholder (the empty space heuristic) is
  not ported (see the core's event handlers below).
  JS's `markIds` state is an `arraySetValue`, and so is the port's
  (`StateValueConfig::from_schema`), whose canonical form `$setState`
  gives the value it is set to. Ids sort by UTF-16 code units, as JS's
  `sort()` does. In a headless editor,
  `MarkNode.canInsertTextBefore` and `canInsertTextAfter` change nothing a
  vector can see, as for `LinkNode`: `$insertText` puts text typed at the
  edge of an inline parent's text outside it either way. `extractWithChild`
  and `excludeFromCopy` are node hooks that `@lexical/clipboard`'s copy
  (the `clipboard` feature) calls, as `@lexical/html`'s will;
  `scripts/mark` calls them directly.
- `@lexical/overflow`'s `updateDOM`. `excludeFromCopy` is called by
  `@lexical/clipboard`'s copy (the `clipboard` feature) and by
  `@lexical/html`'s export, with `'html'`. Of `@lexical/react`'s character
  limit (the `overflow` feature): `findOffset`'s fallback for an engine without
  `Intl.Segmenter` (code points), which Node and the vectors never take;
  and the hook's re-registration when React's dependencies change, which an app does by
  calling the clean-up and registering again. Of the 36
  `property/documents` cases that waited on
  `lexical-overflow`, 11 also hold hashtag nodes and waited on
  `lexical-hashtag`, now ported. That suite now sets each parsed state on the editor
  and reads the text there, as JS does: a parsed state is all dirty, so
  the transforms run, and `OverflowNode`'s removes the empty ones a random
  document holds.
- Of `@lexical/hashtag`: within `registerLexicalTextEntity`,
  `$carryNodeState`, `splitOffInUpdate`, `demotedInUpdate` and
  `$isInsertedText` are ported (`scripts/node-state`'s hashtag cases), and
  so is the NodeState half of `$wouldMerge` (`haveEquivalentNodeState` with
  `readStateValue`), but no vector reaches it: only a token or segmented
  entity kept as one does, and `createNode` makes a hashtag in normal mode
  (a hashtag given token or segmented mode in a document is replaced by a
  new one before that check, as JS records), so a unit test checks the
  rule against JS's. The text edit log isn't ported, so
  `INTERNAL_$recordTextRetype` is not called and `INTERNAL_$recordTextMove`
  just runs its callback. The per-update
  maps JS keeps in `WeakMap`s by pending editor state hold the entry of
  the last state asked about, as one state is pending at a time. JS's
  `targetNode` class is a node type here, as `registerNodeTransform`
  takes one.
- Of `@lexical/file`: the DOM halves of `importFile` and `exportFile`
  (`readTextFileFromSystem`'s `<input type="file">` and `FileReader`,
  `exportBlob`'s `Blob`, object URL and download link), which an app
  supplies as callbacks. `exportFile`'s `toISOString` is written for the
  years 1970 to 9999, which `Date.now()` stays within.
- Of `@lexical/devtools-core`: `generateContent`'s `exportDOM` form is
  built with the `html` feature (without it, it fails with an error), and
  its mark branch with `mark`; its table branch (`$isTableSelection`,
  `printTableSelection`) waits for `@lexical/table`; and the React parts, `TreeView.tsx` and
  `useLexicalCommandsLog.ts` (a log of commands is the caller's, as
  `LexicalCommandLogEntry`s with the payload already printed).
  `obfuscateText`'s `*` and the `^` widths count UTF-16
  code units, as JS does; no vector obfuscates.
- In `$setBlocksType`, the branch for an empty declared block container
  (`$isBlockContainer` of a class declaring `children: 'block'` where
  `NodeStructureExtension` enforces node structure) is ported, but no
  vector reaches it yet: `scripts/node-structure` covers declared
  containers through `$insertList` and `$isBlockContainer` itself.
- The text edit log: `$removeTextFromCaretRange` records no deletions.
- Of `@lexical/history`: `register_history` takes the delay and
  `maxDepth` as values, where JS also takes signals (`HistoryExtension`
  passes its signals to the same code), and `maxDepth` as a `usize`.
  `HistoryExtension`'s deviations are with the extension API above.
- Of `registerRichText`'s and `registerPlainText`'s key and pointer
  handlers, what a platform without `beforeinput` or a DOM has no
  counterpart for, as in Swift: the `IS_IOS`/`IS_SAFARI` returns that
  leave Backspace and Enter to `beforeinput`, inline-grid line navigation
  (CSS grid layout), the embedded-input checks
  (`$isEventFromEmbeddedInput`, `$isTargetWithinDecorator`) and the iOS
  text-file drop. `$isParentRTL` reads the root element's computed style
  (the selection row above); `scripts/key-handlers` stubs it. The arrows reach
  `Selection.modify` only through the editor's `SelectionMovement`, so the
  vectors record the cases the model decides (headless JS throws past
  them). A click's target is the node the app hit (`MouseEvent::target_key`),
  not a DOM element (JS's target is the DOM the reconciler made for it);
  `scripts/key-handlers`' clicks name no target, so a node selection is
  cleared. `escapeFormatTriggers` keeps its formats in order
  (`Vec<(String, Option<TriggerConfig>)>`), as a JS object does.
  `CONTROLLED_TEXT_INSERTION_COMMAND`'s `beforeinput` form and `updateDOM`
  aren't ported, nor is `PlainTextExtension`'s and `RichTextExtension`'s
  dependency `DragonExtension`, which needs an input layer (they depend
  on `NormalizeTripleClickSelectionExtension`, as in JS and Swift).
  `quoteShadowRootState` is a flat NodeState, as in JS
  (`quote_shadow_root_state`; `scripts/node-state` sets and reads it). A
  shadow root quote's `collapseAtStart` (dissolving into
  its blocks) is ported but no vector reaches it: headless JS needs the
  DOM's `Selection.modify` to get there and throws, which `scripts/rich-text`
  records.
- Of `@lexical/html` and the core's DOM side (the `html` feature):
  `$inlineStylesFromStyleSheets` parses each `<style>`
  sheet itself (`dom::parse_style_sheet`) and matches its selectors with
  `DomNode::query_selector_all`, which takes compound selectors, combinators
  and attribute operators; a rule whose selector needs more (pseudo-classes,
  namespaces, escapes) is skipped where a browser would apply it, and a
  shorthand property (`font`, `margin`) is copied without the longhands a
  browser expands it into, so its parts don't reach the import rules (JS
  imports `font: bold 12px` as bold). `html/import-extension` has a case of
  every other kind of rule.
  Also `updateDOM`, `getDOMSlot` and the
  rest of what only the reconciler calls. `$setTextContent`'s composition
  suffix in `TextNode.createDOM` is `COMPOSITION_SUFFIX` outside Safari and
  iOS, a zero width space, as in Node (Swift writes the no-break space of
  Safari); a unit test checks it against JS. So the render config
  and `DOMRenderExtension`'s overrides have only the hooks export reaches
  (`$createDOM`, `$exportDOM`, `$extractWithChild`, `$shouldInclude`,
  `$shouldExclude`), and toggling an override through
  `$setRenderContextValue` recompiles the config without the full
  reconcile JS runs to recreate the DOM. `precompileDOMRenderConfigOverrides`'
  table (consecutive class overrides merged into `'types'` buckets by
  `getRegisteredSubtypeMap`, then the guards) isn't kept: each class
  override is a step behind an `instanceof` guard, in the same order, which
  composes the same hooks (`html::internal::compile_dom_render_config_overrides`).
- Of `@lexical/clipboard` (the `clipboard` feature):
  - `trustHTML`: there are no Trusted Types outside a browser, so the
    HTML is parsed as is (as in Swift). `ClipboardDOMImportExtension` puts
    a copy of the `DataTransfer` in `ImportSourceDataTransfer`, since the
    import stacks get it by reference. `scripts/clipboard` records copies
    without `text/html` (`ops.ts`'s `$clipboardDataWithoutHtml`, which
    the Rust runner mirrors); `html/clipboard` records them with it.
  - Of drag and drop: `caretFromPoint.ts`'s DOM hit test, which the root
    element does (`RootElement::caret_from_point`; `None` by default, as
    jsdom resolves no caret, so the vectors' drops insert nothing); and a
    drop from another editor's `deleteByDrag` at the source editor's DOM
    root, which is the source platform's to do. The drag start of a range
    selection in rich text writes its clipboard data, HTML among it, which
    headless JS can't make (`$generateHtmlFromNodes` needs a DOM), so no
    vector records it; plain text's marker is recorded.
  - `copyToClipboard`'s DOM and timer plumbing (without an event it fires a
    `copy` at the document, with a 50 ms timeout and a guard against a copy
    in flight) and the promise it returns. The copy and cut handlers take
    JS's payload (`ClipboardOrKeyboardEvent`, or `None`) and, without a
    clipboard event, copy nothing; a cut still removes the selection. `$copyToClipboardEvent` also declines in JS
    when the window has no DOM selection or it is outside the editor,
    which in headless JS is always (a copy writes nothing); the port, like
    Swift, writes the selection, since without a DOM the platform only
    sends the editor's own copy. No vector records the handlers, as
    headless JS can't run them; unit tests cover them.
  - `INTERNAL_$withExportMemo`, a cache that changes no output.
  - `$sliceSelectedTextNodeContent`'s `'self'` mode, which writes a node's
    text without `getWritable`; every ported caller passes `'clone'`.
- The DOM half of the reconciler (the elements, the text and direction
  caches, the DOM `editor._keyToDOMMap` holds) and `RootNode.__cachedText`
  itself (only its effect on
  the dirty root is ported; `getTextContent` always walks the tree). With a root element an update
  starts with the last selection, as in a headless editor and in Swift:
  JS reads the DOM selection (`$internalCreateSelection`), which an app's
  view keeps in step (in Node JS has none, so the vectors select in each
  update).
- Of `@lexical/a11y`: the focus trap, the roving tabindex and the focus
  manager, which move DOM focus between HTML elements, and the region's
  visually hidden style. Of the menus: the React rendering, the anchor element, its scroll
  and resize handling. An `onClose` that returns a promise is
  `on_close_async`, given a `MenuCloseDone` that the app finishes (or
  drops, as a rejected promise) where JS awaits it. Auto-embed's `parseUrl` is synchronous, so the menu
  opens in the paste's mutation listener where JS awaits it.
  `BlockWithAlignableContents` (`@lexical/react`) is a React component:
  its commands are an extension, as in Swift (see its row), which aligns
  every selected block where React's one handler per block aligns the
  first whose handler runs, and its rendering isn't ported.
- Of `LexicalEvents.ts` (the event commands and core handlers rows): the
  root element listeners themselves, which an app replaces by calling
  `on_key_down`, `on_before_input`, `on_composition_start` and
  `on_composition_end` with its own events; `$handleInput` (the
  `INPUT_COMMAND` handler, which reads what the browser wrote to the DOM,
  so it isn't registered), the click, focus, blur, drag and drop
  handlers, and the Android, iOS, Safari and Firefox workarounds, which
  read the DOM or the browser. `onSelectionChange` is ported as
  `on_selection_change`, its model half, as Swift's views run it: the app
  passes the points its view selected (or none) and whether the editor is
  the active one; the points become the selection, a caret takes the
  format and style where it is (`$updateCollapsedSelectionFormatStyle`), a
  range the format all its text has, and `$dispatchSelectionChangeCommand`
  announces it. Whether the selection is within the editor, the mouse drag
  adjustment, `postDeleteSelectionToRestore` and the skip of a selection
  the reconciler applied (`isSelectionChangeFromDOMUpdate`) read the DOM
  and are the view's; nothing calls `markCollapsedSelectionFormat`, which
  runs where JS writes the DOM selection, so a caret always takes the
  format where it is. The commit's own notification
  (`$notifyPendingSelectionChange`) was already ported (the update cycle
  row). `$handleBeforeInput` runs in
  controlled mode: there are no target ranges (`getTargetRanges()`), so an
  app sets the model selection to the input's range before it dispatches,
  and `deleteContentForward` dispatches `DELETE_LINE_COMMAND(false)` as JS
  does outside a slot. `$handleCompositionStart` doesn't apply the empty
  space heuristic (the placeholder JS inserts for the browser to compose
  into, or `$prepareCompositionTarget`), and `$onCompositionEndImpl` reads
  nothing back from the DOM (`$updateTextNodeFromDOMContent`,
  `$updateSelectedTextFromDOM`): an app writes an input method's marked
  text through the model, as Swift's views do. `scripts/events` records
  composition at a clock past the last keydown, where JS skips the
  heuristic too. `IS_APPLE` is the target's vendor, not a user agent.

- Of `@lexical/markdown` (the `markdown` feature): the mdast converter,
  which Swift doesn't port either. `markdown/import-export`'s
  export cases read the state JS built from each case's HTML from its
  `exportState` JSON instead of building it with `$generateNodesFromDOM`
  (which `html/import` checks); the imported state, the HTML
  `$generateHtmlFromNodes` renders of it (`importHtml`), the absence of a
  selection after importing and the exported markdown are checked in every
  case. The transformers are `Rc`s of structs with public fields, so a
  variant of a built-in is a copy with fields replaced, where JS spreads
  the object; `replace` returns `false` for JS's `false` and `true` for
  its `true` and `undefined`, and a text match transformer's `export` is
  `None` for an unset one. `register_markdown_shortcuts` returns an error
  where JS's invariant throws (a transformer's node not registered).
How `getHashtagMatch`'s regular expression runs without one (the
`hashtag` feature): JS builds it with `i` and without `u`, so it matches
UTF-16 code units, and the matcher works on the text's code units too.
Its four classes (`[alpha]`, `[alphanumeric]`, the boundary
`[^&/alphanumeric]` and `[#＃]`) are range tables that the generator gets
by asking JS's own regular expression about every code unit, so they
include the 73 code units `i`'s case folding adds (U+037B, U+13F8,
U+1C80, U+1C90, U+2C62...) and the truncation `String.fromCharCode`
applies to the CJK extension ranges in the source. A surrogate is never
alphanumeric and always a boundary, so a hashtag stops at a character
outside the BMP and may follow one. The pattern
`(^|$|[^&/A])([H])(A*LA*)` is matched as a backtracking engine would:
the first index where `^` or a boundary unit, a hash and an alphanumeric
run holding a letter follow, group 3 being the whole run. The generator
fails if upstream's pattern changes shape.

How `registerAutoLink`'s regular expressions run on the `regex` crate (the
`autolink` feature), where JS's `RegExp` behaves differently:

- `regex` reports UTF-8 byte offsets; `LinkMatcherResult`'s `index` and
  `length` are UTF-16 code units, as JS's `match.index` and
  `match[0].length`.
- JS's `\s` is ECMAScript's WhiteSpace and LineTerminator, with U+FEFF and
  without U+0085; `regex`'s is Unicode's White_Space, the other way round.
  The patterns spell out JS's set (`link::JS_WHITESPACE`), in `EMAIL_REGEX`,
  the default `separatorRegex` (`/[.,;\s]/`) and `getTextNodesToMatch`'s
  `/[\s]/`.
- JS's `.` excludes `\n`, `\r`, U+2028 and U+2029; `regex`'s only `\n`.
  `EMAIL_REGEX`'s quoted local part spells out JS's.
- `URL_REGEX` has the `u` flag and matches code points, as `regex` does.
  `EMAIL_REGEX` doesn't, so JS matches it on UTF-16 code units: a
  character outside the BMP counts twice towards `{1,64}`, and a match may
  start at the second half of a pair. The port runs it on a string with one
  character per code unit (a surrogate standing as U+E000, which every
  class in the pattern treats alike). `createLinkMatcherWithRegExp` takes a
  `regex::Regex`, which matches as a `u` regex does, and JS's `g` and `y`
  flags have no counterpart.
- JS reads `[` in a class as itself; `regex` starts a nested class, so it
  is escaped.
- Both engines' `find` is leftmost-first with the same preferences (the
  patterns have no backreferences or lookaround), so they find the same
  match.
- `separatorRegex` is tested on one UTF-16 code unit, as JS tests
  `text[i]` (half a surrogate pair as U+FFFD, under the unpaired surrogate
  exception below), or on the string `undefined` past either end of a text,
  as JS does.

How `@lexical/markdown`'s regular expressions run (the `markdown`
feature): its transformers are JS `RegExp`s whose sources and flags are
public API (custom transformers bring their own, and import rebuilds them
with `new RegExp(regExp.source, flags)`), and the `regex` crate has no
lookaround or backreferences, so `markdown::RegExp` is the module's own
backtracking engine. It follows ECMA-262's pattern semantics on UTF-16
code units: offsets are code units, a pattern without `u` reads a
character outside the BMP as two, and one with `u` as one code point. It
has groups (capturing, non-capturing and named), lookahead and lookbehind,
backreferences, greedy and lazy quantifiers with the spec's capture reset
and empty-iteration check, classes, `\p{..}` and `\P{..}` for the general
categories `rust/scripts/generate-reg-exp-tables.mjs` reads from Node
(Unicode 17), and the flags `d g i m s u y`; not the `v` flag, nor Annex
B's legacy octal escapes and `\c` in classes, which fail to compile. Two
approximations, which no upstream pattern reaches: with both `u` and `i`
characters fold with the `Canonicalize` of `i` without `u` (JS uses
Unicode simple case folding there), and a lookbehind is matched forwards
from each earlier position, so it matches when JS's does but may capture a
different substring inside it. `split` leaves out a separator's groups
that did not participate, which JS splices in as `undefined`. Swift's port
runs the same patterns on `NSRegularExpression` (`JSRegExp.swift`).

## Documented exceptions

Cases that match JS only under one of these are counted apart in the
report.

- **Unpaired surrogates** (inherited from Swift, `swift/DESIGN.md`, "The
  Tier 1 exception"). A JS string can hold half of a surrogate pair; a Rust
  `String` cannot, so `JSON.parse` of a lone `\uD83D` gives U+FFFD. Lengths
  and UTF-16 offsets still agree with JS (U+FFFD is one unit); only the text
  differs. The vectors keep offsets off pair interiors, so no case relies on
  this yet.

- **Unicode version of `\p{L}` and `\p{N}`** (the `autolink` feature).
  `regex` 1.13's tables are Unicode 16.0; Node 24's, which recorded the
  vectors, are Unicode 17.0. The 4,657 letters and numbers Unicode 17 added
  (among them U+088F, U+0C5C, U+0CDC, U+A7CE–U+A7CF, U+A7D2, U+A7D4,
  U+A7F1, Sidetic U+10940–U+10959, Tolong Siki U+11DB0–U+11DE9, Beria Erfe
  U+16EA0–U+16ED3, Tai Yo U+1E6C0–U+1E6FF and CJK Extension J
  U+323B0–U+33479) are part of a URL in JS and not in Rust. No vector uses
  them, so no case is counted under this exception.

Swift's other Tier 1 deviation, integer properties (`model/integer-properties`),
does not apply: Rust keeps those properties as the `f64` JS stores.
