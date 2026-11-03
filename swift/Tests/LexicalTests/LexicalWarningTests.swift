import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalWarning.test.ts.

@Suite struct LexicalWarningTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWarning.test.ts::LexicalWarning > is an Error with a code and data"
    ))
  func errorWithCodeAndData() {
    let warning = LexicalWarning("@example/pkg/thing", "a thing", data: ["n": 1])
    let error: any Error = warning
    #expect(error is LexicalWarning)
    #expect(warning.name == "LexicalWarning")
    #expect(warning.message == "a thing")
    #expect(warning.code == "@example/pkg/thing")
    #expect(warning.data == ["n": 1])
    #expect("\(warning)" == "LexicalWarning: a thing")
    #expect(LexicalWarning("@example/pkg/other", "no data").data == nil)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWarning.test.ts::LexicalWarning > accepts any code (a switch needs a default branch)"
    ))
  func acceptsAnyCode() {
    func describeCode(_ code: String) -> String {
      switch code {
      case "@example/pkg/known": "known"
      default: "other"
      }
    }
    #expect(describeCode(LexicalWarning("@example/pkg/known", "").code) == "known")
    #expect(describeCode("@example/pkg/new-code") == "other")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalWarning.test.ts::LexicalWarning > is passed to the editor's onWarn as it is"
    ))
  func passedToOnWarn() throws {
    // The extension half (`defineExtension({onWarn})`) waits for
    // @lexical/extension.
    var seen: [any Error] = []
    let editor = try createEditor(CreateEditorArgs(onWarn: { seen.append($0) }))
    let warning = LexicalWarning("@example/pkg/thing", "a thing")
    try editor._onWarn(warning)
    #expect(seen.count == 1)
    #expect(seen.first as? LexicalWarning == warning)
  }
}
