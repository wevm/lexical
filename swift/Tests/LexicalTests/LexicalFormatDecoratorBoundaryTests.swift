import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts.

nonisolated(unsafe) private let formatState = createState(
  "format", parse: { Int($0?.numberValue ?? 0) })

/// `DecoratorTextNode` from @lexical/extension, which is not ported yet: an
/// inline decorator whose text format is a flat NodeState.
private final class DecoratorTextNode: DecoratorNode, InlineFormattableNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      DecoratorTextNode.self, type: "decorator-text", extends: DecoratorNode.self,
      stateConfigs: [RequiredNodeStateConfig(formatState, flat: true)])
  }
  func getFormat() throws -> Int { try getState(self, formatState) }
  @discardableResult func setFormat(_ format: Int) throws -> Self {
    try setState(self, formatState, format)
  }
  override func isInline() throws -> Bool { true }
}

@Suite struct LexicalFormatDecoratorBoundaryTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=partial)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=whole)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=multiple)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=empty-end)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=partial)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=whole)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=multiple)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=empty-end)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=partial)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=whole)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=multiple)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=empty-end)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=partial)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=whole)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=multiple)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=empty-end)"
    ),
    arguments: [
      (false, false, "partial"),
      (false, false, "whole"),
      (false, false, "multiple"),
      (false, false, "empty-end"),
      (false, true, "partial"),
      (false, true, "whole"),
      (false, true, "multiple"),
      (false, true, "empty-end"),
      (true, false, "partial"),
      (true, false, "whole"),
      (true, false, "multiple"),
      (true, false, "empty-end"),
      (true, true, "partial"),
      (true, true, "whole"),
      (true, true, "multiple"),
      (true, true, "empty-end"),
    ] as [(Bool, Bool, String)])
  func formatsTextEdgeAndDecorator(backward: Bool, active: Bool, shape: String) throws {
    let editor = try createEditor(
      CreateEditorArgs(
        namespace: "format-decorator-boundary", nodes: [.klass(DecoratorTextNode.self)],
        onError: { throw $0 }))
    try editor.update(discrete: true) { () throws in
      let first = try createTextNode("A")
      let decorator = try create(DecoratorTextNode.self)
      let text = try createTextNode("BC")
      let last = try createTextNode("D").setStyle("color: red;")
      let paragraph = try createParagraphNode().append(first, decorator, text)
      let withLast = shape == "multiple" || shape == "empty-end"
      if withLast { try paragraph.append(last) }
      try getRoot().append(paragraph)
      let selection = createRangeSelection()
      let (start, end) =
        backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
      try start.set(first.getKey(), 1, .text)
      try end.set(
        withLast ? last.getKey() : text.getKey(),
        shape == "empty-end" ? 0 : shape == "whole" ? 2 : 1, .text)
      if active { try setSelection(selection) }
      #expect(try decorator.isSelected(selection))
      try selection.formatText("bold")
      #expect(try decorator.getFormat() == IS_BOLD)
      #expect(try decorator.isSelected(selection) == (shape == "whole"))
      #expect(start.key == (shape == "whole" ? first.getKey() : text.getKey()))
      #expect(start.offset == (shape == "whole" ? 1 : 0))
      #expect(try selection.isBackward() == backward)
      try selection.formatText("bold")
      #expect(try text.getFormat() == 0)
      #expect(try decorator.getFormat() == (shape == "whole" ? 0 : IS_BOLD))
    }
  }
}
