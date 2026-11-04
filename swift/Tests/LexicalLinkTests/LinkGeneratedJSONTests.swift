import Lexical
import LexicalLink
import LexicalTesting
import Testing

// Port of packages/lexical-link/src/__tests__/unit/LinkGeneratedJSON.test.ts.
// JS compiles per-class exporters from the schema as a speed-up and checks
// that they agree with the schema-driven walk, in both forms and in key
// order. Swift has only the walk, so the port checks what both must
// produce: the JSON text upstream exports for the same nodes, in both
// forms (taken from the upstream walk).

@Suite struct LinkGeneratedJSONTests {
  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkGeneratedJSON.test.ts::link generated exportJSON > LinkNode agrees with the schema-driven walk"
    ))
  func linkNode() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) { () throws in
      let plain = try LinkNode()
      #expect(
        try plain.exportJSON().stringify()
          == #"{"children":[],"rel":null,"target":null,"title":null,"url":"","direction":null,"format":"","indent":0,"type":"link","version":1}"#
      )
      #expect(try plain.exportJSON(true).stringify() == #"{"type":"link","children":[]}"#)
      let full = try LinkNode(
        "https://example.com/",
        LinkAttributes(rel: "noreferrer", target: "_blank", title: "Example")
      ).setIndent(1)
      #expect(
        try full.exportJSON().stringify()
          == #"{"children":[],"rel":"noreferrer","target":"_blank","title":"Example","url":"https://example.com/","direction":null,"format":"","indent":1,"type":"link","version":1}"#
      )
      #expect(
        try full.exportJSON(true).stringify()
          == #"{"type":"link","children":[],"rel":"noreferrer","target":"_blank","title":"Example","url":"https://example.com/","indent":1}"#
      )
    }
  }

  @Test(
    .upstream(
      "lexical-link/src/__tests__/unit/LinkGeneratedJSON.test.ts::link generated exportJSON > AutoLinkNode agrees with the schema-driven walk"
    ))
  func autoLinkNode() throws {
    let editor = try createLinkTestEditor()
    try editor.update(discrete: true) { () throws in
      let plain = try AutoLinkNode()
      #expect(
        try plain.exportJSON().stringify()
          == #"{"children":[],"isUnlinked":false,"rel":null,"target":null,"title":null,"url":"","direction":null,"format":"","indent":0,"type":"autolink","version":1}"#
      )
      #expect(try plain.exportJSON(true).stringify() == #"{"type":"autolink","children":[]}"#)
      let unlinked = try AutoLinkNode(
        "https://example.com/", AutoLinkAttributes(target: "_blank", isUnlinked: true))
      #expect(
        try unlinked.exportJSON().stringify()
          == #"{"children":[],"isUnlinked":true,"rel":null,"target":"_blank","title":null,"url":"https://example.com/","direction":null,"format":"","indent":0,"type":"autolink","version":1}"#
      )
      #expect(
        try unlinked.exportJSON(true).stringify()
          == #"{"type":"autolink","children":[],"isUnlinked":true,"target":"_blank","url":"https://example.com/"}"#
      )
    }
  }
}
