# Changelog

Each release names the upstream Lexical version it conforms to. The release
workflow publishes the section headed `## <swift/VERSION>`.

## Unreleased

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
