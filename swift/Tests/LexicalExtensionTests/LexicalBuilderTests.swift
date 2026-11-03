import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts.

private let InitialStateExtensionName = "@lexical/extension/InitialState"

private struct ABConfig: Equatable {
  var a = 1
  var b: String? = "b"
}

private let ConfigExtension = defineExtension(name: "Config", config: ABConfig())

private func reps(_ editor: Editor) throws -> [ExtensionRep] {
  try LexicalBuilder.fromEditor(editor).sortedExtensionReps()
}

private func config<C>(of extension_: any AnyLexicalExtension, in editor: Editor) throws -> C {
  let rep = try #require(try reps(editor).first { $0.extension_ === extension_ })
  return try rep.getState().config(as: C.self)
}

@Suite struct LexicalBuilderTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (siblings)"
    ))
  func mergesSiblings() throws {
    let editor = try buildEditorFromExtensions(
      ConfigExtension, configExtension(ConfigExtension) { $0.set(\.b, nil) })
    defer { editor.dispose() }
    let reps = try reps(editor)
    #expect(reps.count == 2)
    #expect(reps.last!.extension_ === ConfigExtension)
    #expect(try reps.last!.getState().config(as: ABConfig.self) == ABConfig(a: 1, b: nil))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (parent override child)"
    ))
  func mergesParentOverrideChild() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "parent", dependencies: [configExtension(ConfigExtension) { $0.set(\.b, nil) }]))
    defer { editor.dispose() }
    #expect(try reps(editor).count == 3)
    #expect(try config(of: ConfigExtension, in: editor) == ABConfig(a: 1, b: nil))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (grandparent override child) override after"
    ))
  func mergesGrandparentAfter() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "grandparent",
        dependencies: [
          defineExtension(name: "parent", dependencies: [ConfigExtension]),
          configExtension(ConfigExtension) { $0.set(\.b, nil) },
        ]))
    defer { editor.dispose() }
    #expect(try reps(editor).count == 4)
    #expect(try config(of: ConfigExtension, in: editor) == ABConfig(a: 1, b: nil))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > merges extension configs (grandparent override child) override before"
    ))
  func mergesGrandparentBefore() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "grandparent",
        dependencies: [
          configExtension(ConfigExtension) { $0.set(\.b, nil) },
          defineExtension(name: "parent", dependencies: [ConfigExtension]),
        ]))
    defer { editor.dispose() }
    #expect(try reps(editor).count == 4)
    #expect(try config(of: ConfigExtension, in: editor) == ABConfig(a: 1, b: nil))
  }

  private struct PairConfig: Equatable {
    var first = "default"
    var second = "default"
  }

  private static let PairExtension = defineExtension(name: "Pair", config: PairConfig())

  private func configOf(_ dependencies: any AnyLexicalExtensionArgument...) throws -> PairConfig {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "root", dependencies: dependencies))
    defer { editor.dispose() }
    return try config(of: Self.PairExtension, in: editor)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > a dependency configured more than once by one extension > applies every config rather than only the last"
    ))
  func appliesEveryConfig() throws {
    #expect(
      try configOf(
        configExtension(Self.PairExtension) { $0.first = "one" },
        configExtension(Self.PairExtension) { $0.second = "two" })
        == PairConfig(first: "one", second: "two"))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > a dependency configured more than once by one extension > applies them in order, so the last one wins a conflict"
    ))
  func lastConfigWins() throws {
    #expect(
      try configOf(
        configExtension(Self.PairExtension) { $0.first = "one" },
        configExtension(Self.PairExtension) { $0.first = "two" })
        == PairConfig(first: "two", second: "default"))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > a dependency configured more than once by one extension > keeps a peer dependency config alongside a direct one"
    ))
  func peerConfigAlongsideDirect() throws {
    let PeerExtension = defineExtension(
      name: "Peer",
      peerDependencies: [
        declarePeerDependency("Pair", config: PairConfig.self) { $0.second = "peer" }
      ])
    #expect(
      try configOf(configExtension(Self.PairExtension) { $0.first = "direct" }, PeerExtension)
        == PairConfig(first: "direct", second: "peer"))
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > handles circular dependencies"
    ))
  func circularDependencies() throws {
    let ExtensionA = defineExtension(name: "A", dependencies: [])
    let ExtensionB = defineExtension(name: "B", dependencies: [ExtensionA])
    let ExtensionC = defineExtension(name: "C", dependencies: [ExtensionB])
    // This is silly and hard to do but why not prevent it
    ExtensionA.dependencies.append(ExtensionC)
    #expect {
      try buildEditorFromExtensions(ExtensionA)
    } throws: {
      "\($0)" == "LexicalBuilder: Circular dependency detected for Extension A from B"
    }
  }

  final class NodeA: TextNode {
    override class func config() -> NodeConfig? {
      NodeConfig(NodeA.self, type: "node-a", extends: TextNode.self)
    }
  }

  final class NodeB: TextNode {
    override class func config() -> NodeConfig? {
      NodeConfig(NodeB.self, type: "node-b", extends: TextNode.self)
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > nodes configuration > can mix deferred and direct node config"
    ))
  func mixDeferredAndDirectNodes() throws {
    let ExtDefer = defineExtension(name: "A", nodes: { [.klass(NodeA.self)] })
    let ExtDirect = defineExtension(
      name: "B", editorConfig: InitialEditorConfig(nodes: [.klass(NodeB.self)]))
    let editor = try buildEditorFromExtensions(
      ExtDefer, ExtDirect,
      defineExtension(
        name: "state",
        editorConfig: InitialEditorConfig(
          initialEditorState: .update { _ in
            try getRoot().append(
              try createParagraphNode().append(
                try create(NodeA.self).setTextContent("defer"),
                try create(NodeB.self).setTextContent("direct")))
          })))
    defer { editor.dispose() }
    try editor.read {
      let types = try getRoot().getAllTextNodes().map {
        "\($0.getType()):\(try $0.getTextContent())"
      }
      #expect(types == ["node-a:defer", "node-b:direct"])
    }
  }

  private static let PeerA = defineExtension(
    name: "A",
    peerDependencies: [declarePeerDependency("Config", config: ABConfig.self) { $0.b = "A" }])

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > handles peer dependency configuration > peer-first"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/LexicalBuilder.test.ts::LexicalBuilder > handles peer dependency configuration > peer-last"
    ))
  func peerDependencyConfiguration() throws {
    // Upstream's two cases build the same list.
    let editor = try buildEditorFromExtensions(Self.PeerA, ConfigExtension)
    defer { editor.dispose() }
    let reps = try reps(editor)
    #expect(reps.map(\.extension_.name) == [InitialStateExtensionName, "Config", "A"])
    #expect(try reps[1].getState().config(as: ABConfig.self) == ABConfig(a: 1, b: "A"))
  }
}
