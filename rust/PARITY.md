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
| `registerRichText` (the `rich-text` feature) | `packages/lexical-rich-text/src/index.ts` `registerRichText`: the handlers of `DELETE_CHARACTER_COMMAND` (with `NodeSelection.deleteNodes`), `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `FORMAT_TEXT_COMMAND`, `SET_TEXT_FORMAT_COMMAND`, `FORMAT_ELEMENT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND`, `OUTDENT_CONTENT_COMMAND`, `SELECT_ALL_COMMAND`, and `KEY_ENTER_COMMAND` (with `$escapeFormatsForTrigger` on `DEFAULT_ESCAPE_FORMAT_TRIGGERS`); those commands from `LexicalCommands.ts`, with a `KeyboardEvent` of the modifier keys and `preventDefault`; `ElementNode.canIndent` | `rich_text::register_rich_text`, `commands`, `selection_deletion`, `utils` | `scripts/rich-text-commands`, `property/sequences` |
| `registerPlainText` (the `plain-text` feature) | `packages/lexical-plain-text/src/index.ts` `registerPlainText`: the handlers of `DELETE_CHARACTER_COMMAND`, `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND` (a line break), `KEY_ENTER_COMMAND` and `SELECT_ALL_COMMAND` | `plain_text` | `scripts/plain-text` |
| List nodes (the `list` feature) | `packages/lexical-list/src/LexicalListNode.ts` (`ListNode`: `getListType`, `setListType`, `getTag`, `getStart`, `setStart`, `canBeEmpty`, `canIndent`, `splice`, its `$transform`), `LexicalListItemNode.ts` (`ListItemNode`: `append`, `replace`, `insertAfter`, `remove`, `resetOnCopyNodeFrom`, `insertNewAfter`, `collapseAtStart`, `getValue`, `setValue`, `getChecked`, `setChecked`, `toggleChecked`, `getIndent`, `setIndent`, `canMergeWith`, `isParentRequired`, `createParentElementNode`, `canMergeWhenEmpty`, its `$transform`), `$createListNode`, `$isListNode`, `$createListItemNode`, `$isListItemNode`; their generated JSON (`LexicalListGeneratedJSON.ts`); `utils.ts`; the `LexicalNode` hooks they override (`append`, `splice`, `replace`, `insertAfter`, `remove`, `getIndent`, `setIndent`, `resetOnCopyNodeFrom`, a class's own `$transform`); from the core `$copyNode`, `getPreviousSiblings`, `getNextSiblings`, `$splitTextPointCaret`, `$splitAtPointCaretNext`, `$insertNodeToNearestRootAtCaret`; from `@lexical/utils` `$getNearestNodeOfType` | `list`, `lexical_utils`, `caret_utils`, `node`, `nodes::element`, `utils` | `serialization/list`, the list cases of `property/documents`, `scripts/list` |
| `formatList.ts`, `registerList` and `registerCheckList` (the `list` feature) | `formatList.ts` (`$insertList`, `$createListOrMerge`, `mergeLists`, `$removeList`, `updateChildrenListItemValue`, `mergeNextSiblingListIfSameType`, `$handleIndent`, `$handleOutdent`, `$outdentFromItemWithContent`, `$handleListInsertParagraph`); `registerList.ts` `registerList` (`INSERT_ORDERED_LIST_COMMAND`, `INSERT_UNORDERED_LIST_COMMAND`, `REMOVE_LIST_COMMAND`, `UPDATE_LIST_START_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, and the `ListItemNode` and `TextNode` transforms), with `RegisterListOptions`; `checkList.ts` `INSERT_CHECK_LIST_COMMAND` and its handler | `list::format_list`, `list::register_list` | `scripts/list`, `property/sequences` |
| Link nodes (the `link` feature) | `packages/lexical-link/src/LexicalLinkNode.ts` (`LinkNode`: `getURL`, `setURL`, `getTarget`, `setTarget`, `getRel`, `setRel`, `getTitle`, `setTitle`, `insertNewAfter`, `canInsertTextBefore`, `canInsertTextAfter`, `canBeEmpty`, `isInline`, `extractWithChild`, `isEmailURI`, `isWebSiteURI`, `shouldMergeAdjacentLink`; `AutoLinkNode`: `getIsUnlinked`, `setIsUnlinked`, its `shouldMergeAdjacentLink`), `$createLinkNode`, `$isLinkNode`, `$createAutoLinkNode`, `$isAutoLinkNode`, `$linkNodeTransform`, `formatUrl`; their generated JSON (`LexicalLinkGeneratedJSON.ts`, with `afterCloneLinkNode` and `afterCloneAutoLinkNode` as `Clone`); `ElementNode.extractWithChild` as a node hook and `isParentOf`; from the core `RangeSelection.extract` and `NodeSelection.extract` | `link`, `node`, `selection_editing` | `serialization/link`, the link cases of `property/documents`, `scripts/link` |
| `$toggleLink` and `registerLink` (the `link` feature) | `LexicalLinkNode.ts` (`TOGGLE_LINK_COMMAND`, `$toggleLink`, `$withSelectedNodes`, `$getPointNode`, `$splitLinkAtSelection`); `LexicalLinkExtension.ts` `registerLink` (the `LinkNode` transform and the `TOGGLE_LINK_COMMAND` handler, with `validateUrl` and `attributes` from `LinkConfig`) | `link::register_link`, `link::toggle_link` | `scripts/link`, `property/sequences` |
| `registerAutoLink` (the `autolink` feature) | `packages/lexical-link/src/LexicalAutoLinkExtension.ts`: `registerAutoLink` (the `TextNode` transform, `$handleLinkCreation`, `handleLinkEdit`, `handleBadNeighbors`, `$createAutoLinkNode_`, `extractMatchingNodes`, `isContentAroundIsValid`, `isPreviousNodeValid`, `isNextNodeValid`, `startsWithTLD`, the separator checks, `getTextNodesToMatch`, `replaceWithChildren`, and the `TOGGLE_LINK_COMMAND` handler that flips `isUnlinked`), `AutoLinkConfig` and its defaults (`changeHandlers`, `excludeParents`, `matchers`, `separatorRegex`), `ChangeHandler`, `LinkMatcher`, `LinkMatcherResult`, `createLinkMatcherWithRegExp`, `autoLinkUrlMatcher` (with its parenthesis balancing) and `autoLinkEmailMatcher`, on the `regex` crate | `link::register_auto_link` | `scripts/autolink`, `entities/autolink-match` |
| Code nodes (the `code` feature) | `packages/lexical-code-core/src/CodeNode.ts` (`CodeNode`: `getLanguage`, `setLanguage`, `getTheme`, `setTheme`, `getIsSyntaxHighlightSupported`, `setIsSyntaxHighlightSupported`, `insertNewAfter` (the path of an editor without `CodeExtension`), `canIndent`, `collapseAtStart`; `$createCodeNode`, `$isCodeNode`, `DEFAULT_CODE_LANGUAGE`, `getDefaultCodeLanguage`, `$exitCodeNodeOnEnter`), `CodeHighlightNode.ts` (`CodeHighlightNode`: `getHighlightType`, `setHighlightType`, `setFormat` as a no-op, `isParentRequired`, `createParentElementNode`; `$createCodeHighlightNode`, `$isCodeHighlightNode`); their generated JSON (`LexicalCodeCoreGeneratedJSON.ts`, with `afterCloneCodeNode` and `afterCloneCodeHighlightNode` as `Clone`); `TextNode.setFormat` as a node hook; from the core `getTextDirection` and `RTL_REGEX`/`LTR_REGEX` (`LexicalUtils.ts`, `LexicalConstants.ts`) | `code`, `node`, `nodes::text`, `lexical_utils`, `constants` | `serialization/code`, the code cases of `property/documents`, `scripts/code` |
| `FlatStructureUtils.ts` (the `code` feature) | `$getFirstCodeNodeOfLine`, `$getLastCodeNodeOfLine`, `$getCodeLineDirection`, `$getStartOfCodeInLine` (with `findNextNonBlankInLine`), `$getEndOfCodeInLine`, `$plainifyCodeContent`, `$outdentLeadingSpaces` | `code::flat_structure_utils` | `scripts/code`; a unit test against JS's output for each function |
| `CodeExtension`'s register and `registerCodeIndentation` (the `code` feature) | `CodeExtension.ts` `register` (the `TabNode` transform and the `KEY_ENTER_COMMAND` handler, `$exitCodeNodeOnEnter`), as `register_code`; `CodeIndentation.ts` `registerCodeIndentation`'s handlers of `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND` and `OUTDENT_CONTENT_COMMAND` (`$isSelectionInCode`, `$getCodeLines`, `$outdentLineAtCaret`, `$handleMultilineIndent`), with `tabSize` | `code::register_code`, `code::register_code_indentation` | `scripts/code` |
| Code highlighting (the `code-highlight` feature) | `packages/lexical-code-prism/src/CodeHighlighterPrism.ts`: `registerCodeHighlighting` (with `registerCodeIndentation`) and `registerHighlightingOnly`, without `registerCodeGutter`; `$codeNodeTransform` (`defaultLanguage`, `isSyntaxHighlightSupported` from the tokenizer's languages, the `nodesCurrentlyHighlighting` guard cleared by `$onUpdate`), `$textNodeTransform` with `$copyNodeState`, `$updateAndRetainSelection`, `getDiffRange`, `isEqual`, the `Tokenizer` interface (`defaultLanguage`, `tokenize`, `$tokenize`) with Prism's `Token` shape; `FacadePrism.ts`'s `$mapTokensToLexicalStructure` and `$getHighlightNodes` without the `diff-<language>` grammars; `CodeHighlighterShiki.ts`'s lazy languages (`loadCodeLanguage`'s update tagged `history-merge`), as Swift's `CodeLanguageState`; with `extension`, `CodePrismExtension` (`disabled`, `tokenizer`) as Swift's `CodeHighlighterExtension` | `code::register_code_highlighting`, `code::register_highlighting_only`, `code::Tokenizer`, `code::map_tokens_to_lexical_structure`, `code::code_highlighter_extension` | `scripts/code-highlight`, over `ops.ts`'s conformance tokenizer and over tokens recorded from Prism |
| Mark node (the `mark` feature) | `packages/lexical-mark/src/MarkNode.ts` (`MarkNode`: `hasID`, `getIDs`, `setIDs`, `addID`, `deleteID`, `insertNewAfter`, `canInsertTextBefore`, `canInsertTextAfter`, `canBeEmpty`, `isInline`, `extractWithChild`, `excludeFromCopy`; `$createMarkNode`, `$isMarkNode`), its generated JSON (`LexicalMarkGeneratedJSON.ts`, with `afterCloneMarkNode` as `Clone`); `index.ts` (`$unwrapMarkNode`, `$wrapSelectionInMarkNode` with `createNode`, `$getMarkIDs`) | `mark` | `serialization/mark`, `scripts/mark` |
| Mark ids as NodeState (the `mark` feature) | `packages/lexical-mark/src/MarkStateExtension.ts`, the parts that need no extension, for `MarkStateExtension`'s ids with the default `markableNodes`: `MARK_STATE_KEY`, `isValidMarkID`, the `markIds` state (`createMarkIDsState`, `arraySetValue(markIDValue())`'s parse, and `$setState`'s `canonicalValue` for it), `$isInlineDecoratorNode`, `$isMarkableNode`, `$getNodeMarkIDs`, `$addMarkID`, `$removeMarkID` (by selection, or walking the document), `$getMarkIDsAtSelection` (with `$getPointMarkIDs` and `$getAdjacentLeaf`), `$migrateMarkNodesToMarkState`, and their helpers (`$readNodeMarkIDs`, `$getMarkableIDs`, `$addNodeMarkIDs`, `$removeNodeMarkIDs`, `assertValidMarkID`, `$markableNodesInSelection`, `$collectMarkableNodes`, `$markableNodesIn`) | `mark::mark_state_extension` | `scripts/mark`, `serialization/mark` |
| Overflow node (the `overflow` feature) | `packages/lexical-overflow/src/index.ts` (`OverflowNode`: `insertNewAfter` (its parent's), `excludeFromCopy`, the `$transform` of its `$config` that removes it once empty; `$createOverflowNode`, `$isOverflowNode`), with `ElementNode`'s generated JSON; `ElementNode.excludeFromCopy` as a node hook | `overflow`, `node` | `serialization/overflow`, the overflow cases of `property/documents`, `scripts/overflow` |
| The character limit (the `overflow` feature) | `packages/lexical-react/src/shared/useCharacterLimit.ts`, its logic without React's hook plumbing: `useCharacterLimit`'s effects as `register_character_limit` (the `OverflowNode` invariant, the count at registration, the text content and update listeners, `history-merge` updates, the low-priority `DELETE_CHARACTER_COMMAND` handler; `strlen` and `remainingCharacters`), `findOffset` (its `Intl.Segmenter` path), `$wrapOverflowedNodes` (with `$dfsWithSlots`, the `\n\n` between blocks and slot value leaves), `$wrapNode`, `$mergePrevious`; `Intl.Segmenter`'s extended grapheme clusters as UAX #29's rules on tables that `rust/scripts/generate-grapheme-tables.mjs` makes from the Unicode data of Node's ICU (Unicode 17.0.0), checked against every line of GraphemeBreakTest.txt by `Intl.Segmenter` and by a unit test | `overflow::character_limit`, `overflow::grapheme`, `overflow::grapheme_tables` | `scripts/character-limit`; a unit test of GraphemeBreakTest.txt |
| Table of contents (the `rich-text` feature) | `packages/lexical-react/src/LexicalTableOfContentsPlugin.tsx`, its logic without React: `TableOfContentsEntry` (key, text, tag), `toEntry`, `$insertHeadingIntoTableOfContents`, `$deleteHeadingFromTableOfContents`, `$updateHeadingInTableOfContents`, `$updateHeadingPosition`, `$getPreviousHeading`, and `TableOfContentsPlugin`'s effect as `register_table_of_contents` (the initial entries, the root update listener, the `HeadingNode` and `TextNode` mutation listeners with `skipInitialization`), reporting each `setTableOfContents` to a closure, on the core's root element and mutation listeners (the root element and mutation listeners row) | `rich_text::table_of_contents` | `scripts/table-of-contents` |
| Hashtag node and matcher (the `hashtag` feature) | `packages/lexical-hashtag/src/LexicalHashtagNode.ts` (`HashtagNode`: `canInsertTextBefore`, `isTextEntity`; `$createHashtagNode`, `$isHashtagNode`), with `TextNode`'s generated JSON; `LexicalHashtagExtension.ts` (`getHashtagRegexStringChars`, `getHashtagRegexString` and `getHashtagMatch` as the default matcher, `HashtagConfig`, `registerLexicalHashtag`), the character classes as range tables generated from the JS source by `rust/scripts/generate-hashtag-tables.mjs`; `TextNode.isTextEntity` as a node hook | `hashtag`, `node` | `entities/hashtag-match`, `entities/hashtag-documents`, `entities/hashtag-typing`, the hashtag cases of `property/documents` |
| `@lexical/extension`'s decorators (the `extension` feature) | `packages/lexical-extension/src/HorizontalRuleExtension.ts` (`HorizontalRuleNode`: a block decorator whose text is `\n`; `$createHorizontalRuleNode`, `$isHorizontalRuleNode`), `DecoratorTextExtension.ts` (`DecoratorTextNode`: inline, `__isInlineFormattable`, `getFormat`, `hasFormat`, `setFormat`, `toggleFormat`, its `format` as a flat NodeState; `$isDecoratorTextNode`), with `LexicalNode`'s generated JSON | `extension` | `scripts/decorator` |
| Extension core (the `extension` feature) | `packages/lexical/src/extension-core/`: `defineExtension.ts` (`defineExtension`, `configExtension`, `declarePeerDependency`), `safeCast.ts`, `shallowMergeConfig.ts`, `types.ts` (`LexicalExtension` with `name`, `conflictsWith`, `dependencies`, `peerDependencies`, `config`, `mergeConfig`, `nodes`, `$initialEditorState`, `init`, `build`, `register`, `afterRegistration`; `AnyLexicalExtension`, `AnyLexicalExtensionArgument`, `NormalizedPeerDependency`, `ExtensionConfigBase`, `ExtensionInitState`, `ExtensionBuildState`, `ExtensionRegisterState`, `LexicalExtensionDependency`, `InitialEditorConfig`, `InitialEditorStateType`, `LexicalEditorWithDispose`); `AbortController`/`AbortSignal` as `getSignal` uses them; `utils/mergeRegister.ts` | `extension_core` (re-exported at the crate root), `merge_register` | `scripts/extensions` |
| Extension builder (the `extension` feature) | `packages/lexical-extension/src/LexicalBuilder.ts` (`buildEditorFromExtensions`, `LexicalBuilder`: `fromExtensions`, `maybeFromEditor`, `fromEditor`, `constructEditor`, `buildEditor` with `dispose`, `hasExtensionByName`, `getExtensionRep`, `addEdge`, `addExtension` with the duplicate-name and conflict invariants, `sortedExtensionReps`' depth-first sort, cycle invariant and config order, `registerEditor`, `buildCreateEditorArgs` for nodes and `$initialEditorState`), `internal/ExtensionRep.ts` (the state ids, `mergeConfigs`, `init`, `build`, `register` with its rollback, `afterRegistration`, `getSignal`, `getInitResult`, `getInitPeer`, `getExtensionInitDependency`, `getPeer`, `getInitDependency`, `getDependency`, `getDirectDependentNames`, `getPeerNameSet`, `getExtensionDependency`), `config.ts` (`getKnownTypesAndNodes`, `getNodeConfig`), `getExtensionDependencyFromEditor.ts`, `getPeerDependencyFromEditor.ts` (with `OrThrow`), `getExtensionDependency.ts` (`$getExtensionDependency`, `$getExtensionOutput`, `$getPeerDependency`) | `extension::lexical_builder`, `extension::extension_rep`, `extension::config`, `extension::get_*` | `scripts/extensions` |
| Signals (the `extension` feature) | `signals.ts`, a port of `@preact/signals-core` 1.14.4 (`signal` with `watched`/`unwatched`, `computed`, `effect` with clean-ups, `batch` with its snapshots, `untracked`, `peek`, `subscribe`); `namedSignals.ts`; `watchedSignal.ts` | `extension::signals`, `extension::named_signals`, `extension::watched_signal` | `scripts/extensions` (`namedSignals toggle a registration`, `a signal disabled by config`, `EditorStateExtension as a signal`) |
| Model-level extensions (the `extension` feature) | `InitialStateExtension.ts` (always first; its config and every form of `$initialEditorState`), `ClearEditorExtension.ts` (`registerClearEditor`, `$defaultOnClear`, `$onClear` as a signal), `EditorStateExtension.ts`, `DecoratorTextExtension.ts`'s extension | `extension::initial_state_extension`, `extension::clear_editor_extension`, `extension::editor_state_extension`, `extension::decorator_text_extension` | `scripts/extensions` |
| `NormalizeInlineElementsExtension` (the `extension` feature) | `packages/lexical-extension/src/NormalizeInlineElementsExtension.ts` (`deleteEmptyInline`, and the `register` that adds it to the transforms of every registered `ElementNode` class with an `isInline` of its own, without marking their nodes dirty, as `register_normalize_inline_elements`; the extension with its `disabled` signal); `RichTextExtension` and `PlainTextExtension` depend on it | `extension` | `property/sequences`, `scripts/package-extensions` |
| `@lexical/extension`'s model-level extensions (the `extension` feature) | `TabIndentationExtension.ts` (`registerTabIndentation`'s `INDENT_CONTENT_COMMAND` handler with `$indentOverTab`, `canIndentPredicate` and `maxIndent`, as `register_tab_indentation`; the extension with its `disabled`, `maxIndent` and `$canIndent` signals); `SelectBlockExtension.ts` (the `SELECT_ALL_COMMAND` handler that selects the block, then the document, with `disabled` and `cascadeSelection`); `PreventSelectAllExtension.ts`'s config and `disabled` signal, which `SelectBlockExtension`'s drives | `extension::tab_indentation_extension`, `extension::select_block_extension`, `extension::prevent_select_all_extension` | `scripts/package-extensions` |
| The packages' extensions (each behind its package's feature and `extension`) | `LexicalRichTextExtension.ts` (`RichTextExtension`: its nodes, `conflictsWith`, the `NormalizeInlineElementsExtension` dependency, `registerRichText`); `PlainTextExtension` (`@lexical/plain-text`); `@lexical/history`'s `HistoryExtension` (`HistoryConfig` with `delay`, `createInitialHistoryState`, `disabled`, `maxDepth` and `now`, the init's `canUndo`/`canRedo` signals, the output's config signals, `registerHistory` reading `delay` and `maxDepth` as signals) and `SharedHistoryExtension` (its own `HistoryExtension`, disabled, takes the `delay`, `historyState`, `now`, `maxDepth` and `disabled` of the parent editor's peer `HistoryExtension`); `LexicalListExtension.ts` (`ListExtension` with `hasStrictIndent`, which registers `registerListStrictIndentTransform`, and `shouldPreserveNumbering`; `CheckListExtension`'s `registerCheckList`); `LexicalLinkExtension.ts` (`LinkExtension` with its `mergeConfig` of `attributes`, and `registerLink` reading `validateUrl` and `attributes` as signals); `LexicalAutoLinkExtension.ts` (`AutoLinkExtension` with its concatenating `mergeConfig`); `LexicalHashtagExtension.ts` (`HashtagExtension`); `OverflowExtension`; `CodeExtension.ts` (`CodeExtension`, and `CodeNode.insertNewAfter`'s path for an editor built with it) and `CodeIndentExtension` (`tabSize`, `disabled`); `Editor` equality by identity, as JS's `===` | `rich_text::rich_text_extension`, `plain_text::plain_text_extension`, `history::history_extension`, `history::shared_history_extension`, `list::list_extension`, `list::check_list_extension`, `link::link_extension`, `link::auto_link_extension`, `hashtag::hashtag_extension`, `overflow::overflow_extension`, `code::code_extension`, `code::code_indent_extension` | `scripts/package-extensions`; `property/sequences` and the hashtag `entities` suites build their editors with them |
| `registerLexicalTextEntity` (the `text` feature) | `packages/lexical-text/src/registerLexicalTextEntity.ts`: `EntityMatch`, `registerLexicalTextEntity` (the `TextNode` transform and `$reverseNodeTransform`, `$replaceWithSimpleText`, `$wasTurnedIntoText`, `getFirstMatchAfter`, `isNextTextEntity`, `isTextAfterOutsideSource`, `isEntityAfter`, `$checkNextSibling`, `$moveSelectionIntoEntity`, `$wouldMerge`, `MAX_MATCHES_PASSED`, `MAX_TURNED_INTO_TEXT`, the per-update `markedInUpdate` and `turnedIntoTextInUpdate`); from the core `INTERNAL_getActiveEditorState` | `text`, `updates` | `entities/hashtag-documents`, `entities/hashtag-typing`, `property/sequences` |
| `@lexical/text`'s root helpers (the `text` feature) | `packages/lexical-text/src/rootTextContent.ts` (`$rootTextContent`), `isRootTextContentEmpty.ts` (`$isRootTextContentEmpty`, with `String#trim`'s whitespace, and `$isRootTextContentEmptyCurry`), `canShowPlaceholder.ts` (`$canShowPlaceholder`, `$canShowPlaceholderCurry`), `findTextIntersectionFromCharacters.ts` (`$findTextIntersectionFromCharacters`, deprecated upstream, with UTF-16 counts), `index.ts`'s `TextNodeWithOffset` | `text` | `scripts/text` |
| `@lexical/file` (the `file` feature) | `packages/lexical-file/src/fileImportExport.ts`: `SerializedDocument`, `serializedDocumentFromEditorState` (an empty `source` is `"Lexical"`, a `lastSaved` of 0 or NaN is `Date.now()`, the version is the upstream release), `editorStateFromSerializedDocument` (from a string or a document, either JSON form), `importFile` and `exportFile` (with `toISOString` for the default file name), with the app reading and saving the file where JS's `readTextFileFromSystem` and `exportBlob` use the DOM | `file::file_import_export` | `file/export`, `file/import`; unit tests for `toISOString` against Node and an export-then-import round trip |
| `@lexical/devtools-core`'s tree view (the `devtools` feature) | `packages/lexical-devtools-core/src/generateContent.ts`: `generateContent` (the text form), `$printRangeSelection`, `printNodeSelection`, `$visitTree`, `normalize` (with `obfuscateText`), `printNode` (text, link with `link`, paragraph), `printTextFormatProperties`, `printAllTextNodeProperties`, `printAllLinkNodeProperties`, `printDetailProperties`, `printModeProperties`, `printFormatProperties`, `printTargetProperties`, `printRelProperties`, `printTitleProperties`, `printStateProperties`, `$printSelectedCharsLine`, `$getSelectionStartEnd`, `CustomPrintNodeFn`, `LexicalCommandLog`'s entries; from the core `LexicalNode.isSelected` (`$isKeyInSelectionNodes` and the inline decorator rule), `isBefore`, `LexicalEditor.version` (the port's own, `lexical-rust@0.53.0`, as Swift prints `lexical-swift@...`), `_config.namespace` (`createUID` when none is given), `isEditable` and `setEditable` | `devtools`, `node`, `editor`, `lexical_utils` | `devtools/tree` (keys mapped to the port's and the version line to the port's, as Swift's runner does) |
| `$setBlocksType` (the `selection` feature) | `packages/lexical-selection/src/range-selection.ts` (`$setBlocksType`, with its default `$afterCreateElement`, `$copyBlockFormatIndent`, and `$isPointAtBlockEdge`), through `LexicalNode.replace` and `ListItemNode.replace` | `selection` | `scripts/set-blocks-type`, the block-decorator case of `scripts/decorator`, `property/sequences` |
| The DOM (the `html` feature) | what a browser or jsdom gives JS: `Node` (`nodeType`, `nodeName`, `parentNode`, `childNodes`, siblings, `textContent`, `insertBefore`, `appendChild`, `append`, `prepend`, `removeChild`, `replaceChild`, `remove`, `replaceWith`, `replaceChildren`, `contains`, `cloneNode`), `Element` (attributes in order, `id`, `dir`, `className`, `classList`, `style`, `innerHTML`, `outerHTML`, `getElementsByTagName`), `Text`, `Comment`, `DocumentFragment`, `Document` (`createElement` and the other factories, `head`, `body`), `DOMTokenList`, `CSSStyleDeclaration` (`setProperty`, `getPropertyValue`, `getPropertyPriority`, `removeProperty`, `cssText`, `item`) with jsdom's normalization, `DOMParser.parseFromString`, the HTML fragment serialization algorithm; ported from the Swift port's `DOM/` | `dom` | `html/parse`, `html/entities`, `css/properties`, `css/values` |
| Core DOM hooks (the `html` feature) | `LexicalNode.createDOM`, `exportDOM` and the static `importDOM` as node hooks (`create_dom`, `export_dom`, `import_dom`), `ElementNode.exportDOM` (indent as `padding-inline-start` and `data-lexical-indent`, `dir`); `DOMExportOutput`, `DOMConversionOutput`, `DOMChildConversion`, `DOMConversion`, `DOMConversionFn`, `DOMConversionMap`; `createEditor`'s `theme` and `html` (`export` overrides, `import` conversions), `editor._config`, `initializeConversionCache`; `DEFAULT_EDITOR_DOM_CONFIG` (`$createDOM`, `$exportDOM`, `$extractWithChild`, `$shouldExclude`, `$shouldInclude`); `LexicalNode.isSelected`; `TextNode` (`createDOM` with `getElementOuterTag`, `getElementInnerTag`, `$createTextInnerDOM` and `setTextThemeClassNames`; `exportDOM`; `importDOM` with `$convertTextDOMNode`, `findParentPreDOMNode`, `findTextInLine`, `convertSpanElement`, `convertBringAttentionToElement`, `convertTextFormatElement`, `applyTextFormatFromStyle`), `TabNode.createDOM`, `LineBreakNode` (`createDOM`, `importDOM`), `ParagraphNode` (`createDOM`, `exportDOM`, `importDOM` with `$convertParagraphElement`, `extractWithChild`), `ArtificialNode__DO_NOT_USE`; from `LexicalUtils.ts` `isInlineDomNode`, `isBlockDomNode`, `getCachedClassNameArray`, `removeEmptyDOMAttribute`, `setNodeIndentFromDOM`, `$setDirectionFromDOM`, `$setFormatFromDOM`, `isOnlyChildInBlockNode`, `isLastChildInBlockNode`; `utils/classNames.ts` | `lexical_dom`, `node`, `nodes::*`, `editor` | `html/export`, `html/import` |
| The node packages' DOM parts (the `html` feature with each package's) | `createDOM`, `exportDOM` and `importDOM` of `HeadingNode` (with the Google Docs title), `QuoteNode` (`$convertHeadingElement`, `$convertBlockquoteElement`), `ListNode` (`$setListThemeClassNames`, `$convertListNode`, `$normalizeChildren`, `isDomChecklist`, `HTMLOListElement.start`), `ListItemNode` (`updateListItemDOM` with `updateListItemChecked`, `$setListItemThemeClassNames` and `applyMarkerStyles`, the nested item's `after`, `$convertListItemElement`, `$convertCheckboxInput`, `setFormatFromChildren`), `LinkNode` (`updateLinkDOM` for a new element, `sanitizeUrl`, `$convertAnchorElement`), `AutoLinkNode`, `CodeNode` (`$convertPreElement`, `$convertDivElement`, the GitHub table converters), `CodeHighlightNode`, `OverflowNode`, `HashtagNode`, `HorizontalRuleNode` and `DecoratorTextNode`, with `applyFormatFromStyle` and `applyFormatToDom` | `rich_text`, `list`, `link`, `code`, `overflow`, `hashtag`, `extension` | the HTML of `property/documents`, `html/import-nodes` |
| `@lexical/html` (the `html` feature) | `packages/lexical-html/src/index.ts`: `$generateHtmlFromNodes`, `$generateDOMFromNodes`, `$generateDOMFromRoot`, `$appendNodeToHTML` (`$appendNodesToHTML` with `$appendTerminatingLineBreak`, and `@lexical/selection`'s `$sliceSelectedTextNodeContent` for a selection), `$generateNodesFromDOM` (`IGNORE_TAGS`, `getConversionFunction`, `$createNodesFromDOM`, `wrapContinuousInlines`, `$unwrapArtificialNodes`, `isDomNodeBetweenTwoInlineNodes`) | `html` | `html/export`, `html/import` |
| `DOMImportExtension` (the `html` and `extension` features) | `packages/lexical-html/src/import/`: `DOMImportExtension.ts` (`DOMImportConfig` and its `mergeConfig`, `$runPreprocessStack`, `DefaultHoistRule`, `$generateNodesFromDOMViaExtension`), `CoreImportExtension.ts`, `HorizontalRuleImportExtension.ts`, `coreImportRules.ts` (the inline format, `#text` with `collapseWhitespace` and `findAdjacentTextOnLine`, script and style, `<br>`, `<p>`, `<hr>` and transparent block rules, `isAlignmentValue`), `defineImportRule.ts`, `compileImportRules.ts`, `defineOverlayRules.ts`, `ImportContext.ts` (`createImportState`, `ImportSource`, `ImportTextFormat`, `ImportTextStyle`, `ImportWhitespaceConfig` with `defaultPreservesWhitespace` and `defaultIsInline`, `ImportOverlays`, `ImportSessionImpl`, `$getImportContextValue`, `$withImportContext`), `runImport.ts`, `schemas.ts` (`$isBlockLevel`, `$distributeInlineWrapper`, `$applySchema`, `$propagateTextAlignToBlockChildren`, `BlockSchema`, `InlineSchema`, `NestedBlockSchema`, `RootSchema`), `sel.ts` (with `RegExp` matchers as functions), `parseCss.ts`, `inlineStylesFromStyleSheets.ts`; `internal/ContextRecord.ts`'s import half; `CSSStyleSheet.cssRules` and `querySelectorAll` in the DOM | `html::import`, `html::internal`, `dom` | `html/import-extension`; unit tests of the style sheet parser and the selectors against jsdom |
| `DOMRenderExtension` (the `html` and `extension` features) | `packages/lexical-html/src/internal/`: `DOMRenderExtension.ts` (its `init`, `build`, `mergeConfig` and the `RootNode` export), `domOverride.ts`, `compileDOMRenderConfigOverrides.ts` (`sortedOverrides`, `getPredicate`, the merge of each export hook), `DOMRenderRuntime.ts` (`createEditorContextRecord`, `filterEditorInstalled`, `setContextValue`, `getSessionConfig` with its cache), `RenderContext.ts` (`createRenderState`, `RenderContextRoot`, `RenderContextExport`, `$getRenderContextValue`, `$setRenderContextValue`, `$updateRenderContextValue`, `$getSessionDOMRenderConfig`, `$withRenderContext`), `types.ts`'s render half, `ContextRecord.ts`'s render half; from the core `EditorDOMRenderConfig`'s export hooks, `$getEditorDOMRenderConfig` and `createEditor`'s `dom`; from `@lexical/extension` `buildCreateEditorArgs`'s `html` merge; `$generateDOMFromNodes` and `$generateDOMFromRoot` with the render context and the session's render config | `html`, `html::internal`, `lexical_dom`, `extension` | `html/export-render` |
| The packages' import rules (the `html` and `extension` features with each package's) | `packages/lexical-rich-text/src/RichTextImportExtension.ts` (`RichTextImportRules`, the heading, Google Docs title and blockquote rules, and the shadow root quote rule), `packages/lexical-list/src/ListImportExtension.ts` (`ListImportRules`, `isDomChecklist`, the list schema with its flatten step), `packages/lexical-list/src/WordListImportExtension.ts` (the `mso-list` paragraph preprocess and rules), `packages/lexical-link/src/LinkImportExtension.ts` (`LinkImportRules`), `packages/lexical-code-core/src/CodeImportExtension.ts` (`CodeImportRules` and the VS Code paste overlay); each package extension depends on `CoreImportExtension` and its rules, as in JS | `rich_text::rich_text_import_extension`, `list::list_import_extension`, `list::word_list_import_extension`, `link::link_import_extension`, `code::code_import_extension` | `html/import-packages` |
| Inline CSS text | `utils/setDOMStyle.ts` (`getStyleObjectFromCSS`, with JS's property order; with the `html` feature `setDOMStyleObject` and `setDOMStyleFromCSS`) | `set_dom_style` | through `css/values`; unit tests against JS's output |
| Clipboard, JSON and plain text (the `clipboard` feature) | `packages/lexical-clipboard/src/clipboard.ts`: `$generateJSONFromSelectedNodes` (`$appendNodesToJSON`: `isSelected`, `excludeFromCopy('clone')`, `extractWithChild`, the text sliced at the selection's edges, empty slices left out, a node selection's element taking its subtree), `$generateNodesFromSerializedNodes`, `$insertGeneratedNodes` (`SELECTION_INSERT_CLIPBOARD_NODES_COMMAND`, `$updateSelectionOnInsert`), `$getLexicalContent`, `$insertDataTransferForPlainText`, `$insertDataTransferForRichText` (the default stack of `ClipboardImportExtension.ts`: `$runImport` with `$defaultLexicalEditorImporter` and `$defaultPlainTextImporter` for `text/plain` and `text/uri-list`, by `DEFAULT_IMPORT_MIME_TYPE_PRIORITY`), `$getClipboardDataFromSelection` (the default `ExportMimeTypeConfig` but `text/html`), `setLexicalClipboardDataTransfer`; `$sliceSelectedTextNodeContent` (`@lexical/selection`, `'clone'`); from the core `LexicalNode.isSelected`, `createEditor`'s `namespace` and `getNamespace` (as the devtools row has them), `insertNodes`' CASE 1 (inside a code block), `PASTE_COMMAND`, `COPY_COMMAND`, `CUT_COMMAND` and `SELECTION_INSERT_CLIPBOARD_NODES_COMMAND`, and `DataTransfer` (`getData`, `setData`, `types`, as Swift's in `LexicalEvents.swift`); the `extractWithChild` of `ParagraphNode`, `ListNode` and `ListItemNode` | `clipboard`, `events`, `selection`, `node`, `editor`, `commands`, `selection_insert_nodes`, `nodes::paragraph`, `list` | `scripts/clipboard` |
| Clipboard, HTML and the extensions (the `clipboard` feature with `html`, `extension` or both) | `packages/lexical-clipboard/src/clipboard.ts`: `$getHtmlContent`, the `text/html` entry of `DEFAULT_EXPORT_MIME_TYPE`, `$getExportConfig`, `callExportMimeTypeFunctionStack`, `$exportMimeTypeFromSelection`, `GetClipboardDataExtension` (`GetClipboardDataConfig`, `ExportMimeTypeFunction`, its `mergeConfig`); `ClipboardImportExtension.ts`: `$defaultHtmlImporter`, `DEFAULT_IMPORT_MIME_TYPE`, `DEFAULT_IMPORT_MIME_TYPE_PRIORITY`, `$callImportMimeTypeFunctionStack` (with `$next`'s data and selection), `orderedMimeTypes`, `$runImport` (with the iOS Safari skip of HTML equal to the plain text), `$getImportOutput`, `ClipboardImportExtension`, `ClipboardDOMImportExtension`; `registerPlainText`'s copy writing `text/html`; `@lexical/html`'s `ImportSourceDataTransfer`; from the core, `insertNodes`' CASE 3a (`$insertNodeToNearestRootAtCaret` for blocks with no block ancestor) | `clipboard`, `plain_text`, `html::import`, `selection_insert_nodes` | `html/clipboard` |
| Copy, cut and paste handlers (the `rich-text` and `plain-text` features) | `registerRichText`'s `COPY_COMMAND` (`copyToClipboard` with an event, `$copyToClipboardEvent`), `CUT_COMMAND` (`onCutForRichText`, with `INTERNAL_$expandSelectionToWholeDocument`) and `PASTE_COMMAND` (`onPasteForRichText`) handlers; `registerPlainText`'s (`onCopyForPlainText`, `onCutForPlainText`, `onPasteForPlainText`), with JS's `paste` and `cut` tags | `rich_text`, `plain_text`, `clipboard` | unit tests (no vector: headless JS has no `ClipboardEvent`) |
| The rest of `@lexical/selection`'s model (the `selection` feature) | `packages/lexical-selection/src/lexical-node.ts` (`$sliceSelectedTextNodeContent`, its `'clone'` mode only, `$isAtNodeEnd`, `$trimTextContentFromAnchor`, `$addNodeStyle` as the deprecated no-op it is, `$patchStyleText` with values, `null` and functions of the previous value, `$forEachSelectedTextNode`, `$ensureForwardRangeSelection`), `range-selection.ts` (`$wrapNodes` with `$wrapNodesImpl`, `$shouldOverrideDefaultCharacterSelection`, `$isParentElementRTL`, `$moveCaretSelection`, `$moveCharacter`, `$getSelectionStyleValueForProperty`), `utils.ts` (`getCSSFromStyleObject`, `getStyleObjectFromRawCSS`), and its re-exports of `getStyleObjectFromCSS`, `$cloneWithProperties` and `$selectAll` (with a selection to scope it, `select_all_in`); from the core `getStyleObjectFromCSS` (`StyleObject`, in a JS object's key order), `$cloneWithProperties`, `$splitNode`, `ElementNode.getDescendantByIndex`, `TextNode.canHaveFormat` | `selection`, `set_dom_style`, `lexical_utils`, `selection_insert_nodes` | `scripts/selection-utils` |
| `@lexical/utils`'s model (the `utils` feature) | `packages/lexical-utils/src/index.ts`: `DFSNode`, `$dfs`, `$reverseDfs`, `$dfsIterator`, `$reverseDfsIterator` (Rust iterators), `$getAdjacentCaret`, `$getNextSiblingOrParentSibling`, `$getDepth`, `$getNextRightPreorderNode`, `$getNearestNodeOfType`, `$getNearestBlockElementAncestorOrThrow`, `registerNestedElementResolver` (by node type), `$restoreEditorState`, `$insertNodeToNearestRoot`, `$insertNodeIntoLeaf`, `$wrapNodeInElement`, `$filter`, `$handleIndentAndOutdent`, `$insertFirst`, `$unwrapAndFilterDescendants`, `$descendantsMatching`, `$firstToLastIterator`, `$lastToFirstIterator`, `$unwrapNode`, `StateConfigWrapper`, `makeStateWrapper`, `$onEscapeUp`, `$onEscapeDown`, `$isAtStartOfNode`, `$isAtEndOfNode`; its re-exports of `$findMatchingParent`, `$getAdjacentSiblingOrParentSiblingCaret`, `$insertNodeToNearestRootAtCaret`, `$isBlockFullySelected`, `$isRootOrShadowRoot`, `$splitNode` and `mergeRegister` (no longer behind the `extension` feature) | `utils`, `lexical_utils`, `caret_utils`, `selection_deletion`, `merge_register` | `scripts/selection-utils` |
| Slots | `LexicalSlot.ts` (`$isSlotHost`, `$isSlotChild`, `$getSlotHost`, `$getSlotNameWithinHost`, `$getSlotFrame`, `$getSelectionSlotFrame`, `$getSlotNames` in canonical order, `$getSlot`, `getDeclaredSlots` as `LexicalNode::declared_slots`, `$setSlot` with its refusals and moves, `$removeSlot`, the cycle check, `_slotsUsed`); their call sites in `LexicalNode.ts` (`__slotHost` and `__slots` on elements and decorators, `remove`, `replace`, `isAttached`, `getTextContent` and its size, `selectNext`/`selectPrevious` through the host), `LexicalElementNode.ts` (`isEmpty`, `getAllTextNodes`, text content with slots first), `LexicalEditorState.ts` and `LexicalUpdates.ts` (`"$slots"` in both JSON forms, written after `children`, and parsed through `$setSlot`), `LexicalGC.ts` (a removed host's slot subtrees), `LexicalSelection.ts` (`$clampRangeSelectionToSlotFrame` in `$setSelection` and at commit, `$getPointSlotFrame`, insertion and deletion stopping at a slot boundary, `$selectAll` scoped to the slot frame), `caret/LexicalCaret.ts` (`$filterByMode` stopping at slotted nodes, `$getCommonAncestorAcrossSlots`, `$comparePointCaretNextAcrossSlots`), `caret/LexicalCaretTree.ts`, `caret/LexicalCaretUtils.ts`; in the packages, `SELECT_ALL_COMMAND`'s scoping (`rich-text`, `plain-text`), `$setBlocksType` and `$wrapNodes` skipping slot values, `SelectBlockExtension`'s slot frames, `@lexical/clipboard`'s copy of a host's `$slots` from the selection's slot frame, `@lexical/mark`'s `$forEachSlot` walks, and `@lexical/utils`' `$dfsWithSlots`, `$reverseDfsWithSlots`, their iterators and `$getDepth` across hosts | `slot`, `node`, `nodes::element`, `editor_state`, `updates`, `gc`, `lexical_selection`, `selection_deletion`, `selection_insert_nodes`, `caret`, `caret_tree`, `caret_utils`, `rich_text`, `plain_text`, `selection`, `extension`, `clipboard`, `mark`, `utils` | `scripts/slots`, the named slots case of `serialization/core` |
| Node structure (`@experimental node-structure`) | `LexicalStructure.ts`: the `$config()` keys `inline`, `children` (`'inline'`, `'block'` or a list of `'inline'` and classes), `mergeAdjacent` (`{by: [...]}` of schema fields and NodeStates, or `false`) and the values declared `derived`, read along the class's config chain (`iterStaticNodeConfigChain`, the nearest declaration winning; a key the class only inherits and contradicts reads as undeclared), `getDeclaredNodeStructure`, `validateNodeStructure` (when an editor is created, with JS's messages), the synthesized `isInline()`, `INTERNAL_$isInlineContent`, `$isBlockContainer`, `INTERNAL_registerNodeStructure` (counted registrations, the dirty marking of every element and decorator, a full normalization on registering), `INTERNAL_isNodeStructureEnforced`, `INTERNAL_$normalizeNodeStructure`, and the normalization of each dirty subtree: lifting blocks out of holders of inline content and splitting them (the anchor saved across), paragraphs joined as lines, inline runs wrapped in a block container's paragraph, a stray child wrapped in a list's first class, empty inline elements removed (not an `OverflowNode`), equal adjacent siblings merged with the seam merged in turn, and the class hooks `INTERNAL_$afterStructureMerge`, `INTERNAL_$afterStructureSplit` and `INTERNAL_$liftsFromStructure`; its call sites in the core (`ElementNode`'s shadow-root transform, `insertNodes`, `$setBlocksType`, `$wrapNodes`, `$insertNodeToNearestRoot`, the `is_inline` and `declares_inline_children` defaults); the packages' declarations (`ParagraphNode`, `HeadingNode`, `QuoteNode`, `CodeNode`, `MarkNode`, `LinkNode`, `AutoLinkNode`'s `mergeAdjacent: false`, `ListNode`, `ListItemNode`) and their structure code (`$structureMerges`, `ListNode`'s merge and split hooks, `ListItemNode`'s lift hook and `$pointsAround`, `formatList.ts`'s `$keptTops`, `$listTarget`, `$isListItemContent` and the `onlyChild` seam merge, `$linkNodeTransform` leaving lifts and merges to the normalization); `NodeStructureExtension.ts` (`registerNodeStructure`, `NodeStructureExtension` with its `disabled` signal) | `structure` (`LexicalNode::static_node_config_chain` and the `internal_after_structure_merge`, `internal_after_structure_split` and `internal_lifts_from_structure` hooks), `NodeClass::get_declared_node_structure`, `create`, `extension::node_structure_extension`, `link`, `list` | `scripts/node-structure` |
| `BlockIdExtension` (the `extension` feature) | `packages/lexical-extension/src/BlockIdExtension.ts`: `blockIdState`, `$getBlockId`, `$getBlockById`, `$getBlockHandle`, `$getBlockHandles`, `$resolveBlockHandle` with `BlockHandleOptions.read`, `BlockHandleResult`, `$isRootChildBlock`, `$defaultIsIdentifiedBlock` (with `LexicalNode::declares_inline_children` for a class whose `$config` declares `children: 'inline'`), `generateBlockId`, `BlockIdConfig` (`disabled`, `$isIdentifiedBlock`, `generateId`, `repairTransforms`), `BlockIdOutput`, and the extension: the index, the transforms of every registered class and the root transform (`$assignBlockIds`: lineage by children and by position, slots included, uniqueness, provisional ids across transform passes), the update listener (`$syncIndex`, `$nameUnnamed`), the repair update, document order (`$compareDocumentOrder`), the handle views; `internal/positionalBlockId.ts` (SHA-256, `canonicalJSON`, `$canonicalDocument`, `$documentKey`, `derivePositionalId`, `deriveDuplicateId`); `internal/blockHandles.ts` (`MIN_BLOCK_HANDLE_LENGTH`, `handleKey`, `buildHandleTable`, `handleInTable`, `idsWithPrefix`); from the core `$hasUpdateTag` and `COLLABORATION_TAG` | `extension::block_id_extension`, `extension::positional_block_id`, `extension::block_handles`, `editor` | `scripts/block-id`; a unit test of SHA-256 against FIPS 180-4's digests |
| Root element and mutation listeners | `LexicalEditor.ts`'s `setRootElement` (its model half: `resetEditor` keeping the pending state and the queue, a full reconcile tagged `history-merge`, then the root listeners), `getRootElement`, `registerRootListener`, `registerMutationListener` (by node type, with `skipInitialization` and `initializeMutationListener`), `registerEditableListener` and `setEditable`'s notification; `LexicalReconciler.ts`'s walk as far as it reports mutations (`$reconcileRoot`, `$reconcileNode`, `$createNode` with a cross-parent move reconciled, `$destroyNode`, `$reconcileChildren`, `$reconcileNodeChildren`), with `setMutatedNode` (a destroy then a create is an update) and each node's `updateDOM` as a model hook (`LexicalNode::update_dom`: text by its tags, `HeadingNode` by tag, `ListNode` by type, `AutoLinkNode` by `isUnlinked`); `triggerMutationListeners` before the text content and update listeners; `$onUpdate` and the deferred callbacks; the root element is the app's (`RootElement`, compared by identity); `RootElementExtension.ts` | `editor`, `reconciler`, `updates`, `node`, `extension::root_element_extension` | `scripts/a11y` and `scripts/auto-embed` (the `mutations` logs) |
| `@lexical/a11y`'s announcements (the `a11y` feature) | `packages/lexical-a11y/src/index.ts`: `AriaPoliteness`, the live region `createLiveRegion` makes (role, `aria-atomic`, `aria-live`, `textContent`) as `LiveRegion`, mounted on the app's `LiveRegionHost` (the configured `owner`, or what the root element names, where JS uses its document's body), `AriaLiveRegionExtension` (`announce` with U+200B on a repeat, its three effects: mounting per root or `owner`, the politeness, the message without replaying it into a new region), `HistoryAnnounceExtension`, `EditorModeAnnounceExtension`; `packages/lexical-rich-text/src/HeadingAnnounceExtension.ts` (with `rich-text`) and `packages/lexical-link/src/AutoLinkAnnounceExtension.ts` (with `link`), `%s` replaced once as JS's `replace` does | `a11y`, `rich_text::heading_announce_extension`, `link::auto_link_announce_extension` | `scripts/a11y` |
| `DecoratorBlockNode` (the `decorator-block` feature) | `packages/lexical-react/src/LexicalDecoratorBlockNode.ts`: `DecoratorBlockNode` as data a node returns from `LexicalNode::decorator_block` (a block decorator, `isInline` and `canIndent` false), `getFormat`, `setFormat`, `decoratorBlockNodeSchema`'s `format` (`enumValue`, left out of the compact form when `''`), `$isDecoratorBlockNode` | `decorator_block`, `node` | `scripts/decorator-block`, `serialization/decorator-block` |
| The typeahead menu's text matching (the `menu` feature) | `packages/lexical-react/src/LexicalTypeaheadMenuPluginUtils.ts` (`PUNCTUATION`, `useBasicTypeaheadTriggerMatch` as `basic_typeahead_trigger_match`, its `RegExp` matched by hand over UTF-16 code units, with the trigger and punctuation read as character class bodies); `LexicalMenuOption.ts` (`MenuOption`); `shared/LexicalMenu.tsx` (`MenuTextMatch`, `MenuResolution` anchored at a model range, `getFullMatchOffset`, `$splitNodeContainingQuery`, `selectOptionAndCleanUp`, the highlighted index with `preselectFirstItem`); `LexicalTypeaheadMenuPlugin.tsx` (`getTextUpToAnchor`, `isSelectionOnEntityBoundary`, the update listener with `tryToPositionRange`'s model half, `closeTypeahead`/`openTypeahead`, the editable listener); `LexicalNodeMenuPlugin.tsx`'s `positionOrCloseMenu` | `menu` | `scripts/menu` |
| Auto-embed (the `auto-embed` feature) | `packages/lexical-react/src/LexicalAutoEmbedPluginUtils.ts` (`EmbedMatchResult`, `EmbedConfig`, `URL_MATCHER` matched by hand as `url_matcher`, `INSERT_EMBED_COMMAND`, `AutoEmbedOption`); `LexicalAutoEmbedPlugin.tsx`'s logic as `register_auto_embed`: the `PASTE_COMMAND` handler at `COMMAND_PRIORITY_BEFORE_EDITOR` with `/^\S+$/` (`is_single_token`) and `$onUpdate`, the `LinkNode` and `AutoLinkNode` mutation listeners, `checkIfLinkNodeIsEmbeddable` (the last config wins, `latestCheck`, `$getCurrentLink`, `$disableMenuFor`), `embedLinkViaActiveEmbedConfig`, `getMenuOptions` and the `INSERT_EMBED_COMMAND` handler, driving a `NodeMenu` | `auto_embed` | `scripts/auto-embed` (`URL_MATCHER`, the single-token test, the command and the mutations a paste makes) |
| Markdown (the `markdown` feature) | `packages/lexical-markdown/src`: `MarkdownTransformers.ts` (the four transformer types; `HEADING`, `QUOTE`, `CODE`, `UNORDERED_LIST`, `ORDERED_LIST`, `CHECK_LIST`, the text formats, `LINK`, `TRANSFORMERS` and the groups by type; the list marker, code fence, code meta and hard line break NodeStates; `normalizeMarkdown`, `isTableRowDivider`), `MarkdownImport.ts` (`createMarkdownImport`, `$importMultiline`, `$importBlocks`), `importTextTransformers.ts`, `importTextFormatTransformer.ts`, `importTextMatchTransformer.ts`, `MarkdownExport.ts` (`createMarkdownExport`, with selection), `utils.ts`; `$convertFromMarkdownString`, `$convertToMarkdownString` and `$convertSelectionToMarkdownString` (`index.ts`); `MarkdownShortcuts.ts` (`registerMarkdownShortcuts`) | `markdown` | `markdown/import-export`, `property/documents`, `scripts/markdown-shortcuts` |
| UTF-16 offsets | `String#length`, `String#slice` | `utf16` | through the text methods; unit tests against Node |
| Upstream version | `meta.json` | `UPSTREAM_VERSION` | `meta` |

## Not ported yet

Everything else; [DESIGN.md](DESIGN.md) has the order. Within what M0
touches:

- Of slots: the `'deletion'` mode of `$setPointAtSlotBoundary`, where JS
  lets `Selection.modify` carry a deletion into another slot frame and
  pulls it back to the boundary; a headless editor's `SelectionMovement`
  moves within text only, as Swift's does. `LexicalMutations.ts`' and the
  reconciler's slot handling are DOM-only.
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
  the `devtools` feature, and `@lexical/clipboard`'s copy), though no vector reaches that rule;
  `getTopLevelElement` is not. `@lexical/extension`'s
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
  The other extensions of `@lexical/extension` (`AutoFocus`,
  `ClickAfterLastBlock`, `IME`, `KeyboardShortcuts`, `NodeSelection`,
  `NodeSelectionDataSelected`, `NormalizeTripleClickSelection`,
  `SelectionAlwaysOnDisplay`, `WatchEditable`) read the
  DOM, a selection's DOM, focus or keyboard events. Of the model-level
  ones: `HorizontalRuleExtension` needs `NodeSelectionExtension` and clicks
  (its node is ported); `NestedEditorExtension` sets a nested editor's
  `_parentEditor`, theme and editable listeners, none of which a Rust
  editor has. `NodeStructureExtension` is ported (see the rows above), so
  `BlockIdExtension`'s peer dependency on `@lexical/NodeStructure` resolves
  where an editor has both, and the builder registers it first.
  Of `BlockIdExtension`: `generateBlockId` hashes a counter and the clock
  with the standard library's randomly keyed `RandomState` where JS uses
  `crypto.getRandomValues` (no dependencies: it is unpredictable but not
  a cryptographic generator, and the config takes a `generate_id` of an
  app's own); the `__DEV__` warning for a repair update that left blocks
  without a unique id; and `frozenLookup`'s per-state cache, which only
  saves repeated lookups and changes no result. `$getBlockHandles()`
  returns its `Map`'s entries in order, and `BlockHandleOptions.read`
  takes them so. `PreventSelectAllExtension`'s
  `register` is a capturing `keydown` listener on the root element, so
  only its config and signal are ported; `TabIndentationExtension`'s
  `KEY_TAB_COMMAND` handler reads a `KeyboardEvent`, which the core
  doesn't have yet (the code marks where it goes); `SelectBlockExtension`'s
  `cascadeSelection` is kept but has no parent editor to select. Of the
  packages' extensions, the DOM, click and focus ones
  (`ClickableLinkExtension`, `DragonExtension`,
  `NormalizeTripleClickSelectionExtension`, `CheckListExtension`'s
  pointer, focus and key handlers, `registerCodeGutter`) aren't ported
  (the HTML import ones come with `html`), nor are the `KEY_*` handlers
  (`KEY_ENTER_COMMAND` in `CodeExtension`, `KEY_TAB_COMMAND` and the arrow
  keys in `CodeIndentExtension`, whose `escapeWithArrows` is kept, with no
  handler to read it). `HistoryExtension`'s `disabled` defaults to `false`,
  as Swift's does, where JS's is `typeof window === 'undefined'` (true in a
  headless JS editor); `SharedHistoryExtension`'s `parentEditor` must be
  given, as a Rust editor has no `_parentEditor` to fall back to, and its
  HMR re-link (`hmrRestoreCount`) isn't ported. A config's functions
  (`validateUrl`, `createInitialHistoryState`, `$canIndent`, the matchers)
  are `Rc` closures, which compare by identity as JS's do.
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
  `$selectAll(selection)`'s scoping isn't ported.
- Of `@lexical/selection` and `@lexical/utils`, what needs the DOM, a
  browser or pieces the port doesn't have: `createDOMRange`,
  `createRectsFromDOMRange`, `positionNodeOnRange`, `markSelection`,
  `selectionAlwaysOnDisplay`, `dedupeSelectionRects` (DOM ranges and rects);
  `addClassNamesToElement`, `removeClassNamesFromElement`, `isHTMLElement`,
  `isHTMLAnchorElement`, `isBlockDomNode`, `isInlineDomNode`,
  `getScrollParent`, `calculateZoomLevel` (DOM elements); `mediaFileReader`,
  `isMimeType` and `eventFiles` (`File`, which the port has no type for, and
  a `DataTransfer`'s files, which the port's `DataTransfer` doesn't carry); the `CAN_USE_*` and `IS_*` environment flags (a
  browser's user agent); `$getComputedStyleForElement`,
  `$getComputedStyleForParent` and `$isParentRTL` (computed styles, so
  `$isParentElementRTL` and the vertical writing mode `$moveCharacter` and
  `$shouldOverrideDefaultCharacterSelection` read are always false, as in
  a headless JS editor); `$sliceSelectedTextNodeContent`'s default `'self'`
  mode (it writes the node's text without `getWritable`, and a node's
  committed value can't change in place here);
  `objectKlassEquals` (JS constructors, which a node type string stands for
  here); `$isEditorIsNestedEditor` (an editor has no parent editor yet);
  and `trimTextContentFromAnchor`, JS's deprecated other name for
  `$trimTextContentFromAnchor`.
- Of `@lexical/list`'s DOM parts, `updateDOM` (the reconciler's);
  `ListImportRules`' flatten step never lifts a nested list's blocks out
  (`INTERNAL_$liftsFromStructure` is always false), as
  `NodeStructureExtension` isn't ported;
  `registerList`'s `KEY_BACKSPACE_COMMAND` handler and
  `registerCheckList`'s pointer, focus, arrow, Space and Escape handlers,
  which need an input layer (`deleteCharacter` reaches the same
  `ListItemNode.collapseAtStart`, and a toggle is `toggle_checked`), and
  `$insertList`'s skipping of a named slot's value, which no vector covers
  yet. What `@lexical/list` does where `NodeStructureExtension` enforces
  node structure is ported (see the node structure row). `ListNode` reads its tag off its list type rather
  than keeping `__tag` apart: JS sets both together everywhere. Backspace
  at the start of a document's first item, with nothing before it, needs
  `Selection.modify` in headless JS, which throws, as `scripts/list`
  records.
- Of `@lexical/link`'s DOM parts, `updateDOM`. `sanitizeUrl` asks `new URL` whether a URL
  parses and for its protocol; as in Swift, that is a model of the WHATWG
  parser's scheme and host rules without IDNA or IPv4 parsing, so a host
  only those reject (`https://1.2.3.4.5`) is taken as parsing.
  `registerLink`'s
  `PASTE_COMMAND` handler (registered only with `validateUrl`), which reads
  a `ClipboardEvent`'s data;
  `ClickableLinkExtension` and `AutoLinkAnnounceExtension`, which need a
  DOM. Where `NodeStructureExtension` enforces node structure,
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
- Of `@lexical/code-core`'s DOM parts, `updateDOM`
  and `registerCodeGutter`, which writes to the
  DOM; `registerCodeIndentation`'s handlers of `KEY_TAB_COMMAND`,
  the arrow keys (`$handleTab`, `$handleShiftLines`, `escapeWithArrows`),
  `MOVE_TO_START` and `MOVE_TO_END` (`$handleMoveTo`), which read a
  `KeyboardEvent` and wait for an input layer. In an editor without
  `CodeExtension` (one made with `create_editor`, or built without it),
  `CodeNode.insertNewAfter` takes the path JS takes there (exiting the
  block after two line breaks itself, as `scripts/code` records with
  `createEditor`), less the deprecation warning; in one built with it,
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
- `@lexical/mark`'s `createDOM` and `updateDOM` (the `mark` and
  `markOverlap` theme classes), which come with `html`, and
  `MarkExtension`, which needs the extension API (`mark_nodes()` is its
  `nodes`). Of `MarkStateExtension.ts`, everything that needs the
  extension API waits for it: `MarkStateExtension`, `MarkBaseExtension`,
  `defineMarkExtension` and mark flavors (the flavor-first forms of the `$`
  functions, `$getMarkIDsByFlavor`, `AnyMarkFlavorExtension`), the
  `markableNodes` config (the functions use `DEFAULT_MARKABLE_NODES`, which
  JS uses too when the extension is not installed), `disabled`, the
  `markIDNodeKeys` output and `$getMarkIDCandidates` (so `$removeMarkID`
  without a selection always walks the document, as JS does without the
  extension), `registerMarkIDsListener` and `reportMarkIDs`, and
  `registerMarkState`'s inserted-text rule (`$applyInsertedTextMarkIDs` and
  the segment matching, composition, the `CONTROLLED_TEXT_INSERTION_COMMAND`
  and `PASTE_COMMAND` recorders). Rendering (`$decorateMarkIDs`,
  `isMarkIDShown` and its rerender, `exportIDsToHTML`) and the HTML import
  rule (`data-lexical-mark-ids`) come with `html` and the DOM.
  JS's `markIds` state is an `arraySetValue`; the port has no schema
  builders yet, so its config is a `StateValueConfig` with the same parse,
  JSON and equality, and the canonical form `$setState` gives a set-valued
  state is applied where the module writes it. Ids sort by UTF-16 code
  units, as JS's `sort()` does. In a headless editor,
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
  `editor.isComposing()`, false in a headless editor; and the hook's
  re-registration when React's dependencies change, which an app does by
  calling the clean-up and registering again. Of the 36
  `property/documents` cases that waited on
  `lexical-overflow`, 11 also hold hashtag nodes and waited on
  `lexical-hashtag`, now ported. That suite now sets each parsed state on the editor
  and reads the text there, as JS does: a parsed state is all dirty, so
  the transforms run, and `OverflowNode`'s removes the empty ones a random
  document holds.
- Of `@lexical/hashtag`: within `registerLexicalTextEntity`,
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
  runner; no `devtools/tree` case holds one); the printing of slots
  (`$resolveSlotContext`, `$slotValueMetaSuffix`), which Swift has but no
  `devtools/tree` vector holds a slot to check; the `compositionKey` line, as a headless editor never
  composes; and the React parts, `TreeView.tsx` and
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
- `registerRichText`'s handlers of DOM events (click, the arrow keys,
  Backspace, Delete, Enter, Escape, Space and Tab keys, `MOVE_TO_START` and
  `MOVE_TO_END`, drop and drag), which wait for an input layer, with its
  `escapeFormatTriggers` and `shouldHandlePasteAsFiles` (`DRAG_DROP_PASTE`,
  for a paste of files, which the port's `DataTransfer` doesn't carry; so
  `RichTextExtension`'s config, `RichTextConfig`, which only those read,
  isn't ported);
  `CONTROLLED_TEXT_INSERTION_COMMAND`'s `beforeinput` form; and
  `updateDOM`.
- `registerPlainText`'s handlers of DOM events (the arrow keys, Backspace,
  Delete, Enter, drop and drag) and two of `PlainTextExtension`'s
  dependencies (`DragonExtension`,
  `NormalizeTripleClickSelectionExtension`), which need an input layer.
  `quoteShadowRootState` is a flat NodeState, as in JS
  (`quote_shadow_root_state`; `scripts/node-state` sets and reads it). A
  shadow root quote's `collapseAtStart` (dissolving into
  its blocks) is ported but no vector reaches it: headless JS needs the
  DOM's `Selection.modify` to get there and throws, which `scripts/rich-text`
  records.
- Of `@lexical/html` and the core's DOM side (the `html` feature):
  `BlockIdHTMLExtension`. `$inlineStylesFromStyleSheets` parses each `<style>`
  sheet itself (`dom::parse_style_sheet`) and matches its selectors with
  `DomNode::query_selector_all`, which takes compound selectors, combinators
  and attribute operators; a rule whose selector needs more (pseudo-classes,
  namespaces, escapes) is skipped where a browser would apply it, and a
  shorthand property (`font`, `margin`) is copied without the longhands a
  browser expands it into, so its parts don't reach the import rules (JS
  imports `font: bold 12px` as bold). `html/import-extension` has a case of
  every other kind of rule.
  Also `updateDOM`, `getDOMSlot` and the
  rest of what only the reconciler calls, and `$setTextContent`'s
  composition suffix (nothing composes headless). So the render config
  and `DOMRenderExtension`'s overrides have only the hooks export reaches
  (`$createDOM`, `$exportDOM`, `$extractWithChild`, `$shouldInclude`,
  `$shouldExclude`), and toggling an override through
  `$setRenderContextValue` recompiles the config without the full
  reconcile JS runs to recreate the DOM.
- Of `@lexical/clipboard` (the `clipboard` feature):
  - `trustHTML`: there are no Trusted Types outside a browser, so the
    HTML is parsed as is (as in Swift). `ClipboardDOMImportExtension` puts
    a copy of the `DataTransfer` in `ImportSourceDataTransfer`, since the
    import stacks get it by reference. `scripts/clipboard` records copies
    without `text/html` (`ops.ts`'s `$clipboardDataWithoutHtml`, which
    the Rust runner mirrors); `html/clipboard` records them with it.
  - Drag and drop (`$handleRichTextDrop`, `$handlePlainTextDrop`,
    `$doDrop`, `$writeDragSourceToDataTransfer`, `caretFromPoint.ts`),
    which hit-test the DOM.
  - `copyToClipboard`'s DOM and timer plumbing (without an event it fires a
    `copy` at the document, with a 50 ms timeout and a guard against a copy
    in flight) and the promise it returns. The copy and cut handlers take
    the clipboard event's `DataTransfer` as their payload (`None` for a
    keyboard event or no event) and, with none, copy nothing; a cut still
    removes the selection. `$copyToClipboardEvent` also declines in JS
    when the window has no DOM selection or it is outside the editor,
    which in headless JS is always (a copy writes nothing); the port, like
    Swift, writes the selection, since without a DOM the platform only
    sends the editor's own copy. No vector records the handlers, as
    headless JS can't run them; unit tests cover them.
  - `INTERNAL_$withExportMemo`, a cache that changes no output.
  - `$sliceSelectedTextNodeContent`'s `'self'` mode, which writes a node's
    text without `getWritable`; every ported caller passes `'clone'`.
  - `DataTransfer`'s files, `clearData`, `dropEffect` and `effectAllowed`,
    which nothing ported reads.
- Composition, the DOM half of the reconciler (the elements, the text,
  format and direction caches, `editor._keyToDOMMap`), its slot walks
  (`$mountSlotChildren`, `$reconcileSlotChildren` and `$destroyNode`'s of a
  host's slot subtrees, so a node in a slot reports no mutation), selection-change
  notifications and `RootNode.__cachedText`. With a root element an update
  starts with the last selection, as in a headless editor and in Swift:
  JS reads the DOM selection (`$internalCreateSelection`), which an app's
  view keeps in step (in Node JS has none, so the vectors select in each
  update).
- Of `@lexical/a11y`: the focus trap, the roving tabindex and the focus
  manager, which move DOM focus between HTML elements, and the region's
  visually hidden style. Of the menus: their keyboard commands
  (`KEY_ARROW_DOWN_COMMAND`, `KEY_ARROW_UP_COMMAND`, `KEY_ESCAPE_COMMAND`,
  `KEY_TAB_COMMAND`, `KEY_ENTER_COMMAND`) and
  `SCROLL_TYPEAHEAD_OPTION_INTO_VIEW_COMMAND`, which wait for an input
  layer; an `onClose` that returns a promise; `closeUnlessComposing`'s
  composition check; the React rendering, the anchor element, its scroll
  and resize handling. Auto-embed's `parseUrl` is synchronous, so the menu
  opens in the paste's mutation listener where JS awaits it.
  `BlockWithAlignableContents` (`@lexical/react`) is a React component,
  and isn't ported; Swift has its commands as an extension. So `history` passes
  `false` where JS passes `editor.isComposing()`, as a headless JS editor
  does; the `composition-start` and `composition-end` tags work as in JS.

- Of `@lexical/markdown` (the `markdown` feature):
  `$generateNodesFromMarkdownString`, which imports into
  an `ArtificialNode__DO_NOT_USE` the core doesn't have; and the mdast
  converter, which Swift doesn't port either. `markdown/import-export`'s
  export cases read the state JS built from each case's HTML from its
  `exportState` JSON, as `$generateNodesFromDOM` comes with `html`, and its
  `importHtml` (what `$generateHtmlFromNodes` renders of the import) is
  checked once `html` is ported; the imported state, the absence of a
  selection after importing and the exported markdown are checked in every
  case. The transformers are `Rc`s of structs with public fields, so a
  variant of a built-in is a copy with fields replaced, where JS spreads
  the object; `replace` returns `false` for JS's `false` and `true` for
  its `true` and `undefined`, and a text match transformer's `export` is
  `None` for an unset one. `register_markdown_shortcuts` returns an error
  where JS's invariant throws (a transformer's node not registered), and
  reads the editor as never composing: an app keeps an input method's
  composition outside Lexical until it commits, and the commit's
  `composition-end` tag works as in JS (`scripts/markdown-shortcuts`).
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
