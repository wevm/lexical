# Changelog

Each release names the upstream Lexical version it conforms to. The release
workflow publishes the section headed `## <swift/VERSION>`.

## Unreleased

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
