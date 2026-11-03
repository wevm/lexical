import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalHTML
import LexicalTesting
import Testing

// Port of packages/lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts.

private func initialEditorState() throws {
  _ = try getRoot().append(createParagraphNode()).select()
}

private func host(
  _ dependencies: [any AnyLexicalExtensionArgument] = [],
  _ initialState: @escaping () throws -> Void = initialEditorState
) -> LexicalExtension<EmptyConfig, Void, Void> {
  defineExtension(
    name: "host", dependencies: dependencies,
    editorConfig: InitialEditorConfig(initialEditorState: .update { _ in try initialState() }))
}

private func dataTransferWithHtml(_ html: String) -> DataTransfer {
  let dt = DataTransfer()
  dt.setData("text/html", html)
  return dt
}

private func dataTransferWithPlainText(_ text: String) -> DataTransfer {
  let dt = DataTransfer()
  dt.setData("text/plain", text)
  return dt
}

private func pasteAtSelection(_ editor: Editor, _ dataTransfer: DataTransfer) throws {
  try editor.update(discrete: true) {
    let selection = try #require(try getSelection() as? RangeSelection)
    try insertDataTransferForRichText(dataTransfer, selection, editor)
  }
}

private func pasteHtml(_ editor: Editor, _ html: String) throws {
  try pasteAtSelection(editor, dataTransferWithHtml(html))
}

private func lastParagraphText(_ editor: Editor) throws -> String {
  try editor.read { () throws in
    let lastChild = try #require(try getRoot().getLastChild() as? ParagraphNode)
    return try lastChild.getTextContent()
  }
}

private func paragraphTexts(_ editor: Editor) throws -> [String] {
  try editor.read { try getRoot().getChildren().map { try $0.getTextContent() } }
}

private final class Box<T> {
  var value: T
  init(_ value: T) { self.value = value }
}

@Suite struct ClipboardImportExtensionTests {
  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > default importer handles a basic <p> paste (no extension configured)"
    ))
  func defaultImporter() throws {
    let editor = try buildEditorFromExtensions(host())
    defer { editor.dispose() }
    try pasteHtml(editor, "<p>hello</p>")
    #expect(try lastParagraphText(editor) == "hello")
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > a registered text/html handler runs before the default and can stop the chain"
    ))
  func handlerStopsChain() throws {
    let called = Box(0)
    let editor = try buildEditorFromExtensions(
      host([
        configExtension(ClipboardImportExtension) {
          $0.importMimeType = [
            "text/html": [
              { _, selection, _, _ in
                called.value += 1
                let p = try createParagraphNode().append(createTextNode("[custom]"))
                try insertGeneratedNodes(getEditor(), [p], selection)
                return true
              }
            ]
          ]
        }
      ]))
    defer { editor.dispose() }
    try pasteHtml(editor, "<p>ignored</p>")
    #expect(try lastParagraphText(editor) == "[custom]")
    #expect(called.value == 1)
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > handler can call next() to defer to the default"
    ))
  func handlerDefers() throws {
    let deferred = Box(false)
    let editor = try buildEditorFromExtensions(
      host([
        configExtension(ClipboardImportExtension) {
          $0.importMimeType = [
            "text/html": [
              { _, _, next, _ in
                deferred.value = true
                return try next()
              }
            ]
          ]
        }
      ]))
    defer { editor.dispose() }
    try pasteHtml(editor, "<p>hello</p>")
    #expect(try lastParagraphText(editor) == "hello")
    #expect(deferred.value)
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards replacement data through the remaining handlers via next()"
    ),
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards empty replacement data through the remaining handlers via next()"
    ),
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards replacement selection through the remaining handlers via next()"
    ),
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards both replacements through the remaining handlers via next()"
    ),
    arguments: [
      ("replacement", false), ("", false), ("original", true), ("replacement", true),
    ])
  func forwardsThroughNext(replacementData: String, replaceSelection: Bool) throws {
    let dt = dataTransferWithPlainText("original")
    let seen = Box<[(String, any BaseSelection, DataTransfer)]>([])
    let replacementSelection = Box<(any BaseSelection)?>(nil)
    let editor = try buildEditorFromExtensions(
      host(
        [
          configExtension(ClipboardImportExtension) {
            $0.importMimeType = [
              "text/plain": [
                { data, selection, next, dataTransfer in
                  seen.value.append((data, selection, dataTransfer))
                  return try next(data, selection)
                },
                { data, selection, next, dataTransfer in
                  seen.value.append((data, selection, dataTransfer))
                  return try next()
                },
                { _, _, next, _ in
                  replaceSelection
                    ? try next(replacementData, replacementSelection.value)
                    : try next(replacementData)
                },
              ]
            ]
          }
        ],
        {
          let first = try createTextNode("first")
          _ = try getRoot().append(
            createParagraphNode().append(first),
            createParagraphNode().append(createTextNode("second")))
          _ = try first.select()
        }))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let originalSelection = try #require(try getSelection() as? RangeSelection)
      let second = try getRoot().getLastChildOrThrow()
      let selection = createRangeSelection()
      try selection.anchor.set(second.getKey(), 0, .element)
      try selection.focus.set(second.getKey(), 1, .element)
      replacementSelection.value = selection

      try insertDataTransferForRichText(dt, originalSelection, editor)

      #expect(seen.value.count == 2)
      for (data, forwardedSelection, dataTransfer) in seen.value {
        #expect(data == replacementData)
        #expect(
          forwardedSelection
            === (replaceSelection ? replacementSelection.value! : originalSelection))
        #expect(dataTransfer === dt)
        #expect(dataTransfer.getData("text/plain") == "original")
      }
    }
    #expect(
      try paragraphTexts(editor)
        == (replaceSelection ? ["first", replacementData] : ["first\(replacementData)", "second"]))
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > app-defined MIME type is reached when added to both stack and priority"
    ))
  func appDefinedMimeType() throws {
    let saw = Box("")
    let editor = try buildEditorFromExtensions(
      host([
        configExtension(ClipboardImportExtension) {
          $0.importMimeType = [
            "application/vnd.myapp+json": [
              { data, selection, _, _ in
                saw.value = data
                let p = try createParagraphNode().append(createTextNode("[\(data)]"))
                try insertGeneratedNodes(getEditor(), [p], selection)
                return true
              }
            ]
          ]
          // Slot the custom MIME type between lexical-editor (0) and
          // text/html (10). The other built-in weights inherit from the
          // defaults.
          $0.priority = ["application/vnd.myapp+json": 5]
        }
      ]))
    defer { editor.dispose() }
    let dt = DataTransfer()
    dt.setData("text/html", "<p>html-fallback</p>")
    dt.setData("application/vnd.myapp+json", #"{"a":1}"#)
    try pasteAtSelection(editor, dt)
    #expect(saw.value == #"{"a":1}"#)
    #expect(try lastParagraphText(editor) == #"[{"a":1}]"#)
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > priority weights compose without coordination between extensions"
    ))
  func priorityWeightsCompose() throws {
    let myAppCalls = Box(0)
    let htmlCalls = Box(0)
    let editor = try buildEditorFromExtensions(
      host([
        configExtension(ClipboardImportExtension) {
          $0.importMimeType = [
            "application/vnd.myapp+json": [
              { _, _, _, _ in
                myAppCalls.value += 1
                return true
              }
            ],
            "text/html": [
              { _, _, next, _ in
                htmlCalls.value += 1
                return try next()
              }
            ],
          ]
          // myapp gets weight 1, so it runs ahead of html (10).
          $0.priority = ["application/vnd.myapp+json": 1]
        }
      ]))
    defer { editor.dispose() }
    let dt = DataTransfer()
    dt.setData("text/html", "<p>x</p>")
    dt.setData("application/vnd.myapp+json", "{}")
    try pasteAtSelection(editor, dt)
    #expect(myAppCalls.value == 1)
    #expect(htmlCalls.value == 0)
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > text/html can be routed through DOMImportExtension"
    ))
  func routedThroughDOMImport() throws {
    let editor = try buildEditorFromExtensions(
      host([
        CoreImportExtension,
        configExtension(ClipboardImportExtension) {
          $0.importMimeType = [
            "text/html": [
              { html, selection, _, _ in
                let dom = DOMParser().parseFromString(html, "text/html")
                let nodes = try generateNodesFromDOMViaExtension(
                  dom, GenerateNodesFromDOMOptions(context: [contextValue(ImportSource, .paste)]))
                try insertGeneratedNodes(getEditor(), nodes, selection)
                return true
              }
            ]
          ]
        },
      ]))
    defer { editor.dispose() }
    try pasteHtml(editor, "<p>via <strong>new</strong> pipeline</p>")
    #expect(try lastParagraphText(editor) == "via new pipeline")
  }
}

// Two paragraphs, with the editor's selection parked at the end of the
// second one so that inserting at the current selection is distinguishable
// from inserting at the supplied one.
private func makeEditor() throws -> Editor {
  try buildEditorFromExtensions(
    host([]) {
      let second = try createTextNode("second")
      _ = try getRoot().clear().append(
        createParagraphNode().append(createTextNode("first")),
        createParagraphNode().append(second))
      _ = try second.select()
    })
}

private func insertOverFirstParagraph(_ editor: Editor, _ dataTransfer: DataTransfer) throws {
  try editor.update(discrete: true) {
    let firstParagraph = try getRoot().getChildren()[0]
    let selection = createRangeSelection()
    try selection.anchor.set(firstParagraph.getKey(), 0, .element)
    try selection.focus.set(firstParagraph.getKey(), 1, .element)
    try insertDataTransferForRichText(dataTransfer, selection, editor)
  }
}

@Suite struct InsertDataTransferSelectionArgumentTests {
  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/plain is inserted at the supplied selection"
    ))
  func plainText() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try insertOverFirstParagraph(editor, dataTransferWithPlainText("replacement"))
    #expect(try paragraphTexts(editor) == ["replacement", "second"])
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > multi-line text/plain is inserted at the supplied selection"
    ))
  func multiLinePlainText() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    try insertOverFirstParagraph(editor, dataTransferWithPlainText("one\ntwo"))
    #expect(try paragraphTexts(editor) == ["one", "two", "second"])
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/uri-list is inserted at the supplied selection"
    ))
  func uriList() throws {
    let editor = try makeEditor()
    defer { editor.dispose() }
    let dt = DataTransfer()
    dt.setData("text/uri-list", "https://lexical.dev")
    try insertOverFirstParagraph(editor, dt)
    #expect(try paragraphTexts(editor) == ["https://lexical.dev", "second"])
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/html is inserted at the supplied selection"
    ))
  func html() throws {
    // Control: the text/html handler already honored the argument.
    let editor = try makeEditor()
    defer { editor.dispose() }
    try insertOverFirstParagraph(editor, dataTransferWithHtml("<p>replacement</p>"))
    #expect(try paragraphTexts(editor) == ["replacement", "second"])
  }

  @Test(
    .upstream(
      "lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/plain still lands at the caret on the ordinary paste path"
    ))
  func ordinaryPastePath() throws {
    // Control: when the argument is the editor's selection, behavior is
    // unchanged.
    let editor = try makeEditor()
    defer { editor.dispose() }
    try pasteAtSelection(editor, dataTransferWithPlainText("!"))
    #expect(try paragraphTexts(editor) == ["first", "second!"])
  }
}
