import Lexical
import LexicalHashtag
import LexicalTesting
import Testing

// Port of packages/lexical-hashtag/src/__tests__/unit/LexicalHashtagNode.test.ts.

@Suite struct LexicalHashtagNodeTests {
  @Test(
    .upstream(
      "lexical-hashtag/src/__tests__/unit/LexicalHashtagNode.test.ts::LexicalHashtagNode tests > HashtagNode.exportJSON() should return and object conforming to the expected schema"
    ))
  func exportJSON() throws {
    let editor = try createMountedTestEditor(nodes: [.klass(HashtagNode.self)])
    try editor.update {
      let node = try createHashtagNode("therickestrickofall")
      // If you broke this test, you changed the public interface of a
      // serialized Lexical Core Node. Please ensure the correct adapter
      // logic is in place in the corresponding importJSON  method
      // to accommodate these changes.
      #expect(
        try node.exportJSON().stringify()
          == #"{"detail":0,"format":0,"mode":"normal","style":"","text":"therickestrickofall","type":"hashtag","version":1}"#
      )
    }
  }
}
