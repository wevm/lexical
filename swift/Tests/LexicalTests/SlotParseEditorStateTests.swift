import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/SlotParseEditorState.test.ts.

/// A shadow-root block used as a slot value.
private final class SlotContainerNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(SlotContainerNode.self, type: "slot_container_parse", extends: ElementNode.self)
  }
  override func isShadowRoot() -> Bool { true }
}

private func buildEditor() throws -> Editor {
  try createEditor(
    CreateEditorArgs(
      namespace: "[slot-parse-editor-state]", nodes: [.klass(SlotContainerNode.self)],
      onError: { throw $0 }))
}

private func seedSlotDocument(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let host = try createParagraphNode()
    let container = try create(SlotContainerNode.self).append(
      try createParagraphNode().append(try createTextNode("Title")))
    try setSlot(host, "title", container)
    try getRoot().clear().append(host)
  }
}

@Suite struct SlotParseEditorStateTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > a state built by update() carries the flag"
    ))
  func updateCarriesFlag() throws {
    let editor = try buildEditor()
    try seedSlotDocument(editor)
    #expect(editor._slotsUsed)
    #expect(editor.getEditorState()._slotsUsed)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > a parsed state carries the flag"
    ))
  func parsedCarriesFlag() throws {
    let source = try buildEditor()
    try seedSlotDocument(source)
    let json = try source.getEditorState().toJSON().stringify()
    let parsed = try buildEditor().parseEditorState(json)
    try parsed.read { () throws in
      #expect(try getRoot().getFirstChildOrThrow().getTextContent().contains("Title"))
    }
    #expect(parsed._slotsUsed)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > setEditorState latches the flag on an editor that did not parse it"
    ))
  func setEditorStateLatchesFlag() throws {
    let source = try buildEditor()
    try seedSlotDocument(source)
    let json = try source.getEditorState().toJSON().stringify()
    let parsed = try buildEditor().parseEditorState(json)
    let target = try buildEditor()
    #expect(target._slotsUsed == false)
    try target.setEditorState(parsed)
    #expect(target._slotsUsed)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/SlotParseEditorState.test.ts::_slotsUsed survives parseEditorState > parsing a slotless document leaves the flag alone"
    ))
  func slotlessLeavesFlag() throws {
    let source = try buildEditor()
    try source.update(discrete: true) {
      try getRoot().clear().append(try createParagraphNode().append(try createTextNode("plain")))
    }
    let json = try source.getEditorState().toJSON().stringify()
    let target = try buildEditor()
    let parsed = try target.parseEditorState(json)
    #expect(parsed._slotsUsed == false)
    try target.setEditorState(parsed)
    #expect(target._slotsUsed == false)
  }
}
