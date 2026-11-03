import Lexical
import LexicalExtension
import LexicalHTML
import LexicalHeadless
import LexicalRichText
import LexicalTesting
import Testing

// Ports of packages/lexical-headless/src/__tests__/unit
// LexicalHeadlessEditor.test.ts and HeadlessExtension.test.ts.

private let helloWorldJSON =
  #"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"hello world","type":"text","version":1}],"direction":"ltr","format":"","indent":0,"type":"paragraph","version":1}],"direction":"ltr","format":"","indent":0,"type":"root","version":1}}"#

private func headlessEditor() throws -> Editor {
  try createHeadlessEditor(CreateEditorArgs(namespace: "", onError: { throw $0 }))
}

/// `assertEditorState`: the node map's values, in insertion order, each
/// matching the expected fields.
private func nodeSummary(_ editorState: EditorState) -> [String] {
  editorState._nodeMap.valuesArray.map { node in
    if let text = node as? TextNode {
      return "text:\(text.__format):\(text.__text)"
    }
    return node.__key == "root" ? "root" : node.__type
  }
}

@Suite struct LexicalHeadlessEditorTests {
  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can update editor"
    ))
  func canUpdate() throws {
    let editor = try headlessEditor()
    defer { editor.dispose() }
    try update(editor) {
      try getRoot().append(
        try createParagraphNode().append(
          try createTextNode("Hello").toggleFormat("bold"), try createTextNode("world")))
    }
    #expect(
      nodeSummary(editor.getEditorState()) == ["root", "paragraph", "text:1:Hello", "text:0:world"])
  }

  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can set editor state from json"
    ))
  func setFromJSON() throws {
    let editor = try headlessEditor()
    defer { editor.dispose() }
    try editor.setEditorState(
      try editor.parseEditorState(
        #"{"root":{"children":[{"children":[{"detail":0,"format":1,"mode":"normal","style":"","text":"Hello","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":"world","type":"text","version":1}],"direction":"ltr","format":"","indent":0,"type":"paragraph","version":1}],"direction":"ltr","format":"","indent":0,"type":"root","version":1}}"#
      ))
    #expect(
      nodeSummary(editor.getEditorState()) == ["root", "paragraph", "text:1:Hello", "text:0:world"])
  }

  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can register listeners"
    ))
  func registerListeners() throws {
    let editor = try headlessEditor()
    defer { editor.dispose() }
    var updates = 0
    var commands: [String] = []
    var transformed: [String] = []
    var textContents: [String] = []
    editor.registerUpdateListener { _ in updates += 1 }
    editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { payload, _ in
        commands.append(payload)
        return false
      }, COMMAND_PRIORITY_NORMAL)
    try editor.registerNodeTransform(ParagraphNode.self) { transformed.append($0.__type) }
    editor.registerTextContentListener { textContents.append($0) }
    try update(editor) {
      try getRoot().append(
        try createParagraphNode().append(
          try createTextNode("Hello").toggleFormat("bold"), try createTextNode("world")))
      _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "foo")
    }
    #expect(updates > 0)
    #expect(commands == ["foo"])
    #expect(transformed.contains("paragraph"))
    #expect(textContents.last == "Helloworld")
  }

  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can preserve selection for pending editor state (within update loop)"
    ))
  func preservesSelection() throws {
    let editor = try headlessEditor()
    defer { editor.dispose() }
    try update(editor) {
      let textNode = try createTextNode("Hello world")
      try getRoot().append(try createParagraphNode().append(textNode))
      try textNode.select(1, 2)
    }
    try update(editor) {
      let selection = try #require(try getSelection() as? RangeSelection)
      #expect(selection.anchor.offset == 1 && selection.anchor.type == .text)
      #expect(selection.focus.offset == 2 && selection.focus.type == .text)
    }
  }

  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/LexicalHeadlessEditor.test.ts::LexicalHeadlessEditor > can generate html from the nodes when dom is set"
    ))
  func generatesHTML() throws {
    let editor = try headlessEditor()
    defer { editor.dispose() }
    try editor.setEditorState(try editor.parseEditorState(helloWorldJSON))
    let html = try editor.read(.latest) { try generateHtmlFromNodes(editor, nil) }
    #expect(html == #"<p dir="ltr"><span style="white-space: pre-wrap;">hello world</span></p>"#)
  }

  @Test func refusesRootElement() throws {
    let editor = try headlessEditor()
    defer { editor.dispose() }
    #expect(throws: LexicalError("setRootElement is not supported in headless mode")) {
      try editor.setRootElement(TestRootElement())
    }
  }
}

private func buildHeadlessEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "[root]", dependencies: [HeadlessExtension, RichTextExtension]))
}

@Suite struct HeadlessExtensionTests {
  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > marks the editor headless and still runs updates and listeners"
    ))
  func marksHeadless() throws {
    let editor = try buildHeadlessEditor()
    defer { editor.dispose() }
    #expect(editor._headless)
    var last: String?
    editor.registerUpdateListener { payload in
      last = try payload.editorState.read { try getRoot().getTextContent() }
    }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        try createParagraphNode().append(try createTextNode("Hello")))
    }
    #expect(last == "Hello")
    #expect(try editor.read { try getRoot().getTextContent() } == "Hello")
  }

  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > refuses a root element but allows setting it to null"
    ))
  func refusesRoot() throws {
    let editor = try buildHeadlessEditor()
    defer { editor.dispose() }
    #expect(
      throws: LexicalError("HeadlessExtension: setRootElement is not supported in headless mode")
    ) { try editor.setRootElement(TestRootElement()) }
    #expect(editor.getRootElement() == nil)
    #expect(throws: Never.self) { try editor.setRootElement(nil) }
  }

  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > keeps listener registration working for other extensions"
    ))
  func rootListener() throws {
    let editor = try buildHeadlessEditor()
    defer { editor.dispose() }
    var calls: [(AnyObject?, AnyObject?)] = []
    let unregister = editor.registerRootListener { root, prev in
      calls.append((root, prev))
      return nil
    }
    #expect(calls.count == 1 && calls[0].0 == nil && calls[0].1 == nil)
    unregister()
  }

  @Test(
    .upstream(
      "lexical-headless/src/__tests__/unit/HeadlessExtension.test.ts::HeadlessExtension > makes nested editors headless"
    ))
  func nestedHeadless() throws {
    let editor = try buildHeadlessEditor()
    defer { editor.dispose() }
    let nested = try createEditor(CreateEditorArgs(parentEditor: editor))
    #expect(nested._headless)
  }
}
