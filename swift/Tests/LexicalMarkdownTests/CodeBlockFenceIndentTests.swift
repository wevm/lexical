import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalMarkdown
import LexicalTesting
import Testing

// Port of packages/lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts.

private func importCodeText(_ markdown: String) throws -> String? {
  let editor = try buildEditorFromExtensions(MarkdownTestExtension)
  defer { editor.dispose() }
  var text: String?
  try editor.update(discrete: true) {
    try convertFromMarkdownString(markdown, TRANSFORMERS)
    let first = try getRoot().getFirstChild()
    text = try (first as? CodeNode)?.getTextContent()
  }
  return text
}

@Suite struct CodeBlockFenceIndentTests {
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > removes the fence indentation from the content"
    ))
  func removesFenceIndentation() throws {
    #expect(try importCodeText("  ```js\n  code\n  ```") == "code")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > preserves indentation relative to the fence"
    ))
  func preservesRelativeIndentation() throws {
    #expect(try importCodeText("   ```\n   a\n     b\n   ```") == "a\n  b")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > removes at most the fence indentation"
    ))
  func removesAtMostFenceIndentation() throws {
    #expect(try importCodeText("  ```\ncode\n  ```") == "code")
    #expect(try importCodeText("  ```\n a\n  b\n  ```") == "a\nb")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > handles a tab-indented fence"
    ))
  func tabIndentedFence() throws {
    #expect(try importCodeText("\t```\n\tcode\n\t```") == "code")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > keeps content indentation when the fence is not indented"
    ))
  func keepsIndentationWithoutFenceIndent() throws {
    #expect(try importCodeText("```js\n  code\n```") == "  code")
    #expect(try importCodeText("```js\ncode\n```") == "code")
  }
}
