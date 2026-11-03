import Lexical
import LexicalExtension
import LexicalHTML
import LexicalRichText
import Testing

// Port of packages/lexical-html/src/__tests__/unit/BlockIdHTMLExtension.test.ts.
// Upstream's editors register ListExtension too; the tests that import or
// export lists are skipped until @lexical/list is ported, and the rest need
// no list.

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
  prefix: String = "b"
) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[block-id-html]",
      dependencies: [
        RichTextExtension,
        configExtension(BlockIdExtension) { $0.generateId = withIds(ids, counter(prefix)) },
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
