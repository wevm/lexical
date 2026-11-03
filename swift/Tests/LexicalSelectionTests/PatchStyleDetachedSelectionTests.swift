import Lexical
import LexicalExtension
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts.

private enum Active { case none, collapsed, range }

private struct Fixture {
  let paragraph: ParagraphNode
  let text: TextNode
  let expectActiveUnchanged: () throws -> Void
}

private func createFixture(_ active: Active) throws -> Fixture {
  let text = try createTextNode("abcdef")
  let paragraph = try createParagraphNode().append(text)
  let other = try createTextNode("xyz")
  try getRoot().append(paragraph, try createParagraphNode().append(other))
  let activeSelection: RangeSelection? =
    active == .none ? nil : try other.select(0, active == .range ? 2 : 0)
  try setSelection(activeSelection)
  let originalActive = activeSelection?.clone()
  return Fixture(paragraph: paragraph, text: text) {
    let children = try other.getParentOrThrow().getChildren()
    #expect(children.count == 1 && children[0].is(other))
    #expect(try other.getStyle() == "")
    #expect(try getSelection() === activeSelection)
    if let activeSelection, let originalActive {
      #expect(activeSelection.is(originalActive))
    }
  }
}

private func withEditor(_ name: String, _ body: @escaping () throws -> Void) throws {
  let editor = try buildEditorFromExtensions(defineExtension(name: name))
  defer { editor.dispose() }
  try editor.update(discrete: true, body)
}

private func stylesTextRange(_ active: Active, backward: Bool, partial: Bool) throws {
  try withEditor("detached-style-range") {
    let fixture = try createFixture(active)
    let text = fixture.text
    let paragraph = fixture.paragraph
    let selection = createRangeSelection()
    let (start, end) =
      backward ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
    try start.set(text.getKey(), partial ? 2 : 0, .text)
    try end.set(text.getKey(), partial ? 4 : 6, .text)
    let expectedText = partial ? "cd" : "abcdef"
    try patchStyleText(selection, ["color": "red"])
    let styled = try #require(try paragraph.getChildAtIndex(partial ? 1 : 0) as? TextNode)
    #expect(selection.anchor.key == styled.getKey())
    #expect(selection.anchor.offset == 0)
    #expect(selection.anchor.type == .text)
    #expect(selection.focus.key == styled.getKey())
    #expect(selection.focus.offset == expectedText.utf16.count)
    #expect(selection.focus.type == .text)
    #expect(try selection.getTextContent() == expectedText)
    let nodes = try selection.getNodes()
    #expect(nodes.count == 1 && nodes[0].is(styled))
    try patchStyleText(selection, ["font-size": "20px"])
    #expect(try styled.getStyle() == "color: red;font-size: 20px;")
    #expect(try selection.getTextContent() == expectedText)
    #expect(try paragraph.getTextContent() == "abcdef")
    if partial {
      #expect(try (paragraph.getFirstChildOrThrow() as? TextNode)?.getStyle() == "")
      #expect(try (paragraph.getLastChildOrThrow() as? TextNode)?.getStyle() == "")
    }
    try fixture.expectActiveUnchanged()
  }
}

private func stylesCaret(_ active: Active) throws {
  try withEditor("detached-style-caret") {
    let fixture = try createFixture(active)
    let selection = createRangeSelection()
    try selection.anchor.set(fixture.text.getKey(), 2, .text)
    try selection.focus.set(fixture.text.getKey(), 2, .text)
    try patchStyleText(selection, ["color": "red"])
    #expect(selection.style == "color: red;")
    #expect(try fixture.text.getStyle() == "")
    try fixture.expectActiveUnchanged()
  }
}

private func stylesNodeSelection(_ active: Active) throws {
  try withEditor("detached-style-node") {
    let fixture = try createFixture(active)
    let selection = createNodeSelection()
    selection.add(fixture.text.getKey())
    try patchStyleText(selection, ["color": "red"])
    #expect(try fixture.text.getStyle() == "color: red;")
    try fixture.expectActiveUnchanged()
  }
}

@Suite struct PatchStyleDetachedSelectionTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=false, partial=false)"
    ))
  func range_none_false_false() throws {
    try stylesTextRange(.none, backward: false, partial: false)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=false, partial=true)"
    ))
  func range_none_false_true() throws { try stylesTextRange(.none, backward: false, partial: true) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=true, partial=false)"
    ))
  func range_none_true_false() throws { try stylesTextRange(.none, backward: true, partial: false) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=none, backward=true, partial=true)"
    ))
  func range_none_true_true() throws { try stylesTextRange(.none, backward: true, partial: true) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=false, partial=false)"
    ))
  func range_collapsed_false_false() throws {
    try stylesTextRange(.collapsed, backward: false, partial: false)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=false, partial=true)"
    ))
  func range_collapsed_false_true() throws {
    try stylesTextRange(.collapsed, backward: false, partial: true)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=true, partial=false)"
    ))
  func range_collapsed_true_false() throws {
    try stylesTextRange(.collapsed, backward: true, partial: false)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=collapsed, backward=true, partial=true)"
    ))
  func range_collapsed_true_true() throws {
    try stylesTextRange(.collapsed, backward: true, partial: true)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=false, partial=false)"
    ))
  func range_range_false_false() throws {
    try stylesTextRange(.range, backward: false, partial: false)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=false, partial=true)"
    ))
  func range_range_false_true() throws {
    try stylesTextRange(.range, backward: false, partial: true)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=true, partial=false)"
    ))
  func range_range_true_false() throws {
    try stylesTextRange(.range, backward: true, partial: false)
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::repeatedly styles a detached text range (active=range, backward=true, partial=true)"
    ))
  func range_range_true_true() throws { try stylesTextRange(.range, backward: true, partial: true) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached caret without styling active text (none)"
    ))
  func caret_none() throws { try stylesCaret(.none) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached node selection (none)"
    ))
  func node_none() throws { try stylesNodeSelection(.none) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached caret without styling active text (collapsed)"
    ))
  func caret_collapsed() throws { try stylesCaret(.collapsed) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached node selection (collapsed)"
    ))
  func node_collapsed() throws { try stylesNodeSelection(.collapsed) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached caret without styling active text (range)"
    ))
  func caret_range() throws { try stylesCaret(.range) }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/PatchStyleDetachedSelection.test.ts::styles a detached node selection (range)"
    ))
  func node_range() throws { try stylesNodeSelection(.range) }

}
