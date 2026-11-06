import Lexical
import LexicalMark
import LexicalTesting
import Testing

// Port of packages/lexical-mark/src/__tests__/unit/MarkGeneratedJSON.test.ts.
// Swift has no generated JSON implementations: the schema walk is the only
// path, so these tests check its output, which the generated code in JS is
// held to.

/// `DedupeMarkNode`: a subclass whose `setIDs` normalizes.
final class DedupeMarkNode: MarkNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(DedupeMarkNode.self, type: "dedupe-mark", extends: MarkNode.self)
  }

  @discardableResult
  override func setIDs(_ ids: [String]) throws -> Self {
    var seen = Set<String>()
    return try super.setIDs(ids.filter { seen.insert($0).inserted })
  }
}

@Suite struct MarkGeneratedJSONTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkGeneratedJSON.test.ts::mark generated exportJSON > MarkNode agrees with the schema-driven walk"
    ))
  func agreesWithTheSchemaWalk() throws {
    let editor = try createTestEditor(nodes: [.klass(MarkNode.self)])
    try editor.update(discrete: true) {
      let empty = try MarkNode()
      let emptyArray = try MarkNode([])
      let ab = try MarkNode(["a", "b"])
      #expect(
        try empty.exportJSON().stringify()
          == #"{"children":[],"ids":[],"direction":null,"format":"","indent":0,"type":"mark","version":1}"#
      )
      #expect(
        try emptyArray.exportJSON().stringify()
          == #"{"children":[],"ids":[],"direction":null,"format":"","indent":0,"type":"mark","version":1}"#
      )
      #expect(
        try ab.exportJSON().stringify()
          == #"{"children":[],"ids":["a","b"],"direction":null,"format":"","indent":0,"type":"mark","version":1}"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkGeneratedJSON.test.ts::mark generated exportJSON > the compact form compares an empty-array default by content"
    ))
  func compactFormComparesAnEmptyArrayByContent() throws {
    let editor = try createTestEditor(nodes: [.klass(MarkNode.self)])
    try editor.update(discrete: true) {
      let empty = try MarkNode()
      let emptyArray = try MarkNode([])
      let x = try MarkNode(["x"])
      #expect(try empty.exportJSON(true).stringify() == #"{"type":"mark","children":[]}"#)
      #expect(try emptyArray.exportJSON(true).stringify() == #"{"type":"mark","children":[]}"#)
      #expect(
        try x.exportJSON(true).stringify()
          == #"{"type":"mark","children":[],"ids":["x"]}"#)
      // The legacy form writes it unconditionally either way.
      #expect(try empty.exportJSON()["ids"] == .array([]))
    }
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkGeneratedJSON.test.ts::the field `ids` stands in for > a subclass override of setIDs still decides"
    ))
  func aSubclassOverrideOfSetIDsStillDecides() throws {
    let editor = try createTestEditor(
      nodes: [.klass(MarkNode.self), .klass(DedupeMarkNode.self)])
    try editor.update(discrete: true) {
      let json = try #require(
        try JSONValue.parse(
          #"{"children":[],"direction":null,"format":"","ids":["a","a","b"],"indent":0,"type":"dedupe-mark","version":1}"#
        ).objectValue)
      let node = try #require(try DedupeMarkNode.importJSON(json) as? DedupeMarkNode)
      #expect(try node.getIDs() == ["a", "b"])
    }
  }
}
