# Parity with JS Lexical

What the Rust port (`lexical-editor`) has ported from the upstream packages,
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
| Core nodes | `nodes/LexicalRootNode.ts`, `LexicalParagraphNode.ts`, `LexicalTextNode.ts`, `LexicalTabNode.ts`, `LexicalLineBreakNode.ts`, `LexicalElementNode.ts` (fields, `getTextContent`, `shouldSerializeTextStyles`) | `nodes::*` | `serialization/core`, `property/documents` |
| Node methods | `LexicalNode.ts` (`getLatest`, `getWritable`, `isAttached`, `getIndexWithinParent`, `remove`/`$removeNode`, `replace`, `insertBefore`, `insertAfter`, `select*`, `markDirty`) | `node`, `utils` | `scripts/updates`, `scripts/transforms` |
| Element methods | `LexicalElementNode.ts` (`splice` with selection repair, `append`, `clear`, `select`, children and descendants, the setters), `RootNode.splice` and its other overrides | `nodes::element`, `node` | `scripts/updates` |
| Text methods | `LexicalTextNode.ts` (`splitText`, `mergeWithSibling`, `spliceText`, `setTextContent`, `select`, the format, detail, mode and style setters and toggles), `toggleTextFormatType`, `TabNode`'s overrides | `nodes::text` | `scripts/updates`, `scripts/listeners`, `scripts/transforms` |
| Tree surgery | `caret/LexicalCaretTree.ts` (`$insertNodeBetween`, `$detachNode`, `$detachSiblingRange`, `$detachNodeWithSelection`, `$insertSibling`, `$selectAdjacentNode`) | `caret_tree` | `scripts/updates` |
| Selection model | `LexicalSelection.ts`: points, `RangeSelection` and `NodeSelection` fields, `$getSelection`, `$setSelection`, `$createRangeSelection`, and the helpers node methods use to keep a selection valid | `selection` | `scripts/updates` |
| Selection reading | `LexicalSelection.ts`: `Point.isBefore`, `isBackward` across nodes, `RangeSelection.getNodes` (`$getNodesFromCaretRangeCompat`) and `getTextContent`, `NodeSelection.getNodes` and `getTextContent`, the `_cachedNodes` cache | `selection` | `scripts/caret`; a unit test against JS's output |
| Carets | `caret/LexicalCaret.ts`: sibling, child and text point carets in both directions and their methods (`getAdjacentCaret`, `getNodeAtCaret`, `getFlipped`, `getParentCaret`, `getChildCaret`, `insert`, `remove`, `replaceOrInsert`, `splice`, iteration), `$getSiblingCaret`, `$getChildCaret`, `$getTextPointCaret`, `$getTextNodeOffset`, text slices, caret ranges (`$getCaretRange`, `iterNodeCarets`, `getTextSlices`, `isCollapsed`), `$getAdjacentChildCaret`, `$getCaretInDirection`, `$extendCaretToRange`, `$comparePointCaretNext`, `$getCommonAncestor`, `makeStepwiseIterator` | `caret` | `scripts/caret` |
| Caret utilities | `caret/LexicalCaretUtils.ts`: `$caretFromPoint`, `$setPointFromCaret`, `$caretRangeFromSelection`, `$setSelectionFromCaretRange`, `$updateRangeSelectionFromCaretRange`, `$rewindSiblingCaret`, `$removeTextFromCaretRange` (block merges, empty-container repair), `$normalizeCaret`, `$isExtendableTextPointCaret`, `$getCaretRangeInDirection`, `$getChildCaretAtIndex`, `$getAdjacentSiblingOrParentSiblingCaret`, `$getTextPointCaretSliceForNode`; with `$getNodeByKeyOrThrow`, `$removeFromParent`, `$restoreEmptyContainerParagraph`, `$isShadowRootNode` and `INTERNAL_$isBlock` from `LexicalUtils.ts` | `caret_utils`, `utils` | `scripts/caret` |
| Selection editing | `LexicalSelection.ts`: `insertText` (`$insertText`, `$insertTextAtPoint`), `insertRawText` (`tokenizeRawText`, `$generateNodesFromRawText`), `insertNodes` for both selection kinds (`$removeTextAndSplitBlock`, `$splitNodeAtPoint`, `$wrapInlineNodes`), `insertParagraph`, `insertLineBreak`, `removeText`, `formatText` (`$updateTextFormat`, `$setTextFormat`, `$getCharacterOffsets`), `toggleFormat`, `setFormat`, `setStyle`, `deleteCharacter`, `deleteWord`, `deleteLine` and `modify` (`forwardDeletion`, `$extendSelectionForDeletion`, `$modifySelectionAroundDecoratorsAndBlocks`, `$collapseAtStart`, `$removeSegment`, `$updateCaretSelectionForUnicodeCharacter`, `$stopLineDeletionAtLineBreak`, `$shrinkSelectionToRoot`, and the model side of `applyDOMRange`), with an editor's `SelectionMovement` standing in for `Selection.modify`, `$insertNodes`, `$selectAll`; `$splitTextPointCaretSlice` and `$isBlockFullySelected`; `ParagraphNode.insertNewAfter`, `canInsertTextBefore`/`After` | `selection_editing`, `selection_insert_text`, `selection_insert_nodes`, `selection_format`, `selection_deletion` | `scripts/selection`, `fuzz/core`, `property/sequences`; unit tests against JS's output for paths no vector covers |
| Update cycle | `LexicalUpdates.ts` (`$beginUpdate`, nested and queued updates, `$applyAllTransforms`, `$commitPendingUpdates`, `$discardPendingUpdate`, listeners, `triggerCommandListeners`, `updateEditorSync`), `LexicalNormalization.ts` (`$normalizeTextNode`), `LexicalGC.ts` (nodes) | `updates`, `normalization`, `gc` | `scripts/updates`, `scripts/listeners`, `scripts/transforms`, `scripts/commands` |
| Editor state | `LexicalEditorState.ts` (`toJSON`, `read`, `isEmpty`, `_parsed`) | `editor_state` | `serialization/core` |
| Parsing | `parseEditorState`, `$parseSerializedNode` (`LexicalUpdates.ts`) | `updates` | `serialization/core`, `property/documents` |
| NodeState | `LexicalNodeState.ts`: `createState` (`StateConfig` with `parse`, `unparse`, `isEqual`, `defaultValue` and `resetOnCopyNode`), `$getState` (latest and direct), `$setState` with a value or an updater, `$getStateChange`, `$getWritableNodeState`, `NodeState` (`getValue`, `toJSON`, `getWritable` with `parseAndPruneNextUnknownState`, `updateFromKnown`, `updateFromUnknown`, `updateFromJSON`, `resetOnCopyNode`, the size count), `$getSharedNodeState` and `createSharedNodeState` (a class's `stateConfigs`, flat keys, key collisions), `$updateStateFromJSON` (`Object.entries` of a `"$"` that is not an object, too), `$applyFlatStates`, `$cloneNodeState`, `$copyNodeState`, `$carryNodeState`, `nodeStatesAreEquivalent`; their call sites: the `"$"` key and flat states in `exportJSON` and `$parseSerializedNode`, `$copyNode`, `splitText`, the merge of `$normalizeTextNode`, history's JSON, a segmented node made plain text (`$insertText`, `$removeTextFromCaretRange`); `QuoteNode`'s `quoteShadowRootState` as a flat state | `node_state`, `node`, `nodes::text`, `normalization`, `updates`, `rich_text`, `caret_utils`, `selection_insert_text` | `scripts/node-state`, the node state case of `serialization/core` |
| DecoratorNode | `nodes/LexicalDecoratorNode.ts` (`isInline`, `isIsolated`, `isKeyboardSelectable`, `decorate` as a hook returning an app's value, `$isDecoratorNode`) as `LexicalNode` hooks (`is_decorator`, `decorate`, `is_isolated`, `is_keyboard_selectable`); where the model treats decorators: `INTERNAL_$isBlock`, `$isLeafNode` (`formatList.ts`), `$isInlineElementOrDecoratorNode`, `$needsBlockCursorBeside`, `RootNode.splice` and `errorOnInsertTextNodeOnRoot`, `RangeSelection.getTextContent`, `insertNodes` (CASE 2's `notInline`, `$extractInlineFromBlocks`, `$isInlineRunNode`), `deleteCharacter`'s decorator branch (an isolated decorator stays; one beside an empty block is selected as a node, else removed), `$modifySelectionAroundDecoratorsAndBlocks` (stepping over, stopping at an isolated one, a node selection of a keyboard-selectable one); `$isInlineFormattable` and `InlineFormattableNode` (`is_inline_formattable`, `get_inline_format`, `set_inline_format`) in `$updateTextFormat` | `node`, `lexical_utils`, `caret_tree`, `nodes::element`, `lexical_selection`, `selection_insert_nodes`, `selection_deletion`, `selection_format`, `list` | `scripts/decorator` |
| Editor | `createEditor` (node registration), `update`, `read`, `getEditorState`, `setEditorState`, `registerNodeTransform`, `registerUpdateListener`, `registerTextContentListener`, `registerCommand`, `dispatchCommand`, `createCommand` | `editor` | `scripts/*` |
| History (the `history` feature) | `packages/lexical-history/src/index.ts`: `registerHistory` (with `dateNow`, `onHistoryStateChange` and `maxDepth`), `createEmptyHistoryState`, `createMergeActionGetter`, `getChangeType`, `getDirtyNodes`, `isTextNodeUnchanged`, `areNodesUnchanged`, `getInverseEntry`, `getUndoEntry`, `undo`, `redo`, `clearHistory`; the commands it handles and dispatches from `LexicalCommands.ts` (`UNDO_COMMAND`, `REDO_COMMAND`, `CAN_UNDO_COMMAND`, `CAN_REDO_COMMAND`, `CLEAR_HISTORY_COMMAND`, `CLEAR_EDITOR_COMMAND`) and its tags from `LexicalUpdateTags.ts` | `history`, `commands`, `editor` | `scripts/history`, `property/sequences`; unit tests for a history shared by two editors (upstream's #8623 test) and `maxDepth` |
| Rich text nodes (the `rich-text` feature) | `packages/lexical-rich-text/src/index.ts`: `HeadingNode` (`getTag`, `setTag`, `insertNewAfter`, `collapseAtStart`, `extractWithChild`), `QuoteNode` (`insertNewAfter`, `collapseAtStart`, `canMergeWhenEmpty`, the opt-in shadow root), `$createHeadingNode`, `$isHeadingNode`, `$createQuoteNode`, `$isQuoteNode`; their generated JSON (`LexicalRichTextGeneratedJSON.ts`); `ElementNode.collapseAtStart` as a node hook, and `ElementNode`'s own `$transform` (`$normalizeShadowRootChildren`); from `@lexical/selection` and `@lexical/utils` only `$isAtEdgeOfElement`, `$isAtStartOfNode`, `$isAtEndOfNode`, `$findMatchingParent` and `$handleIndentAndOutdent` | `rich_text`, `selection`, `utils`, `updates` | `scripts/rich-text`, the heading and quote cases of `property/documents` |
| `registerRichText` (the `rich-text` feature) | `packages/lexical-rich-text/src/index.ts` `registerRichText`: the handlers of `DELETE_CHARACTER_COMMAND` (with `NodeSelection.deleteNodes`), `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `FORMAT_TEXT_COMMAND`, `SET_TEXT_FORMAT_COMMAND`, `FORMAT_ELEMENT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND`, `OUTDENT_CONTENT_COMMAND`, `SELECT_ALL_COMMAND`; those commands from `LexicalCommands.ts`; `ElementNode.canIndent` | `rich_text::register_rich_text`, `commands`, `selection_deletion`, `utils` | `scripts/rich-text-commands`, `property/sequences` |
| `registerPlainText` (the `plain-text` feature) | `packages/lexical-plain-text/src/index.ts` `registerPlainText`: the handlers of `DELETE_CHARACTER_COMMAND`, `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND` (a line break) and `SELECT_ALL_COMMAND` | `plain_text` | `scripts/plain-text` |
| List nodes (the `list` feature) | `packages/lexical-list/src/LexicalListNode.ts` (`ListNode`: `getListType`, `setListType`, `getTag`, `getStart`, `setStart`, `canBeEmpty`, `canIndent`, `splice`, its `$transform`), `LexicalListItemNode.ts` (`ListItemNode`: `append`, `replace`, `insertAfter`, `remove`, `resetOnCopyNodeFrom`, `insertNewAfter`, `collapseAtStart`, `getValue`, `setValue`, `getChecked`, `setChecked`, `toggleChecked`, `getIndent`, `setIndent`, `canMergeWith`, `isParentRequired`, `createParentElementNode`, `canMergeWhenEmpty`, its `$transform`), `$createListNode`, `$isListNode`, `$createListItemNode`, `$isListItemNode`; their generated JSON (`LexicalListGeneratedJSON.ts`); `utils.ts`; the `LexicalNode` hooks they override (`append`, `splice`, `replace`, `insertAfter`, `remove`, `getIndent`, `setIndent`, `resetOnCopyNodeFrom`, a class's own `$transform`); from the core `$copyNode`, `getPreviousSiblings`, `getNextSiblings`, `$splitTextPointCaret`, `$splitAtPointCaretNext`, `$insertNodeToNearestRootAtCaret`; from `@lexical/utils` `$getNearestNodeOfType` | `list`, `lexical_utils`, `caret_utils`, `node`, `nodes::element`, `utils` | `serialization/list`, the list cases of `property/documents`, `scripts/list` |
| `formatList.ts`, `registerList` and `registerCheckList` (the `list` feature) | `formatList.ts` (`$insertList`, `$createListOrMerge`, `mergeLists`, `$removeList`, `updateChildrenListItemValue`, `mergeNextSiblingListIfSameType`, `$handleIndent`, `$handleOutdent`, `$outdentFromItemWithContent`, `$handleListInsertParagraph`); `registerList.ts` `registerList` (`INSERT_ORDERED_LIST_COMMAND`, `INSERT_UNORDERED_LIST_COMMAND`, `REMOVE_LIST_COMMAND`, `UPDATE_LIST_START_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, and the `ListItemNode` and `TextNode` transforms), with `RegisterListOptions`; `checkList.ts` `INSERT_CHECK_LIST_COMMAND` and its handler | `list::format_list`, `list::register_list` | `scripts/list`, `property/sequences` |
| Link nodes (the `link` feature) | `packages/lexical-link/src/LexicalLinkNode.ts` (`LinkNode`: `getURL`, `setURL`, `getTarget`, `setTarget`, `getRel`, `setRel`, `getTitle`, `setTitle`, `insertNewAfter`, `canInsertTextBefore`, `canInsertTextAfter`, `canBeEmpty`, `isInline`, `extractWithChild`, `isEmailURI`, `isWebSiteURI`, `shouldMergeAdjacentLink`; `AutoLinkNode`: `getIsUnlinked`, `setIsUnlinked`, its `shouldMergeAdjacentLink`), `$createLinkNode`, `$isLinkNode`, `$createAutoLinkNode`, `$isAutoLinkNode`, `$linkNodeTransform`, `formatUrl`; their generated JSON (`LexicalLinkGeneratedJSON.ts`, with `afterCloneLinkNode` and `afterCloneAutoLinkNode` as `Clone`); `ElementNode.extractWithChild` as a node hook and `isParentOf`; from the core `RangeSelection.extract` and `NodeSelection.extract` | `link`, `node`, `selection_editing` | `serialization/link`, the link cases of `property/documents`, `scripts/link` |
| `$toggleLink` and `registerLink` (the `link` feature) | `LexicalLinkNode.ts` (`TOGGLE_LINK_COMMAND`, `$toggleLink`, `$withSelectedNodes`, `$getPointNode`, `$splitLinkAtSelection`); `LexicalLinkExtension.ts` `registerLink` (the `LinkNode` transform and the `TOGGLE_LINK_COMMAND` handler, with `validateUrl` and `attributes` from `LinkConfig`) | `link::register_link`, `link::toggle_link` | `scripts/link`, `property/sequences` |
| `registerAutoLink` (the `autolink` feature) | `packages/lexical-link/src/LexicalAutoLinkExtension.ts`: `registerAutoLink` (the `TextNode` transform, `$handleLinkCreation`, `handleLinkEdit`, `handleBadNeighbors`, `$createAutoLinkNode_`, `extractMatchingNodes`, `isContentAroundIsValid`, `isPreviousNodeValid`, `isNextNodeValid`, `startsWithTLD`, the separator checks, `getTextNodesToMatch`, `replaceWithChildren`, and the `TOGGLE_LINK_COMMAND` handler that flips `isUnlinked`), `AutoLinkConfig` and its defaults (`changeHandlers`, `excludeParents`, `matchers`, `separatorRegex`), `ChangeHandler`, `LinkMatcher`, `LinkMatcherResult`, `createLinkMatcherWithRegExp`, `autoLinkUrlMatcher` (with its parenthesis balancing) and `autoLinkEmailMatcher`, on the `regex` crate | `link::register_auto_link` | `scripts/autolink`, `entities/autolink-match` |
| Code nodes (the `code` feature) | `packages/lexical-code-core/src/CodeNode.ts` (`CodeNode`: `getLanguage`, `setLanguage`, `getTheme`, `setTheme`, `getIsSyntaxHighlightSupported`, `setIsSyntaxHighlightSupported`, `insertNewAfter` (the path of an editor without `CodeExtension`), `canIndent`, `collapseAtStart`; `$createCodeNode`, `$isCodeNode`, `DEFAULT_CODE_LANGUAGE`, `getDefaultCodeLanguage`, `$exitCodeNodeOnEnter`), `CodeHighlightNode.ts` (`CodeHighlightNode`: `getHighlightType`, `setHighlightType`, `setFormat` as a no-op, `isParentRequired`, `createParentElementNode`; `$createCodeHighlightNode`, `$isCodeHighlightNode`); their generated JSON (`LexicalCodeCoreGeneratedJSON.ts`, with `afterCloneCodeNode` and `afterCloneCodeHighlightNode` as `Clone`); `TextNode.setFormat` as a node hook; from the core `getTextDirection` and `RTL_REGEX`/`LTR_REGEX` (`LexicalUtils.ts`, `LexicalConstants.ts`) | `code`, `node`, `nodes::text`, `lexical_utils`, `constants` | `serialization/code`, the code cases of `property/documents`, `scripts/code` |
| `FlatStructureUtils.ts` (the `code` feature) | `$getFirstCodeNodeOfLine`, `$getLastCodeNodeOfLine`, `$getCodeLineDirection`, `$getStartOfCodeInLine` (with `findNextNonBlankInLine`), `$getEndOfCodeInLine`, `$plainifyCodeContent`, `$outdentLeadingSpaces` | `code::flat_structure_utils` | `scripts/code`; a unit test against JS's output for each function |
| `CodeExtension`'s register and `registerCodeIndentation` (the `code` feature) | `CodeExtension.ts` `register` (the `TabNode` transform), as `register_code`; `CodeIndentation.ts` `registerCodeIndentation`'s handlers of `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND` and `OUTDENT_CONTENT_COMMAND` (`$isSelectionInCode`, `$getCodeLines`, `$outdentLineAtCaret`, `$handleMultilineIndent`), with `tabSize` | `code::register_code`, `code::register_code_indentation` | `scripts/code` |
| Overflow node (the `overflow` feature) | `packages/lexical-overflow/src/index.ts` (`OverflowNode`: `insertNewAfter` (its parent's), `excludeFromCopy`, the `$transform` of its `$config` that removes it once empty; `$createOverflowNode`, `$isOverflowNode`), with `ElementNode`'s generated JSON; `ElementNode.excludeFromCopy` as a node hook | `overflow`, `node` | `serialization/overflow`, the overflow cases of `property/documents`, `scripts/overflow` |
| Hashtag node and matcher (the `hashtag` feature) | `packages/lexical-hashtag/src/LexicalHashtagNode.ts` (`HashtagNode`: `canInsertTextBefore`, `isTextEntity`; `$createHashtagNode`, `$isHashtagNode`), with `TextNode`'s generated JSON; `LexicalHashtagExtension.ts` (`getHashtagRegexStringChars`, `getHashtagRegexString` and `getHashtagMatch` as the default matcher, `HashtagConfig`, `registerLexicalHashtag`), the character classes as range tables generated from the JS source by `rust/scripts/generate-hashtag-tables.mjs`; `TextNode.isTextEntity` as a node hook | `hashtag`, `node` | `entities/hashtag-match`, `entities/hashtag-documents`, `entities/hashtag-typing`, the hashtag cases of `property/documents` |
| `@lexical/extension`'s decorators (the `extension` feature) | `packages/lexical-extension/src/HorizontalRuleExtension.ts` (`HorizontalRuleNode`: a block decorator whose text is `\n`; `$createHorizontalRuleNode`, `$isHorizontalRuleNode`), `DecoratorTextExtension.ts` (`DecoratorTextNode`: inline, `__isInlineFormattable`, `getFormat`, `hasFormat`, `setFormat`, `toggleFormat`, its `format` as a flat NodeState; `$isDecoratorTextNode`), with `LexicalNode`'s generated JSON | `extension` | `scripts/decorator` |
| Extension core (the `extension` feature) | `packages/lexical/src/extension-core/`: `defineExtension.ts` (`defineExtension`, `configExtension`, `declarePeerDependency`), `safeCast.ts`, `shallowMergeConfig.ts`, `types.ts` (`LexicalExtension` with `name`, `conflictsWith`, `dependencies`, `peerDependencies`, `config`, `mergeConfig`, `nodes`, `$initialEditorState`, `init`, `build`, `register`, `afterRegistration`; `AnyLexicalExtension`, `AnyLexicalExtensionArgument`, `NormalizedPeerDependency`, `ExtensionConfigBase`, `ExtensionInitState`, `ExtensionBuildState`, `ExtensionRegisterState`, `LexicalExtensionDependency`, `InitialEditorConfig`, `InitialEditorStateType`, `LexicalEditorWithDispose`); `AbortController`/`AbortSignal` as `getSignal` uses them; `utils/mergeRegister.ts` | `extension_core` (re-exported at the crate root), `merge_register` | `scripts/extensions` |
| Extension builder (the `extension` feature) | `packages/lexical-extension/src/LexicalBuilder.ts` (`buildEditorFromExtensions`, `LexicalBuilder`: `fromExtensions`, `maybeFromEditor`, `fromEditor`, `constructEditor`, `buildEditor` with `dispose`, `hasExtensionByName`, `getExtensionRep`, `addEdge`, `addExtension` with the duplicate-name and conflict invariants, `sortedExtensionReps`' depth-first sort, cycle invariant and config order, `registerEditor`, `buildCreateEditorArgs` for nodes and `$initialEditorState`), `internal/ExtensionRep.ts` (the state ids, `mergeConfigs`, `init`, `build`, `register` with its rollback, `afterRegistration`, `getSignal`, `getInitResult`, `getInitPeer`, `getExtensionInitDependency`, `getPeer`, `getInitDependency`, `getDependency`, `getDirectDependentNames`, `getPeerNameSet`, `getExtensionDependency`), `config.ts` (`getKnownTypesAndNodes`, `getNodeConfig`), `getExtensionDependencyFromEditor.ts`, `getPeerDependencyFromEditor.ts` (with `OrThrow`), `getExtensionDependency.ts` (`$getExtensionDependency`, `$getExtensionOutput`, `$getPeerDependency`) | `extension::lexical_builder`, `extension::extension_rep`, `extension::config`, `extension::get_*` | `scripts/extensions` |
| Signals (the `extension` feature) | `signals.ts`, a port of `@preact/signals-core` 1.14.4 (`signal` with `watched`/`unwatched`, `computed`, `effect` with clean-ups, `batch` with its snapshots, `untracked`, `peek`, `subscribe`); `namedSignals.ts`; `watchedSignal.ts` | `extension::signals`, `extension::named_signals`, `extension::watched_signal` | `scripts/extensions` (`namedSignals toggle a registration`, `a signal disabled by config`, `EditorStateExtension as a signal`) |
| Model-level extensions (the `extension` feature) | `InitialStateExtension.ts` (always first; its config and every form of `$initialEditorState`), `ClearEditorExtension.ts` (`registerClearEditor`, `$defaultOnClear`, `$onClear` as a signal), `EditorStateExtension.ts`, `DecoratorTextExtension.ts`'s extension | `extension::initial_state_extension`, `extension::clear_editor_extension`, `extension::editor_state_extension`, `extension::decorator_text_extension` | `scripts/extensions` |
| `NormalizeInlineElementsExtension`'s register (the `extension` feature) | `packages/lexical-extension/src/NormalizeInlineElementsExtension.ts` (`deleteEmptyInline`, and the `register` that adds it to the transforms of every registered `ElementNode` class with an `isInline` of its own, without marking their nodes dirty), as `register_normalize_inline_elements`; `RichTextExtension` depends on it | `extension` | `property/sequences` |
| `registerLexicalTextEntity` (the `text` feature) | `packages/lexical-text/src/registerLexicalTextEntity.ts`: `EntityMatch`, `registerLexicalTextEntity` (the `TextNode` transform and `$reverseNodeTransform`, `$replaceWithSimpleText`, `$wasTurnedIntoText`, `getFirstMatchAfter`, `isNextTextEntity`, `isTextAfterOutsideSource`, `isEntityAfter`, `$checkNextSibling`, `$moveSelectionIntoEntity`, `$wouldMerge`, `MAX_MATCHES_PASSED`, `MAX_TURNED_INTO_TEXT`, the per-update `markedInUpdate` and `turnedIntoTextInUpdate`); from the core `INTERNAL_getActiveEditorState` | `text`, `updates` | `entities/hashtag-documents`, `entities/hashtag-typing`, `property/sequences` |
| `@lexical/text`'s root helpers (the `text` feature) | `packages/lexical-text/src/rootTextContent.ts` (`$rootTextContent`), `isRootTextContentEmpty.ts` (`$isRootTextContentEmpty`, with `String#trim`'s whitespace, and `$isRootTextContentEmptyCurry`), `canShowPlaceholder.ts` (`$canShowPlaceholder`, `$canShowPlaceholderCurry`), `findTextIntersectionFromCharacters.ts` (`$findTextIntersectionFromCharacters`, deprecated upstream, with UTF-16 counts), `index.ts`'s `TextNodeWithOffset` | `text` | `scripts/text` |
| `@lexical/file` (the `file` feature) | `packages/lexical-file/src/fileImportExport.ts`: `SerializedDocument`, `serializedDocumentFromEditorState` (an empty `source` is `"Lexical"`, a `lastSaved` of 0 or NaN is `Date.now()`, the version is the upstream release), `editorStateFromSerializedDocument` (from a string or a document, either JSON form), `importFile` and `exportFile` (with `toISOString` for the default file name), with the app reading and saving the file where JS's `readTextFileFromSystem` and `exportBlob` use the DOM | `file::file_import_export` | `file/export`, `file/import`; unit tests for `toISOString` against Node and an export-then-import round trip |
| `@lexical/devtools-core`'s tree view (the `devtools` feature) | `packages/lexical-devtools-core/src/generateContent.ts`: `generateContent` (the text form), `$printRangeSelection`, `printNodeSelection`, `$visitTree`, `normalize` (with `obfuscateText`), `printNode` (text, link with `link`, paragraph), `printTextFormatProperties`, `printAllTextNodeProperties`, `printAllLinkNodeProperties`, `printDetailProperties`, `printModeProperties`, `printFormatProperties`, `printTargetProperties`, `printRelProperties`, `printTitleProperties`, `printStateProperties`, `$printSelectedCharsLine`, `$getSelectionStartEnd`, `CustomPrintNodeFn`, `LexicalCommandLog`'s entries; from the core `LexicalNode.isSelected` (`$isKeyInSelectionNodes` and the inline decorator rule), `isBefore`, `LexicalEditor.version` (the port's own, `lexical-rust@0.53.0`, as Swift prints `lexical-swift@...`), `_config.namespace` (`createUID` when none is given), `isEditable` and `setEditable` | `devtools`, `node`, `editor`, `lexical_utils` | `devtools/tree` (keys mapped to the port's and the version line to the port's, as Swift's runner does) |
| `$setBlocksType` (the `selection` feature) | `packages/lexical-selection/src/range-selection.ts` (`$setBlocksType`, with its default `$afterCreateElement`, `$copyBlockFormatIndent`, and `$isPointAtBlockEdge`), through `LexicalNode.replace` and `ListItemNode.replace` | `selection` | `scripts/set-blocks-type`, the block-decorator case of `scripts/decorator`, `property/sequences` |
| UTF-16 offsets | `String#length`, `String#slice` | `utf16` | through the text methods; unit tests against Node |
| Upstream version | `meta.json` | `UPSTREAM_VERSION` | `meta` |

## Not ported yet

Everything else; [DESIGN.md](DESIGN.md) has the order. Within what M0
touches:

- **Slots** (`"$slots"`): a document that carries them fails to parse with
  an error naming them, rather than losing them.
- Of `DecoratorNode`: `editor._decorators`, `getDecorators()` and
  `registerDecoratorListener`, which only JS's DOM reconciler fills (it
  calls `decorate` for each decorator it renders; a headless JS editor
  skips the reconciler, so its map stays empty and the listener never
  fires); with them `$garbageCollectDetachedDecorators`. They come with the
  DOM; an app's renderer can call `decorate` itself. The parts of
  `$extendSelectionForDeletion` and `applyDOMRange` that read a decorator's
  DOM (a line boundary landing in its private text, a DOM range inside
  one) have no model counterpart. `isSelected()`, with its rule for an
  inline decorator at the end of its parent, is ported (for the tree view,
  the `devtools` feature), though no vector reaches that rule;
  `getTopLevelElement` is not, nor are slots on decorators (`__slotHost`, `__slots`). `@lexical/extension`'s
  `HorizontalRuleExtension` (`INSERT_HORIZONTAL_RULE_COMMAND`, clicks, the
  selected class) isn't ported (see the extension API below), and
  `applyFormatFromStyle` reads CSS.
- Of the extension API (the `extension` feature): `InitialEditorConfig`'s
  `namespace`, `theme` (and `deepThemeMergeInPlace`), `html`, `editable`,
  `onError`, `onWarn`, `parentEditor`, `disableEvents` and `dom`, which a
  headless Rust editor doesn't have; node replacements (`{replace, with}`)
  and their conflict invariant, as `createEditor` takes classes only;
  `buildEditor`'s `setRootElement(null)` and `Symbol.dispose` (dropping a
  `LexicalEditorWithDispose` doesn't dispose it); `builderSymbol`'s version
  and copy checks in `maybeFromEditor`, which one crate can't break; the
  `nodes` thunk form (`nodes` is a list); `HMRExtension`. Configs are Rust
  types that name their overrides' type (`ExtensionConfigBase::Partial`);
  a JSON `Object` merges as JS does. Signal values compare with `==`
  (`SignalValue`; an `EditorState` by identity) where JS's `!==` compares
  objects by identity, and effect and computed bodies can't throw, so
  preact's error paths have no counterpart and a cycle panics. `named_signals`
  takes a JSON object; a typed config makes its signals field by field.
  Every other extension of `@lexical/extension` (`AutoFocus`, `BlockId`,
  `ClickAfterLastBlock`, `HorizontalRule`, `IME`, `KeyboardShortcuts`,
  `NestedEditor`, `NodeSelection`, `NodeSelectionDataSelected`,
  `NodeStructure`, `NormalizeInlineElements`, `NormalizeTripleClickSelection`,
  `PreventSelectAll`, `RootElement`, `SelectBlock`,
  `SelectionAlwaysOnDisplay`, `TabIndentation`, `WatchEditable`) reads the
  DOM, a selection's DOM, keyboard events or slots, or waits on pieces that
  aren't ported (`HorizontalRuleExtension` on `NodeSelectionExtension`), and
  the extension forms of the other packages (`RichTextExtension`,
  `PlainTextExtension`, `HistoryExtension`, `ListExtension`,
  `LinkExtension`, `HashtagExtension`, `CodeExtension`, `OverflowExtension`)
  aren't ported yet: their `register_*` functions are what they register.
- Of NodeState: the `SerializationSchema` builders as `parse` functions
  (with them `canonicalValue`'s `arraySetValue` form and a config's
  `derived` and `schema`), which come with the schema facility;
  `$getStateChange`'s use in `updateDOM`, which comes with the DOM; and the
  check that a flat key is not an `Object.prototype` property, which has no
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
  `$selectAll(selection)`'s scoping isn't ported, nor
  `insertNodes`' CASE 3a (blocks with no block ancestor, which needs
  `$insertNodeToNearestRootAtCaret`).
- `@lexical/list`'s DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
  `importDOM`, the theme class names, `ListImportExtension`/`ListImportRules`
  and `WordListImportExtension`), which come with `html`; `ListExtension`
  and `CheckListExtension`, which aren't ported as extensions yet
  (`ListExtension`'s defaults are what `register_list` with `RegisterListOptions::default()`
  does); `registerListStrictIndentTransform` (`hasStrictIndent`, off by
  default); `registerList`'s `KEY_BACKSPACE_COMMAND` handler and
  `registerCheckList`'s pointer, focus, arrow, Space and Escape handlers,
  which need an input layer (`deleteCharacter` reaches the same
  `ListItemNode.collapseAtStart`, and a toggle is `toggle_checked`). What
  `@lexical/list` does where `NodeStructureExtension` enforces node
  structure (`INTERNAL_$afterStructureMerge`, `INTERNAL_$afterStructureSplit`,
  `INTERNAL_$liftsFromStructure`, `$pointsAround`, `$keptTops`,
  `$listTarget`, the `onlyChild` seam merge) and for lists in named slots,
  as neither is ported. `ListNode` reads its tag off its list type rather
  than keeping `__tag` apart: JS sets both together everywhere. Backspace
  at the start of a document's first item, with nothing before it, needs
  `Selection.modify` in headless JS, which throws, as `scripts/list`
  records.
- `@lexical/link`'s DOM parts (`createDOM`, `updateDOM`, `updateLinkDOM`,
  `exportDOM`, `importDOM`, `$convertAnchorElement`, `LinkImportRules` and
  `LinkImportExtension`), which come with `html`, and `sanitizeUrl`, which
  only `updateLinkDOM` calls and which needs the WHATWG URL parser
  (`new URL`); `LinkExtension`, which isn't ported yet
  (`register_link` with `LinkConfig::default()` is what its defaults do,
  and the config is a value where JS reads signals); `registerLink`'s
  `PASTE_COMMAND` handler (registered only with `validateUrl`), which reads
  a `ClipboardEvent`'s data; `AutoLinkExtension`, which isn't ported yet
  (`register_auto_link` with `AutoLinkConfig::default()` and the matchers is what it does with them; its `mergeConfig`, which concatenates
  configured arrays, is the caller's to do);
  `ClickableLinkExtension` and `AutoLinkAnnounceExtension`, which need a
  DOM. What `NodeStructureExtension` does for links where it enforces node
  structure (lifting blocks, merging equal adjacent links, which
  `$linkNodeTransform` then leaves to it) isn't ported, so the transform
  always does both. `extractWithChild` is a node hook that nothing calls
  yet: its caller is `@lexical/html`'s copy. `HeadingNode`'s override is
  ported; `ParagraphNode`'s, which reads the selection, comes with `html`. In a headless editor,
  `LinkNode.canInsertTextBefore` and `canInsertTextAfter` change nothing a
  vector can see: `insertText` at the edge of an inline parent's text puts
  the text outside the parent either way, and
  `$resolveSelectionPointOnBoundary`, which also reads them, runs only
  after `Selection.modify`.
- `@lexical/code-core`'s DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
  `importDOM`, `$convertPreElement` and the other converters,
  `CodeImportRules`/`CodeImportExtension` and the VS Code paste overlay),
  which come with `html`, and `registerCodeGutter`, which writes to the
  DOM; `CodeExtension` and `CodeIndentExtension`, which aren't ported as
  extensions yet (`register_code` and `register_code_indentation(editor,
  None)` are what their defaults register); `CodeExtension`'s `KEY_ENTER_COMMAND`
  handler and `registerCodeIndentation`'s handlers of `KEY_TAB_COMMAND`,
  the arrow keys (`$handleTab`, `$handleShiftLines`, `escapeWithArrows`),
  `MOVE_TO_START` and `MOVE_TO_END` (`$handleMoveTo`), which read a
  `KeyboardEvent` and wait for an input layer. Without `CodeExtension`,
  `CodeNode.insertNewAfter` always takes the path JS takes for an editor
  without `CodeExtension` (exiting the block after two line breaks itself,
  as `scripts/code` records with `createEditor`), less the deprecation
  warning. `property/sequences`' editor has `CodeExtension`, so there JS's
  `INSERT_PARAGRAPH_COMMAND` adds a line even after two line breaks; no
  case of it gets there (one would fail). JS's `CodeHighlightNode`
  constructor keeps a `null` highlight type it is given; nothing but a
  direct `new` makes one, and Rust has none. Highlighting (`@lexical/code-prism`, `@lexical/code-shiki`) is
  other packages: no vector needs a tokenizer, since every recorded
  document carries its `highlightType`s, and `scripts/code` registers no
  highlighter. Of the 25 `property/documents` cases with code nodes, 4
  also hold hashtag or overflow nodes; they waited on those packages,
  which are now ported.
- `@lexical/overflow`'s `createDOM` and `updateDOM`, which come with
  `html`, and `OverflowExtension`, which isn't ported yet
  (`overflow_nodes()` is its `nodes`). `excludeFromCopy` is a node hook
  that nothing calls yet: its callers are `@lexical/clipboard`'s and
  `@lexical/html`'s copies. `@lexical/react`'s character limit
  (`useCharacterLimit.ts`: `$wrapOverflowedNodes`, `findOffset`,
  `$wrapNode`, `$unwrapNode`, `$mergePrevious`), which is what makes
  overflow nodes, isn't ported: no vector runs it (every vector's overflow
  nodes are in its documents or built by a script), and it lives in a
  React package. Of the 36 `property/documents` cases that waited on
  `lexical-overflow`, 11 also hold hashtag nodes and waited on
  `lexical-hashtag`, now ported. That suite now sets each parsed state on the editor
  and reads the text there, as JS does: a parsed state is all dirty, so
  the transforms run, and `OverflowNode`'s removes the empty ones a random
  document holds.
- `@lexical/hashtag`'s `createDOM`, which comes with `html`, and
  `HashtagExtension`, which isn't ported yet (`hashtag_nodes()` is
  its `nodes` and `register_lexical_hashtag(editor,
  HashtagConfig::default())` its `register`). Within `registerLexicalTextEntity`,
  `$carryNodeState`, `splitOffInUpdate`, `demotedInUpdate` and
  `$isInsertedText` are ported (`scripts/node-state`'s hashtag cases), but
  the NodeState half of `$wouldMerge` (`haveEquivalentNodeState` with
  `readStateValue`) is not, so it compares format, style and detail: only
  a token or segmented entity kept as one reaches it, and `createNode`
  makes a hashtag in normal mode (a hashtag given token or segmented mode
  in a document is replaced by a new one before that check, as JS
  records), so no vector can. The text edit log isn't ported, so
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
- Of `@lexical/devtools-core`: `generateContent`'s `exportDOM` form
  (`$generateHtmlFromNodes`, `printPrettyHTML`, `prettifyHTML`), which
  needs `@lexical/html` and fails with an error until it is ported; its
  mark and table branches (`$isMarkNode`, `$isTableSelection`,
  `printTableSelection`), as `@lexical/mark` and `@lexical/table` aren't
  ported (a tree view holding their nodes would be `NotPorted` in the
  runner; no `devtools/tree` case holds one); slots (`$getSlotNames`,
  `$getSlot`, `$resolveSlotContext`, `$slotValueMetaSuffix`), which the
  core doesn't have; the `compositionKey` line, as a headless editor never
  composes; and the React parts, `TreeView.tsx` and
  `useLexicalCommandsLog.ts` (a log of commands is the caller's, as
  `LexicalCommandLogEntry`s with the payload already printed). The editable
  listeners `setEditable` triggers in JS (`registerEditableListener`)
  aren't ported. `obfuscateText`'s `*` and the `^` widths count UTF-16
  code units, as JS does; no vector obfuscates.
- In `$setBlocksType`, the skipping of a block that is a named slot's value
  (`$getSlotHost(prevNode) !== null`), as slots aren't ported. Its branch for a
  declared block container (`$isBlockContainer`) is ported but unreachable
  until `NodeStructureExtension`'s declared containers are: without them,
  only the root and shadow roots are containers, and neither is a block.
- The slot-aware caret comparisons (`$comparePointCaretNextAcrossSlots`,
  `$getCommonAncestorAcrossSlots`). Without slots or the text edit log,
  `$removeTextFromCaretRange` never meets a slot host and records no
  deletions.
- `HistoryExtension` and `SharedHistoryExtension` (`@lexical/history`), which
  aren't ported as extensions yet. Sharing one `HistoryState` between
  editors through `register_history` works. `register_history` takes the delay and
  `maxDepth` as values, where JS also takes signals, and `maxDepth` as a
  `usize`.
- `registerRichText`'s handlers of DOM events (click, the arrow keys,
  Backspace, Delete, Enter, Escape, Space and Tab keys, `MOVE_TO_START` and
  `MOVE_TO_END`, copy, cut, paste, drop and drag), which wait for an input
  layer, with its `escapeFormatTriggers` and `shouldHandlePasteAsFiles`;
  `CONTROLLED_TEXT_INSERTION_COMMAND`'s `beforeinput` form; and
  `SELECT_ALL_COMMAND`'s scoping to a slot (slots aren't ported). The DOM
  parts of `@lexical/rich-text` (`createDOM`, `exportDOM`, `importDOM`)
  come with `html`.
- `registerPlainText`'s handlers of DOM events (the arrow keys, Backspace,
  Delete, Enter, copy, cut, paste, drop and drag) and `PlainTextExtension`
  with its dependencies (`DragonExtension`,
  `NormalizeInlineElementsExtension`, whose transform
  `register_normalize_inline_elements` registers,
  `NormalizeTripleClickSelectionExtension`), which need an input layer or
  aren't ported as extensions yet.
  `quoteShadowRootState` is a flat NodeState, as in JS
  (`quote_shadow_root_state`; `scripts/node-state` sets and reads it). A
  shadow root quote's `collapseAtStart` (dissolving into
  its blocks) is ported but no vector reaches it: headless JS needs the
  DOM's `Selection.modify` to get there and throws, which `scripts/rich-text`
  records.
- Composition, the DOM reconciler, mutation listeners, selection-change
  notifications and `$onUpdate` callbacks (none of which a headless editor
  runs, except `$onUpdate`), and `RootNode.__cachedText`. So `history` passes
  `false` where JS passes `editor.isComposing()`, as a headless JS editor
  does; the `composition-start` and `composition-end` tags work as in JS.

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
