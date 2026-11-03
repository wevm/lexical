import Lexical
import LexicalExtension
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts.
// A named-slot value has no parent, so it has no parent style to read.

private func runInEditor(
  _ build: @escaping () throws -> Void, _ check: @escaping () throws -> Void
) throws {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "@computed-style-slots-test", nodes: { [.klass(TestShadowRootNode.self)] },
      editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try build() }),
      afterRegistration: { editor, _, _ in
        let root = StyledRootElement()
        root.editor = editor
        try editor.setRootElement(root)
        return { try? editor.setRootElement(nil) }
      }))
  defer { editor.dispose() }
  try editor.update(discrete: true, check)
}

@Suite struct LexicalComputedStyleSlotsTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts::computed styles and named slots > $getComputedStyleForParent returns null for a bare-block slot value"
    ))
  func nullForSlotValue() throws {
    var slotParaKey = ""
    try runInEditor(
      {
        let host = try createParagraphNode()
        try getRoot().append(host)
        let slotPara = try createParagraphNode()
        slotParaKey = slotPara.getKey()
        try setSlot(host, "title", slotPara)
      },
      {
        let slotPara = try #require(try getNodeByKeyOrThrow(slotParaKey) as? ParagraphNode)
        #expect(try getComputedStyleForParent(slotPara) == nil)
      })
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts::computed styles and named slots > $isParentRTL is false for the anchor inside an empty bare-block slot value"
    ))
  func notRTLInEmptySlot() throws {
    var slotParaKey = ""
    try runInEditor(
      {
        let host = try createParagraphNode()
        try getRoot().append(host)
        let slotPara = try createParagraphNode()
        slotParaKey = slotPara.getKey()
        try setSlot(host, "title", slotPara)
      },
      {
        let slotPara = try #require(try getNodeByKeyOrThrow(slotParaKey) as? ParagraphNode)
        let selection = try slotPara.select()
        #expect(try !isParentRTL(selection.anchor.getNode()))
      })
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalComputedStyleSlots.test.ts::computed styles and named slots > $getComputedStyleForParent still resolves the parent inside a slot"
    ))
  func resolvesParentInsideSlot() throws {
    var slotParaKey = ""
    var slotRootKey = ""
    try runInEditor(
      {
        let host = try createParagraphNode()
        try getRoot().append(host)
        let slotRoot = try createTestShadowRootNode()
        let slotPara = try createParagraphNode().append(try createTextNode("title"))
        try slotRoot.append(slotPara)
        slotParaKey = slotPara.getKey()
        slotRootKey = slotRoot.getKey()
        try setSlot(host, "title", slotRoot)
      },
      {
        let slotPara = try #require(try getNodeByKeyOrThrow(slotParaKey) as? ParagraphNode)
        #expect(try slotPara.getParent()?.getKey() == slotRootKey)
        #expect(try getComputedStyleForParent(slotPara) != nil)
      })
  }
}
