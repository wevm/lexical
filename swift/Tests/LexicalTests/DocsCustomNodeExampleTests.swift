import Lexical
import LexicalTesting
import Testing

// Port of src/__tests__/unit/DocsCustomNodeExample.test.ts: the CalloutNode
// from docs/concepts/nodes.mdx (here DocsCalloutNode), written with `@LexicalNode`.

@LexicalNode(type: "callout", extends: ElementNode.self)
final class DocsCalloutNode: ElementNode {
  @Field(stringValue()) var __label: String = ""
  @Field(try numberValue(1, integer: true, min: 1, max: 3)) var __level: Double = 1
  @Field(try enumValue(["info", "warning", "danger"])) var __tone: String = "info"
}

@Suite struct DocsCustomNodeExampleTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/DocsCustomNodeExample.test.ts::the CalloutNode from docs/concepts/nodes.mdx > does what the section claims it does"
    ))
  func calloutClaims() throws {
    let editor = try createTestEditor(nodes: [.klass(DocsCalloutNode.self)])
    try update(editor) {
      let callout = try DocsCalloutNode()
      try getRoot().clear().append(callout)
      // "level: 9 or level: "big" or a missing level all land on 1 here"
      try callout.updateFromJSON(jsonObject(#"{"label":"Note","level":9,"tone":"warning"}"#))
      let exported = try callout.exportJSON()
      #expect(exported["label"] == "Note")
      #expect(exported["level"] == 1)
      #expect(exported["tone"] == "warning")
      try callout.updateFromJSON(jsonObject(#"{"level":"big"}"#))
      #expect(try callout.exportJSON()["level"] == 1)
      try callout.updateFromJSON(jsonObject("{}"))
      #expect(try callout.exportJSON()["level"] == 1)
      // …and an in-domain value is kept.
      try callout.updateFromJSON(jsonObject(#"{"level":3}"#))
      #expect(try callout.exportJSON()["level"] == 3)
      // An out-of-domain enum falls back to the first value.
      try callout.updateFromJSON(jsonObject(#"{"tone":"banana"}"#))
      #expect(try callout.exportJSON()["tone"] == "info")
    }
  }
}
