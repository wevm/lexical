// Port of packages/lexical/src/LexicalSerializedExport.ts.

/// Runs `f` writing the compact form of the document (or, with `false`, the
/// legacy form) for any export it performs: `$withCompactExport`.
///
/// The compact form omits every property parsing would restore anyway — one
/// whose value is its schema default, one the parser derives, and the
/// deprecated `version`. Only a Lexical new enough to restore them can read
/// it.
public func withCompactExport<T>(_ compact: Bool, _ f: () throws -> T) rethrows -> T {
  let context = ThreadContext.current
  let previous = context.compactExport
  context.compactExport = compact
  defer { context.compactExport = previous }
  return try f()
}

/// Whether the export walk in progress writes the compact form:
/// `$isCompactExport`.
public func isCompactExport() -> Bool {
  ThreadContext.current.compactExport
}

/// One node's JSON in the form the active export asks for, with the checks
/// every export walk relies on: `$exportNodeJSON`.
public func exportNodeJSON(_ node: LexicalNode) throws -> JSONObject {
  let serializedNode = try node.exportJSON(isCompactExport())
  let nodeClass = type(of: node)
  if serializedNode["type"]?.stringValue != nodeClass.getType() {
    throw LexicalError(
      "LexicalNode: Node \(nodeClass) does not match the serialized type. Check if .exportJSON() is implemented and it is returning the correct type."
    )
  }
  if node is ElementNode, serializedNode["children"]?.arrayValue == nil {
    throw LexicalError(
      "LexicalNode: Node \(nodeClass) is an element but .exportJSON() does not have a children array."
    )
  }
  return serializedNode
}
