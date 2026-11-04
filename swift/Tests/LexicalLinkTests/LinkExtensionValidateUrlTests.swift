import Lexical
import LexicalExtension
import LexicalLink
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts.

private let ALLOWED = "https://lexical.dev/"
// Rejected by the configured policy below (https-only): an allowlist, not a
// scheme blocklist.
private let REJECTED = "http://not-allowed.example.com/"

private func makeExtension(_ validateUrl: ((String) -> Bool)?) -> any AnyLexicalExtensionArgument {
  defineExtension(
    name: "[root-validate-url]",
    dependencies: [
      validateUrl == nil
        ? configExtension(LinkExtension) { _ in }
        : configExtension(LinkExtension) { $0.validateUrl = validateUrl },
      RichTextExtension,
    ],
    editorConfig: InitialEditorConfig(
      initialEditorState: .update { _ in
        try getRoot().append(createParagraphNode().append(createTextNode("Hello")))
      }))
}

/// Dispatches `payload` over a selection of "Hello", and reports whether
/// the command claimed it and whether a LinkNode was made.
private func toggle(
  _ payload: ToggleLinkPayload, _ validateUrl: ((String) -> Bool)? = nil
) throws
  -> (handled: Bool, linked: Bool)
{
  let editor = try buildEditorFromExtensions(makeExtension(validateUrl))
  defer { editor.dispose() }
  var handled = false
  try editor.update(discrete: true) {
    let textNode = try getRoot().getLastDescendant() as! TextNode
    try textNode.select(0, textNode.getTextContentSize())
    handled = try editor.dispatchCommand(TOGGLE_LINK_COMMAND, payload)
  }
  let linked = try editor.read {
    try (getRoot().getFirstChild() as! ParagraphNode).getFirstChild() is LinkNode
  }
  return (handled, linked)
}

private func isHttps(_ url: String) -> Bool { url.hasPrefix("https://") }

@Suite struct LinkExtensionValidateUrlTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > rejects a URL the validator refuses, for an object payload"
    ))
  func rejectsObjectPayload() throws {
    let result = try toggle(.attributes(url: REJECTED, LinkAttributes()), isHttps)
    #expect(result == (false, false))
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > rejects a URL the validator refuses, for a string payload"
    ))
  func rejectsStringPayload() throws {
    let result = try toggle(.url(REJECTED), isHttps)
    #expect(result == (false, false))
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > accepts a URL the validator allows, for an object payload"
    ))
  func acceptsObjectPayload() throws {
    let result = try toggle(.attributes(url: ALLOWED, LinkAttributes()), isHttps)
    #expect(result == (true, true))
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkExtensionValidateUrl.test.ts::LinkExtension validateUrl > accepts any URL when no validator is configured"
    ))
  func acceptsWithoutValidator() throws {
    let object = try toggle(.attributes(url: REJECTED, LinkAttributes()))
    #expect(object == (true, true))
    let string = try toggle(.url(REJECTED))
    #expect(string == (true, true))
  }
}
