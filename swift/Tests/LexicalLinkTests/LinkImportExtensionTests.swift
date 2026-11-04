import Lexical
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LinkImportExtension.test.ts.

/// LinkExtension registers its own import rules (and the shared
/// CoreImportExtension baseline); no dedicated import extension is needed.
private func buildEditor(
  _ dependencies: [any AnyLexicalExtensionArgument] = [LinkExtension], name: String = "link-host"
) throws -> Editor {
  try buildEditorFromExtensions(defineExtension(name: name, dependencies: dependencies))
}

private func buildRichEditor() throws -> Editor {
  try buildEditor([LinkExtension, RichTextExtension], name: "rich-link-host")
}

private func generate(_ html: String) throws -> [LexicalNode] {
  let dep = try getExtensionDependencyFromEditor(getEditor(), DOMImportExtension)
  let dom = DOMParser().parseFromString("<!doctype html><html><body>\(html)</body></html>")
  return try dep.output.generateNodesFromDOM(dom)
}

private func importInto(_ editor: Editor, _ html: String) throws {
  try editor.update(discrete: true) { try getRoot().clear().append(contentsOf: generate(html)) }
}

private func firstLink() throws -> LinkNode {
  let para = try #require(try getRoot().getFirstChild() as? ParagraphNode, "expected paragraph")
  return try #require(try para.getFirstChild() as? LinkNode, "expected LinkNode")
}

@Suite struct LinkImportExtensionTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension > <a href=\"…\">text</a> → LinkNode with TextNode child"
    ))
  func anchorToLink() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<p><a href="https://example.com">click</a></p>"#)
    try editor.read { () throws -> Void in
      let link = try firstLink()
      #expect(try link.getURL() == "https://example.com")
      #expect(try link.getTextContent() == "click")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension > rel, target, title preserved"
    ))
  func attributesPreserved() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(
      editor, #"<p><a href="/x" target="_blank" rel="noopener" title="hello">x</a></p>"#)
    try editor.read { () throws -> Void in
      let link = try firstLink()
      #expect(try link.getTarget() == "_blank")
      #expect(try link.getRel() == "noopener")
      #expect(try link.getTitle() == "hello")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension > empty <a> with no children is skipped"
    ))
  func emptyAnchorSkipped() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<p>before<a href="/x"></a>after</p>"#)
    try editor.read { () throws -> Void in
      let para = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      #expect(try para.getTextContent() == "beforeafter")
    }
  }
}

@Suite struct LinkImportBlockChildrenTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension — block children lifted out of inline parent > <a><h1>x</h1><div>y</div></a> lifts the heading and re-wraps both runs with the link"
    ))
  func liftsHeadingAndDiv() throws {
    let editor = try buildRichEditor()
    defer { editor.dispose() }
    try importInto(
      editor, #"<a href="https://example.com"><h1>some text</h1><div>more text</div></a>"#)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 2)
      let heading = try #require(children[0] as? HeadingNode, "expected HeadingNode at index 0")
      let headingLink = try #require(
        try heading.getFirstChild() as? LinkNode, "expected LinkNode inside heading")
      #expect(try headingLink.getURL() == "https://example.com")
      #expect(try headingLink.getTextContent() == "some text")
      let paragraph = try #require(
        children[1] as? ParagraphNode, "expected ParagraphNode at index 1")
      let paragraphLink = try #require(
        try paragraph.getFirstChild() as? LinkNode, "expected LinkNode inside paragraph")
      #expect(try paragraphLink.getURL() == "https://example.com")
      #expect(try paragraphLink.getTextContent() == "more text")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension — block children lifted out of inline parent > <a><h1>x</h1>y<h1>z</h1></a> wraps the middle inline run in its own paragraph + link"
    ))
  func wrapsMiddleRun() throws {
    let editor = try buildRichEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<a href="/u"><h1>x</h1>middle<h1>z</h1></a>"#)
    try editor.read { () throws -> Void in
      let children = try getRoot().getChildren()
      #expect(children.count == 3)
      let h1 = try #require(children[0] as? HeadingNode)
      let h1Link = try #require(try h1.getFirstChild() as? LinkNode)
      #expect(try h1Link.getTextContent() == "x")
      let mid = try #require(children[1] as? ParagraphNode)
      let midLink = try #require(try mid.getFirstChild() as? LinkNode)
      #expect(try midLink.getTextContent() == "middle")
      // Each lifted block gets its own fresh link wrapper.
      #expect(!midLink.is(h1Link))
      let h3 = try #require(children[2] as? HeadingNode)
      let h3Link = try #require(try h3.getFirstChild() as? LinkNode)
      #expect(try h3Link.getTextContent() == "z")
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkImportExtension.test.ts::LinkImportExtension — block children lifted out of inline parent > all-inline <a> retains the single-wrapping fast path"
    ))
  func allInline() throws {
    let editor = try buildRichEditor()
    defer { editor.dispose() }
    try importInto(editor, #"<p><a href="/x">just text</a></p>"#)
    try editor.read { () throws -> Void in
      let para = try #require(try getRoot().getFirstChild() as? ParagraphNode)
      let link = try #require(try para.getFirstChild() as? LinkNode)
      #expect(try link.getTextContent() == "just text")
      #expect(try link.getURL() == "/x")
    }
  }
}
