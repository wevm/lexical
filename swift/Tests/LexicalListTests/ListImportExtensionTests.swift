import Lexical
import LexicalExtension
import LexicalHTML
import LexicalList
import LexicalTesting
import Testing

// Port of packages/lexical-list/src/__tests__/unit/ListImportExtension.test.ts.
// The pasted link case waits for @lexical/link.

private final class CustomListItemNode: ListItemNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(CustomListItemNode.self, type: "custom-listitem", extends: ListItemNode.self)
  }
}

private final class CustomListNode: ListNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(CustomListNode.self, type: "custom-list", extends: ListNode.self)
  }
  override func createListItemNode() throws -> ListItemNode { try CustomListItemNode() }
}

/// ListExtension registers its own import rules (and the shared
/// CoreImportExtension baseline); no dedicated import extension is needed.
private func buildEditor(
  _ dependencies: [any AnyLexicalExtensionArgument] = [ListExtension], name: String = "list-host"
) throws -> Editor {
  try buildEditorFromExtensions(defineExtension(name: name, dependencies: dependencies))
}

private func generate(_ html: String) throws -> [LexicalNode] {
  let dep = try getExtensionDependencyFromEditor(getEditor(), DOMImportExtension)
  let dom = DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
  return try dep.output.generateNodesFromDOM(dom)
}

private func importInto(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) { try getRoot().clear().append(contentsOf: generate(html)) }
}

private func rootList() throws -> ListNode {
  try #require(try getRoot().getFirstChild() as? ListNode, "expected ListNode at root")
}

private func items(_ list: ListNode) throws -> [ListItemNode] {
  try list.getChildren().compactMap { $0 as? ListItemNode }
}

@Suite struct ListImportExtensionTests {
  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > <ul><li>a</li><li>b</li></ul> → bullet list with two items"
    ))
  func bulletList() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<ul><li>a</li><li>b</li></ul>")
    try editor.read { () throws -> Void in
      let list = try rootList()
      #expect(try list.getListType() == "bullet")
      #expect(try items(list).map { try $0.getTextContent() } == ["a", "b"])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > <ol start=\"3\"> → number list with start=3"
    ))
  func numberListStart() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<ol start="3"><li>x</li></ol>"#)
    try editor.read { () throws -> Void in
      let list = try rootList()
      #expect(try list.getListType() == "number")
      #expect(try list.getStart() == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > GitHub task-list-item → checklist item"
    ))
  func githubTaskList() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(
      editor,
      #"<ul class="contains-task-list"><li class="task-list-item"><input type="checkbox" checked/>done</li><li class="task-list-item"><input type="checkbox"/>todo</li></ul>"#
    )
    try editor.read { () throws -> Void in
      let list = try rootList()
      #expect(try list.getListType() == "check")
      #expect(try items(list).map { try $0.getChecked() } == [true, false])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > aria-checked drives checklist state"
    ))
  func ariaChecked() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<ul><li aria-checked="true">a</li></ul>"#)
    try editor.read { () throws -> Void in
      let list = try rootList()
      #expect(try list.getListType() == "check")
      #expect(try items(list)[0].getChecked() == true)
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > stray text inside <ul> gets wrapped via $normalizeListChildren"
    ))
  func strayTextWrapped() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<ul>stray <li>real item</li></ul>")
    try editor.read { () throws -> Void in
      let items = try items(rootList())
      // Both the flattening of the unmatched text and the normalize step
      // produce list items; the real item must survive.
      #expect(items.count >= 1)
      #expect(try items.contains { try $0.getTextContent().contains("real item") })
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListImportExtension > wrapper items come from ListNode.createListItemNode()"
    ))
  func wrapperItemsFromCreateListItemNode() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "custom-list-host", dependencies: [ListExtension],
        nodes: {
          [
            .klass(CustomListNode.self), .klass(CustomListItemNode.self),
            .replacement(
              LexicalNodeReplacement(
                replace: ListNode.self,
                with: { node in
                  let list = node as! ListNode
                  return try CustomListNode(list.getListType(), start: list.getStart())
                }, withKlass: CustomListNode.self)),
          ]
        }))
    defer { editor.dispose() }
    // Both normalizeListChildren branches: the nested <ul> is lifted into a
    // wrapper item, and the stray text is wrapped in one.
    try importInto(editor, "<ul>stray<li>parent<ul><li>child</li></ul></li></ul>")
    try editor.read { () throws -> Void in
      let list = try rootList()
      #expect(list.getType() == "custom-list")
      let wrappers = try items(list).filter { try !$0.getTextContent().hasPrefix("parent") }
      #expect(wrappers.count >= 2)
      for wrapper in wrappers {
        #expect(wrapper.getType() == "custom-listitem")
      }
    }
  }

  // MARK: MS Word paste

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::MS Word paste — preprocess-installed overlay > converts MsoListParagraph runs into nested ListNodes"
    ))
  func wordLists() throws {
    let editor = try buildWordPasteEditor()
    defer { editor.dispose() }
    try importHTMLDocument(editor, WORD_HTML_WITH_LISTS)
    try editor.read { () throws -> Void in
      let lists = try getRoot().getChildren().compactMap { $0 as? ListNode }
      try #require(lists.count == 3)
      let (numbered, bullet, outline) = (lists[0], lists[1], lists[2])
      #expect(try numbered.getListType() == "number")
      #expect(
        try itemTexts(numbered)
          == ["Numbered List Item 1", "Numbered List Item 2", "Numbered List Item 3"])
      #expect(try bullet.getListType() == "bullet")
      #expect(try itemTexts(bullet) == ["Bullet List Item 1", "Bullet List Item 2"])
      // Outline list (Lexical convention): the two content items "1" and
      // "2", with a wrapper item between them whose only child is the
      // nested two-item sublist.
      #expect(try outline.getListType() == "number")
      let outlineItems = try items(outline)
      try #require(outlineItems.count == 3)
      let firstContent = try #require(try outlineItems[0].getFirstChild())
      #expect(jsTrimmed(try firstContent.getTextContent()) == "Outline numbered 1")
      let nested = try #require(
        try outlineItems[1].getFirstChild() as? ListNode, "expected nested sublist on wrapper item")
      #expect(try nested.getListType() == "number")
      #expect(try itemTexts(nested) == ["Outline numbered 1.a", "Outline numbered 1.b"])
      let lastContent = try #require(try outlineItems[2].getFirstChild())
      #expect(jsTrimmed(try lastContent.getTextContent()) == "Outline numbered 2")
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::MS Word paste — preprocess-installed overlay > survives the stylesheet-inlining preprocess"
    ))
  func wordListsWithStyleSheet() throws {
    // Real Word output carries a <style> block, which makes the default
    // stylesheet-inlining preprocess rewrite every element's style; the
    // overlay's snapshot of `mso-list` keeps the levels and markers.
    let editor = try buildWordPasteEditor()
    defer { editor.dispose() }
    try importHTMLDocument(
      editor,
      WORD_HTML_WITH_LISTS.replacing(
        "</head>",
        with: """
          <style>
            p.MsoListParagraphCxSpFirst, p.MsoListParagraphCxSpMiddle,
            p.MsoListParagraphCxSpLast, p.MsoListParagraph
              {margin-top:0in; margin-left:.5in;}
          </style></head>
          """))
    try editor.read { () throws -> Void in
      let lists = try getRoot().getChildren().compactMap { $0 as? ListNode }
      try #require(lists.count == 3)
      #expect(try lists[0].getListType() == "number")
      #expect(try lists[1].getListType() == "bullet")
      let outlineItems = try items(lists[2])
      try #require(outlineItems.count == 3)
      let nested = try #require(
        try outlineItems[1].getFirstChild() as? ListNode, "expected nested sublist on wrapper item")
      #expect(try itemTexts(nested) == ["Outline numbered 1.a", "Outline numbered 1.b"])
    }
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::MS Word paste — preprocess-installed overlay > without the Generator meta the overlay is not installed"
    ))
  func wordWithoutGeneratorMeta() throws {
    let editor = try buildWordPasteEditor()
    defer { editor.dispose() }
    try importHTMLDocument(
      editor, WORD_HTML_WITH_LISTS.replacing(/<meta name="Generator"[^>]*>/, with: ""))
    try editor.read { () throws -> Void in
      #expect(try !getRoot().getChildren().contains { $0 is ListNode })
    }
  }

  // MARK: ListItemNode block flattening

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > keeps contiguous inlines together, breaking only at block boundaries"
    ))
  func keepsInlinesTogether() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<ul><li>a <b>b</b><div>c</div></li></ul>")
    // `a ` and `b` are inline siblings and stay on the same line; the <div>
    // (lowered to a ParagraphNode) is the sole boundary.
    #expect(try liChildTypes(editor) == ["text:a ", "text:b", "linebreak:\n", "text:c"])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > treats a non-paragraph block (<hr>) as a boundary, not inline content"
    ))
  func horizontalRuleIsBoundary() throws {
    let editor = try buildEditor([HorizontalRuleExtension, ListExtension], name: "list-hr-host")
    defer { editor.dispose() }
    try importInto(editor, "<ul><li>x<hr />y</li></ul>")
    #expect(
      try liChildTypes(editor).map { String($0.split(separator: ":")[0]) }
        == ["text", "linebreak", "horizontalrule", "linebreak", "text"])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > with NodeStructureExtension, no line break is left next to a block decorator"
    ))
  func noLineBreakNextToBlockDecorator() throws {
    // The normalization splits the list around the rule; a line break next
    // to it would be left at the edge of each half as an empty line.
    let editor = try buildEditor(
      [HorizontalRuleExtension, ListExtension, NodeStructureExtension],
      name: "list-hr-structure-host")
    defer { editor.dispose() }
    func shapeOf(_ node: LexicalNode) throws -> String {
      guard let list = node as? ListNode else { return node.getType() }
      let children = try list.getChildren().map { item in
        guard let element = item as? ElementNode else { return item.getType() }
        return "[\(try element.getChildren().map { $0.getType() }.joined(separator: ", "))]"
      }
      return "[\(([String(try list.getStart())] + children).joined(separator: ", "))]"
    }
    try importInto(editor, "<ol><li>x</li><li>a<hr />b</li><li>y</li></ol>")
    #expect(
      try editor.read { try getRoot().getChildren().map(shapeOf) } == [
        // The second list continues the numbering: "b" is 3, "y" is 4.
        "[1, [text], [text]]", "horizontalrule", "[3, [text], [text]]",
      ])
    try importInto(editor, "<ul><li>a<p>b</p><hr /><p>c</p>d</li></ul>")
    #expect(
      try editor.read { try getRoot().getChildren().map(shapeOf) } == [
        "[1, [text, linebreak, text]]", "horizontalrule", "[1, [text, linebreak, text]]",
      ])
  }

  @Test(
    .upstream(
      "lexical-list/src/__tests__/unit/ListImportExtension.test.ts::ListItemNode block flattening > preserves a nested list instead of flattening it"
    ))
  func preservesNestedList() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, "<ul><li>parent<ul><li>child</li></ul></li></ul>")
    try editor.read { () throws -> Void in
      func findNested(_ node: LexicalNode) throws -> Bool {
        if let item = node as? ListItemNode {
          return try item.getChildren().contains { $0 is ListNode }
        }
        guard let list = node as? ListNode else { return false }
        return try list.getChildren().contains(where: findNested)
      }
      // The nested <ul> survives as a ListNode (lifted into a sibling item
      // by normalizeListChildren), not demoted to line-break-separated text.
      let outer = try #require(try getRoot().getFirstChild() as? ListNode, "expected a ListNode")
      #expect(try outer.getChildren().contains(where: findNested))
    }
  }
}

private func liChildTypes(_ editor: Editor) throws -> [String] {
  try editor.read {
    let list = try #require(try getRoot().getFirstChild() as? ListNode, "expected a ListNode")
    let li = try #require(try list.getFirstChild() as? ListItemNode, "expected a ListItemNode")
    return try li.getChildren().map { "\($0.getType()):\(try $0.getTextContent())" }
  }
}

/// `String.prototype.trim`.
private func jsTrimmed(_ text: String) -> String {
  String(text.drop { $0.isWhitespace }.reversed().drop { $0.isWhitespace }.reversed())
}

private func itemTexts(_ list: ListNode) throws -> [String] {
  try items(list).map { jsTrimmed(try $0.getTextContent()) }
}

private func buildWordPasteEditor() throws -> Editor {
  try buildEditor([WordListImportExtension], name: "word-paste-host")
}

private func importHTMLDocument(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) {
    let dep = try getExtensionDependencyFromEditor(editor, DOMImportExtension)
    let nodes = try dep.output.generateNodesFromDOM(DOMParser().parseFromString(html))
    _ = try getRoot().clear().splice(0, 0, nodes)
  }
}

private let WORD_HTML_WITH_LISTS = """
  <!doctype html>
  <html xmlns:o="urn:schemas-microsoft-com:office:office"
        xmlns:w="urn:schemas-microsoft-com:office:word">
  <head>
    <meta name="Generator" content="Microsoft Word 15">
  </head>
  <body>
    <p class="MsoNormal">Body text<o:p></o:p></p>
    <p class="MsoListParagraphCxSpFirst" style="mso-list:l2 level1 lfo1">
      <span><span style="mso-list:Ignore">1.</span></span>
      Numbered List Item 1<o:p></o:p>
    </p>
    <p class="MsoListParagraphCxSpMiddle" style="mso-list:l2 level1 lfo1">
      <span><span style="mso-list:Ignore">2.</span></span>
      Numbered List Item 2<o:p></o:p>
    </p>
    <p class="MsoListParagraphCxSpLast" style="mso-list:l2 level1 lfo1">
      <span><span style="mso-list:Ignore">3.</span></span>
      Numbered List Item 3<o:p></o:p>
    </p>
    <p class="MsoNormal"><o:p>&nbsp;</o:p></p>
    <p class="MsoListParagraphCxSpFirst" style="mso-list:l0 level1 lfo2">
      <span><span style="mso-list:Ignore">&middot;</span></span>
      Bullet List Item 1<o:p></o:p>
    </p>
    <p class="MsoListParagraphCxSpLast" style="mso-list:l0 level1 lfo2">
      <span><span style="mso-list:Ignore">&middot;</span></span>
      Bullet List Item 2<o:p></o:p>
    </p>
    <p class="MsoNormal"><o:p>&nbsp;</o:p></p>
    <p class="MsoListParagraphCxSpFirst" style="mso-list:l1 level1 lfo3">
      <span><span style="mso-list:Ignore">1)</span></span>
      Outline numbered 1<o:p></o:p>
    </p>
    <p class="MsoListParagraphCxSpMiddle" style="mso-list:l1 level2 lfo3">
      <span><span style="mso-list:Ignore">a)</span></span>
      Outline numbered 1.a<o:p></o:p>
    </p>
    <p class="MsoListParagraphCxSpMiddle" style="mso-list:l1 level2 lfo3">
      <span><span style="mso-list:Ignore">b)</span></span>
      Outline numbered 1.b<o:p></o:p>
    </p>
    <p class="MsoListParagraphCxSpLast" style="mso-list:l1 level1 lfo3">
      <span><span style="mso-list:Ignore">2)</span></span>
      Outline numbered 2<o:p></o:p>
    </p>
  </body>
  </html>
  """
