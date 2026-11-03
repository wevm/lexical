# Changelog

Each release names the upstream Lexical version it conforms to. The release
workflow publishes the section headed `## <swift/VERSION>`.

## Unreleased

- Added HTML import: `LexicalHTML.generateNodesFromDOM`, `importDOM` on
  nodes (`DOMConversionMap`, `DOMConversion`, `DOMConversionOutput`), the
  core nodes' importers, `HTMLConfig.import`, and the helpers
  `setNodeIndentFromDOM`, `setDirectionFromDOM`, `setFormatFromDOM`,
  `isInlineDomNode`, `isOnlyChildInBlockNode`, `isLastChildInBlockNode` and
  `findParentPreDOMNode`. It matches `@lexical/html` on the `html/import`
  vectors. Inlining `<style>` sheets is not ported yet.
- Added `DOMParser`, a pure-Swift HTML parser (WHATWG tokenizer, named
  and numeric character references, and the tree builder's head, body,
  table, list and formatting rules). It builds the same documents as
  jsdom's `DOMParser` for the `html/parse` vectors.
- Added the `LexicalHTML` module's export half: `generateHtmlFromNodes`,
  `generateDOMFromNodes`, `generateDOMFromRoot` and `appendNodeToHTML`,
  which match `@lexical/html` byte for byte (`html/export` vectors).
  `DOMExportOutput` gains `append` and `getChildNodes`; core gains
  `isBlockDomNode`.
- Added `createDOM`, `updateDOM`, `getDOMSlot` and `exportDOM` to nodes,
  with the core nodes' implementations (theme classes, inline styles, the
  `<b>`/`<i>`/`<s>`/`<u>` export wrappers), `EditorDOMRenderConfig`,
  `HTMLConfig` export overrides, `getDocument`, `DOMSlot`,
  `addClassNamesToElement`, `removeClassNamesFromElement`,
  `setDOMStyleFromCSS` and `setDOMStyleObject`.
- Added a small DOM to core (`DOMDocument`, `DOMElement`, `DOMText`,
  `DOMComment`, `DOMDocumentFragment`, `DOMTokenList`,
  `CSSStyleDeclaration`) with `innerHTML`/`outerHTML` serialization. Inline
  styles normalize values as jsdom does (`css/values` vectors), so HTML
  export can match upstream byte for byte.
- Added the `LexicalUtils` module: `dfs`, `reverseDfs`, the slot-aware
  traversals, `firstToLastIterator`/`lastToFirstIterator` (lazy, throwing
  `StepwiseIterator`s), `insertNodeToNearestRoot`, `insertNodeIntoLeaf`,
  `wrapNodeInElement`, `unwrapAndFilterDescendants`, `descendantsMatching`,
  `registerNestedElementResolver`, `restoreEditorState`, `eventFiles`,
  `handleIndentAndOutdent`, `onEscapeUp`/`onEscapeDown`, `makeStateWrapper`
  and `dedupeSelectionRects`. `LexicalSelection` gains `SelectionRect` and
  `filterSelectionRects`, the rect filter of `createRectsFromDOMRange`.
- Added the `LexicalSelection` module: `patchStyleText`, `setBlocksType`,
  `wrapNodes`, `getSelectionStyleValueForProperty`,
  `sliceSelectedTextNodeContent`, `trimTextContentFromAnchor`,
  `shouldOverrideDefaultCharacterSelection`, `moveCharacter` and the other
  `@lexical/selection` helpers. A root element reports computed styles
  (direction, writing mode) by conforming to `ComputedStyleProvider`.
- Added `getStyleObjectFromCSS`, which returns an ordered `StyleObject`, and
  `INTERNAL_isInlineContent` to core.
- Added keyboard and DOM event types (`KeyboardEvent`, `ClipboardEvent`,
  `InputEvent`, `DataTransfer`, …), the `KEY_*`, clipboard, drag and focus
  commands, `isExactShortcutMatch`, `compileKeyboardShortcuts` and a default
  `KEY_DOWN_COMMAND` handler that dispatches the same commands as JS on
  Apple and non-Apple platforms.
- Added `ClearEditorExtension`, `NestedEditorExtension`,
  `NormalizeInlineElementsExtension` and `KeyboardShortcutsExtension`. Extension `init` now takes the editor config
  `inout`, as JS mutates it.
- Added the `LexicalExtension` module: `buildEditorFromExtensions`,
  `LexicalBuilder`, `InitialStateExtension`, signals (a port of
  `@preact/signals-core`: `signal`, `computed`, `effect`, `batch`,
  `untracked`, `namedSignals`, `watchedSignal`) and the dependency helpers
  (`getExtensionDependencyFromEditor`, `getPeerDependencyFromEditor`, …).
  Core gains `defineExtension`, `configExtension`, `declarePeerDependency`,
  `Partial<Config>`, `shallowMergeConfig`, `safeCast` and `Editor.dispose()`.
- With a root element attached, commits now run the reconciler's model
  half: mutation listeners receive created, updated and destroyed nodes,
  `RootNode` caches its text, and elements take `textFormat`/`textStyle`
  from their first text node, as in JS.
- `editor.read()` warns when it is called from a commit listener while
  another update is pending, as JS does in development.
- Fixed: a mutation listener whose teardown was discarded could be replaced
  by the next one registered.
- Update, editable, decorator and text content listeners may return a
  `ListenerCleanup`, called before their next call and when they are
  unregistered, as in JS (`{ _ -> ListenerCleanup? in … }`).
- Added `createRefCountedRegistry(_:)`: registrations reference counted by
  key, activated on the first and torn down after the last release.
- Added `LexicalWarning`, an `Error` with a namespaced `code` and optional
  `data`, for `onWarn`.
- Added `mergeRegister(_:)`, which `lexical` exports: one clean-up that runs
  the given clean-ups in reverse order, once.
- Added `Editor.setRootElement(_:)` and `registerRootListener(_:)`. The root
  is the native view; attaching or detaching it resets the editor and
  commits the pending state again, keeping queued updates and their tags.
  A root conforming to `LexicalRootElement` reports `isConnected`; range
  selection changes notify `SELECTION_CHANGE_COMMAND` only while it is.
- `LexicalNSTextView` now builds its TextKit 2 stack itself; it had no text
  system, so it showed nothing.
- A committed state's node map is frozen as in JS's dev build: writing to it
  (for example after an `editor.read()` committed mid-update) throws.
  `drainMicrotasks()` also runs `setTimeout(0)` work, so the update-cascade
  guard fires as in JS. Errors from a queued commit go to
  `withUncaughtErrorHandler(_:_:)` (standard error by default) instead of
  `onError`, and an update started from a read-only context warns.
- An update that `onError` starts while handling a failed update is now
  queued until the failed update is cleaned up, as in JS, instead of running
  at once.
- `getRegisteredSubtypeMap(_:)`, and `withWarningHandler(_:_:)` to capture
  the warnings Lexical writes to standard error, such as the one
  `createEditor` now gives for a replacement without `withKlass`.
- `LexicalTextKit`: a first typeable editor, `LexicalNSTextView` and
  `LexicalUITextView` on TextKit 2, which turn typing, deletion, Enter and
  selection changes into commands and show what the model renders.
  `Editor.selectionMovement` lets a view supply the layout step that
  `Selection.modify` gives JS, so deletion within text works.
- `@LexicalNode(type:extends:…)` and `@Field(schema)`: a node class's
  `config()`, with one schema field per `@Field` property and a
  `getX`/`setX` pair for each. Adds a dependency on swift-syntax.
- Core model: nodes, editor, updates and transforms, commands and listeners,
  selection, NodeState, slots, carets, and byte-exact legacy and compact JSON.
- Serialization schemas: `arrayValue`, `arraySetValue`, `objectValue`,
  `unionValue` and `transformValue`, with `accepts` and union member
  selection as in JS. Schema declarations JS refuses now throw, so
  `config()` is `throws`.
- Node accessors a schema property reads or writes (`getStyle`, `setIndent`,
  `setTextContent` and the rest) are `open`, and importing JSON goes through
  them, so a subclass that overrides one owns its property as in JS.
- RangeSelection editing: `insertText`, `insertRawText`, `removeText`,
  `insertNodes`, `insertParagraph`, `insertLineBreak`, `extract` and
  `formatText`, with `insertNodes`, `formatText`, `setTextFormat`,
  `getCharacterOffsets`, `tokenizeRawText` and `generateNodesFromRawText`.
  `deleteCharacter`, `deleteWord`, `deleteLine` and `modify` handle the
  moves across decorators and blocks; moving by a character, word or line
  within text waits for the TextKit views, as JS waits for the DOM.
- Caret editing: `removeTextFromCaretRange`, `splitAtPointCaretNext`,
  `insertNodeToNearestRootAtCaret`, `splitTextPointCaret`,
  `splitTextPointCaretSlice`, `getAdjacentSiblingOrParentSiblingCaret` and
  `getAdjacentNodes`.
- `SchemaField(_:_:field:)` is `withField`: a property stored in a key-path
  field, carried across a clone with no `afterCloneFrom`.
  `getComposedSchemaFields(_:)` returns a class's composed schema.
- `setState` stores an `arraySetValue` state in its canonical form, and
  `createState(_:schema:)` takes an `unparse`. A text node without NodeState
  no longer merges with a neighbor that has state.
