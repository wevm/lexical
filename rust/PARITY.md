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
| Update cycle | `LexicalUpdates.ts` (`$beginUpdate`, nested and queued updates, `$applyAllTransforms`, `$commitPendingUpdates`, `$discardPendingUpdate`, listeners, `triggerCommandListeners`, `updateEditorSync`), `LexicalNormalization.ts` (`$normalizeTextNode`), `LexicalGC.ts` (nodes) | `updates`, `normalization`, `gc` | `scripts/updates`, `scripts/listeners`, `scripts/transforms`, `scripts/commands` |
| Editor state | `LexicalEditorState.ts` (`toJSON`, `read`, `isEmpty`, `_parsed`) | `editor_state` | `serialization/core` |
| Parsing | `parseEditorState`, `$parseSerializedNode` (`LexicalUpdates.ts`) | `updates` | `serialization/core`, `property/documents` |
| Editor | `createEditor` (node registration), `update`, `read`, `getEditorState`, `setEditorState`, `registerNodeTransform`, `registerUpdateListener`, `registerTextContentListener`, `registerCommand`, `dispatchCommand`, `createCommand` | `editor` | `scripts/*` |
| UTF-16 offsets | `String#length`, `String#slice` | `utf16` | through the text methods; unit tests against Node |
| Upstream version | `meta.json` | `UPSTREAM_VERSION` | `meta` |

## Not ported yet

Everything else; [DESIGN.md](DESIGN.md) has the order. Within what M0
touches:

- **NodeState** (`"$"`) and **slots** (`"$slots"`): a document that carries
  either fails to parse with an error naming it, rather than losing them.
- `RangeSelection` editing (`insertText`, `insertParagraph`,
  `deleteCharacter`, `formatText`, ...), `$selectAll` and `$insertNodes`.
  `scripts/selection` cases, and the one case each of `scripts/listeners`
  and `scripts/transforms` that types text, are counted as waiting on them.
- The caret utilities that split or insert (`$splitTextPointCaret`,
  `$splitTextPointCaretSlice`, `$splitAtPointCaretNext`,
  `$insertNodeToNearestRootAtCaret`), `$isBlockFullySelected`, and the
  slot-aware caret comparisons (`$comparePointCaretNextAcrossSlots`,
  `$getCommonAncestorAcrossSlots`). Without slots, NodeState or the text
  edit log, `$removeTextFromCaretRange` never meets a slot host and carries
  no NodeState or deletion records.
- Composition, the DOM reconciler, mutation listeners, selection-change
  notifications and `$onUpdate` callbacks (none of which a headless editor
  runs, except `$onUpdate`), and `RootNode.__cachedText`.

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
