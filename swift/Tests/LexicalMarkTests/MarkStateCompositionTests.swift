import Lexical
import LexicalMark
import LexicalTesting
import Testing

// Port of the "composition over a selection" describe of
// packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts.
//
// The composition start runs the core's empty space heuristic, which
// `dispatchCompositionStart` adds after the command's handlers, as the
// core handler runs after the handlers of a higher priority in JS.

private func startComposition(_ editor: Editor) throws {
  try update(editor) { try dispatchCompositionStart(editor) }
  #expect(editor.isComposing())
}

private func unit(_ units: [UInt16], _ at: Int) -> UInt16? {
  at >= 0 && at < units.count ? units[at] : nil
}

/// The input method writes `composed` at the caret, over the placeholder.
private func compose(_ editor: Editor, _ composed: String) throws {
  try update(editor) {
    guard let selection = try getSelection() as? RangeSelection else {
      throw LexicalError("no selection")
    }
    let target = try selection.anchor.getNode() as! TextNode
    let content = Array(try target.getTextContent().utf16)
    var at = selection.anchor.offset
    if unit(content, at) != 0x200B && unit(content, at - 1) == 0x200B {
      at -= 1
    }
    _ = try target.spliceText(
      at, unit(content, at) == 0x200B ? 1 : 0, composed, moveSelection: true)
  }
}

/// Composes `composed` over the placeholder at the caret, if there is one.
private func composeOverPlaceholder(_ editor: Editor, _ composed: String) throws {
  try update(editor) {
    guard let selection = try getSelection() as? RangeSelection else {
      throw LexicalError("no selection")
    }
    let target = try selection.anchor.getNode() as! TextNode
    let content = Array(try target.getTextContent().utf16)
    let at = selection.anchor.offset
    if unit(content, at - 1) == 0x200B || unit(content, at - 1) == 0xA0 {
      _ = try target.spliceText(at - 1, 1, composed, moveSelection: true)
    } else {
      _ = try target.spliceText(
        at, unit(content, at) == 0x200B ? 1 : 0, composed, moveSelection: true)
    }
  }
}

private func setupMarked(
  _ text: String, _ marks: [(Int, Int, String)], _ range: (Int, Int)
)
  throws -> Editor
{
  let editor = try createMarkEditor()
  try update(editor) { try initDocument(text) }
  for (start, end, id) in marks {
    try mark(editor, start, end, id)
  }
  try update(editor) { try select(range.0, range.1) }
  return editor
}

/// Collapses the selection to its focus when a composition starts.
private func collapseOnCompositionStart(_ editor: Editor) {
  _ = editor.registerCommand(
    COMPOSITION_START_COMMAND,
    { _, _ in
      if let selection = try getSelection() as? RangeSelection {
        let focus = selection.focus
        try selection.anchor.set(focus.key, focus.offset, focus.type)
      }
      return false
    }, COMMAND_PRIORITY_HIGH)
}

/// The ids of the first text node that includes `text`, or nil.
private func composedIDs(_ editor: Editor, _ text: String) throws -> [String]? {
  try editor.read {
    try getRoot().getAllTextNodes().first { try $0.getTextContent().contains(text) }
      .map(getNodeMarkIDs)
  }
}

struct CompositionCase: Sendable, CustomTestStringConvertible {
  let label: String
  let text: String
  let marked: (Int, Int)
  let replaced: (Int, Int)
  let composed: String
  let result: String
  var testDescription: String { label }

  // label, text, marked, replaced, composed, result
  static let all: [CompositionCase] = [
    CompositionCase(
      label: "the end of a run, by longer text", text: "abcd", marked: (0, 4), replaced: (2, 4),
      composed: "cde", result: ##"paragraph("abcde"[c1])"##),
    CompositionCase(
      label: "the end of a run followed by plain text, by longer text", text: "abcd xy",
      marked: (0, 4), replaced: (2, 4), composed: "cde",
      result: ##"paragraph("abcde"[c1] " xy")"##),
    CompositionCase(
      label: "a word inside a run", text: "x teh y", marked: (0, 7), replaced: (2, 5),
      composed: "the", result: ##"paragraph("x the y"[c1])"##),
    CompositionCase(
      label: "text only partly in the run", text: "abcd xy", marked: (0, 4), replaced: (2, 6),
      composed: "zz", result: ##"paragraph("ab"[c1] "zzy")"##),
  ]
}

@Suite struct MarkStateCompositionTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing the end of a run, by longer text gives the text the ids all of the replaced text had"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing the end of a run followed by plain text, by longer text gives the text the ids all of the replaced text had"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing a word inside a run gives the text the ids all of the replaced text had"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing text only partly in the run gives the text the ids all of the replaced text had"
    ),
    arguments: CompositionCase.all)
  func replacing(_ c: CompositionCase) throws {
    let editor = try setupMarked(c.text, [(c.marked.0, c.marked.1, "c1")], c.replaced)
    try startComposition(editor)
    try compose(editor, c.composed)
    try update(editor) { try setCompositionKey(nil) }
    #expect(try tree(editor) == c.result)
  }

  /// "abcd xy" with c1 on "abcd", in the block (`where`) the test names.
  private func setupSlotted(_ inSlot: Bool) throws -> Editor {
    let editor = try createMarkEditor()
    try update(editor) {
      let title = try createParagraphNode().append(createTextNode(inSlot ? "abcd xy" : "Title"))
      let block = try create(TitledParagraphNode.self).append(
        createTextNode(inSlot ? "Body" : "abcd xy"))
      _ = try setSlot(block, "title", title)
      try getRoot().clear().append(block)
    }
    try update(editor) { try addMarkID("c1", slottedTarget(inSlot).select(0, 4)) }
    try update(editor) { _ = try slottedTarget(inSlot).select(2, 4) }
    return editor
  }

  /// The block (or its slot) holding "abcd xy".
  private func slottedParent(_ inSlot: Bool) throws -> ElementNode {
    let block = try paragraph()
    return inSlot ? try getSlot(block, "title") as! ElementNode : block
  }

  private func slottedTarget(_ inSlot: Bool) throws -> TextNode {
    try slottedParent(inSlot).getFirstChildOrThrow() as! TextNode
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > in a block with a named slot > replacing the end of a run in its children gives the text the ids all of the replaced text had"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > in a block with a named slot > replacing the end of a run in its slot gives the text the ids all of the replaced text had"
    ),
    arguments: [false, true])
  func replacingInSlottedBlock(_ inSlot: Bool) throws {
    let editor = try setupSlotted(inSlot)
    try startComposition(editor)
    try compose(editor, "cde")
    try update(editor) { try setCompositionKey(nil) }
    let marked = try editor.read {
      try slottedParent(inSlot).getChildren()
        .filter { try getNodeMarkIDs($0).contains("c1") }
        .map { try $0.getTextContent() }.joined()
    }
    #expect(marked == "abcde")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > replacing the end of a run inside an inline element that adds text of its own gives the text the ids all of the replaced text had"
    ))
  func replacingInsideBracketedInline() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          create(BracketedInlineNode.self).append(createTextNode("abcd xy"))))
    }
    let text = { try getRoot().getAllTextNodes()[0] }
    try update(editor) { try addMarkID("c1", text().select(0, 4)) }
    try update(editor) { _ = try text().select(2, 4) }
    try startComposition(editor)
    try compose(editor, "cde")
    try update(editor) { try setCompositionKey(nil) }
    let marked = try editor.read {
      try getRoot().getAllTextNodes().filter { try getNodeMarkIDs($0).contains("c1") }
        .map { try $0.getTextContent() }.joined()
    }
    #expect(marked == "abcde")
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection replaced by nothing (a start that removes the selection) at the composition start gives the composed text the ids all of it had"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection replaced by a zero-width space at the composition start gives the composed text the ids all of it had"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection replaced by a no-break space at the composition start gives the composed text the ids all of it had"
    ),
    arguments: ["", "\u{200B}", "\u{00A0}"])
  func selectionReplacedAtStart(_ placeholder: String) throws {
    let editor = try setupMarked("abcd", [(0, 4, "c1")], (2, 4))
    _ = editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { payload, _ in
        guard payload == "\u{200B}", placeholder != "\u{200B}",
          let selection = try getSelection() as? RangeSelection
        else { return false }
        if placeholder.isEmpty {
          try selection.removeText()
        } else {
          try selection.insertText(placeholder)
        }
        return true
      }, COMMAND_PRIORITY_HIGH)
    try startComposition(editor)
    try composeOverPlaceholder(editor, "cde")
    try update(editor) { try setCompositionKey(nil) }
    // A no-break space the composition kept is not the point here: the
    // composed text and the text before it have the run's id
    #expect(
      try tree(editor).replacingOccurrences(of: "\u{00A0}", with: "")
        == ##"paragraph("abcde"[c1])"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection of an inline decorator without text that a handler kept was not replaced"
    ))
  func emptyDecoratorSelectionKept() throws {
    let editor = try createMarkEditor()
    try update(editor) {
      try getRoot().clear().append(
        createParagraphNode().append(
          createTextNode("ab"), create(EmptyInlineDecoratorNode.self), createTextNode("cd")))
    }
    let selectDecorator = { () throws -> RangeSelection in
      let key = try paragraph().getKey()
      let selection = createRangeSelection()
      try selection.anchor.set(key, 1, .element)
      try selection.focus.set(key, 2, .element)
      try setSelection(selection)
      return selection
    }
    try update(editor) { try addMarkID("c1", selectDecorator()) }
    try update(editor) { _ = try selectDecorator() }
    collapseOnCompositionStart(editor)
    try startComposition(editor)
    try compose(editor, "X")
    try update(editor) { try setCompositionKey(nil) }
    #expect(try composedIDs(editor, "X") == [])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection that is a no-break space, kept by a start that wrote nothing, was not replaced"
    ))
  func noBreakSpaceKept() throws {
    let editor = try setupMarked("a\u{00A0}b", [(0, 3, "c1"), (1, 2, "c2")], (1, 2))
    collapseOnCompositionStart(editor)
    // The start writes nothing (as a browser that composes in place)
    _ = editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND, { payload, _ in payload == "\u{200B}" },
      COMMAND_PRIORITY_HIGH)
    try startComposition(editor)
    try compose(editor, "X")
    try update(editor) { try setCompositionKey(nil) }
    #expect(try composedIDs(editor, "X") == ["c1"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection that a handler collapsed before the composition started was not replaced"
    ))
  func handlerCollapsedSelection() throws {
    let editor = try setupMarked("abcd", [(0, 4, "c1")], (2, 4))
    collapseOnCompositionStart(editor)
    try startComposition(editor)
    try compose(editor, "X")
    try update(editor) { try setCompositionKey(nil) }
    #expect(try tree(editor) == ##"paragraph("abcd"[c1] "X")"##)
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts::MarkStateExtension > composition over a selection > a selection that a handler of the composition placeholder kept was not replaced"
    ))
  func placeholderHandlerKeptSelection() throws {
    let editor = try setupMarked("x teh y", [(0, 7, "c1"), (2, 4, "c2")], (2, 5))
    // Quotes the selection instead of replacing it with the placeholder
    _ = editor.registerCommand(
      CONTROLLED_TEXT_INSERTION_COMMAND,
      { payload, _ in
        guard payload == "\u{200B}", let selection = try getSelection() as? RangeSelection,
          !selection.isCollapsed()
        else { return false }
        let (start, end) =
          try selection.isBackward()
          ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
        let startNode = try start.getNode() as! TextNode
        let endNode = try end.getNode() as! TextNode
        let (startAt, endAt) = (start.offset, end.offset)
        _ = try endNode.spliceText(endAt, 0, "\"")
        _ = try startNode.spliceText(startAt, 0, "\"")
        let offset = endAt + (startNode.is(endNode) ? 2 : 1)
        _ = try endNode.select(offset, offset)
        return true
      }, COMMAND_PRIORITY_HIGH)
    try startComposition(editor)
    try update(editor) { try setCompositionKey(nil) }
    #expect(try tree(editor) == ##"paragraph("x \""[c1] "te"[c1,c2] "h\" y"[c1])"##)
  }
}
