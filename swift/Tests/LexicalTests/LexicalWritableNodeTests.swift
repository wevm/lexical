import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalWritableNode.test.ts.

private func headlessEditor() throws -> Editor {
  try createEditor(CreateEditorArgs(onError: { throw $0 }))
}

private func staleMessage(_ node: TextNode) -> String {
  "Lexical node does not exist in active editor state (key \(node.getKey()), type text). "
    + "The reference was probably kept from another read or update, for example in a closure, event handler or component state, "
    + "and the node has since been removed or is not part of this editor state. "
    + "Before using a node reference that may be stale, check node.isAttached() or look the node up again with "
    + "$getNodeByKey(node.getKey()), inside the read or update where it is used."
}

private func expectThrowsMessage(_ message: String, _ body: () throws -> Void) {
  #expect {
    try body()
  } throws: {
    "\($0)".contains(message)
  }
}

private func removedTextNode() throws -> (Editor, TextNode) {
  let editor = try headlessEditor()
  var text: TextNode!
  try editor.update(discrete: true) {
    text = try createTextNode("removed")
    try getRoot().append(try createParagraphNode().append(text))
  }
  try editor.update(discrete: true) { try text.getParentOrThrow().remove() }
  return (editor, text)
}

@Suite(.serialized) struct LexicalWritableNodeTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > returns cached writable nodes without looking them up in the node map"
    ))
  func returnsCachedWritable() throws {
    // Upstream also spies on the node map's `get`; the port checks identity.
    let editor = try headlessEditor()
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("text")
      try getRoot().append(try createParagraphNode().append(text))
      let writable = try text.getWritable()
      #expect(try text.getWritable() === writable)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > reuses new nodes and clones committed nodes once per update"
    ))
  func clonesCommittedOncePerUpdate() throws {
    let editor = try headlessEditor()
    var original: TextNode!
    try editor.update(discrete: true) { () throws in
      original = try createTextNode("original")
      #expect(try original.getWritable() === original)
      try getRoot().append(try createParagraphNode().append(original))
    }
    let originalState = editor.getEditorState()
    var firstClone: TextNode!
    try editor.update(discrete: true) { () throws in
      firstClone = try original.setTextContent("first")
      #expect(firstClone !== original)
      #expect(try original.getWritable() === firstClone)
      #expect(try firstClone.getWritable() === firstClone)
      try originalState.read { () throws in
        #expect(try original.getLatest() === original)
        #expect(try original.getTextContent() == "original")
        #expect(throws: (any Error).self) { try original.getWritable() }
      }
      #expect(try original.getWritable() === firstClone)
    }
    try editor.update(discrete: true) { () throws in
      let secondClone = try original.setTextContent("second")
      #expect(secondClone !== firstClone)
      #expect(secondClone !== original)
      #expect(firstClone.__text == "first")
      #expect(original.__text == "original")
      #expect(try original.getWritable() === secondClone)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > reuses live writable nodes across batched updates until commit"
    ))
  func reusesAcrossBatchedUpdates() throws {
    let editor = try headlessEditor()
    var original: TextNode!
    try editor.update(discrete: true) {
      original = try createTextNode("original")
      try getRoot().append(try createParagraphNode().append(original))
    }
    var writable: TextNode!
    try editor.update { () throws in
      writable = try original.setTextContent("first")
      #expect(writable !== original)
    }
    #expect(editor._pendingEditorState != nil)
    try editor.update(discrete: true) { () throws in
      #expect(try original.getWritable() === writable)
      #expect(try original.setTextContent("second") === writable)
      #expect(original.__text == "original")
    }
    try editor.update(discrete: true) { () throws in
      #expect(try original.getWritable() !== writable)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > marks the current parent dirty when writing through a stale reference"
    ))
  func marksCurrentParentDirty() throws {
    let editor = try headlessEditor()
    var original: TextNode!
    var oldParentKey: NodeKey = ""
    var currentParentKey: NodeKey = ""
    try editor.update(discrete: true) {
      original = try createTextNode("original")
      let paragraph = try createParagraphNode().append(original)
      oldParentKey = paragraph.getKey()
      try getRoot().append(paragraph)
    }
    try editor.update(discrete: true) {
      let paragraph = try createParagraphNode()
      currentParentKey = paragraph.getKey()
      try getRoot().append(paragraph)
      try paragraph.append(original)
    }
    #expect(original.__parent == oldParentKey)
    try editor.update(discrete: true) {
      _ = try original.setTextContent("changed")
      #expect(editor._dirtyElements.has(currentParentKey))
      #expect(editor._dirtyElements.has(oldParentKey) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > does not reuse a collected subtree in a subsequent batched update"
    ))
  func doesNotReuseCollectedSubtree() throws {
    let editor = try headlessEditor()
    var text: TextNode!
    try editor.update {
      text = try createTextNode("detached")
      let paragraph = try createParagraphNode().append(text)
      try getRoot().append(paragraph)
      try paragraph.remove()
    }
    // GC has run, but the pending state has not committed yet.
    #expect(editor._pendingEditorState != nil)
    try editor.update(discrete: true) { () throws in
      #expect(try getNodeByKey(text.getKey()) == nil)
      expectThrowsMessage("Lexical node does not exist in active editor state") {
        _ = try text.getWritable()
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > does not reuse a text node collected by normalization"
    ))
  func doesNotReuseNormalizedNode() throws {
    let editor = try headlessEditor()
    var merged: TextNode!
    try editor.update {
      merged = try createTextNode("second")
      try getRoot().append(try createParagraphNode().append(try createTextNode("first"), merged))
    }
    try editor.update(discrete: true) { () throws in
      #expect(try getRoot().getTextContent() == "firstsecond")
      #expect(try getNodeByKey(merged.getKey()) == nil)
      expectThrowsMessage("Lexical node does not exist in active editor state") {
        _ = try merged.getWritable()
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > restores writable nodes after parsing a separate editor state"
    ))
  func restoresWritableAfterParsing() throws {
    let editor = try headlessEditor()
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode().append(try createTextNode("a")))
    }
    let serialized = try editor.getEditorState().toJSON()
    try editor.update(discrete: true) { () throws in
      let root = try getRoot().getWritable()
      let parsed = try editor.parseEditorState(serialized) {
        #expect(try getRoot().getWritable() !== root)
        try getRoot().append(try createParagraphNode().append(try createTextNode("b")))
      }
      #expect(try getRoot().getWritable() === root)
      #expect(try root.getChildrenSize() == 1)
      try parsed.read { () throws in
        #expect(try getRoot() !== root)
        #expect(try getRoot().getChildrenSize() == 2)
      }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::writable node reuse > invalidates selection caches even when reusing a writable node"
    ))
  func invalidatesSelectionCaches() throws {
    let editor = try headlessEditor()
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("text")
      try getRoot().append(try createParagraphNode().append(text))
      _ = try text.select(0, 4)
      let selection = try #require(try getSelection() as? RangeSelection)
      _ = try selection.getNodes()
      #expect(selection.getCachedNodes() != nil)
      #expect(try text.getWritable() === text)
      #expect(selection.getCachedNodes() == nil)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::stale node references > explains how to guard a stale reference when reading"
    ))
  func staleReferenceWhenReading() throws {
    let (editor, text) = try removedTextNode()
    try editor.read {
      expectThrowsMessage(staleMessage(text)) { _ = try text.getTextContent() }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::stale node references > explains how to guard a stale reference when writing"
    ))
  func staleReferenceWhenWriting() throws {
    let (editor, text) = try removedTextNode()
    try editor.update(discrete: true) {
      expectThrowsMessage(staleMessage(text)) { _ = try text.setTextContent("changed") }
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWritableNode.test.ts::stale node references > the suggested guards do not throw for a stale reference"
    ))
  func suggestedGuardsDoNotThrow() throws {
    let (editor, text) = try removedTextNode()
    try editor.update(discrete: true) { () throws in
      #expect(try text.isAttached() == false)
      #expect(try getNodeByKey(text.getKey()) == nil)
    }
  }
}
