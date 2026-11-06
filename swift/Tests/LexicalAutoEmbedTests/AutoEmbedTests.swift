import Lexical
import LexicalAutoEmbed
import LexicalExtension
import LexicalLink
import LexicalMenu
import LexicalRichText
import LexicalTesting
import LexicalUtils
import Testing

// Port of packages/lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx.
// JS renders the menu with a `menuRenderFn` and clicks its button; here
// the menu's options stand for what it draws, and choosing one is the
// click.

private let YOUTUBE_URL = "https://www.youtube.com/watch?v=jNQXAC9IVRw"
private let SHORT_YOUTUBE_URL = "youtu.be/jNQXAC9IVRw"
nonisolated(unsafe) private let youTubeID = try! Regex(
  #"(?:youtube\.com\/watch\?v=|youtu\.be\/)([\w-]+)"#)

/// A `parseUrl` that records its calls, and can be made to wait.
@MainActor
private final class TestEmbedConfig: EmbedConfig {
  let type = "youtube-video"
  var parsed: [String] = []
  var inserted: [(id: String, url: String)] = []
  /// Each parse waits for the next of these, while there are any.
  var deferred: [CheckedContinuation<EmbedMatchResult?, Never>] = []
  var deferNext = 0

  func parseUrl(_ url: String) async -> EmbedMatchResult? {
    parsed.append(url)
    if deferNext > 0 {
      deferNext -= 1
      return await withCheckedContinuation { deferred.append($0) }
    }
    guard let match = url.firstMatch(of: youTubeID), let id = match.output[1].substring else {
      return nil
    }
    return EmbedMatchResult(url: url, id: String(id))
  }

  func insertNode(_ editor: Editor, _ result: EmbedMatchResult) throws {
    inserted.append((result.id, result.url))
  }

  /// `deferParseUrl`: the next parse waits until the returned function
  /// gives its result.
  func deferParseUrl() -> (EmbedMatchResult?) -> Void {
    deferNext += 1
    let index = deferred.count
    return { [self] result in deferred[index].resume(returning: result) }
  }
}

@MainActor
private struct Fixture {
  let editor: Editor
  let config = TestEmbedConfig()
  let menu: NodeMenu<AutoEmbedOption>
  let unregister: () -> Void

  init(disableMenuFor: ((LexicalNode) throws -> Bool)? = nil) throws {
    editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[test-auto-embed]",
        dependencies: [
          RichTextExtension,
          configExtension(AutoLinkExtension) {
            $0.matchers = [
              createLinkMatcherWithRegExp(URL_MATCHER),
              // A scheme-less matcher, broader than URL_MATCHER, as an app
              // might add.
              createLinkMatcherWithRegExp(try! Regex(#"youtu\.be\/[\w-]+"#)),
            ]
          },
        ]))
    // Mounted, so commits reconcile and report mutations.
    try editor.setRootElement(TestRootElement())
    (menu, unregister) = try registerAutoEmbed(
      editor, embedConfigs: [config],
      getMenuOptions: { _, embed, _ in [AutoEmbedOption("Embed", onSelect: { _ in embed() })] },
      disableMenuFor: disableMenuFor)
  }

  /// Whether the menu is drawn: open, with options.
  var isMenuShown: Bool { menu.menu.resolution != nil && !menu.menu.options.isEmpty }

  func paste(
    _ data: [(String, String)], select: @escaping () throws -> Void = { try getRoot().selectEnd() }
  ) async throws {
    let clipboardData = DataTransfer()
    for (type, value) in data { clipboardData.setData(type, value) }
    try editor.update(discrete: true) {
      try select()
      _ = try editor.dispatchCommand(
        PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: clipboardData)))
    }
    await settle()
  }

  /// Lets the `parseUrl` checks run.
  func settle() async {
    for _ in 0..<20 { await Task.yield() }
  }

  func selectEmbedOption() throws {
    let option = try #require(menu.menu.options.first)
    menu.menu.selectOptionAndCleanUp(option)
  }

  func paragraph() throws -> ParagraphNode {
    try #require(try getRoot().getFirstChild() as? ParagraphNode)
  }

  func pastedLink() throws -> LinkNode {
    try #require(try paragraph().getFirstChild() as? LinkNode)
  }

  func text() throws -> String { try editor.read { try getRoot().getTextContent() } }
}

private func plain(_ text: String) -> [(String, String)] { [("text/plain", text)] }

@MainActor
@Suite struct AutoEmbedTests {
  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > embeds an unchanged link without parsing its URL again"
    ))
  func embedsAnUnchangedLink() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste(plain(YOUTUBE_URL))
    try f.selectEmbedOption()
    #expect(f.config.parsed.count == 1)
    #expect(f.config.inserted.map(\.id) == ["jNQXAC9IVRw"])
    #expect(f.config.inserted.map(\.url) == [YOUTUBE_URL])
    #expect(try f.text() == "")
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > embeds an unchanged link with no selection"
    ))
  func embedsWithNoSelection() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste(plain(YOUTUBE_URL))
    try f.editor.update { try setSelection(nil) }
    try f.selectEmbedOption()
    #expect(f.config.parsed.count == 1)
    #expect(f.config.inserted.map(\.id) == ["jNQXAC9IVRw"])
    #expect(try f.text() == "")
    #expect(try f.editor.read { try getSelection() } != nil)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not embed a link removed by a pending update"
    ))
  func skipsARemovedLink() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste(plain(YOUTUBE_URL))
    let option = try #require(f.menu.menu.options.first)
    try f.editor.update {
      try f.pastedLink().remove()
      _ = try f.paragraph().selectEnd()
    }
    f.menu.menu.selectOptionAndCleanUp(option)
    #expect(f.config.inserted.isEmpty)
    #expect(try f.text() == "")
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not embed a link whose URL changed in a pending update"
    ))
  func skipsALinkWhoseURLChanged() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste(plain(YOUTUBE_URL))
    let option = try #require(f.menu.menu.options.first)
    try f.editor.update { _ = try f.pastedLink().setURL("https://example.com/") }
    f.menu.menu.selectOptionAndCleanUp(option)
    #expect(f.config.inserted.isEmpty)
    #expect(try f.editor.read { try f.pastedLink().getURL() } == "https://example.com/")
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not let an older paste that resolves late replace the menu"
    ))
  func aLateOlderPasteKeepsTheNewerMenu() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    let otherURL = "https://www.youtube.com/watch?v=OTHERvid123"
    let resolveFirst = f.config.deferParseUrl()
    try await f.paste(plain(YOUTUBE_URL))
    #expect(!f.isMenuShown)
    try await f.paste(plain(otherURL)) {
      let paragraph = try createParagraphNode()
      try getRoot().append(paragraph)
      _ = try paragraph.select()
    }
    #expect(f.isMenuShown)
    resolveFirst(EmbedMatchResult(url: YOUTUBE_URL, id: "jNQXAC9IVRw"))
    await f.settle()
    try f.selectEmbedOption()
    #expect(f.config.inserted.map(\.id) == ["OTHERvid123"])
    #expect(f.config.inserted.map(\.url) == [otherURL])
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a link whose URL changed while parsing"
    ))
  func noMenuForALinkThatChangedWhileParsing() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    let resolve = f.config.deferParseUrl()
    try await f.paste(plain(YOUTUBE_URL))
    #expect(!f.isMenuShown)
    try f.editor.update { _ = try f.pastedLink().setURL("https://example.com/") }
    resolve(EmbedMatchResult(url: YOUTUBE_URL, id: "jNQXAC9IVRw"))
    await f.settle()
    #expect(!f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a link with a pending URL change when parsing resolves"
    ))
  func noMenuForAPendingURLChange() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    let resolve = f.config.deferParseUrl()
    try await f.paste(plain(YOUTUBE_URL))
    #expect(!f.isMenuShown)
    resolve(EmbedMatchResult(url: YOUTUBE_URL, id: "jNQXAC9IVRw"))
    // Not committed before the check runs.
    try f.editor.update { _ = try f.pastedLink().setURL("https://example.com/") }
    await f.settle()
    #expect(!f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a bare URL pasted as plain text"
    ))
  func offersABareURL() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste(plain(YOUTUBE_URL))
    #expect(f.config.parsed == [YOUTUBE_URL])
    #expect(f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a pasted sentence that contains a URL"
    ))
  func noMenuForASentenceWithALink() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste([
      ("text/html", #"<p>Look at this <a href="\#(YOUTUBE_URL)">\#(YOUTUBE_URL)</a> please</p>"#),
      ("text/plain", "Look at this \(YOUTUBE_URL) please"),
    ])
    #expect(f.config.parsed.isEmpty)
    #expect(!f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a pasted plain text sentence that contains a URL"
    ))
  func noMenuForAPlainSentence() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste(plain("Look at this \(YOUTUBE_URL) please"))
    #expect(f.config.parsed.isEmpty)
    #expect(!f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a copied link whose text is its URL"
    ))
  func offersACopiedLink() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste([
      ("text/html", #"<a href="\#(YOUTUBE_URL)">\#(YOUTUBE_URL)</a>"#), ("text/plain", YOUTUBE_URL),
    ])
    #expect(f.config.parsed == [YOUTUBE_URL])
    #expect(f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not offer to embed a copied link with a label"
    ))
  func noMenuForALabeledLink() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste([
      ("text/html", #"<a href="\#(YOUTUBE_URL)">first video</a>"#), ("text/plain", "first video"),
    ])
    #expect(f.config.parsed.isEmpty)
    #expect(!f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a bare address that only a custom matcher links"
    ))
  func offersACustomMatchersAddress() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste(plain(SHORT_YOUTUBE_URL))
    #expect(f.config.parsed == [SHORT_YOUTUBE_URL])
    #expect(f.isMenuShown)
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > still offers to embed a bare URL pasted after typed text"
    ))
  func offersAURLAfterTypedText() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try f.editor.update {
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("Check this out: ")))
    }
    try await f.paste(plain(YOUTUBE_URL))
    #expect(f.config.parsed == [YOUTUBE_URL])
    #expect(f.isMenuShown)
    #expect(try f.text() == "Check this out: \(YOUTUBE_URL)")
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > offers to embed a bare URL pasted into formatted text"
    ))
  func offersAURLPastedIntoFormattedText() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    var middleKey = ""
    try f.editor.update {
      let middle = try createTextNode(" and ")
      middleKey = middle.getKey()
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("Read "), createTextNode("this").toggleFormat("bold"), middle,
          createTextNode("that").toggleFormat("italic"), createTextNode(" now")))
    }
    var pasteDirtyLeaves = 0
    let removeUpdateListener = f.editor.registerUpdateListener { payload in
      if payload.tags.has(PASTE_TAG) { pasteDirtyLeaves = payload.dirtyLeaves.size }
    }
    // Paste over the word between the bold and italic runs.
    try await f.paste(plain(YOUTUBE_URL)) {
      _ = try #require(try getNodeByKey(middleKey) as? TextNode).select(1, 4)
    }
    removeUpdateListener()
    // The formatted siblings and the split text nodes are all dirty, so a
    // dirty leaf count cannot tell this paste apart from pasted prose.
    #expect(pasteDirtyLeaves > 3)
    #expect(f.config.parsed == [YOUTUBE_URL])
    #expect(f.isMenuShown)
    #expect(try f.text() == "Read this \(YOUTUBE_URL) that now")
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not throw when the clipboard reports no plain text"
    ))
  func noPlainText() async throws {
    let f = try Fixture()
    defer { f.unregister() }
    try await f.paste([("text/html", "replaced")])
    #expect(f.config.parsed.isEmpty)
    #expect(!f.isMenuShown)
    #expect(try f.text() == "replaced")
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > does not show the menu when a $disableMenuFor predicate matches the link location"
    ))
  func disableMenuForSuppressesTheMenu() async throws {
    let f = try Fixture(disableMenuFor: { try getNearestNodeOfType($0, QuoteNode.self) != nil })
    defer { f.unregister() }
    try f.editor.update(discrete: true) {
      try getRoot().append(createQuoteNode().append(createParagraphNode()))
    }
    try await f.paste(plain(YOUTUBE_URL)) {
      let quote = try #require(try getRoot().getLastChild() as? QuoteNode)
      _ = try #require(try quote.getFirstChild() as? ParagraphNode).selectEnd()
    }
    // The menu never opens, so the URL is never even parsed and the pasted
    // link is left untouched.
    #expect(f.config.parsed.isEmpty)
    #expect(!f.isMenuShown)
    try f.editor.read {
      let quote = try #require(try getRoot().getLastChild() as? QuoteNode)
      let paragraph = try #require(try quote.getFirstChild() as? ParagraphNode)
      #expect(try paragraph.getFirstChild() is LinkNode)
    }
  }

  @Test(
    .upstream(
      "lexical-react/src/__tests__/unit/LexicalAutoEmbedPlugin.test.tsx::LexicalAutoEmbedPlugin > shows the menu when no $disableMenuFor predicate matches"
    ))
  func disableMenuForLeavesOtherLinks() async throws {
    let f = try Fixture(disableMenuFor: { try getNearestNodeOfType($0, QuoteNode.self) != nil })
    defer { f.unregister() }
    // The default paste target is the root paragraph, outside any quote.
    try await f.paste(plain(YOUTUBE_URL))
    #expect(f.isMenuShown)
    try f.selectEmbedOption()
    #expect(f.config.inserted.map(\.id) == ["jNQXAC9IVRw"])
  }
}
