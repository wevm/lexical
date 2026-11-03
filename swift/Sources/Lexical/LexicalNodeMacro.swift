/// Generates a node class's `config()`: its type, the class it extends, and a
/// JSON schema field for each `@Field` property, in declaration order (the
/// order JS writes the keys in). It also generates a `getX()`/`setX(_:)`
/// pair for each `@Field` property. Other `NodeConfig` arguments pass
/// through.
///
/// ```swift
/// @LexicalNode(type: "custom", extends: ElementNode.self)
/// final class CustomNode: ElementNode {
///   @Field(stringValue()) var __label: String = ""
/// }
/// ```
///
/// is `$config('custom', {extends: ElementNode, json: nodeSchema({label:
/// withField(stringValue(), {field: '__label'})})})` with `getLabel` and
/// `setLabel`. Like that config, it derives `clone`, `importJSON`,
/// `exportJSON` (both forms) and carries the fields across clones.
@attached(member, names: named(config), arbitrary)
public macro LexicalNode(
  type: String, extends: LexicalNode.Type? = nil,
  transform: ((LexicalNode) throws -> Void)? = nil,
  importJSON: ((JSONObject) throws -> LexicalNode)? = nil,
  stateConfigs: [RequiredNodeStateConfig] = [], inline: Bool? = nil, slots: [String]? = nil,
  children: NodeChildrenConfig? = nil, mergeAdjacent: MergeAdjacentDeclaration? = nil
) = #externalMacro(module: "LexicalMacros", type: "LexicalNodeMacro")

/// Declares a stored property as a JSON schema field of a `@LexicalNode`
/// class, under its name without leading underscores (`__label` is
/// `label`). `accessors: false` skips its `getX`/`setX`.
@attached(peer)
public macro Field(
  _ schema: @autoclosure () throws -> SerializationSchema, accessors: Bool = true
) = #externalMacro(module: "LexicalMacros", type: "FieldMacro")

/// Declares a stored property as the JSON schema field `key` of a
/// `@LexicalNode` class.
@attached(peer)
public macro Field(
  _ key: String, _ schema: @autoclosure () throws -> SerializationSchema,
  accessors: Bool = true
) = #externalMacro(module: "LexicalMacros", type: "FieldMacro")
