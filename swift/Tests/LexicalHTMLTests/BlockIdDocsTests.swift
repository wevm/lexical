import Lexical
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalList
import LexicalMark
import LexicalRichText
import Testing

// Port of packages/lexical-html/src/__tests__/unit/BlockIdDocs.test.ts: the
// code examples of the Block IDs documentation page
// (packages/lexical-website/docs/extensions/block-ids.md). The examples that
// need marks or tables are skipped until those packages are ported.

private struct Op {
  enum Kind { case replace, insertAfter, delete }
  let op: Kind
  let id: String
  let text: String
}

private func paragraph(_ text: String) throws -> ElementNode {
  try createParagraphNode().append(createTextNode(text))
}

/// Applying what the agent answers, as on the page: one update per
/// operation.
private func applyOps(_ editor: Editor, _ ops: [Op]) throws {
  for op in ops {
    try editor.update(discrete: true) {
      guard let block = try getBlockById(op.id) else {
        return  // Deleted meanwhile, or a mistyped id: tell the agent.
      }
      if op.op == .delete {
        try block.remove()
        return
      }
      let node = try paragraph(op.text)
      if op.op == .replace {
        try block.replace(node)  // Keeps the id.
        // A selection that ended inside the replaced block lost that end:
        // move it to the new block.
        if let selection = try getSelection() as? RangeSelection,
          try [selection.anchor, selection.focus].contains(where: { try !$0.getNode().isAttached() }
          )
        {
          _ = try node.selectEnd()
        }
      } else {
        try block.insertAfter(node)  // Gets a fresh id.
      }
    }
  }
}

private func editorJSON(_ editor: Editor) throws -> String {
  try editor.getEditorState().toJSON().stringify()
}

private func seedSevenEightNine(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    _ = try getRoot().clear().append(contentsOf: ["seven", "eight", "nine"].map(paragraph))
  }
}

private final class EmptyInlineNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      EmptyInlineNode.self, type: "block-id-docs-empty-inline", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { true }
}

nonisolated(unsafe) private let EmptyInlineExtension = defineExtension(
  name: "block-id-docs/empty-inline", nodes: { [.klass(EmptyInlineNode.self)] })

@Suite struct BlockIdDocsTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Setup and Addressing blocks"
    ))
  func setupAndAddressingBlocks() throws {
    // ## Setup
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "my-editor", dependencies: [RichTextExtension, BlockIdExtension]))
    defer { editor.dispose() }
    try seedSevenEightNine(editor)

    // 1. What the agent reads (here, the top-level blocks).
    let ids = try editor.read {
      try getRoot().getChildren().map { try #require(try getBlockId($0)) }
    }
    let (b7, b8, b9) = (ids[0], ids[1], ids[2])
    // A person's caret sits inside the block the agent rewrites.
    try editor.update(discrete: true) { _ = try getRoot().getAllTextNodes()[0].select(1, 1) }

    // 2. What the agent answers.
    try applyOps(
      editor,
      [
        Op(op: .replace, id: b7, text: "New text"),
        Op(op: .insertAfter, id: b7, text: "Another paragraph"),
        Op(op: .delete, id: b9, text: ""),
      ])
    let after = try editor.read {
      try getRoot().getChildren().map { (try getBlockId($0), try $0.getTextContent()) }
    }
    #expect(after.map(\.1) == ["New text", "Another paragraph", "eight"])
    #expect(after[0].0 == b7)
    #expect(after[2].0 == b8)
    #expect(![b7, b8, b9].contains(after[1].0))
    try editor.read { () throws in #expect(try getBlockById(b9) == nil) }

    // Why one update per operation: within one update, a block replaced
    // earlier is not found yet, and a block inserted where a removed block
    // sat takes the removed block's id.
    try editor.update(discrete: true) {
      let again = try paragraph("Again")
      try #require(try getBlockById(b7)).replace(again)
      _ = try again.selectEnd()  // The caret was in b7.
      #expect(try getBlockById(b7) == nil)
      try #require(try getBlockById(b8)).remove()
      try getRoot().append(paragraph("Appended"))
    }
    try editor.read { () throws in
      #expect(try getBlockById(b7)?.getTextContent() == "Again")
      #expect(try getBlockById(b8)?.getTextContent() == "Appended")
    }

    // $setState(node, blockIdState, 'b7-intro')
    try editor.update(discrete: true) {
      let node = try createParagraphNode()
      try setState(node, blockIdState, "b7-intro")
      try getRoot().append(node)
    }
    try editor.read { () throws in #expect(try getBlockById("b7-intro") != nil) }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Addressing blocks: a selection from or into the replaced block"
    ))
  func selectionIntoTheReplacedBlock() throws {
    for (from, to) in [(0, 1), (1, 2), (2, 1), (1, 1)] {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "my-editor", dependencies: [RichTextExtension, BlockIdExtension]))
      defer { editor.dispose() }
      try seedSevenEightNine(editor)
      let ids = try editor.read {
        try getRoot().getChildren().map { try #require(try getBlockId($0)) }
      }
      // The person's selection runs from block `from` to block `to`, and
      // the agent replaces the second block.
      try editor.update(discrete: true) {
        let texts = try getRoot().getAllTextNodes()
        try texts[from].select(2, 2).focus.set(texts[to].getKey(), 3, .text)
      }
      try applyOps(editor, [Op(op: .replace, id: ids[1], text: "New text")])
      try editor.read { () throws in
        let blocks = try getRoot().getChildren().map {
          "\(try getBlockId($0) ?? "null"):\(try $0.getTextContent())"
        }
        #expect(
          blocks == ["\(ids[0]):seven", "\(ids[1]):New text", "\(ids[2]):nine"], "\(from) to \(to)")
        let selection = try #require(try getSelection() as? RangeSelection)
        for point in [selection.anchor, selection.focus] {
          #expect(try point.getNode().isAttached())
        }
      }
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Headless"
    ))
  func headless() throws {
    let writer = try buildEditorFromExtensions(
      defineExtension(
        name: "writer", dependencies: [RichTextExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in try getRoot().append(paragraph("Intro")) })))
    defer { writer.dispose() }
    try writer.read {}
    let storedJSON = try editorJSON(writer)

    let pipeline = try buildEditorFromExtensions(
      defineExtension(
        name: "docs-pipeline", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { pipeline.dispose() }
    try pipeline.setEditorState(pipeline.parseEditorState(storedJSON))
    let withIds = try editorJSON(pipeline)
    #expect(try withIds.firstMatch(of: Regex(#""blockId":"[A-Za-z0-9_-]{21}""#)) != nil)
    // Running it again changes nothing.
    try pipeline.setEditorState(pipeline.parseEditorState(withIds))
    #expect(try editorJSON(pipeline) == withIds)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > HTML and Configuration"
    ))
  func htmlAndConfiguration() throws {
    var n = 0
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "html",
        dependencies: [
          RichTextExtension,
          configExtension(BlockIdExtension) {
            $0.generateId = {
              n += 1
              return "b\(n)"
            }
          },
          configExtension(BlockIdHTMLExtension) { $0.export = false },
        ]))
    defer { editor.dispose() }
    // The first content of an empty editor is a load (positional ids);
    // later blocks get ids from generateId.
    for _ in 0..<2 {
      try editor.update(discrete: true) { try getRoot().append(createParagraphNode()) }
    }
    try editor.read { () throws in
      #expect(try getBlockId(getRoot().getLastChildOrThrow()) == "b1")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model"
    ))
  func showingBlocksToAModel() throws {
    let ids = ["V1StGXR8_Z5jdHi6B-myT", "9xQeLm2Pq0aK7cTb_wYuA"]
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "my-editor", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            for (i, text) in ["Q3 plan", "Revenue grew 4%."].enumerated() {
              let node = try paragraph(text)
              try setState(node, blockIdState, ids[i])
              try getRoot().append(node)
            }
          })))
    defer { editor.dispose() }

    // 1. What the model reads, and the handles it is shown (id → handle).
    let (blocks, shown) = try editor.read {
      (
        try getRoot().getChildren().map {
          "\(try getBlockHandle($0) ?? "null"):\(try $0.getTextContent())"
        }, try getBlockHandles()
      )
    }
    #expect(blocks == ["2bNo:Q3 plan", "gTtF:Revenue grew 4%."])
    let keyBefore = try editor.read { try #require(try getBlockById(ids[1])).getKey() }

    // 2. What the model answers, then 3. applying it, in its own update.
    var feedback: [String] = []
    func apply(_ block: String, _ text: String) throws {
      try editor.update(discrete: true) {
        switch try resolveBlockHandle(block, BlockHandleOptions(read: shown)) {
        case .found(let node):
          if try !setBlockText(node, text) {
            feedback.append("\(block) holds line breaks or other blocks: edit its parts.")
          }
        case .notFound(let handle):
          feedback.append("No block \(handle): it may have been deleted.")
        case .ambiguous(let handle, let candidates):
          feedback.append(
            "\(handle) names several blocks: use one of \(candidates.joined(separator: ", ")).")
        case .invalid(let handle, let reason):
          feedback.append(
            reason == .tooShort
              ? "\(handle) is too short: block handles have at least 4 characters."
              : "\(handle) is not a block handle.")
        }
      }
    }
    try apply("gTtF", "Revenue grew 6%.")
    try editor.read { () throws in
      let block = try #require(try getBlockById(ids[1]))
      #expect(try block.getTextContent() == "Revenue grew 6%.")
      // Changed in place: the same block, not a new one.
      #expect(block.getKey() == keyBefore)
    }
    try apply("gTtG", "typo")
    try apply("gTt", "too short")
    try apply("gTtF!", "invalid")
    // A collaborator adds a block whose handle would start with 2bNo: the
    // handle still names the block the model read.
    try editor.update(discrete: true) {
      let node = try createParagraphNode()
      try setState(node, blockIdState, "qfapyr")
      try getRoot().append(node)
    }
    try apply("2bNo", "Q4 plan")
    // And deletes the other one.
    try editor.update(discrete: true) { try #require(try getBlockById(ids[1])).remove() }
    try apply("gTtF", "Revenue grew 7%.")
    // A paragraph with a line break is not changed in place.
    try editor.update(discrete: true) {
      try #require(try getBlockById(ids[0]) as? ElementNode).append(createLineBreakNode())
    }
    try apply("2bNo", "Q5 plan")
    #expect(
      feedback == [
        "No block gTtG: it may have been deleted.",
        "gTt is too short: block handles have at least 4 characters.",
        "gTtF! is not a block handle.",
        "No block gTtF: it may have been deleted.",
        "2bNo holds line breaks or other blocks: edit its parts.",
      ])
    try editor.read { () throws in
      #expect(try getBlockById(ids[0])?.getTextContent() == "Q4 plan\n")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText refuses blocks that are not just text"
    ))
  func setBlockTextRefusesBlocksNotJustText() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "set-block-text-refuses",
        dependencies: [RichTextExtension, ListExtension, BlockIdExtension, EmptyInlineExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      func item(_ text: String) throws -> ListItemNode {
        try createListItemNode().append(createTextNode(text))
      }
      _ = try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("a"), createLineBreakNode(), createTextNode("b")),
        createListNode("bullet").append(item("one"), item("two")),
        // One item: the list's text is its item's text.
        createListNode("number").append(item("one")),
        // An inline decorator without text.
        createParagraphNode().append(
          createTextNode("left"), EmptyInlineNode(), createTextNode("right")))
    }
    let before = try editorJSON(editor)
    try editor.update(discrete: true) {
      let children = try getRoot().getChildren()
      let (lineBreak, list, oneItem, decorated) = (
        children[0], children[1], children[2], children[3]
      )
      #expect(try !setBlockText(lineBreak, "a\nb"))
      #expect(try !setBlockText(lineBreak, ""))
      #expect(try !setBlockText(list, "one\n\ntwo"))
      #expect(try oneItem.getTextContent() == "one")
      #expect(try !setBlockText(oneItem, "rewritten"))
      #expect(try decorated.getTextContent() == "leftright")
      #expect(try !setBlockText(decorated, "changed"))
    }
    // Nothing changed.
    #expect(try editorJSON(editor) == before)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText refuses a block or inline element that hosts a slot"
    ))
  func setBlockTextRefusesSlotHosts() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "set-block-text-slots",
        dependencies: [RichTextExtension, LinkExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      // A text-only block whose caption slot holds a line break.
      let host = try createParagraphNode().append(createTextNode("body"))
      try setSlot(
        host, "caption",
        createParagraphNode().append(
          createTextNode("a"), createLineBreakNode(), createTextNode("b")))
      // A link, inline, that hosts a slot.
      let link = try createLinkNode("https://example.com").append(createTextNode("link"))
      try setSlot(link, "note", createParagraphNode().append(createTextNode("note")))
      _ = try getRoot().clear().append(
        host, createParagraphNode().append(createTextNode("see "), link))
    }
    let before = try editorJSON(editor)
    try editor.update(discrete: true) {
      let children = try getRoot().getChildren()
      let (host, withLink) = (children[0], children[1])
      #expect(try !setBlockText(host, host.getTextContent()))
      #expect(try !setBlockText(host, "changed"))
      #expect(try !setBlockText(withLink, withLink.getTextContent()))
      #expect(try !setBlockText(withLink, "see changed"))
    }
    #expect(try editorJSON(editor) == before)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText refuses special text nodes"
    ))
  func setBlockTextRefusesSpecialTextNodes() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "set-block-text-special", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try getRoot().clear().append(
        // A tab: TabNode.spliceText throws for anything but a no-op.
        createParagraphNode().append(createTextNode("a"), createTabNode(), createTextNode("b")),
        // A token, which edits replace whole.
        createParagraphNode().append(
          createTextNode("see "), createTextNode("@Ada").setMode("token")))
    }
    let before = try editorJSON(editor)
    try editor.update(discrete: true) {
      let children = try getRoot().getChildren()
      let (tab, token) = (children[0], children[1])
      #expect(try !setBlockText(tab, "a\tb"))
      #expect(try !setBlockText(tab, "a\tbc"))
      #expect(try !setBlockText(tab, "ab"))
      #expect(try !setBlockText(token, "see @Bob"))
    }
    #expect(try editorJSON(editor) == before)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: text added at the edge of a link in a mark goes outside both"
    ))
  func textAddedAtTheEdgeOfALinkInAMark() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "set-block-text-mark-link",
        dependencies: [RichTextExtension, LinkExtension, MarkExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    func children() throws -> [String] {
      try editor.read {
        let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
        return try paragraph.getChildren().map {
          "\($0 is MarkNode ? "mark" : "text"):\(try $0.getTextContent())"
        }
      }
    }
    for (next, after) in [
      ("See plan!", ["text:See ", "mark:plan", "text:!"]),
      ("See the plan", ["text:See the ", "mark:plan"]),
    ] {
      try editor.update(discrete: true) {
        _ = try getRoot().clear().append(
          createParagraphNode().append(
            createTextNode("See "),
            createMarkNode(["comment"]).append(
              createLinkNode("https://example.com").append(createTextNode("plan")))))
      }
      try editor.update(discrete: true) { () throws -> Void in
        #expect(try setBlockText(getRoot().getFirstChildOrThrow(), next))
      }
      #expect(try children() == after, "\(next)")
    }
    // At the start of the block: before both.
    try editor.update(discrete: true) {
      _ = try getRoot().clear().append(
        createParagraphNode().append(
          createMarkNode(["comment"]).append(
            createLinkNode("https://example.com").append(createTextNode("Plan"))),
          createTextNode(" is ready.")))
    }
    try editor.update(discrete: true) { () throws -> Void in
      #expect(try setBlockText(getRoot().getFirstChildOrThrow(), "The Plan is ready."))
    }
    #expect(try children() == ["text:The ", "mark:Plan", "text: is ready."])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: text added at a link edge goes outside the link"
    ))
  func textAddedAtLinkEdgeGoesOutside() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "set-block-text-link-edges",
        dependencies: [RichTextExtension, LinkExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    // [links: before, new text, children after: [type, text]]
    let cases: [(String, String, [[String]])] = [
      (
        "mid", "See the plans for Q3.",
        [["text", "See "], ["link", "the plan"], ["text", "s for Q3."]]
      ),
      (
        "mid", "See all the plan for Q3.",
        [["text", "See all "], ["link", "the plan"], ["text", " for Q3."]]
      ),
      (
        "first", "Read the plan for Q3.",
        [["text", "Read "], ["link", "the plan"], ["text", " for Q3."]]
      ),
      ("last", "See the plan!", [["text", "See "], ["link", "the plan"], ["text", "!"]]),
      // Inside the link, it stays in the link.
      (
        "mid", "See the new plan for Q3.",
        [["text", "See "], ["link", "the new plan"], ["text", " for Q3."]]
      ),
    ]
    for (place, next, after) in cases {
      try editor.update(discrete: true) {
        let link = try createLinkNode("https://example.com").append(createTextNode("the plan"))
        let paragraph = try createParagraphNode()
        if place == "first" {
          try paragraph.append(link, createTextNode(" for Q3."))
        } else if place == "last" {
          try paragraph.append(createTextNode("See "), link)
        } else {
          try paragraph.append(createTextNode("See "), link, createTextNode(" for Q3."))
        }
        _ = try getRoot().clear().append(paragraph)
      }
      try editor.update(discrete: true) { () throws -> Void in
        #expect(try setBlockText(getRoot().getFirstChildOrThrow(), next))
      }
      try editor.read { () throws -> Void in
        let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
        #expect(
          try paragraph.getChildren().map {
            [isLinkNode($0) ? "link" : "text", try $0.getTextContent()]
          }
            == after, "\(place) → \(next)")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText edits text in inline elements"
    ))
  func setBlockTextEditsInlineElements() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "set-block-text-link",
        dependencies: [RichTextExtension, LinkExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("See "),
          createLinkNode("https://example.com").append(createTextNode("the plan")),
          createTextNode(" for Q3.")))
    }
    for text in ["See the new plan for Q3.", "See the new plan for Q4."] {
      try editor.update(discrete: true) { () throws -> Void in
        #expect(try setBlockText(getRoot().getFirstChildOrThrow(), text))
      }
    }
    try editor.read { () throws -> Void in
      let paragraph = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
      #expect(try paragraph.getTextContent() == "See the new plan for Q4.")
      let link = try #require(try paragraph.getChildren()[1] as? LinkNode)
      #expect(try link.getTextContent() == "the new plan")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Showing blocks to a model: $setBlockText edits only what differs"
    ))
  func setBlockTextEditsOnlyWhatDiffers() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "set-block-text", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    // [text nodes before, new text, text nodes after]
    let cases: [([String], String, [String])] = [
      (["Revenue grew 4%."], "Revenue grew 6%.", ["Revenue grew 6%."]),
      (["Revenue ", "grew", " 4%."], "Revenue fell 4%.", ["Revenue ", "fell", " 4%."]),
      (["Revenue ", "grew", " 4%."], "Revenue 4%.", ["Revenue 4%."]),
      (["ab", "cd", "ef"], "aXf", ["aXf"]),
      (["ab", "cd"], "abXcd", ["abX", "cd"]),
      (["ab", "cd"], "abd", ["ab", "d"]),
      (["abc"], "abc", ["abc"]),
      (["abc"], "", []),
      (["abc"], "abcdef", ["abcdef"]),
      (["abc"], "Xabc", ["Xabc"]),
      ([], "new", ["new"]),
    ]
    for (before, next, after) in cases {
      try editor.update(discrete: true) {
        let block = try createParagraphNode()
        for (i, text) in before.enumerated() {
          // Alternate formats, so that Lexical keeps the nodes apart.
          try block.append(createTextNode(text).setFormat(i % 2 == 1 ? IS_BOLD : 0))
        }
        _ = try getRoot().clear().append(block)
      }
      let key = try editor.read { try getRoot().getFirstChildOrThrow().getKey() }
      try editor.update(discrete: true) { try setBlockText(getRoot().getFirstChildOrThrow(), next) }
      try editor.read { () throws in
        let block = try #require(try getRoot().getFirstChildOrThrow() as? ElementNode)
        #expect(block.getKey() == key, "\(before)")
        #expect(
          try block.getAllTextNodes().map { try $0.getTextContent() } == after,
          "\(before) → \(next)")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Handles and collaboration: a block added since whose hash starts the same way"
    ))
  func handlesAndCollaboration() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "my-editor", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            let node = try paragraph("shown")
            try setState(node, blockIdState, "p13e")
            try getRoot().append(node)
          })))
    defer { editor.dispose() }
    let shown = try editor.read { try getBlockHandles() }
    #expect(shown.get("p13e") == "0ydd")
    try editor.update(discrete: true) {
      try #require(try getBlockById("p13e")).remove()
      // Its hash starts with 0ydd too.
      let node = try paragraph("new")
      try setState(node, blockIdState, "p20d")
      try getRoot().append(node)
    }
    try editor.read { () throws in
      // Without the read, the block added since takes the handle.
      let now = try resolveBlockHandle("0ydd")
      #expect(try now.node.map(getBlockId) == "p20d")
      // With it, the block the model read is gone.
      #expect(try resolveBlockHandle("0ydd", BlockHandleOptions(read: shown)).status == "not-found")
    }
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdDocs.test.ts::Block IDs documentation examples > Handles in HTML"
    ))
  func handlesInHTML() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "my-editor",
        dependencies: [
          RichTextExtension, BlockIdExtension,
          configExtension(BlockIdHTMLExtension) { $0.handles = true },
        ]))
    defer { editor.dispose() }
    let output = try getExtensionDependencyFromEditor(editor, BlockIdHTMLExtension).output
    #expect(output.export.value == true)
    #expect(output.handles.value == true)
  }
}
