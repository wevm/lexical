import Lexical
import LexicalExtension
import LexicalHistory
import LexicalLink
import LexicalRichText
import LexicalTextKit
import Testing

// TextKitRenderExtension: theme keys an app adds to elements from its own
// state (#247), as a web app adds a class to a node's DOM.

/// The app's state the override reads: which pages exist.
private final class Pages: @unchecked Sendable {
  var existing: Set<String> = []
}

@Suite struct TextKitRenderExtensionTests {
  /// The look of the run at `location` in `update`, from its edits or the
  /// text it restyled.
  private func attributes(at location: Int, in update: TextKitUpdate?) -> TextKitTextAttributes? {
    let runs = (update?.edits.flatMap(\.runs) ?? []) + (update?.restyled ?? [])
    return runs.last { $0.range.location <= location && location < $0.range.end }?.attributes
  }

  @Test func anOverrideStylesALinkFromAppStateAndMarkDirtyRestylesIt() throws {
    let pages = Pages()
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test",
        dependencies: [
          RichTextExtension, LinkExtension, HistoryExtension,
          configExtension(TextKitRenderExtension) {
            $0.overrides = [
              textKitOverride([LinkNode.self]) { node, next, _ in
                guard let link = node as? LinkNode, try !pages.existing.contains(link.getURL())
                else { return try next() }
                return try next() + ["linkMissing"]
              }
            ]
          },
        ]))
    defer { editor.dispose() }
    let c = TextKitInputController(editor: editor)
    var theme = TextKitTheme.default
    theme.styles["linkMissing"] = TextKitStyle(strikethrough: true)
    c.theme = theme
    var update: TextKitUpdate?
    c.onRender = { update = $0 }
    var linkKey = ""
    try editor.update(discrete: true) {
      let link = try createLinkNode("https://wiki/Missing")
      try link.append(createTextNode("b"))
      linkKey = link.getKey()
      try getRoot().clear().append(
        try createParagraphNode().append(createTextNode("a"), link, createTextNode("c")))
    }
    #expect(attributes(at: 1, in: update)?.strikethrough == true)
    #expect(attributes(at: 0, in: update)?.strikethrough == false)
    #expect(attributes(at: 2, in: update)?.strikethrough == false)

    // The page is created: the app marks the link dirty, out of history.
    let history = try getExtensionDependencyFromEditor(editor, HistoryExtension).output
    let undoDepth = history.historyState.value.undoStack.count
    pages.existing.insert("https://wiki/Missing")
    update = nil
    try editor.update(tag: HISTORY_MERGE_TAG, discrete: true) {
      try getNodeByKey(linkKey)?.markDirty()
    }
    #expect(attributes(at: 1, in: update)?.strikethrough == false)
    #expect(history.historyState.value.undoStack.count == undoDepth)
  }

  @Test func overridesChainAfterTheNodesOwnClassesAndMatchSubclasses() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test",
        dependencies: [
          RichTextExtension, LinkExtension,
          configExtension(TextKitRenderExtension) {
            $0.overrides = [
              textKitOverride(all: { _, next, _ in try next() + ["first"] }),
              textKitOverride([LinkNode.self]) { _, next, _ in try ["second"] + next() },
            ]
          },
          // A second configuration appends its overrides, which run last.
          configExtension(TextKitRenderExtension) {
            $0.overrides = [
              textKitOverride([LinkNode.self]) { _, next, _ in try next().filter { $0 != "first" } }
            ]
          },
        ], nodes: { [.klass(AutoLinkNode.self)] }))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      // An AutoLinkNode is a LinkNode, so the link overrides apply to it.
      let link = try createAutoLinkNode("https://a").append(createTextNode("a"))
      try getRoot().clear().append(try createParagraphNode().append(link))
    }
    try editor.read {
      let paragraph = try #require(getRoot().getFirstChild() as? ElementNode)
      let link = try #require(paragraph.getFirstChild() as? ElementNode)
      #expect(try TextKitElement(paragraph).classes == ["first"])
      #expect(try TextKitElement(link).classes == ["second"])
      #expect(try TextKitElement(link).themeKey == "link")
    }
  }
}
