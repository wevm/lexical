import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/InlineElementTextFields.test.ts.
// The three "stays linear in the depth" tests count getFirstChild calls with
// vi.spyOn, which Swift can't do; they are skipped (parity/skips.json).

// An element's textFormat and textStyle are persisted only when it has no
// text a reconcile would derive them from (#7968). For an inline element the
// JSON follows the reconcile's rule (first text at any depth); a block keeps
// the direct-children check.

private class InlineWrapperNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(InlineWrapperNode.self, type: "test-inline-wrapper", extends: ElementNode.self)
  }
  override func isInline() throws -> Bool { true }
}

/// An inline element whose clone reads the JSON of the node it is made from.
private final class CloneReadsJSONNode: InlineWrapperNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      CloneReadsJSONNode.self, type: "test-clone-reads-json", extends: InlineWrapperNode.self)
  }
  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    _ = try prevNode.exportJSON()
  }
}

private func wrapper(_ children: LexicalNode...) throws -> InlineWrapperNode {
  try create(InlineWrapperNode.self).setTextFormat(1).setTextStyle("color: red")
    .append(contentsOf: children)
}

private func buildTestEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[inline-element-text-fields]",
      nodes: { [.klass(InlineWrapperNode.self), .klass(CloneReadsJSONNode.self)] },
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
}

private func withEditor(_ fn: @escaping () throws -> Void) throws {
  let editor = try buildTestEditor()
  defer { editor.dispose() }
  try editor.update(fn, EditorUpdateOptions(discrete: true))
}

private struct Fields: Equatable {
  var textFormat: Int?
  var textStyle: String?
}

private func fields(_ node: ElementNode) throws -> Fields {
  let json = try node.exportJSON()
  return Fields(
    textFormat: json["textFormat"]?.numberValue.map { Int($0) },
    textStyle: json["textStyle"]?.stringValue)
}

/// What ElementNode persists for a block (ParagraphNode's exportJSON then
/// backfills them from its first text child for #7971).
private func serialized(_ node: ElementNode) throws -> Fields {
  Fields(
    textFormat: try node.getSerializedTextFormat(), textStyle: try node.getSerializedTextStyle())
}

private let PERSISTED = Fields(textFormat: 1, textStyle: "color: red")
private let OMITTED = Fields(textFormat: nil, textStyle: nil)

@Suite struct InlineElementTextFieldsTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > are omitted when its text sits in an inline element inside it"
    ))
  func omittedWhenTextInNestedInline() throws {
    try withEditor {
      let inner = try wrapper(createTextNode("text"))
      let outer = try wrapper(inner)
      let outermost = try wrapper(wrapper(wrapper(createTextNode("deep"))))
      _ = try getRoot().append(createParagraphNode().append(outer, outermost))
      #expect(try fields(outer) == OMITTED)
      #expect(try fields(inner) == OMITTED)
      #expect(try fields(outermost) == OMITTED)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > are omitted when it holds text of its own, as before"
    ))
  func omittedWhenOwnText() throws {
    try withEditor {
      let w = try wrapper(createTextNode("text"))
      _ = try getRoot().append(createParagraphNode().append(w))
      #expect(try fields(w) == OMITTED)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > are persisted when it holds no text at any depth, as before"
    ))
  func persistedWithoutText() throws {
    try withEditor {
      let empty = try wrapper()
      let lineBreak = try wrapper(createLineBreakNode())
      let nested = try wrapper(wrapper(createLineBreakNode()))
      _ = try getRoot().append(createParagraphNode().append(empty, lineBreak, nested))
      #expect(try fields(empty) == PERSISTED)
      #expect(try fields(lineBreak) == PERSISTED)
      #expect(try fields(nested) == PERSISTED)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of an inline element > a stored value is read and no longer written once it has text inside"
    ))
  func storedValueRead() throws {
    try withEditor {
      let stored = try #require(
        try JSONValue.parse(
          #"""
          {"children":[{"children":[{"detail":0,"format":1,"mode":"normal","style":"color: red","text":"text","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":1,"textStyle":"color: red","type":"test-inline-wrapper","version":1}],"direction":null,"format":"","indent":0,"textFormat":1,"textStyle":"color: red","type":"test-inline-wrapper","version":1}
          """#
        ).objectValue)
      let outer = try #require(try parseSerializedNode(stored) as? InlineWrapperNode)
      #expect(try outer.getTextFormat() == 1)
      #expect(try outer.getTextStyle() == "color: red")
      _ = try getRoot().append(createParagraphNode().append(outer))
      #expect(try fields(outer) == OMITTED)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of a block > are persisted when its text is all inside inline elements, as before"
    ))
  func blockPersistedWhenTextInsideInline() throws {
    try withEditor {
      let paragraph = try createParagraphNode().setTextFormat(1).setTextStyle("color: red")
        .append(wrapper(createTextNode("text")))
      _ = try getRoot().append(paragraph)
      #expect(try serialized(paragraph) == PERSISTED)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::textFormat and textStyle of a block > are omitted when it holds text of its own, as before"
    ))
  func blockOmittedWithOwnText() throws {
    try withEditor {
      let paragraph = try createParagraphNode().setTextFormat(1).setTextStyle("color: red")
        .append(createTextNode("text"), wrapper(createTextNode("more")))
      _ = try getRoot().append(paragraph)
      #expect(try serialized(paragraph) == OMITTED)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::serializing inline elements nested inside each other > a change in an update is seen by the next serialization in it"
    ))
  func changeSeenByNextSerialization() throws {
    try withEditor {
      let text = try createTextNode("text")
      let inner = try wrapper(text)
      let outer = try wrapper(inner)
      _ = try getRoot().append(createParagraphNode().append(outer))
      #expect(try fields(outer) == OMITTED)
      try text.remove()
      _ = try inner.append(createLineBreakNode())
      #expect(try fields(outer) == PERSISTED)
      _ = try inner.append(createTextNode("again"))
      #expect(try fields(outer) == OMITTED)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/InlineElementTextFields.test.ts::serializing inline elements nested inside each other > a clone that reads JSON before the change it is made for does not keep what the change replaces"
    ))
  func cloneReadingJSON() throws {
    let editor = try buildTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try getRoot().append(
        createParagraphNode().append(
          create(CloneReadsJSONNode.self).setTextFormat(1).append(createTextNode("leaf"))))
    }
    var inside = Fields()
    try editor.update(discrete: true) { () throws in
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
      let w = try #require(try paragraph.getFirstChildOrThrow() as? CloneReadsJSONNode)
      try w.getFirstChildOrThrow().remove()
      inside = try fields(w)
    }
    #expect(inside.textFormat == 1)
    let committed = try editor.getEditorState().toJSON()
    let paragraph = committed["root"]?.objectValue?["children"]?.arrayValue?[0].objectValue
    let w = paragraph?["children"]?.arrayValue?[0].objectValue
    #expect(w?["textFormat"] == 1)
  }
}
