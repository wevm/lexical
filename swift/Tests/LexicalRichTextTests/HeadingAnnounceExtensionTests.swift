import Lexical
import LexicalA11y
import LexicalExtension
import LexicalPlainText
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts.
// The live region is a `LiveRegion` on the root element instead of a
// `div[aria-live]` in its document.

private final class LiveRegionHostRoot: LexicalRootElement, LiveRegionHost {
  var isConnected: Bool { true }
  var regions: [LiveRegion] = []

  func mountLiveRegion(_ region: LiveRegion) { regions.append(region) }
  func unmountLiveRegion(_ region: LiveRegion) { regions.removeAll { $0 === region } }
}

private func buildEditor(
  _ dependencies: [any AnyLexicalExtensionArgument]
) throws -> (
  Editor, LiveRegionHostRoot
) {
  let editor = try buildEditorFromExtensions(
    defineExtension(name: "[root]", dependencies: dependencies))
  let root = LiveRegionHostRoot()
  try editor.setRootElement(root)
  return (editor, root)
}

/// The region's text, with the repeat marker stripped.
private func readLiveRegion(_ root: LiveRegionHostRoot) -> String {
  String((root.regions.first?.textContent ?? "").filter { $0 != "\u{200B}" })
}

/// Appends a heading, as typing the markdown shortcut on a fresh block does.
private func addHeading(_ editor: Editor, _ tag: HeadingTagType, _ text: String = "Title") throws {
  try editor.update(discrete: true) {
    let heading = try createHeadingNode(tag)
    try heading.append(try createTextNode(text))
    try getRoot().append(heading)
  }
}

/// Removes the last block, as backspacing a heading away does.
private func removeLastBlock(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    try getRoot().getLastChild()?.remove()
  }
}

@Suite struct HeadingAnnounceExtensionTests {
  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > announces every heading level as it is created"
    ))
  func announcesEveryLevel() throws {
    let (editor, root) = try buildEditor([RichTextExtension])
    defer { editor.dispose() }
    for tag in ["h1", "h2", "h3", "h4", "h5", "h6"] {
      try addHeading(editor, tag)
      #expect(readLiveRegion(root) == "Heading level \(tag.dropFirst())")
    }
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > announces the level a heading had when it is removed"
    ))
  func announcesRemovedLevel() throws {
    let (editor, root) = try buildEditor([RichTextExtension])
    defer { editor.dispose() }
    try addHeading(editor, "h2")
    #expect(readLiveRegion(root) == "Heading level 2")
    try removeLastBlock(editor)
    #expect(readLiveRegion(root) == "Heading level 2 removed")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > stays silent while editing inside a heading"
    ))
  func staysSilentWhileEditing() throws {
    let (editor, root) = try buildEditor([RichTextExtension])
    defer { editor.dispose() }
    try addHeading(editor, "h3", "Before")
    #expect(readLiveRegion(root) == "Heading level 3")

    root.regions.first?.textContent = ""
    try editor.update(discrete: true) {
      let heading = try #require(try getRoot().getFirstChild() as? ElementNode)
      try heading.selectEnd().insertText(" and after")
    }
    #expect(readLiveRegion(root) == "")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > announces the new level when a heading changes level"
    ))
  func announcesNewLevel() throws {
    let (editor, root) = try buildEditor([RichTextExtension])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(try createHeadingNode("h1"))
    }
    #expect(readLiveRegion(root) == "Heading level 1")
    try editor.update(discrete: true) {
      try getRoot().clear().append(try createHeadingNode("h2"))
    }
    #expect(readLiveRegion(root) == "Heading level 2")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > respects message overrides from configExtension"
    ))
  func respectsOverrides() throws {
    let (editor, root) = try buildEditor([
      configExtension(HeadingAnnounceExtension) {
        $0.created = "Now an H%s"
        $0.destroyed = "H%s gone"
      },
      RichTextExtension,
    ])
    defer { editor.dispose() }
    try addHeading(editor, "h4")
    #expect(readLiveRegion(root) == "Now an H4")
    try removeLastBlock(editor)
    #expect(readLiveRegion(root) == "H4 gone")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > reflects message signal changes at runtime"
    ))
  func reflectsSignals() throws {
    let (editor, root) = try buildEditor([RichTextExtension])
    defer { editor.dispose() }
    let output = try getExtensionDependencyFromEditor(editor, HeadingAnnounceExtension).output
    output.created.value = "Section, depth %s"
    try addHeading(editor, "h2")
    #expect(readLiveRegion(root) == "Section, depth 2")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > does not announce while disabled, and resumes when re-enabled"
    ))
  func respectsDisabled() throws {
    let (editor, root) = try buildEditor([RichTextExtension])
    defer { editor.dispose() }
    let output = try getExtensionDependencyFromEditor(editor, HeadingAnnounceExtension).output
    output.disabled.value = true
    try addHeading(editor, "h1")
    #expect(readLiveRegion(root) == "")
    output.disabled.value = false
    try addHeading(editor, "h2")
    #expect(readLiveRegion(root) == "Heading level 2")
  }

  @Test(
    .upstream(
      "lexical-rich-text/src/__tests__/unit/HeadingAnnounceExtension.test.ts::HeadingAnnounceExtension > leaves a plain text editor alone"
    ))
  func leavesPlainTextAlone() throws {
    let (editor, root) = try buildEditor([AriaLiveRegionExtension, PlainTextExtension])
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().append(try createParagraphNode())
    }
    #expect(root.regions.count == 1)
    #expect(readLiveRegion(root) == "")
  }
}
