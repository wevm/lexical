import Lexical
import LexicalA11y
import LexicalExtension
import LexicalLink
import LexicalRichText
import Testing

// Port of packages/lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts.
// The live region is a `LiveRegion` on the root element instead of a
// `div[aria-live]` in its document.

private final class LiveRegionHostRoot: LexicalRootElement, LiveRegionHost {
  var isConnected: Bool { true }
  var regions: [LiveRegion] = []

  func mountLiveRegion(_ region: LiveRegion) { regions.append(region) }
  func unmountLiveRegion(_ region: LiveRegion) { regions.removeAll { $0 === region } }
}

private func mountRoot(_ editor: Editor) throws -> LiveRegionHostRoot {
  let root = LiveRegionHostRoot()
  try editor.setRootElement(root)
  return root
}

/// The region's text. A repeat announcement gets a trailing zero-width space
/// so the DOM registers a change; strip it so assertions read naturally.
private func readLiveRegion(_ root: LiveRegionHostRoot) throws -> String {
  let region = try #require(root.regions.first)
  return String((region.textContent ?? "").filter { $0 != "\u{200B}" })
}

private func clearLiveRegion(_ root: LiveRegionHostRoot) {
  root.regions.first?.textContent = ""
}

/// Without matchers the transform unmakes the node it was just given, which is
/// a different scenario from typing an address.
nonisolated(unsafe) private let withMatchers = configExtension(AutoLinkExtension) {
  $0.matchers = [autoLinkUrlMatcher]
}

private func buildEditor() throws -> (Editor, LiveRegionHostRoot) {
  let editor = try buildEditorFromExtensions(
    defineExtension(name: "[root]", dependencies: [RichTextExtension, withMatchers]))
  return (editor, try mountRoot(editor))
}

/// Put a paragraph holding one automatic link into the document.
private func addAutoLink(_ editor: Editor, _ url: String = "https://example.com") throws {
  try editor.update(discrete: true) {
    let paragraph = try createParagraphNode()
    let link = try createAutoLinkNode(url)
    try link.append(try createTextNode(url))
    try paragraph.append(link)
    try getRoot().append(paragraph)
  }
}

private func onlyLink() throws -> ElementNode {
  let block = try #require(try getRoot().getLastChild() as? ElementNode)
  return try #require(try block.getFirstChild() as? ElementNode)
}

@Suite struct AutoLinkAnnounceExtensionTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > announces a typed address becoming a link"
    ))
  func announcesCreated() throws {
    let (editor, root) = try buildEditor()
    defer { editor.dispose() }
    try addAutoLink(editor)

    #expect(try readLiveRegion(root) == "Link")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > announces a link being removed"
    ))
  func announcesRemoved() throws {
    let (editor, root) = try buildEditor()
    defer { editor.dispose() }
    try addAutoLink(editor)
    clearLiveRegion(root)

    try editor.update(discrete: true) { try onlyLink().remove() }

    #expect(try readLiveRegion(root) == "Link removed")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > says how many links went when a selection takes several"
    ))
  func countsRemoved() throws {
    let (editor, root) = try buildEditor()
    defer { editor.dispose() }
    try addAutoLink(editor, "https://a.example")
    try addAutoLink(editor, "https://b.example")
    try addAutoLink(editor, "https://c.example")
    clearLiveRegion(root)

    try editor.update(discrete: true) {
      for paragraph in try getRoot().getChildren() {
        try paragraph.remove()
      }
    }

    #expect(try readLiveRegion(root) == "3 links removed")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > says how many links arrived when several appear at once"
    ))
  func countsCreated() throws {
    let (editor, root) = try buildEditor()
    defer { editor.dispose() }
    clearLiveRegion(root)

    // Two addresses on their own lines, arriving together as a paste would
    // bring them. Side by side in one paragraph they would run together into a
    // single address, which is a different thing entirely.
    try editor.update(discrete: true) {
      for url in ["https://a.example", "https://b.example"] {
        let paragraph = try createParagraphNode()
        let link = try createAutoLinkNode(url)
        try link.append(try createTextNode(url))
        try paragraph.append(link)
        try getRoot().append(paragraph)
      }
    }

    #expect(try readLiveRegion(root) == "2 links")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > stays silent when a link is rebuilt in place"
    ))
  func silentWhenRebuilt() throws {
    let (editor, root) = try buildEditor()
    defer { editor.dispose() }
    try addAutoLink(editor)
    clearLiveRegion(root)

    // What every keystroke does while an address is still being typed: the old
    // node is destroyed and a new one created in the same update. The link was
    // already there, so there is nothing to report.
    try editor.update(discrete: true) {
      let replacement = try createAutoLinkNode("https://example.com/deeper")
      try replacement.append(try createTextNode("https://example.com/deeper"))
      try onlyLink().replace(replacement)
    }

    #expect(try readLiveRegion(root) == "")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > stays silent while typing inside a link"
    ))
  func silentWhileTyping() throws {
    let (editor, root) = try buildEditor()
    defer { editor.dispose() }
    try addAutoLink(editor)
    clearLiveRegion(root)

    try editor.update(discrete: true) { try onlyLink().append(try createTextNode("/more")) }

    #expect(try readLiveRegion(root) == "")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > honours a configured message"
    ))
  func configuredMessage() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[root]",
        dependencies: [
          RichTextExtension,
          withMatchers,
          configExtension(AutoLinkAnnounceExtension) { $0.created = "Linked" },
        ]))
    defer { editor.dispose() }
    let root = try mountRoot(editor)

    try addAutoLink(editor)
    #expect(try readLiveRegion(root) == "Linked")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > says nothing when disabled"
    ))
  func silentWhenDisabled() throws {
    let (editor, root) = try buildEditor()
    defer { editor.dispose() }
    let disabled = try getExtensionDependencyFromEditor(editor, AutoLinkAnnounceExtension).output
      .disabled

    disabled.value = true
    try addAutoLink(editor)
    #expect(try readLiveRegion(root) == "")

    disabled.value = false
    try addAutoLink(editor, "https://second.example")
    #expect(try readLiveRegion(root) == "Link")
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/AutoLinkAnnounceExtension.test.ts::AutoLinkAnnounceExtension > leaves an editor without automatic links alone"
    ))
  func leavesOtherEditorsAlone() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "[root]", dependencies: [AriaLiveRegionExtension, RichTextExtension]))
    defer { editor.dispose() }
    let root = try mountRoot(editor)

    try editor.update(discrete: true) { try getRoot().append(try createParagraphNode()) }

    #expect(try readLiveRegion(root) == "")
  }
}
