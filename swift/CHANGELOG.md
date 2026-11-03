# Changelog

Each release names the upstream Lexical version it conforms to. The release
workflow publishes the section headed `## <swift/VERSION>`.

## Unreleased

- Core model: nodes, editor, updates and transforms, commands and listeners,
  selection, NodeState, slots, carets, and byte-exact legacy and compact JSON.
- Serialization schemas: `arrayValue`, `arraySetValue`, `objectValue`,
  `unionValue` and `transformValue`, with `accepts` and union member
  selection as in JS. Schema declarations JS refuses now throw, so
  `config()` is `throws`.
- Node accessors a schema property reads or writes (`getStyle`, `setIndent`,
  `setTextContent` and the rest) are `open`, and importing JSON goes through
  them, so a subclass that overrides one owns its property as in JS.
- `SchemaField(_:_:field:)` is `withField`: a property stored in a key-path
  field, carried across a clone with no `afterCloneFrom`.
  `getComposedSchemaFields(_:)` returns a class's composed schema.
