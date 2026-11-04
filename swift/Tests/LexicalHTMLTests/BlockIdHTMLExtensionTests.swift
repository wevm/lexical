import Lexical
import LexicalExtension
import LexicalHTML
import LexicalList
import LexicalRichText
import Testing

// Port of packages/lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts.
// The editors register ListExtension, as upstream's do.

private func counter(_ prefix: String) -> () -> String {
  var n = 0
  return {
    n += 1
    return "\(prefix)\(n)"
  }
}

/// `withIds(ids, then)`: hands out `ids` first, then `then`'s.
private func withIds(_ ids: [String], _ then: @escaping () -> String) -> () -> String {
  var i = 0
  return {
    if i < ids.count {
      i += 1
      return ids[i - 1]
    }
    return then()
  }
}

/// `asEdited(editor)`: the editor has had content once, so the first
/// blocks it adds get ids from `generateId`.
private func asEdited(_ editor: Editor) throws -> Editor {
  try editor.update(tag: HISTORY_MERGE_TAG, discrete: true) {
    try getRoot().append(createParagraphNode())
  }
  try editor.update(tag: HISTORY_MERGE_TAG, discrete: true) { try getRoot().clear() }
  return editor
}

/// `build(options)`. `html: nil` leaves BlockIdHTMLExtension out.
private func build(
  fresh: Bool = false, ids: [String] = [], html: BlockIdHTMLConfig? = BlockIdHTMLConfig(),
  prefix: String = "b", rootOnly: Bool = false
) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-id-html]",
      dependencies: [
        RichTextExtension, ListExtension,
        configExtension(BlockIdExtension) {
          $0.generateId = withIds(ids, counter(prefix))
          // Identify the root's children only.
          if rootOnly { $0.isIdentifiedBlock = isRootChildBlock }
        },
      ]
        + (html.map { config in
          [
            configExtension(BlockIdHTMLExtension) {
              $0.export = config.export
              $0.handles = config.handles
              $0.import = config.import
            }
          ]
        } ?? []),
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  return fresh ? editor : try asEdited(editor)
}

/// Parses a fixed HTML fixture.
private func parseFixture(_ fixture: String) -> DOMDocument {
  DOMParser().parseFromString("<!doctype html><html><body>\(fixture)</body></html>")
}

private func emptyDocument() -> DOMDocument { parseFixture("") }

/// A document whose body holds one element per entry, built with DOM APIs.
private func paragraphsDocument(
  _ entries: [(id: String?, tag: String, text: String)]
) -> DOMDocument {
  let doc = emptyDocument()
  for (id, tag, text) in entries {
    let element = doc.createElement(tag)
    if let id {
      element.setAttribute("data-block-id", id)
    }
    element.textContent = text
    doc.body!.append(element)
  }
  return doc
}

private func append(_ editor: Editor, _ dom: DOMDocument) throws {
  try editor.update(discrete: true) {
    try getRoot().append(
      contentsOf: getExtensionDependencyFromEditor(getEditor(), DOMImportExtension).output
        .generateNodesFromDOM(dom))
  }
}

/// The editor's content exported as a DOM document.
private func exportDocument(_ editor: Editor) throws -> DOMDocument {
  let doc = emptyDocument()
  try editor.read { try generateDOMFromNodes(doc.body!, nil, editor) }
  return doc
}

/// The editor's content exported as a DOM document, and the handles it
/// shows, in one read.
private func exportWithHandles(_ editor: Editor) throws -> (DOMDocument, JSMap<String, String>) {
  let doc = emptyDocument()
  return try editor.read {
    try generateDOMFromNodes(doc.body!, nil, editor)
    return (doc, try getBlockHandles())
  }
}

private func exportHTML(_ editor: Editor) throws -> String {
  try editor.read { try generateHtmlFromNodes(editor) }
}

/// `querySelectorAll('[attribute]')`.
private func withAttribute(_ doc: DOMDocument, _ attribute: String) -> [DOMElement] {
  doc.body!.descendantElements().filter { $0.hasAttribute(attribute) }
}

private func tree(_ editor: Editor) throws -> [String] {
  try editor.read {
    var out: [String] = []
    func walk(_ node: LexicalNode) throws {
      if let element = node as? ElementNode {
        if node.getType() != "root" {
          out.append("\(node.getType()):\(try getBlockId(node) ?? "null")")
        }
        try element.getChildren().forEach(walk)
      }
    }
    try walk(getRoot())
    return out
  }
}

private func idOf(_ entry: String) -> String {
  String(entry[entry.index(after: entry.firstIndex(of: ":")!)...])
}

private func isValidId(_ id: String) -> Bool {
  let alphabet = Set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")
  return (1...64).contains(id.count) && id.allSatisfy { alphabet.contains($0) }
}

private func output(_ editor: Editor) throws -> NamedSignals<BlockIdHTMLConfig> {
  try getExtensionDependencyFromEditor(editor, BlockIdHTMLExtension).output
}

@Suite struct BlockIdHTMLExtensionTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > without it, HTML carries no ids"
    ))
  func withoutItNoIds() throws {
    let editor = try build(html: nil)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(createParagraphNode().append(createTextNode("one")))
    }
    #expect(try !exportHTML(editor).contains("data-block-id"))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > imported ids pass through the uniqueness rule"
    ))
  func importedIdsPassTheUniquenessRule() throws {
    let editor = try build()
    defer { editor.dispose() }
    try append(editor, parseFixture("<p>existing</p>"))
    let existing = try tree(editor)[0]
    try append(
      editor,
      paragraphsDocument([
        ("x", "p", "first x"), ("x", "p", "second x"),
        (idOf(existing), "p", "clashes with the document"), ("kept", "h3", "kept"),
      ]))
    let after = try tree(editor)
    #expect(after[0] == existing)
    #expect(after[1] == "paragraph:x")
    #expect(after[4] == "heading:kept")
    let ids = after.map(idOf)
    #expect(Set(ids).count == ids.count)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > invalid imported ids are ignored; valid foreign ids are kept"
    ))
  func invalidImportedIdsIgnored() throws {
    let editor = try build()
    defer { editor.dispose() }
    let invalid = ["", String(repeating: "z", count: 65), "a b", "line\nbreak", "\"><x", "é"]
    try append(
      editor,
      paragraphsDocument(
        invalid.map { ($0, "p", "invalid") } + [
          (String(repeating: "y", count: 64), "p", "longest valid"),
          ("bk3j9x0a2zq", "p", "an 11-character id"),
        ]))
    let ids = try tree(editor).map(idOf)
    #expect(Array(ids.suffix(2)) == [String(repeating: "y", count: 64), "bk3j9x0a2zq"])
    for id in ids {
      #expect(isValidId(id), "\(id)")
      #expect(!invalid.contains(id))
    }
    #expect(Set(ids).count == ids.count)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > export: false leaves the export override out of every export session"
    ))
  func exportFalseLeavesTheOverrideOut() throws {
    // Upstream also checks whether the session's render config is the
    // editor's own object; a Swift EditorDOMRenderConfig is a struct of
    // closures with no identity to compare, so only the exports are checked.
    let editor = try build(html: BlockIdHTMLConfig(export: false))
    defer { editor.dispose() }
    try append(editor, parseFixture("<p>a</p>"))
    let signals = try output(editor)
    #expect(try !exportHTML(editor).contains("data-block"))
    // handles alone does not turn the export on.
    try signals.handles.set(true)
    #expect(try !exportHTML(editor).contains("data-block"))
    try signals.export.set(true)
    #expect(try exportHTML(editor).contains("data-block-handle"))
    try signals.handles.set(false)
    #expect(try exportHTML(editor).contains("data-block-id"))
    try signals.export.set(false)
    #expect(try !exportHTML(editor).contains("data-block"))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > the released types of export still hold: a boolean, and a Signal<boolean>"
    ))
  func releasedTypesStillHold() throws {
    let editor = try build(html: BlockIdHTMLConfig(handles: true))
    defer { editor.dispose() }
    let signals = try output(editor)
    // The types are checked by the compiler: Bool fields, Signal<Bool>
    // outputs.
    let on: Bool = signals.export.value
    let exportSignal: Signal<Bool> = signals.export
    let handlesSignal: Signal<Bool> = signals.handles
    let importSignal: Signal<Bool> = signals.import
    // A whole config as written for v0.53.0, without handles.
    let config = BlockIdHTMLConfig(export: true, import: true)
    let partial = BlockIdHTMLConfig(export: false)
    #expect([on, exportSignal.value, config.export, partial.export] == [true, true, true, false])
    #expect(handlesSignal.value && importSignal.value)
    // The default configuration is unchanged, with handles off.
    let defaults = try build()
    defer { defaults.dispose() }
    #expect(try output(defaults).handles.value == false)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > import: false ignores the attribute; export: false omits it"
    ))
  func importFalseExportFalse() throws {
    let editor = try build(html: BlockIdHTMLConfig(export: false, import: false))
    defer { editor.dispose() }
    try append(editor, parseFixture(#"<p data-block-id="from-html">a</p>"#))
    #expect(try tree(editor) == ["paragraph:b1"])
    #expect(try !exportHTML(editor).contains("data-block-id"))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: documents read without ids > every reader exports the same data-block-id for a document stored without ids"
    ))
  func everyReaderExportsTheSameIds() throws {
    var html: [[String]] = []
    for prefix in ["x", "y"] {
      let editor = try build(fresh: true, prefix: prefix)
      defer { editor.dispose() }
      // The first content of an empty editor is a load: positional ids, not
      // `generateId`'s.
      try append(editor, paragraphsDocument([(nil, "p", "one"), (nil, "h2", "two")]))
      html.append(
        withAttribute(try exportDocument(editor), "data-block-id").map {
          $0.getAttribute("data-block-id")!
        })
      #expect(try editor.read { try getRoot().getChildren().map(getBlockId) } == html.last!)
    }
    #expect(html[0] == html[1])
    for id in html[0] {
      #expect(id.count == 21 && isValidId(id))
    }
  }
}

private let IDS = [
  "V1StGXR8_Z5jdHi6B-myT",
  "9xQeLm2Pq0aK7cTb_wYuA",
  "Rk8pZc41Uo-HsVd2NnFjB",
  // Its hash starts like the first id's: both handles take 5 characters.
  "qfapyr",
  "u7NnPz2Kq0aK7cTb_wYuA",
]

@Suite struct BlockIdHTMLHandlesTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > import never reads data-block-handle as an id"
    ))
  func importNeverReadsHandles() throws {
    let editor = try build(ids: ["abcdEFGH", "V1StGXR8_Z5jdHi6B-myT"])
    defer { editor.dispose() }
    try append(editor, parseFixture("<p>existing</p>"))
    #expect(try tree(editor) == ["paragraph:abcdEFGH"])
    try append(
      editor,
      parseFixture(
        // A handle of a block of this document, a handle that looks like a
        // whole id, and a handle next to a real id.
        #"<p data-block-handle="abcd">handle of existing</p>"#
          + #"<p data-block-handle="kept-if-read-as-id">no id</p>"#
          + #"<h3 data-block-handle="zzzz" data-block-id="real-id">with id</h3>"#))
    #expect(
      try tree(editor) == [
        "paragraph:abcdEFGH", "paragraph:V1StGXR8_Z5jdHi6B-myT", "paragraph:b1", "heading:real-id",
      ])
    // With the import on, as by default, and an export of handles.
    let handles = try build(html: BlockIdHTMLConfig(handles: true))
    defer { handles.dispose() }
    try append(handles, parseFixture(#"<p data-block-handle="from-html">a</p>"#))
    #expect(try tree(handles) == ["paragraph:b1"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > round trip: export handles, edit by handle, resolve"
    ))
  func roundTripByHandle() throws {
    let editor = try build(ids: IDS, html: BlockIdHTMLConfig(handles: true))
    defer { editor.dispose() }
    try append(
      editor,
      parseFixture("<h2>Q3 plan</h2><p>Revenue grew 4% in Q3.</p><p>Hiring is on track.</p>"))
    // 1. The HTML the model reads, and the handles it is shown, in one read.
    let (exported, shown) = try exportWithHandles(editor)
    let elements = withAttribute(exported, BLOCK_HANDLE_ATTRIBUTE)
    #expect(withAttribute(exported, BLOCK_ID_ATTRIBUTE).isEmpty)
    #expect(elements.map { $0.getAttribute(BLOCK_HANDLE_ATTRIBUTE) } == ["2bNo", "gTtF", "xt6z"])
    let shownHandles = Set(shown.map(\.1))
    #expect(["2bNo", "gTtF", "xt6z"].allSatisfy(shownHandles.contains))
    let keyBefore = try editor.read { try #require(try getBlockById(IDS[1])).getKey() }
    // 2. What the model answers.
    let revenue = try #require(elements.first { $0.textContent.hasPrefix("Revenue") })
    let ops = [
      (try #require(revenue.getAttribute(BLOCK_HANDLE_ATTRIBUTE)), "Revenue grew 6% in Q3."),
      ("9xQf", "a typo"),
    ]
    // 3. Applied by the app, one update per operation, in place.
    var statuses: [String] = []
    for (block, text) in ops {
      try editor.update(discrete: true) {
        let result = try resolveBlockHandle(block, BlockHandleOptions(read: shown))
        statuses.append(result.status)
        if let node = result.node {
          #expect(try setBlockText(node, text))
        }
      }
    }
    #expect(statuses == ["found", "not-found"])
    try editor.read { () throws in
      let block = try #require(try getBlockById(IDS[1]))
      #expect(try block.getTextContent() == "Revenue grew 6% in Q3.")
      // Changed in place: the same block.
      #expect(block.getKey() == keyBefore)
    }
    #expect(
      try exportHTML(editor)
        == [
          #"<h2 data-block-handle="2bNo"><span style="white-space: pre-wrap;">Q3 plan</span></h2>"#,
          #"<p data-block-handle="gTtF"><span style="white-space: pre-wrap;">Revenue grew 6% in Q3.</span></p>"#,
          #"<p data-block-handle="xt6z"><span style="white-space: pre-wrap;">Hiring is on track.</span></p>"#,
        ].joined())
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > a handle from exported HTML never names a block added since whose handle would be the same"
    ))
  func exportedHandleNeverNamesANewBlock() throws {
    let editor = try build(ids: ["p18f", "Rk8pZc41"], html: BlockIdHTMLConfig(handles: true))
    defer { editor.dispose() }
    try append(editor, parseFixture("<p>Revenue grew 4%.</p><p>Hiring.</p>"))
    let (exported, shown) = try exportWithHandles(editor)
    #expect(
      exported.body!.getElementsByTagName("p").first?.getAttribute(BLOCK_HANDLE_ATTRIBUTE)
        == "NnGk")
    // Before the model's answer arrives, the block it was shown is deleted
    // and a block whose hash starts the same way is added.
    try editor.update(discrete: true) {
      try #require(try getBlockById("p18f")).remove()
      let node = try createParagraphNode().append(createTextNode("Not shown to the model."))
      try setState(node, blockIdState, "p2cs")
      try getRoot().append(node)
    }
    try editor.update(discrete: true) {
      // Without the read, the handle names the new block.
      let unpinned = try resolveBlockHandle("NnGk")
      #expect(try unpinned.node.map(getBlockId) == "p2cs")
      // With the handles of the export: not found.
      let result = try resolveBlockHandle("NnGk", BlockHandleOptions(read: shown))
      #expect(result.status == "not-found")
      if let node = result.node {
        try setBlockText(node, "Revenue grew 6%.")
      }
    }
    try editor.read { () throws in
      #expect(try getBlockById("p2cs")?.getTextContent() == "Not shown to the model.")
    }
  }
}

// MARK: lists

/// The ids in `entries` (`type:id`), without the blocks that have none.
private func idsIn(_ entries: [String]) -> [String] {
  entries.map(idOf).filter { $0 != "null" }
}

@Suite struct BlockIdHTMLListTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > exports data-block-id on identified blocks only"
    ))
  func exportsOnIdentifiedBlocks() throws {
    let editor = try build()
    defer { editor.dispose() }
    try append(editor, parseFixture("<h1>Title</h1><p>Text</p><ul><li>a</li><li>b</li></ul>"))
    #expect(
      try tree(editor) == [
        "heading:b1", "paragraph:b2", "list:b3", "listitem:b4", "listitem:b5",
      ])
    #expect(
      try exportHTML(editor)
        == [
          #"<h1 data-block-id="b1"><span style="white-space: pre-wrap;">Title</span></h1>"#,
          #"<p data-block-id="b2"><span style="white-space: pre-wrap;">Text</span></p>"#,
          #"<ul data-block-id="b3">"#,
          #"<li value="1" data-block-id="b4"><span style="white-space: pre-wrap;">a</span></li>"#,
          #"<li value="2" data-block-id="b5"><span style="white-space: pre-wrap;">b</span></li>"#,
          "</ul>",
        ].joined())
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > a root-only predicate exports no list item ids"
    ))
  func rootOnlyPredicate() throws {
    let editor = try build(rootOnly: true)
    defer { editor.dispose() }
    try append(editor, parseFixture("<ul><li>a</li><li>b</li></ul>"))
    #expect(try tree(editor) == ["list:b1", "listitem:null", "listitem:null"])
    #expect(
      try exportHTML(editor)
        == [
          #"<ul data-block-id="b1">"#,
          #"<li value="1"><span style="white-space: pre-wrap;">a</span></li>"#,
          #"<li value="2"><span style="white-space: pre-wrap;">b</span></li>"#,
          "</ul>",
        ].joined())
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > round-trips ids through HTML"
    ))
  func roundTrips() throws {
    let source = try build(prefix: "s")
    defer { source.dispose() }
    try append(
      source,
      parseFixture("<h2>Title</h2><p>Text</p><ul><li>a</li></ul><blockquote>q</blockquote>"))
    let exported = try exportDocument(source)
    #expect(withAttribute(exported, "data-block-id").count == 5)
    let target = try build(prefix: "t")
    defer { target.dispose() }
    try append(target, exported)
    #expect(try tree(target) == tree(source))
    #expect(
      try tree(target) == ["heading:s1", "paragraph:s2", "list:s3", "listitem:s4", "quote:s5"])
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension > an imported id is dropped from a block that is not identified"
    ))
  func importedIdDroppedFromUnidentified() throws {
    let editor = try build()
    defer { editor.dispose() }
    try append(
      editor,
      parseFixture(
        #"<ul data-block-id="outer"><li data-block-id="item">a"#
          + #"<ul data-block-id="nested"><li>b</li></ul></li></ul>"#))
    let outline = try tree(editor)
    #expect(outline[0] == "list:outer")
    #expect(outline[1] == "listitem:item")
    // The nested list is not identified: it keeps no id.
    #expect(outline.contains("list:null"))
    #expect(!(try editor.getEditorState().toJSON().stringify().contains(#""nested""#)))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > exports data-block-handle instead of data-block-id"
    ))
  func exportsHandles() throws {
    let editor = try build(ids: IDS, html: BlockIdHTMLConfig(handles: true))
    defer { editor.dispose() }
    try append(
      editor,
      parseFixture("<h2>Q3 plan</h2><p>Revenue grew 4% in Q3.</p><ul><li>a</li><li>b</li></ul>"))
    #expect(
      try exportHTML(editor)
        == [
          #"<h2 data-block-handle="2bNoq"><span style="white-space: pre-wrap;">Q3 plan</span></h2>"#,
          #"<p data-block-handle="gTtF"><span style="white-space: pre-wrap;">Revenue grew 4% in Q3.</span></p>"#,
          #"<ul data-block-handle="xt6z">"#,
          #"<li value="1" data-block-handle="2bNoV"><span style="white-space: pre-wrap;">a</span></li>"#,
          #"<li value="2" data-block-handle="Wbjb"><span style="white-space: pre-wrap;">b</span></li>"#,
          "</ul>",
        ].joined())
    #expect(BLOCK_HANDLE_ATTRIBUTE == "data-block-handle")
    #expect(BLOCK_ID_ATTRIBUTE == "data-block-id")
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: handles: true > export: true and export: false are unchanged"
    ))
  func exportTrueAndFalseUnchanged() throws {
    let fixture = "<h2>Q3 plan</h2><p>Revenue grew 4% in Q3.</p><ul><li>a</li><li>b</li></ul>"
    var html: [String: String] = [:]
    let configs: [(String, BlockIdHTMLConfig?)] = [
      ("default", BlockIdHTMLConfig()), ("true", BlockIdHTMLConfig(export: true)),
      ("false", BlockIdHTMLConfig(export: false)), ("without", nil),
    ]
    for (name, config) in configs {
      let editor = try build(ids: IDS, html: config)
      defer { editor.dispose() }
      try append(editor, parseFixture(fixture))
      html[name] = try exportHTML(editor)
    }
    #expect(html["true"] == html["default"])
    #expect(
      html["true"]
        == [
          #"<h2 data-block-id="V1StGXR8_Z5jdHi6B-myT"><span style="white-space: pre-wrap;">Q3 plan</span></h2>"#,
          #"<p data-block-id="9xQeLm2Pq0aK7cTb_wYuA"><span style="white-space: pre-wrap;">Revenue grew 4% in Q3.</span></p>"#,
          #"<ul data-block-id="Rk8pZc41Uo-HsVd2NnFjB">"#,
          #"<li value="1" data-block-id="qfapyr"><span style="white-space: pre-wrap;">a</span></li>"#,
          #"<li value="2" data-block-id="u7NnPz2Kq0aK7cTb_wYuA"><span style="white-space: pre-wrap;">b</span></li>"#,
          "</ul>",
        ].joined())
    #expect(html["false"] == html["without"])
    #expect(html["false"]?.contains("data-block") == false)
  }
}

private let NESTED =
  "<ul><li>first</li><li>second<ul><li>inner one</li>"
  + "<li>inner two<ol><li>deep</li></ol></li></ul></li><li>third</li></ul>"

@Suite struct BlockIdHTMLWrapperTests {
  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > exports its id on the nested list, and round-trips it"
    ))
  func exportsOnTheNestedList() throws {
    let source = try build(prefix: "s")
    defer { source.dispose() }
    try append(source, parseFixture(NESTED))
    let outline = try tree(source)
    // first, second, the inner list's wrapper, inner one, inner two, the
    // deep list's wrapper, deep, third.
    #expect(outline.filter { $0.hasPrefix("listitem:") }.count == 8)
    let exported = try exportDocument(source)
    let wrapped = withAttribute(exported, BLOCK_WRAPPER_ID_ATTRIBUTE)
    #expect(wrapped.map(\.tagName) == ["UL", "OL"])
    // Every identified block's id is in the HTML exactly once.
    let written = exported.body!.descendantElements().flatMap { element in
      [BLOCK_ID_ATTRIBUTE, BLOCK_WRAPPER_ID_ATTRIBUTE].compactMap { element.getAttribute($0) }
    }
    #expect(written.sorted() == idsIn(outline).sorted())
    let target = try build(prefix: "t")
    defer { target.dispose() }
    try append(target, exported)
    #expect(try tree(target) == outline)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > pasted twice, the second copy gets fresh ids, the wrapper items too"
    ))
  func pastedTwice() throws {
    let source = try build(prefix: "s")
    defer { source.dispose() }
    try append(source, parseFixture(NESTED))
    let exported = try exportDocument(source)
    let target = try build(prefix: "t")
    defer { target.dispose() }
    try append(target, exported)
    try append(target, exported)
    let ids = idsIn(try tree(target))
    #expect(Set(ids).count == ids.count)
    let sourceTree = try tree(source)
    #expect(Array(try tree(target).prefix(sourceTree.count)) == sourceTree)
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > export: false writes no wrapper id; import: false reads none"
    ))
  func exportFalseImportFalse() throws {
    let off = try build(html: BlockIdHTMLConfig(export: false), prefix: "s")
    defer { off.dispose() }
    try append(off, parseFixture(NESTED))
    #expect(withAttribute(try exportDocument(off), BLOCK_WRAPPER_ID_ATTRIBUTE).isEmpty)
    let source = try build(prefix: "s")
    defer { source.dispose() }
    try append(source, parseFixture(NESTED))
    let target = try build(html: BlockIdHTMLConfig(import: false), prefix: "t")
    defer { target.dispose() }
    try append(target, exportDocument(source))
    #expect(!(try tree(target).contains { $0.contains(":s") }))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > a wrapper id goes only on a block that holds that list alone"
    ))
  func wrapperIdOnlyOnALoneHolder() throws {
    let editor = try build(prefix: "e")
    defer { editor.dispose() }
    try append(
      editor,
      parseFixture(
        // A top-level list: its parent is the root, which takes no id.
        #"<ul data-block-wrapper-id="top"><li>a</li></ul>"#
          // A nested list after the item's text: the importer wraps it in an
          // item of its own, which takes the id.
          + #"<ul><li data-block-id="item">text"#
          + #"<ul data-block-wrapper-id="w"><li>inner</li></ul></li></ul>"#))
    let outline = try tree(editor)
    #expect(outline.contains("listitem:item"))
    #expect(outline.contains("listitem:w"))
    #expect(!outline.contains { $0.hasSuffix(":top") })
    #expect(!(try editor.getEditorState().toJSON().stringify().contains(#""top""#)))
  }

  @Test(
    .upstream(
      "lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts::BlockIdHTMLExtension: a list item that only wraps a nested list > handles: true writes no wrapper id"
    ))
  func handlesWriteNoWrapperId() throws {
    let editor = try build(html: BlockIdHTMLConfig(handles: true), prefix: "h")
    defer { editor.dispose() }
    try append(editor, parseFixture(NESTED))
    let (doc, _) = try exportWithHandles(editor)
    #expect(withAttribute(doc, BLOCK_WRAPPER_ID_ATTRIBUTE).isEmpty)
    #expect(withAttribute(doc, BLOCK_ID_ATTRIBUTE).isEmpty)
  }
}
