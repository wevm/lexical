import Lexical
import LexicalExtension
import LexicalLink
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts.

private final class ExcludedParentNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ExcludedParentNode.self, type: "excluded-parent", extends: ElementNode.self)
  }
}

private func isExcludedParentNode(_ node: LexicalNode?) -> Bool { node is ExcludedParentNode }

private let TestLexicalAutoLinkExtension = defineExtension(
  name: "[test root]", dependencies: [AutoLinkExtension],
  nodes: { [.klass(ExcludedParentNode.self)] },
  editorConfig: InitialEditorConfig(
    theme: [
      "link": "my-autolink-class",
      "text": [
        "bold": "my-bold-class",
        "code": "my-code-class",
        "hashtag": "my-hashtag-class",
        "italic": "my-italic-class",
        "strikethrough": "my-strikethrough-class",
        "underline": "my-underline-class",
        "underlineStrikethrough": "my-underline-strikethrough-class",
      ],
    ]))

nonisolated(unsafe) private let URL_MATCHER = createLinkMatcherWithRegExp(
  try! Regex(#"https?:\/\/[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}"#))
nonisolated(unsafe) private let PORT_URL_MATCHER = createLinkMatcherWithRegExp(
  try! Regex(#"https?:\/\/[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}:\d+"#))
nonisolated(unsafe) private let GH_TAG_MATCHER = createLinkMatcherWithRegExp(
  try! Regex(#"GH-\d+"#))

private func buildEditor(
  _ autoLink: ConfiguredExtension<AutoLinkConfig, Void, Void>,
  _ initialEditorState: @escaping () throws -> Void
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "[test override]", dependencies: [TestLexicalAutoLinkExtension, autoLink],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in try initialEditorState() })
    ))
}

private func paragraphWithText(_ text: String) throws {
  try getRoot().append(createParagraphNode().append(createTextNode(text)))
}

// Every attribute in LinkAttributes is derived from the matched url, so
// re-running the matcher on edited text must refresh all of them.
nonisolated(unsafe) private let ATTRIBUTE_MATCHER: LinkMatcher = { text in
  guard let match = text.firstMatch(of: #/https?:\/\/[a-z]+\.com/#) else { return nil }
  let matched = String(match.0)
  let host = matched.replacing("https://", with: "", maxReplacements: 1)
  return LinkMatcherResult(
    attributes: AutoLinkAttributes(
      rel: "rel-\(host)", target: "target-\(host)", title: "title-\(host)"),
    index: text.utf16.distance(from: text.startIndex, to: match.range.lowerBound),
    length: matched.utf16.count, text: matched, url: matched)
}

private func buildAttributesEditor(changeHandlers: [ChangeHandler]? = nil) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: changeHandlers == nil ? "[test attributes]" : "[test change handler]",
      dependencies: [
        TestLexicalAutoLinkExtension,
        configExtension(AutoLinkExtension) {
          if let changeHandlers { $0.changeHandlers = changeHandlers }
          $0.matchers = [ATTRIBUTE_MATCHER]
        },
      ],
      editorConfig: InitialEditorConfig(
        initialEditorState: .update { _ in try paragraphWithText("https://a.com") })))
}

private func getAutoLink() throws -> AutoLinkNode {
  let paragraph = try #require(try getRoot().getFirstChild() as? ParagraphNode)
  return try #require(try paragraph.getFirstChild() as? AutoLinkNode)
}

private func retypeAsB(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let textNode = try #require(try getAutoLink().getFirstChild() as? TextNode)
    try textNode.setTextContent("https://b.com")
  }
}

@Suite struct LexicalAutoLinkExtensionTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > registerAutoLink does not cause infinite transform loop with #1234.Another"
    ))
  func noInfiniteTransformLoop() throws {
    // Register AutoLink with a hashtag matcher that matches # followed by digits
    let hashtagMatcher = createLinkMatcherWithRegExp(try Regex(#"#\d+"#))
    let editor = try buildEditor(
      configExtension(AutoLinkExtension) { $0.matchers = [hashtagMatcher] }
    ) {
      // Initialize content with #1234.Another - this should not cause an infinite loop
      try paragraphWithText("#1234.Another")
    }
    defer { editor.dispose() }

    // Verify content is correct and that #1234 was converted to an AutoLinkNode
    try editor.read { () throws -> Void in
      let paragraphNode = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "first root child must be a ParagraphNode")
      #expect(try paragraphNode.getTextContent() == "#1234.Another")

      // Verify that #1234 was converted to an AutoLinkNode
      let autoLinkNode = try #require(
        try paragraphNode.getFirstChild() as? AutoLinkNode, "first child must be an AutoLinkNode")

      // The AutoLinkNode should contain "#1234" only (the matched portion)
      #expect(try autoLinkNode.getTextContent() == "#1234")

      // Verify that ".Another" is separate text after the link (unmatched portion)
      let nextSibling = try #require(
        try autoLinkNode.getNextSibling() as? TextNode, "next sibling must be a TextNode")
      #expect(try nextSibling.getTextContent() == ".Another")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > excludeParents prevents auto-linking inside excluded parent nodes"
    ))
  func excludeParentsPrevents() throws {
    let editor = try buildEditor(
      configExtension(AutoLinkExtension) {
        $0.excludeParents = [isExcludedParentNode]
        $0.matchers = [URL_MATCHER]
      }
    ) {
      try getRoot().append(
        create(ExcludedParentNode.self).append(createTextNode("https://example.com")))
    }
    defer { editor.dispose() }

    try editor.read { () throws -> Void in
      let excludedParent = try #require(
        try getRoot().getFirstChild() as? ExcludedParentNode,
        "first root child must be an ExcludedParentNode")
      let child = try #require(
        try excludedParent.getFirstChild() as? TextNode, "child must be a TextNode")
      #expect(try child.getTextContent() == "https://example.com")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > excludeParents does not prevent auto-linking in non-excluded parents"
    ))
  func excludeParentsAllowsOthers() throws {
    let editor = try buildEditor(
      configExtension(AutoLinkExtension) {
        $0.excludeParents = [isExcludedParentNode]
        $0.matchers = [URL_MATCHER]
      }
    ) { try paragraphWithText("https://example.com") }
    defer { editor.dispose() }

    try editor.read { () throws -> Void in
      let paragraphNode = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "first root child must be a ParagraphNode")
      let autoLinkNode = try #require(
        try paragraphNode.getFirstChild() as? AutoLinkNode, "first child must be an AutoLinkNode")
      #expect(try autoLinkNode.getTextContent() == "https://example.com")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > default separator regex does not treat colon as an auto-link boundary"
    ))
  func defaultSeparatorIgnoresColon() throws {
    let editor = try buildEditor(
      configExtension(AutoLinkExtension) { $0.matchers = [GH_TAG_MATCHER] }
    ) { try paragraphWithText("GH-123: investigate") }
    defer { editor.dispose() }

    try editor.read { () throws -> Void in
      let paragraphNode = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "first root child must be a ParagraphNode")
      let firstChild = try #require(
        try paragraphNode.getFirstChild() as? TextNode, "first child must be a TextNode")
      #expect(try firstChild.getTextContent() == "GH-123: investigate")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > custom separator regex can treat colon as an auto-link boundary"
    ))
  func customSeparatorColon() throws {
    let editor = try buildEditor(
      configExtension(AutoLinkExtension) {
        $0.matchers = [GH_TAG_MATCHER]
        $0.separatorRegex = try! Regex(#"[.,;:\s]"#)
      }
    ) { try paragraphWithText("GH-123: investigate") }
    defer { editor.dispose() }

    try editor.read { () throws -> Void in
      let paragraphNode = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "first root child must be a ParagraphNode")
      let autoLinkNode = try #require(
        try paragraphNode.getFirstChild() as? AutoLinkNode, "first child must be an AutoLinkNode")
      #expect(try autoLinkNode.getTextContent() == "GH-123")

      let nextSibling = try #require(
        try autoLinkNode.getNextSibling() as? TextNode, "next sibling must be a TextNode")
      #expect(try nextSibling.getTextContent() == ": investigate")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > custom separator regex does not break auto-linking URLs with embedded colons"
    ))
  func customSeparatorEmbeddedColon() throws {
    let editor = try buildEditor(
      configExtension(AutoLinkExtension) {
        $0.matchers = [PORT_URL_MATCHER]
        $0.separatorRegex = try! Regex(#"[.,;:\s]"#)
      }
    ) { try paragraphWithText("https://example.com:3000 ") }
    defer { editor.dispose() }

    try editor.read { () throws -> Void in
      let paragraphNode = try #require(
        try getRoot().getFirstChild() as? ParagraphNode, "first root child must be a ParagraphNode")
      let autoLinkNode = try #require(
        try paragraphNode.getFirstChild() as? AutoLinkNode, "first child must be an AutoLinkNode")
      #expect(try autoLinkNode.getTextContent() == "https://example.com:3000")

      let nextSibling = try #require(
        try autoLinkNode.getNextSibling() as? TextNode, "next sibling must be a TextNode")
      #expect(try nextSibling.getTextContent() == " ")
    }
  }

  @Suite struct HandleLinkEditKeepsTheMatcherAttributesInSync {
    @Test(
      .upstream(
        "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > the creation path applies every attribute"
      ))
    func creationPathAppliesEveryAttribute() throws {
      let editor = try buildAttributesEditor()
      defer { editor.dispose() }
      try editor.read { () throws -> Void in
        let autoLink = try getAutoLink()
        #expect(try autoLink.getURL() == "https://a.com")
        #expect(try autoLink.getRel() == "rel-a.com")
        #expect(try autoLink.getTarget() == "target-a.com")
        #expect(try autoLink.getTitle() == "title-a.com")
      }
    }

    @Test(
      .upstream(
        "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > editing the link text refreshes rel and target"
      ))
    func editingRefreshesRelAndTarget() throws {
      let editor = try buildAttributesEditor()
      defer { editor.dispose() }
      try retypeAsB(editor)
      try editor.read { () throws -> Void in
        let autoLink = try getAutoLink()
        #expect(try autoLink.getURL() == "https://b.com")
        #expect(try autoLink.getRel() == "rel-b.com")
        #expect(try autoLink.getTarget() == "target-b.com")
      }
    }

    @Test(
      .upstream(
        "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > editing the link text refreshes the title"
      ))
    func editingRefreshesTitle() throws {
      let editor = try buildAttributesEditor()
      defer { editor.dispose() }
      try retypeAsB(editor)
      try editor.read { () throws -> Void in
        #expect(try getAutoLink().getTitle() == "title-b.com")
      }
    }

    @Test(
      .upstream(
        "lexical-link/src/__tests__/unit/LexicalAutoLinkExtension.test.ts::LexicalAutoLinkExtension tests > handleLinkEdit keeps the matcher attributes in sync > an attribute refresh is not reported as a url change"
      ))
    func attributeRefreshIsNotAURLChange() throws {
      var changes: [[String?]] = []
      let editor = try buildAttributesEditor(changeHandlers: [
        { url, prevUrl in
          changes.append([url, prevUrl])
        }
      ])
      defer { editor.dispose() }
      changes.removeAll()
      try retypeAsB(editor)

      // ChangeHandler is documented as (url, prevUrl). Every reported pair must
      // be a url, never a rel/target/title carried over from the same edit.
      #expect(changes == [["https://b.com", "https://a.com"]])
    }
  }
}
