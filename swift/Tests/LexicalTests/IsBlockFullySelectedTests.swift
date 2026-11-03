import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/IsBlockFullySelected.test.ts.

private func block(_ index: Int) throws -> ElementNode {
  try #require(try getRoot().getChildAtIndex(index) as? ElementNode)
}

private func blockText(_ index: Int) throws -> TextNode {
  try #require(try block(index).getFirstDescendant() as? TextNode)
}

private func current() throws -> RangeSelection {
  try #require(try getSelection() as? RangeSelection)
}

private func run(_ body: @escaping () throws -> Void) throws {
  let editor = try createTestEditor()
  try editor.update(discrete: true, body)
}

@Suite struct IsBlockFullySelectedTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > reports whether the selection covers the block"
    ))
  func reportsCoverage() throws {
    try run {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("first")),
        try createParagraphNode().append(try createTextNode("second")))
      let b = try block(1)
      _ = try b.select(0, try b.getChildrenSize())
      let selection = try current()
      #expect(try isBlockFullySelected(b, selection))
      #expect(try isBlockFullySelected(try getRoot(), selection) == false)
      #expect(try isBlockFullySelected(try block(0), selection) == false)
      _ = try blockText(1).select(1, 3)
      #expect(try isBlockFullySelected(b, try current()) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > accepts a CaretRange and backward selections"
    ))
  func caretRangeAndBackward() throws {
    try run {
      try getRoot().append(try createParagraphNode().append(try createTextNode("first")))
      _ = try blockText(0).select(5, 0)
      let selection = try current()
      #expect(try isBlockFullySelected(try block(0), selection))
      #expect(try isBlockFullySelected(try block(0), try caretRangeFromSelection(selection)))
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > a selection that extends beyond the block still fully selects it"
    ))
  func extendsBeyondBlock() throws {
    try run {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("first")),
        try createParagraphNode().append(try createTextNode("second")))
      let selection = try blockText(0).select(0, 0)
      try selection.focus.set(try blockText(1).getKey(), 6, .text)
      #expect(try isBlockFullySelected(try getRoot(), selection))
      #expect(try isBlockFullySelected(try block(0), selection))
      #expect(try isBlockFullySelected(try block(1), selection))
      let partial = try blockText(0).select(2, 2)
      try partial.focus.set(try blockText(1).getKey(), 2, .text)
      #expect(try isBlockFullySelected(try block(0), partial) == false)
      #expect(try isBlockFullySelected(try block(1), partial) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > handles element points on an ancestor"
    ))
  func elementPointsOnAncestor() throws {
    try run {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("first")),
        try createParagraphNode().append(try createTextNode("second")))
      let selection = try getRoot().select(0, 1)
      #expect(try isBlockFullySelected(try block(0), selection))
      #expect(try isBlockFullySelected(try block(1), selection) == false)
      #expect(try isBlockFullySelected(try getRoot(), selection) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > handles blocks that end with a decorator"
    ))
  func blockEndingWithDecorator() throws {
    try run {
      let b = try createParagraphNode().append(
        try createTextNode("text"), try createTestDecoratorNode())
      try getRoot().append(b)
      let selection = try b.select(0, try b.getChildrenSize())
      #expect(try isBlockFullySelected(b, selection))
      let partial = try b.select(0, try b.getChildrenSize() - 1)
      #expect(try isBlockFullySelected(b, partial) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > an empty block is fully selected by its caret"
    ))
  func emptyBlockSelectedByCaret() throws {
    try run {
      try getRoot().append(
        try createParagraphNode().append(try createTextNode("first")), try createParagraphNode())
      let emptyBlock = try block(1)
      _ = try emptyBlock.select(0, 0)
      #expect(try isBlockFullySelected(emptyBlock, try current()))
      _ = try blockText(0).select(0, 5)
      #expect(try isBlockFullySelected(emptyBlock, try current()) == false)
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/IsBlockFullySelected.test.ts::$isBlockFullySelected > a fully selected block fully selects its ancestor block"
    ))
  func fullySelectsAncestor() throws {
    try run {
      let text = try createTextNode("quoted")
      let inner = try createParagraphNode().append(text)
      let quote = try createParagraphNode().append(inner)
      try getRoot().append(quote)
      _ = try text.select(0, 6)
      let selection = try current()
      #expect(try isBlockFullySelected(inner, selection))
      #expect(try isBlockFullySelected(quote, selection))
      _ = try text.select(1, 6)
      let partial = try current()
      #expect(try isBlockFullySelected(inner, partial) == false)
      #expect(try isBlockFullySelected(quote, partial) == false)
    }
  }
}
