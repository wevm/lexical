import Lexical
import LexicalClipboard
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/HandleTextDrop.test.ts.
//
// Upstream mocks `caretFromPoint` to return a DOM point, which
// `$resolveDropPointCaret` turns into a caret. The Swift drop handlers take
// the caret the view resolved (DESIGN.md, Clipboard), so each DOM point here
// is the caret it resolves to: a DOM text node and offset is a text caret,
// and an element and child offset is `getChildCaretAtIndex(element, offset)`.

/// A drop point, resolved to a caret inside the update.
private enum DropPoint {
  case text(NodeKey, Int)
  case element(NodeKey, Int)

  func caret() throws -> PointCaret {
    switch self {
    case .text(let key, let offset):
      let text = try #require(try getNodeByKey(key) as? TextNode)
      return try getTextPointCaret(text, .next, offset)
    case .element(let key, let offset):
      let element = try #require(try getNodeByKey(key) as? ElementNode)
      return try getChildCaretAtIndex(element, offset, .next)
    }
  }
}

private func createDropEvent() -> (DragEvent, DataTransfer) {
  let dataTransfer = DataTransfer()
  return (DragEvent("drop", dataTransfer: dataTransfer), dataTransfer)
}

private func markActiveSelectionAsDragSource(_ dataTransfer: DataTransfer, _ editor: Editor) throws
{
  if let sel = try getSelection() as? RangeSelection, !sel.isCollapsed() {
    writeDragSourceToDataTransfer(dataTransfer, editor)
  }
}

/// `setLexicalClipboardDataTransfer` and the drag marker, as DRAGSTART
/// writes them.
private func writeDragStart(_ dataTransfer: DataTransfer, _ editor: Editor) throws {
  setLexicalClipboardDataTransfer(dataTransfer, try getClipboardDataFromSelection())
  writeDragSourceToDataTransfer(dataTransfer, editor)
}

private func rootText(_ editor: Editor) throws -> String {
  try editor.read { try getRoot().getTextContent() }
}

private func childTexts(_ node: ElementNode) throws -> [String] {
  try node.getChildren().map { $0 is DecoratorNode ? "decorator" : try $0.getTextContent() }
}

/// One paragraph holding `text`, with "foo" (6..<9) selected as `anchor`,
/// `focus`.
private func helloFooWorld(_ editor: Editor, anchor: Int = 6, focus: Int = 9) throws -> NodeKey {
  var textKey = ""
  try editor.update(discrete: true) { () throws in
    let text = try createTextNode("Hello foo world")
    _ = try getRoot().clear().append(createParagraphNode().append(text))
    textKey = text.getKey()
    let selection = createRangeSelection()
    try selection.anchor.set(textKey, anchor, .text)
    try selection.focus.set(textKey, focus, .text)
    try setSelection(selection)
  }
  return textKey
}

@Suite struct HandleTextDropTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > moves a selected word later within the same TextNode"
    ))
  func movesLater() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    let textKey = try helloFooWorld(editor)
    try editor.update(discrete: true) { () throws in
      // Drop at offset 15 ("Hello foo world|"), past the source's end.
      let (event, dataTransfer) = createDropEvent()
      dataTransfer.setData("text/plain", "foo")
      try markActiveSelectionAsDragSource(dataTransfer, editor)
      #expect(try handleRichTextDrop(event, editor, DropPoint.text(textKey, 15).caret()))
      #expect(event.defaultPrevented)
    }
    #expect(try rootText(editor) == "Hello  worldfoo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > moves a selected word earlier within the same TextNode"
    ))
  func movesEarlier() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    let textKey = try helloFooWorld(editor)
    try editor.update(discrete: true) { () throws in
      // Drop at offset 0 (before the source).
      let (event, dataTransfer) = createDropEvent()
      dataTransfer.setData("text/plain", "foo")
      try markActiveSelectionAsDragSource(dataTransfer, editor)
      _ = try handleRichTextDrop(event, editor, DropPoint.text(textKey, 0).caret())
    }
    #expect(try rootText(editor) == "fooHello  world")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > no-op when the drop point is inside the source range"
    ))
  func noOpInsideSource() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    let textKey = try helloFooWorld(editor)
    try editor.update(discrete: true) { () throws in
      // Drop inside the selected "foo" range.
      let (event, dataTransfer) = createDropEvent()
      dataTransfer.setData("text/plain", "foo")
      try markActiveSelectionAsDragSource(dataTransfer, editor)
      #expect(try handleRichTextDrop(event, editor, DropPoint.text(textKey, 7).caret()))
      #expect(event.defaultPrevented)
    }
    #expect(try rootText(editor) == "Hello foo world")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > keeps the dragged text when dropped on the end edge of the source range"
    ))
  func keepsAtEndEdge() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var boldKey = ""
    var restKey = ""
    try editor.update(discrete: true) { () throws in
      let bold = try createTextNode("Hello").toggleFormat("bold")
      let rest = try createTextNode(" world")
      _ = try getRoot().clear().append(createParagraphNode().append(bold, rest))
      boldKey = bold.getKey()
      restKey = rest.getKey()
      _ = try bold.select(0, 5)
    }
    // Drop at the end of "Hello", then at the start of " world".
    for point in [DropPoint.text(boldKey, 5), .text(restKey, 0)] {
      try editor.update(discrete: true) { () throws in
        let (event, dataTransfer) = createDropEvent()
        dataTransfer.setData("text/plain", "Hello")
        try markActiveSelectionAsDragSource(dataTransfer, editor)
        #expect(try handleRichTextDrop(event, editor, point.caret()))
      }
      #expect(try rootText(editor) == "Hello world")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > moves a selection across TextNodes in the same block"
    ))
  func movesAcrossTextNodes() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var destKey = ""
    try editor.update(discrete: true) { () throws in
      let source = try createTextNode("source ").toggleFormat("bold")
      let dest = try createTextNode("destination")
      _ = try getRoot().clear().append(createParagraphNode().append(source, dest))
      destKey = dest.getKey()
      let selection = createRangeSelection()
      try selection.anchor.set(source.getKey(), 0, .text)
      try selection.focus.set(source.getKey(), 6, .text)
      try setSelection(selection)
    }
    try editor.update(discrete: true) { () throws in
      // Drop just after the "t" of "destination" (offset 4).
      let (event, dataTransfer) = createDropEvent()
      dataTransfer.setData("text/plain", "source")
      try markActiveSelectionAsDragSource(dataTransfer, editor)
      _ = try handleRichTextDrop(event, editor, DropPoint.text(destKey, 4).caret())
    }
    // " " from the source TextNode stays where it was, and "source" is
    // inserted into the destination TextNode at offset 4.
    #expect(try rootText(editor) == " destsourceination")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > returns false for an external drag (no marker), letting the browser handle it"
    ))
  func externalDrag() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var textKey = ""
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("Hello world")
      _ = try getRoot().clear().append(createParagraphNode().append(text))
      textKey = text.getKey()
    }
    var handled = true
    let (event, dataTransfer) = createDropEvent()
    try editor.update(discrete: true) { () throws in
      // No drag-source marker: an external drag, which the platform's own
      // drop handling takes over.
      dataTransfer.setData("text/plain", "brave ")
      handled = try handleRichTextDrop(event, editor, DropPoint.text(textKey, 5).caret())
    }
    #expect(!handled)
    #expect(!event.defaultPrevented)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > returns false when caretFromPoint cannot resolve a location"
    ))
  func unresolvedLocation() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    try editor.update(discrete: true) { () throws in
      _ = try getRoot().clear().append(createParagraphNode().append(createTextNode("Hello")))
    }
    var handled = true
    let (event, _) = createDropEvent()
    try editor.update(discrete: true) { () throws in
      handled = try handleRichTextDrop(event, editor, nil)
    }
    #expect(!handled)
    #expect(!event.defaultPrevented)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > handles a backward (right-to-left) source selection"
    ))
  func backwardSource() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    // Backward selection: anchor > focus in "foo".
    let textKey = try helloFooWorld(editor, anchor: 9, focus: 6)
    try editor.read { () throws in
      let current = try #require(try getSelection() as? RangeSelection)
      #expect(try current.isBackward())
    }
    try editor.update(discrete: true) { () throws in
      let (event, dataTransfer) = createDropEvent()
      dataTransfer.setData("text/plain", "foo")
      try markActiveSelectionAsDragSource(dataTransfer, editor)
      _ = try handleRichTextDrop(event, editor, DropPoint.text(textKey, 15).caret())
    }
    #expect(try rootText(editor) == "Hello  worldfoo")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > preserves DecoratorNodes in the source when moved to a new location"
    ))
  func preservesDecorators() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var targetTextKey = ""
    try editor.update(discrete: true) { () throws in
      // Source paragraph: "before" + DecoratorNode + "after".
      let beforeText = try createTextNode("before")
      let afterText = try createTextNode("after")
      let sourceParagraph = try createParagraphNode().append(
        beforeText, createTestDecoratorNode(), afterText)
      // Target paragraph (drop destination).
      let targetText = try createTextNode("target")
      targetTextKey = targetText.getKey()
      _ = try getRoot().clear().append(
        sourceParagraph, createParagraphNode().append(targetText))
      // Select from the start of "before" to the end of "after".
      let selection = createRangeSelection()
      try selection.anchor.set(beforeText.getKey(), 0, .text)
      try selection.focus.set(afterText.getKey(), 5, .text)
      try setSelection(selection)
    }
    // Fill the DataTransfer as DRAGSTART does: Lexical's own serialization,
    // so custom nodes survive the drop, and the drag-source marker.
    try editor.update(discrete: true) { () throws in
      let selection = try #require(try getSelection() as? RangeSelection)
      let (event, dataTransfer) = createDropEvent()
      setLexicalClipboardDataTransfer(dataTransfer, try getClipboardDataFromSelection(selection))
      writeDragSourceToDataTransfer(dataTransfer, editor)
      // Drop at the end of "target".
      _ = try handleRichTextDrop(event, editor, DropPoint.text(targetTextKey, 6).caret())
    }
    try editor.read { () throws in
      let topLevelChildren = try getRoot().getChildren()
      // The (now empty) source and the target, holding the moved content.
      try #require(topLevelChildren.count == 2)
      let sourceParagraph = try #require(topLevelChildren[0] as? ElementNode)
      let targetParagraph = try #require(topLevelChildren[1] as? ElementNode)
      // Exactly one decorator in the tree (preserved, not duplicated), under
      // the target paragraph.
      #expect(try sourceParagraph.getChildren().filter { $0 is DecoratorNode }.count == 0)
      #expect(try targetParagraph.getChildren().filter { $0 is DecoratorNode }.count == 1)
      // TestDecoratorNode's text content is "Hello world".
      #expect(try sourceParagraph.getTextContent() == "")
      #expect(try targetParagraph.getTextContent() == "targetbeforeHello worldafter")
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleTextDrop > no-op when drop is inside a multi-node source range"
    ))
  func noOpInsideMultiNodeSource() throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var innerKey = ""
    try editor.update(discrete: true) { () throws in
      let t1 = try createTextNode("one")
      let t2 = try createTextNode("two").toggleFormat("bold")
      let t3 = try createTextNode("three")
      _ = try getRoot().clear().append(createParagraphNode().append(t1, t2, t3))
      innerKey = t2.getKey()
      // The selection spans all three text nodes.
      let selection = createRangeSelection()
      try selection.anchor.set(t1.getKey(), 0, .text)
      try selection.focus.set(t3.getKey(), 5, .text)
      try setSelection(selection)
    }
    try editor.update(discrete: true) { () throws in
      // Drop inside "two", strictly between the source's anchor and focus.
      let (event, dataTransfer) = createDropEvent()
      dataTransfer.setData("text/plain", "onetwothree")
      try markActiveSelectionAsDragSource(dataTransfer, editor)
      #expect(try handleRichTextDrop(event, editor, DropPoint.text(innerKey, 1).caret()))
      #expect(event.defaultPrevented)
    }
    #expect(try rootText(editor) == "onetwothree")
  }
}

// MARK: - %s drop boundaries

/// The two handlers `describe.each` runs the boundary tests with.
enum DropHandler: String, CaseIterable, Sendable {
  case rich = "rich text"
  case plain = "plain text"

  fileprivate func handle(
    _ event: DragEvent, _ editor: Editor, _ caret: PointCaret?
  ) throws
    -> Bool
  {
    switch self {
    case .rich: try handleRichTextDrop(event, editor, caret)
    case .plain: try handlePlainTextDrop(event, editor, caret)
    }
  }
}

private func boundaryCases<T: Sendable>(_ items: [T]) -> [(DropHandler, Bool, T)] {
  DropHandler.allCases.flatMap { handler in
    [false, true].flatMap { backward in items.map { (handler, backward, $0) } }
  }
}

/// Selects all of `text`, forward or backward.
private func selectWhole(_ text: TextNode, backward: Bool) throws {
  _ = try text.select(backward ? 5 : 0, backward ? 0 : 5)
}

/// Drops the current selection's drag payload at `point`, then checks that
/// the document and selection are unchanged and `keys` still attached.
private func expectDropKeepsState(
  _ editor: Editor, _ handler: DropHandler, _ point: DropPoint, keys: [NodeKey],
  expectPrevented: Bool = true
) throws {
  let original = try editor.getEditorState().toJSON()
  let selection = try editor.read { try #require(try getSelection()).clone() }
  try editor.update(discrete: true) { () throws in
    let (event, dataTransfer) = createDropEvent()
    try writeDragStart(dataTransfer, editor)
    #expect(try handler.handle(event, editor, point.caret()))
    if expectPrevented {
      #expect(event.defaultPrevented)
    }
  }
  #expect(try editor.getEditorState().toJSON() == original)
  try editor.read { () throws in
    #expect(try #require(try getSelection()).is(selection))
    for key in keys {
      #expect(try getNodeByKey(key)?.isAttached() == true)
    }
  }
}

@Suite struct DropBoundaryTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at text start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at text middle"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at text end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a token unchanged at element end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at text start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at text middle"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at text end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a token unchanged at element end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at text start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at text middle"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at text end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a token unchanged at element end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at text start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at text middle"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at text end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a token unchanged at element end"
    ),
    arguments: boundaryCases([
      "text start", "text middle", "text end", "previous end", "next start", "element start",
      "element end",
    ]))
  func keepsToken(handler: DropHandler, backward: Bool, position: String) throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var keys: [String: NodeKey] = [:]
    try editor.update(discrete: true) { () throws in
      let before = try createTextNode("before ")
      let source = try createTextNode("Hello").setMode("token")
      let after = try createTextNode(" world")
      let paragraph = try createParagraphNode().append(before, source, after)
      _ = try getRoot().clear().append(paragraph)
      keys = [
        "source": source.getKey(), "before": before.getKey(), "after": after.getKey(),
        "paragraph": paragraph.getKey(),
      ]
      try selectWhole(source, backward: backward)
    }
    let points: [String: DropPoint] = [
      "element end": .element(keys["paragraph"]!, 2),
      "element start": .element(keys["paragraph"]!, 1),
      "next start": .text(keys["after"]!, 0),
      "previous end": .text(keys["before"]!, 7),
      "text end": .text(keys["source"]!, 5),
      "text middle": .text(keys["source"]!, 2),
      "text start": .text(keys["source"]!, 0),
    ]
    try expectDropKeepsState(
      editor, handler, points[position]!,
      keys: ["paragraph", "before", "source", "after"].map { keys[$0]! })
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps nested inline wrappers unchanged at element end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at previous end"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at next start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element start"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps nested inline wrappers unchanged at element end"
    ),
    arguments: boundaryCases(["previous end", "next start", "element start", "element end"]))
  func keepsNestedInlineWrappers(handler: DropHandler, backward: Bool, position: String) throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var keys: [String: NodeKey] = [:]
    try editor.update(discrete: true) { () throws in
      let before = try createTextNode("before ")
      let source = try createTextNode("Hello")
      let after = try createTextNode(" world")
      let inner = try createTestInlineElementNode().append(source)
      let outer = try createTestInlineElementNode().append(inner)
      let paragraph = try createParagraphNode().append(before, outer, after)
      _ = try getRoot().clear().append(paragraph)
      keys = [
        "before": before.getKey(), "after": after.getKey(), "paragraph": paragraph.getKey(),
        "inner": inner.getKey(), "outer": outer.getKey(),
      ]
      try selectWhole(source, backward: backward)
    }
    let points: [String: DropPoint] = [
      "element end": .element(keys["paragraph"]!, 2),
      "element start": .element(keys["paragraph"]!, 1),
      "next start": .text(keys["after"]!, 0),
      "previous end": .text(keys["before"]!, 7),
    ]
    try expectDropKeepsState(
      editor, handler, points[position]!, keys: [keys["inner"]!, keys["outer"]!],
      expectPrevented: false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a decorator in a multi-node range at the adjacent end edge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a decorator in a multi-node range at the adjacent end edge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a decorator in a multi-node range at the adjacent end edge"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a decorator in a multi-node range at the adjacent end edge"
    ),
    arguments: boundaryCases([0]))
  func keepsDecoratorAtEndEdge(handler: DropHandler, backward: Bool, _: Int) throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var afterKey = ""
    var decoratorKey = ""
    try editor.update(discrete: true) { () throws in
      let first = try createTextNode("before")
      let decorator = try createTestDecoratorNode()
      let last = try createTextNode("after").setMode("token")
      let after = try createTextNode(" rest")
      afterKey = after.getKey()
      decoratorKey = decorator.getKey()
      _ = try getRoot().clear().append(createParagraphNode().append(first, decorator, last, after))
      let selection = createRangeSelection()
      let start = backward ? selection.focus : selection.anchor
      let end = backward ? selection.anchor : selection.focus
      try start.set(first.getKey(), 0, .text)
      try end.set(last.getKey(), 5, .text)
      try setSelection(selection)
    }
    try expectDropKeepsState(
      editor, handler, .text(afterKey, 0), keys: [decoratorKey], expectPrevented: false)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a text destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a paragraph destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=false > keeps a root destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a text destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a paragraph destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::rich text drop boundaries > backward=true > keeps a root destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a text destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a paragraph destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=false > keeps a root destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a text destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a paragraph destination after a cross-paragraph deletion"
    ),
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::plain text drop boundaries > backward=true > keeps a root destination after a cross-paragraph deletion"
    ),
    arguments: boundaryCases(["text", "paragraph", "root"]))
  func keepsDestinationAfterCrossParagraphDeletion(
    handler: DropHandler, backward: Bool, position: String
  ) throws {
    let editor = try createMountedTestEditor()
    defer { editor.dispose() }
    var lastKey = ""
    var paragraphKey = ""
    try editor.update(discrete: true) { () throws in
      let first = try createTextNode("abc")
      let last = try createTextNode("def")
      let paragraph = try createParagraphNode().append(last)
      lastKey = last.getKey()
      paragraphKey = paragraph.getKey()
      _ = try getRoot().clear().append(createParagraphNode().append(first), paragraph)
      let selection = createRangeSelection()
      let start = backward ? selection.focus : selection.anchor
      let end = backward ? selection.anchor : selection.focus
      try start.set(first.getKey(), 1, .text)
      try end.set(last.getKey(), 2, .text)
      try setSelection(selection)
    }
    let points: [String: DropPoint] = [
      "paragraph": .element(paragraphKey, 1),
      "root": .element("root", 2),
      "text": .text(lastKey, 3),
    ]
    try editor.update(discrete: true) { () throws in
      let (event, dataTransfer) = createDropEvent()
      try writeDragStart(dataTransfer, editor)
      #expect(try handler.handle(event, editor, points[position]!.caret()))
      #expect(event.defaultPrevented)
    }
    try editor.read { () throws in
      // The unselected f must precede the moved bc/de, even when its former
      // paragraph was the destination caret's origin.
      #expect(try getRoot().getTextContent().replacing("\n", with: "") == "afbcde")
      #expect(try #require(try getSelection() as? RangeSelection).isCollapsed())
      if position != "root" {
        #expect(
          try getRoot().getChildren().map { try $0.getTextContent() }
            == (handler == .rich ? ["afbc", "de"] : ["afbc\nde"]))
      }
    }
  }
}

// MARK: - $handleRichTextDrop across editors

@Suite struct CrossEditorDropTests {
  // Upstream also checks that the drop dispatches `beforeinput`
  // `deleteByDrag` at the source editor's root element. Swift leaves the
  // source range to the source view (DESIGN.md, Clipboard), so this checks
  // the destination insertion and that the source is untouched, as it is
  // in jsdom, which registers no `beforeinput` deletion.
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleRichTextDrop across editors > inserts in the destination and dispatches deleteByDrag at the source root"
    ))
  func insertsInDestination() throws {
    let sourceEditor = try createMountedTestEditor()
    defer { sourceEditor.dispose() }
    let destEditor = try createMountedTestEditor()
    defer { destEditor.dispose() }
    try sourceEditor.update(discrete: true) { () throws in
      let text = try createTextNode("source-content")
      _ = try getRoot().clear().append(createParagraphNode().append(text))
      let selection = createRangeSelection()
      try selection.anchor.set(text.getKey(), 0, .text)
      try selection.focus.set(text.getKey(), 6, .text)
      try setSelection(selection)
    }
    var destTextKey = ""
    try destEditor.update(discrete: true) { () throws in
      let text = try createTextNode("destination")
      _ = try getRoot().clear().append(createParagraphNode().append(text))
      destTextKey = text.getKey()
    }
    let dataTransfer = DataTransfer()
    try sourceEditor.update(discrete: true) { () throws in
      let selection = try #require(try getSelection() as? RangeSelection)
      setLexicalClipboardDataTransfer(dataTransfer, try getClipboardDataFromSelection(selection))
      writeDragSourceToDataTransfer(dataTransfer, sourceEditor)
    }
    try destEditor.update(discrete: true) { () throws in
      let event = DragEvent("drop", dataTransfer: dataTransfer)
      #expect(try handleRichTextDrop(event, destEditor, DropPoint.text(destTextKey, 11).caret()))
      #expect(event.defaultPrevented)
    }
    #expect(try rootText(destEditor) == "destinationsource")
    #expect(try rootText(sourceEditor) == "source-content")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleRichTextDrop across editors > cancelled cross-editor drag leaves both editors untouched"
    ))
  func cancelledDrag() throws {
    let sourceEditor = try createMountedTestEditor()
    defer { sourceEditor.dispose() }
    let destEditor = try createMountedTestEditor()
    defer { destEditor.dispose() }
    try sourceEditor.update(discrete: true) { () throws in
      let paragraph = try createParagraphNode().append(createTextNode("source-content"))
      _ = try getRoot().clear().append(paragraph)
      let text = try paragraph.getFirstChildOrThrow()
      let selection = createRangeSelection()
      try selection.anchor.set(text.getKey(), 0, .text)
      try selection.focus.set(text.getKey(), 6, .text)
      try setSelection(selection)
    }
    try destEditor.update(discrete: true) { () throws in
      _ = try getRoot().clear().append(createParagraphNode().append(createTextNode("destination")))
    }
    // No drop event ever fires, so the marker-based design has nothing to
    // act on.
    #expect(try rootText(sourceEditor) == "source-content")
    #expect(try rootText(destEditor) == "destination")
  }
}

// MARK: - $handleRichTextDrop beside DOM the editor renders

private final class BlockDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      BlockDecoratorNode.self, type: "test_drop_block_decorator", extends: DecoratorNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

/// A block that renders a non-editable marker ahead of its children
/// upstream; its DOM slot only matters to the DOM-to-caret mapping.
private final class LeadingMarkerElementNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      LeadingMarkerElementNode.self, type: "test_drop_leading_marker", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("div")
  }
  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }
}

private func richTextDropEditor() throws -> (Editor, () -> Void) {
  let editor = try createMountedTestEditor(nodes: [
    .klass(BlockDecoratorNode.self), .klass(LeadingMarkerElementNode.self),
  ])
  return (editor, registerRichText(editor))
}

// Upstream maps a DOM point beside DOM the editor renders (the zero-size
// anchor before a leading block decorator, #8922, and a leading marker in an
// element's DOM slot) to a Lexical point; the view resolves those here, so
// the drop takes the Lexical point the DOM point resolves to.
@Suite struct DropBesideRenderedDOMTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleRichTextDrop beside DOM the editor renders > drops before the decorator at the DOM point before it"
    ))
  func dropsBeforeDecorator() throws {
    let (editor, unregister) = try richTextDropEditor()
    defer {
      unregister()
      editor.dispose()
    }
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("Hello foo world")
      _ = try getRoot().clear().append(
        create(BlockDecoratorNode.self), createParagraphNode().append(text))
      _ = try text.select(6, 9)
    }
    // DOM (root, 1), after the anchor, is the Lexical point (root, 0).
    try editor.update(discrete: true) { () throws in
      let (event, dataTransfer) = createDropEvent()
      try writeDragStart(dataTransfer, editor)
      #expect(try handleRichTextDrop(event, editor, DropPoint.element("root", 0).caret()))
    }
    try editor.read(.latest) { () throws in
      #expect(try childTexts(getRoot()) == ["foo", "decorator", "Hello  world"])
    }
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/HandleTextDrop.test.ts::$handleRichTextDrop beside DOM the editor renders > drops before the first child of an element at the DOM point after its marker"
    ))
  func dropsBeforeFirstChild() throws {
    let (editor, unregister) = try richTextDropEditor()
    defer {
      unregister()
      editor.dispose()
    }
    var elementKey = ""
    try editor.update(discrete: true) { () throws in
      let text = try createTextNode("Hello foo world")
      let element = try create(LeadingMarkerElementNode.self).append(createTestDecoratorNode())
      elementKey = element.getKey()
      _ = try getRoot().clear().append(element, createParagraphNode().append(text))
      _ = try text.select(6, 9)
    }
    // DOM (element, 1), after the marker, is the Lexical point (element, 0).
    try editor.update(discrete: true) { () throws in
      let (event, dataTransfer) = createDropEvent()
      try writeDragStart(dataTransfer, editor)
      #expect(try handleRichTextDrop(event, editor, DropPoint.element(elementKey, 0).caret()))
    }
    try editor.read(.latest) { () throws in
      let element = try #require(try getRoot().getFirstChild() as? ElementNode)
      #expect(try childTexts(element) == ["foo", "decorator"])
    }
  }
}
