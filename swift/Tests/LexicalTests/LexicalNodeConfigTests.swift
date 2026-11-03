import LexicalTesting
import Testing

@testable import Lexical

// Port of the `LexicalNode.$config()` tests of LexicalNode.test.ts. In
// Swift, `config()` is the `$config()` override and the default `clone`,
// `importJSON` and `exportJSON` are what JS synthesizes from it.

private func discreteEditor(_ nodes: [LexicalNode.Type]) throws -> Editor {
  try createEditor(CreateEditorArgs(nodes: nodes.map { .klass($0) }, onError: { throw $0 }))
}

final class CustomTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(CustomTextNode.self, type: "custom-text", extends: TextNode.self)
  }
}

final class SNCVersionedTextNode: TextNode {
  var __version = 0
  override class func config() -> NodeConfig? {
    NodeConfig(SNCVersionedTextNode.self, type: "snc-vtext", extends: TextNode.self)
  }
  override func afterCloneFrom(_ node: LexicalNode) throws {
    try super.afterCloneFrom(node)
    __version = (node as! SNCVersionedTextNode).__version + 1
  }
}

final class ConfigTagNode: ElementNode {
  var __tag = "default"
  override class func config() -> NodeConfig? {
    NodeConfig(ConfigTagNode.self, type: "config-tag", extends: ElementNode.self)
  }
  override func afterCloneFrom(_ node: LexicalNode) throws {
    try super.afterCloneFrom(node)
    __tag = (node as! ConfigTagNode).__tag
  }
  @discardableResult
  func setTag(_ tag: String) throws -> Self {
    let writable = try getWritable()
    writable.__tag = tag
    return writable
  }
}

final class ConfigVersionNode: ElementNode {
  var __version = 0
  override class func config() -> NodeConfig? {
    NodeConfig(ConfigVersionNode.self, type: "config-version", extends: ElementNode.self)
  }
  override func afterCloneFrom(_ node: LexicalNode) throws {
    try super.afterCloneFrom(node)
    __version = (node as! ConfigVersionNode).__version + 1
  }
}

final class InnerTagNode: ElementNode {
  var __tag = "default"
  override class func config() -> NodeConfig? {
    NodeConfig(InnerTagNode.self, type: "inner-tag", extends: ElementNode.self)
  }
  override func afterCloneFrom(_ node: LexicalNode) throws {
    try super.afterCloneFrom(node)
    __tag = (node as! InnerTagNode).__tag
  }
  @discardableResult
  func setTag(_ tag: String) throws -> Self {
    let writable = try getWritable()
    writable.__tag = tag
    return writable
  }
}

final class OuterNode: ElementNode {
  var __reentrantClone: InnerTagNode?
  var __templateSource: InnerTagNode?
  override class func config() -> NodeConfig? {
    NodeConfig(OuterNode.self, type: "outer-reentrant", extends: ElementNode.self)
  }
  override func afterCloneFrom(_ node: LexicalNode) throws {
    try super.afterCloneFrom(node)
    if let source = (node as! OuterNode).__templateSource {
      __reentrantClone = try InnerTagNode.clone(source) as? InnerTagNode
    }
  }
  @discardableResult
  func setTemplateSource(_ source: InnerTagNode) throws -> Self {
    let writable = try getWritable()
    writable.__templateSource = source
    return writable
  }
}

@Suite struct LexicalNodeConfigTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > importJSON() with no boilerplate"
    ))
  func importJSONWithoutBoilerplate() throws {
    let editor = try discreteEditor([CustomTextNode.self])
    try editor.update(discrete: true) { () throws in
      let node = try CustomTextNode.importJSON(
        JSONObject([
          ("detail", 0), ("format", 0), ("mode", "normal"), ("style", ""), ("text", "codegen!"),
          ("type", "custom-text"), ("version", 1),
        ]))
      #expect(node is CustomTextNode)
      #expect(node.getType() == "custom-text")
      #expect(try node.getTextContent() == "codegen!")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > clone() with no boilerplate"
    ))
  func cloneWithoutBoilerplate() throws {
    let editor = try discreteEditor([SNCVersionedTextNode.self])
    var versionedTextNode: SNCVersionedTextNode!
    try editor.update(discrete: true) { () throws in
      versionedTextNode = try create(SNCVersionedTextNode.self).setTextContent("test")
      try getRoot().append(createParagraphNode().append(versionedTextNode))
      #expect(versionedTextNode.__version == 0)
    }
    try editor.update(discrete: true) { () throws in
      #expect(try versionedTextNode.getLatest().__version == 0)
      let latest = try versionedTextNode.setTextContent("update").setMode("token")
      #expect(latest.__text == "update" && latest.__version == 1)
      #expect(versionedTextNode.__text == "test" && versionedTextNode.__version == 0)
    }
    try editor.update(discrete: true) { () throws in
      var latest = try versionedTextNode.getLatest()
      #expect(versionedTextNode.__version == 0)
      #expect(versionedTextNode.__mode == 0)
      #expect(try versionedTextNode.getMode() == "token")
      #expect(latest.__version == 1)
      #expect(latest.__mode == 1)
      latest = try latest.setTextContent("another update")
      #expect(latest.__version == 2)
      #expect(try latest.getWritable().__version == 2)
      #expect(try versionedTextNode.getLatest().getWritable().__version == 2)
      #expect(try versionedTextNode.getLatest().__version == 2)
      #expect(versionedTextNode.__mode == 0)
      #expect(try versionedTextNode.getLatest().__mode == 1)
      #expect(try versionedTextNode.getMode() == "token")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > direct clone() of an auto-synthesized node preserves properties"
    ))
  func directClonePreservesProperties() throws {
    let editor = try discreteEditor([ConfigTagNode.self])
    try editor.update(discrete: true) { () throws in
      let source = try create(ConfigTagNode.self).setTag("custom")
      try getRoot().append(source)
      #expect(source.__tag == "custom")
      let directClone = try #require(try ConfigTagNode.clone(source) as? ConfigTagNode)
      #expect(directClone.__tag == "custom")
      let wrapperClone = try cloneWithProperties(source)
      #expect(wrapperClone.__tag == "custom")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > afterCloneFrom runs exactly once for auto-synthesized clone"
    ))
  func afterCloneFromRunsOnce() throws {
    let editor = try discreteEditor([ConfigVersionNode.self])
    try editor.update(discrete: true) { () throws in
      let source = try create(ConfigVersionNode.self)
      try getRoot().append(source)
      #expect(source.__version == 0)
      let cloned = try cloneWithProperties(source)
      #expect(cloned.__version == 1)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalNode.test.ts::LexicalNode tests > LexicalNode.$config() > reentrant direct clone during another clone preserves properties"
    ))
  func reentrantClone() throws {
    let editor = try discreteEditor([InnerTagNode.self, OuterNode.self])
    try editor.update(discrete: true) { () throws in
      let inner = try create(InnerTagNode.self).setTag("custom")
      let outer = try create(OuterNode.self).setTemplateSource(inner)
      try getRoot().append(inner, outer)
      let clonedOuter = try cloneWithProperties(outer)
      let reentrant = try #require(clonedOuter.__reentrantClone)
      #expect(reentrant.__tag == "custom")
    }
  }
}
