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
| Selection editing | `LexicalSelection.ts`: `insertText` (`$insertText`, `$insertTextAtPoint`), `insertRawText` (`tokenizeRawText`, `$generateNodesFromRawText`), `insertNodes` for both selection kinds (`$removeTextAndSplitBlock`, `$splitNodeAtPoint`, `$wrapInlineNodes`), `insertParagraph`, `insertLineBreak`, `removeText`, `formatText` (`$updateTextFormat`, `$setTextFormat`, `$getCharacterOffsets`), `toggleFormat`, `setFormat`, `setStyle`, `deleteCharacter`, `deleteWord`, `deleteLine` and `modify` (`forwardDeletion`, `$extendSelectionForDeletion`, `$modifySelectionAroundDecoratorsAndBlocks`, `$collapseAtStart`, `$removeSegment`, `$updateCaretSelectionForUnicodeCharacter`, `$stopLineDeletionAtLineBreak`, `$shrinkSelectionToRoot`, and the model side of `applyDOMRange`), with an editor's `SelectionMovement` standing in for `Selection.modify`, `$insertNodes`, `$selectAll`; `$splitTextPointCaretSlice` and `$isBlockFullySelected`; `ParagraphNode.insertNewAfter`, `canInsertTextBefore`/`After` | `selection_editing`, `selection_insert_text`, `selection_insert_nodes`, `selection_format`, `selection_deletion` | `scripts/selection`, `fuzz/core`; unit tests against JS's output for paths no vector covers |
| Update cycle | `LexicalUpdates.ts` (`$beginUpdate`, nested and queued updates, `$applyAllTransforms`, `$commitPendingUpdates`, `$discardPendingUpdate`, listeners, `triggerCommandListeners`, `updateEditorSync`), `LexicalNormalization.ts` (`$normalizeTextNode`), `LexicalGC.ts` (nodes) | `updates`, `normalization`, `gc` | `scripts/updates`, `scripts/listeners`, `scripts/transforms`, `scripts/commands` |
| Editor state | `LexicalEditorState.ts` (`toJSON`, `read`, `isEmpty`, `_parsed`) | `editor_state` | `serialization/core` |
| Parsing | `parseEditorState`, `$parseSerializedNode` (`LexicalUpdates.ts`) | `updates` | `serialization/core`, `property/documents` |
| Editor | `createEditor` (node registration), `update`, `read`, `getEditorState`, `setEditorState`, `registerNodeTransform`, `registerUpdateListener`, `registerTextContentListener`, `registerCommand`, `dispatchCommand`, `createCommand` | `editor` | `scripts/*` |
| History (the `history` feature) | `packages/lexical-history/src/index.ts`: `registerHistory` (with `dateNow`, `onHistoryStateChange` and `maxDepth`), `createEmptyHistoryState`, `createMergeActionGetter`, `getChangeType`, `getDirtyNodes`, `isTextNodeUnchanged`, `areNodesUnchanged`, `getInverseEntry`, `getUndoEntry`, `undo`, `redo`, `clearHistory`; the commands it handles and dispatches from `LexicalCommands.ts` (`UNDO_COMMAND`, `REDO_COMMAND`, `CAN_UNDO_COMMAND`, `CAN_REDO_COMMAND`, `CLEAR_HISTORY_COMMAND`, `CLEAR_EDITOR_COMMAND`) and its tags from `LexicalUpdateTags.ts` | `history`, `commands`, `editor` | `scripts/history`; unit tests for a history shared by two editors (upstream's #8623 test) and `maxDepth` |
| Rich text nodes (the `rich-text` feature) | `packages/lexical-rich-text/src/index.ts`: `HeadingNode` (`getTag`, `setTag`, `insertNewAfter`, `collapseAtStart`, `extractWithChild`), `QuoteNode` (`insertNewAfter`, `collapseAtStart`, `canMergeWhenEmpty`, the opt-in shadow root), `$createHeadingNode`, `$isHeadingNode`, `$createQuoteNode`, `$isQuoteNode`; their generated JSON (`LexicalRichTextGeneratedJSON.ts`); `ElementNode.collapseAtStart` as a node hook, and `ElementNode`'s own `$transform` (`$normalizeShadowRootChildren`); from `@lexical/selection` and `@lexical/utils` only `$isAtEdgeOfElement`, `$isAtStartOfNode`, `$isAtEndOfNode`, `$findMatchingParent` and `$handleIndentAndOutdent` | `rich_text`, `selection`, `utils`, `updates` | `scripts/rich-text`, the heading and quote cases of `property/documents` |
| `registerRichText` (the `rich-text` feature) | `packages/lexical-rich-text/src/index.ts` `registerRichText`: the handlers of `DELETE_CHARACTER_COMMAND` (with `NodeSelection.deleteNodes`), `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `FORMAT_TEXT_COMMAND`, `SET_TEXT_FORMAT_COMMAND`, `FORMAT_ELEMENT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, `INSERT_TAB_COMMAND`, `INDENT_CONTENT_COMMAND`, `OUTDENT_CONTENT_COMMAND`, `SELECT_ALL_COMMAND`; those commands from `LexicalCommands.ts`; `ElementNode.canIndent` | `rich_text::register_rich_text`, `commands`, `selection_deletion`, `utils` | `scripts/rich-text-commands` |
| `registerPlainText` (the `plain-text` feature) | `packages/lexical-plain-text/src/index.ts` `registerPlainText`: the handlers of `DELETE_CHARACTER_COMMAND`, `DELETE_WORD_COMMAND`, `DELETE_LINE_COMMAND`, `CONTROLLED_TEXT_INSERTION_COMMAND` (string form), `REMOVE_TEXT_COMMAND`, `INSERT_LINE_BREAK_COMMAND`, `INSERT_PARAGRAPH_COMMAND` (a line break) and `SELECT_ALL_COMMAND` | `plain_text` | `scripts/plain-text` |
| List nodes (the `list` feature) | `packages/lexical-list/src/LexicalListNode.ts` (`ListNode`: `getListType`, `setListType`, `getTag`, `getStart`, `setStart`, `canBeEmpty`, `canIndent`, `splice`, its `$transform`), `LexicalListItemNode.ts` (`ListItemNode`: `append`, `replace`, `insertAfter`, `remove`, `resetOnCopyNodeFrom`, `insertNewAfter`, `collapseAtStart`, `getValue`, `setValue`, `getChecked`, `setChecked`, `toggleChecked`, `getIndent`, `setIndent`, `canMergeWith`, `isParentRequired`, `createParentElementNode`, `canMergeWhenEmpty`, its `$transform`), `$createListNode`, `$isListNode`, `$createListItemNode`, `$isListItemNode`; their generated JSON (`LexicalListGeneratedJSON.ts`); `utils.ts`; the `LexicalNode` hooks they override (`append`, `splice`, `replace`, `insertAfter`, `remove`, `getIndent`, `setIndent`, `resetOnCopyNodeFrom`, a class's own `$transform`); from the core `$copyNode`, `getPreviousSiblings`, `getNextSiblings`, `$splitTextPointCaret`, `$splitAtPointCaretNext`, `$insertNodeToNearestRootAtCaret`; from `@lexical/utils` `$getNearestNodeOfType` | `list`, `lexical_utils`, `caret_utils`, `node`, `nodes::element`, `utils` | `serialization/list`, the list cases of `property/documents`, `scripts/list` |
| `formatList.ts`, `registerList` and `registerCheckList` (the `list` feature) | `formatList.ts` (`$insertList`, `$createListOrMerge`, `mergeLists`, `$removeList`, `updateChildrenListItemValue`, `mergeNextSiblingListIfSameType`, `$handleIndent`, `$handleOutdent`, `$outdentFromItemWithContent`, `$handleListInsertParagraph`); `registerList.ts` `registerList` (`INSERT_ORDERED_LIST_COMMAND`, `INSERT_UNORDERED_LIST_COMMAND`, `REMOVE_LIST_COMMAND`, `UPDATE_LIST_START_COMMAND`, `INSERT_PARAGRAPH_COMMAND`, and the `ListItemNode` and `TextNode` transforms), with `RegisterListOptions`; `checkList.ts` `INSERT_CHECK_LIST_COMMAND` and its handler | `list::format_list`, `list::register_list` | `scripts/list` |
| UTF-16 offsets | `String#length`, `String#slice` | `utf16` | through the text methods; unit tests against Node |
| Upstream version | `meta.json` | `UPSTREAM_VERSION` | `meta` |

## Not ported yet

Everything else; [DESIGN.md](DESIGN.md) has the order. Within what M0
touches:

- **NodeState** (`"$"`) and **slots** (`"$slots"`): a document that carries
  either fails to parse with an error naming it, rather than losing them.
- Deletion and `modify` past the model: JS's `deleteCharacter`,
  `deleteWord`, `deleteLine` and `modify` measure moves within text with
  the DOM's `Selection.modify`. A headless JS editor has no window and
  throws (`window object not found`), and so does this port unless the
  editor has a `SelectionMovement` (`Editor::set_selection_movement`),
  which an app implements from its text layout; the rest follows JS from
  where it lands (a unit test checks that against JS with a stubbed
  `Selection.modify`). The vectors record the headless behavior. The DOM
  details around the measurement (block cursor element, shadow DOM ranges,
  Firefox's line-affinity retry) have no counterpart. `extract`, and
  `$selectAll(selection)`'s scoping aren't ported, nor
  `insertNodes`' CASE 3a (blocks with no block ancestor, which needs
  `$insertNodeToNearestRootAtCaret`).
- `@lexical/list`'s DOM parts (`createDOM`, `updateDOM`, `exportDOM`,
  `importDOM`, the theme class names, `ListImportExtension`/`ListImportRules`
  and `WordListImportExtension`), which come with `html`; `ListExtension`
  and `CheckListExtension`, which need the extension API (`ListExtension`'s
  defaults are what `register_list` with `RegisterListOptions::default()`
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
- The slot-aware caret comparisons (`$comparePointCaretNextAcrossSlots`,
  `$getCommonAncestorAcrossSlots`). Without slots, NodeState or the text
  edit log, `$removeTextFromCaretRange` never meets a slot host and carries
  no NodeState or deletion records.
- `HistoryExtension` and `SharedHistoryExtension` (`@lexical/history`), which
  need the extension API. Sharing one `HistoryState` between editors through
  `register_history` works. `register_history` takes the delay and
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
  `NormalizeInlineElementsExtension`,
  `NormalizeTripleClickSelectionExtension`), which need an input layer and
  the extension API.
  `quoteShadowRootState` is a flat NodeState in JS; with NodeState not
  ported, `QuoteNode` keeps it in a field that parses and serializes as the
  flat state does. A shadow root quote's `collapseAtStart` (dissolving into
  its blocks) is ported but no vector reaches it: headless JS needs the
  DOM's `Selection.modify` to get there and throws, which `scripts/rich-text`
  records.
- Composition, the DOM reconciler, mutation listeners, selection-change
  notifications and `$onUpdate` callbacks (none of which a headless editor
  runs, except `$onUpdate`), and `RootNode.__cachedText`. So `history` passes
  `false` where JS passes `editor.isComposing()`, as a headless JS editor
  does; the `composition-start` and `composition-end` tags work as in JS.

## Documented exceptions

Cases that match JS only under one of these are counted apart in the
report.

- **Unpaired surrogates** (inherited from Swift, `swift/DESIGN.md`, "The
  Tier 1 exception"). A JS string can hold half of a surrogate pair; a Rust
  `String` cannot, so `JSON.parse` of a lone `\uD83D` gives U+FFFD. Lengths
  and UTF-16 offsets still agree with JS (U+FFFD is one unit); only the text
  differs. The vectors keep offsets off pair interiors, so no case relies on
  this yet.

Swift's other Tier 1 deviation, integer properties (`model/integer-properties`),
does not apply: Rust keeps those properties as the `f64` JS stores.
