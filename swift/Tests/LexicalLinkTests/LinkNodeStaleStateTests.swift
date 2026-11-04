import Lexical
import LexicalLink
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts.

@Suite struct LinkNodeStaleStateTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts::LinkNode stale state readers > isEmailURI and isWebSiteURI agree with getURL after setURL"
    ))
  func uriReadersAfterSetURL() throws {
    let editor = try createLinkTestEditor()
    var link: LinkNode!

    try update(editor) {
      link = try createLinkNode("https://lexical.dev/")
      try link.append(createTextNode("Hello"))
      try getRoot().append(createParagraphNode().append(link))
    }

    try update(editor) {
      // setURL() goes through getWritable(), which in a later update
      // clones the node. `link` still points at the previous version, so
      // any reader that skips getLatest() answers from stale state.
      try link.setURL("mailto:someone@example.com")

      #expect(try link.getURL() == "mailto:someone@example.com")
      #expect(try link.isEmailURI() == true)
      #expect(try link.isWebSiteURI() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts::LinkNode stale state readers > shouldMergeAdjacentLink compares the latest urls"
    ))
  func shouldMergeAdjacentLinkUsesLatest() throws {
    let editor = try createLinkTestEditor()
    var first: LinkNode!
    var second: LinkNode!

    try update(editor) {
      let paragraph = try createParagraphNode()
      first = try createLinkNode("https://lexical.dev/")
      second = try createLinkNode("https://lexical.dev/")
      try first.append(createTextNode("a"))
      try second.append(createTextNode("b"))
      try paragraph.append(first, second)
      try getRoot().append(paragraph)
    }

    try update(editor) {
      try first.setURL("https://example.com/")

      #expect(try first.getURL() != second.getURL())
      #expect(try first.shouldMergeAdjacentLink(second) == false)
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkNodeStaleState.test.ts::LinkNode stale state readers > getIsUnlinked reflects the latest value"
    ))
  func getIsUnlinkedUsesLatest() throws {
    let editor = try createLinkTestEditor()
    var autoLink: AutoLinkNode!

    try update(editor) {
      autoLink = try createAutoLinkNode("https://lexical.dev/")
      try autoLink.append(createTextNode("Hello"))
      try getRoot().append(createParagraphNode().append(autoLink))
    }

    try update(editor) {
      try autoLink.setIsUnlinked(true)

      #expect(try autoLink.getIsUnlinked() == true)
    }
  }
}
