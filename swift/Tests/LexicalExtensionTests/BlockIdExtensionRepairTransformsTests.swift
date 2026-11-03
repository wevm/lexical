import Lexical
import LexicalExtension
import LexicalRichText
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/
// BlockIdExtension.repairTransforms.test.ts. `repairTransforms`: whether the
// update that names the blocks an update applied with skipTransforms (or
// tagged COLLABORATION_TAG or HISTORIC_TAG) left without an id runs the
// editor's transforms.

private func build(_ repairTransforms: Bool? = nil) throws -> Editor {
  var n = 0
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [
        RichTextExtension,
        NodeStructureExtension,
        configExtension(BlockIdExtension) {
          $0.generateId = {
            n += 1
            return "g\(n)"
          }
          if let repairTransforms {
            $0.repairTransforms = repairTransforms
          }
        },
      ], editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  try discrete(editor) { try getRoot().append(paragraph("one")) }
  return editor
}

/// Each block as `type(text)#id`, nested blocks in brackets.
private func describeDoc(_ editor: Editor) throws -> String {
  try editor.getEditorState().read(editor: editor) {
    func visit(_ n: LexicalNode) throws -> String {
      guard let element = n as? ElementNode, try !element.isInline() else {
        return JSONValue.string(try n.getTextContent()).stringify()
      }
      let children = try element.getChildren().map(visit).joined(separator: " ")
      return "\(n.getType())#\(try getBlockId(n) ?? "none")[\(children)]"
    }
    return try getRoot().getChildren().map(visit).joined(separator: " / ")
  }
}

/// A quote holding two paragraphs, which NodeStructureExtension joins.
private func appendQuoteOfParagraphs() throws {
  try getRoot().append(createQuoteNode().append(paragraph("a"), paragraph("b")))
}

private func matches(_ string: String, _ pattern: String) throws -> Bool {
  try string.wholeMatch(of: Regex(pattern)) != nil
}

private func contains(_ string: String, _ pattern: String) throws -> Bool {
  try string.firstMatch(of: Regex(pattern)) != nil
}

@Suite struct BlockIdRepairTransformsTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > defaults to true: the follow-up update runs the transforms"
    ))
  func defaultsToTrue() throws {
    let editor = try build()
    defer { editor.dispose() }
    // The update leaves the blocks it adds without ids and the quote's
    // paragraphs unjoined; the follow-up names them and, running
    // NodeStructureExtension's transforms, joins the paragraphs.
    try discrete(editor, skipTransforms: true) { try appendQuoteOfParagraphs() }
    let doc = try describeDoc(editor)
    #expect(
      try matches(doc, #"paragraph#[\w-]+\["one"\] / quote#[\w-]+\["a" "\\n" "b"\]"#), "\(doc)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > false: the follow-up of an update applied with skipTransforms only sets ids"
    ))
  func falseOnlySetsIds() throws {
    let editor = try build(false)
    defer { editor.dispose() }
    try discrete(editor, skipTransforms: true) { try appendQuoteOfParagraphs() }
    // The structure is as the update left it; every block is named, with
    // the positional ids every reader of that document derives, not
    // generated ones.
    let doc = try describeDoc(editor)
    #expect(
      try matches(
        doc,
        #"paragraph#[\w-]+\["one"\] / quote#[\w-]+\[paragraph#[\w-]+\["a"\] paragraph#[\w-]+\["b"\]\]"#
      ), "\(doc)")
    #expect(try !contains(doc, #"#g\d|#none"#), "\(doc)")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > false: the follow-up of an update tagged COLLABORATION_TAG names the blocks alike"
    ))
  func falseCollaborationAlike() throws {
    let a = try build(true)
    defer { a.dispose() }
    let b = try build(false)
    defer { b.dispose() }
    for editor in [a, b] {
      try discrete(editor, COLLABORATION_TAG) { try appendQuoteOfParagraphs() }
    }
    // The update ran its own transforms; the follow-up only names.
    #expect(try describeDoc(b) == describeDoc(a))
    #expect(try !contains(describeDoc(b), #"#g\d|#none"#))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > false: the blocks it names get the ids the default gives them"
    ))
  func falseNamesLikeTheDefault() throws {
    let a = try build(true)
    defer { a.dispose() }
    let b = try build(false)
    defer { b.dispose() }
    for editor in [a, b] {
      // Content that the structure normalization keeps as it is.
      try discrete(editor, skipTransforms: true) {
        try getRoot().append(paragraph("two"), paragraph("three"))
      }
    }
    #expect(try describeDoc(b) == describeDoc(a))
    // A later local edit gets a generated id, as with the default.
    try discrete(b) {
      try #require(try getRoot().getLastChild()).insertAfter(paragraph("four"))
    }
    #expect(try contains(describeDoc(b), #"paragraph#g1\["four"\]$"#))
  }
}
