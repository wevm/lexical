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
- The caret utilities that split or insert (`$splitTextPointCaret`,
  `$splitAtPointCaretNext`, `$insertNodeToNearestRootAtCaret`), and the
  slot-aware caret comparisons (`$comparePointCaretNextAcrossSlots`,
  `$getCommonAncestorAcrossSlots`). Without slots, NodeState or the text
  edit log, `$removeTextFromCaretRange` never meets a slot host and carries
  no NodeState or deletion records.
- `HistoryExtension` and `SharedHistoryExtension` (`@lexical/history`), which
  need the extension API. Sharing one `HistoryState` between editors through
  `register_history` works. `register_history` takes the delay and
  `maxDepth` as values, where JS also takes signals, and `maxDepth` as a
  `usize`.
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
