import Foundation
import Lexical
import LexicalExtension
import LexicalHistory
import LexicalList
import LexicalPlainText
import LexicalRichText
import LexicalTesting
import LexicalTextKit
import Testing

// Native input through the controller, without a view: each input is the
// event a browser would send, handled by the real extensions. Checked
// against the model and the rendered text.

/// A clock the history reads, so tests decide which changes merge.
private final class Clock: @unchecked Sendable {
  var now = 0.0
}

private func makeController(
  plainText: Bool = false, clock: Clock = Clock()
) throws -> TextKitInputController {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "test",
      dependencies: [
        plainText ? PlainTextExtension : RichTextExtension,
        configExtension(HistoryExtension) {
          $0.delay = 300
          $0.now = { clock.now }
        },
      ]))
  let controller = TextKitInputController(editor: editor)
  try controller.setSelectedRange(location: 0, length: 0)
  return controller
}

private func json(_ controller: TextKitInputController) throws -> String {
  try controller.editor.getEditorState().toJSON(true).stringify()
}

private func type(_ c: TextKitInputController, _ text: String) throws {
  for character in text {
    try c.insertText(String(character))
  }
}

private func key(
  _ key: String, code: String = "", shift: Bool = false, meta: Bool = false, alt: Bool = false
) -> KeyboardEvent {
  KeyboardEvent(key: key, code: code, altKey: alt, metaKey: meta, shiftKey: shift)
}

@Suite struct TextKitInputTests {
  @Test func typesAndRenders() throws {
    let c = try makeController()
    var rendered: [String] = []
    c.onRender = { update in rendered.append(update.edits.map(\.edit.string).joined()) }
    try type(c, "Hello")
    try c.insertText(" world")
    #expect(c.document.string == "Hello world")
    #expect(try c.selectedRange() == TextKitRange(location: 11, length: 0))
    // Each keystroke edits only the text node it types into.
    #expect(rendered.last == "Hello world")
    #expect(rendered == ["H", "He", "Hel", "Hell", "Hello", "Hello world"])
    try c.beforeInput("insertParagraph")
    try type(c, "Next")
    #expect(c.document.string == "Hello world\nNext")
    #expect(try c.selectedRange() == TextKitRange(location: 16, length: 0))
  }

  @Test func keyDownGoesThroughTheExtensions() throws {
    let c = try makeController()
    try type(c, "ab")
    // Enter and Backspace are handled at keydown, as in JS.
    #expect(try c.keyDown(key("Enter")))
    try type(c, "cd")
    #expect(c.document.string == "ab\ncd")
    #expect(try c.keyDown(key("Backspace")))
    #expect(c.document.string == "ab\nc")
    #expect(try c.keyDown(key("Enter", shift: true)))
    #expect(c.document.string == "ab\nc\u{2028}")
    // Arrow keys are left to the view's own movement.
    #expect(try !c.keyDown(key("ArrowLeft")))
    // A character key is left to the text input that follows it.
    #expect(try !c.keyDown(key("x", code: "KeyX")))
  }

  @Test func formatsWithShortcuts() throws {
    try withIsApple(true) {
      let c = try makeController()
      try type(c, "Hello world")
      try c.setSelectedRange(location: 6, length: 5)
      var update = TextKitUpdate()
      c.onRender = { update = $0 }
      #expect(try c.keyDown(key("b", code: "KeyB", meta: true)))
      #expect(update.edits.flatMap(\.runs).map(\.attributes.bold).contains(true))
      #expect(c.document.runs.map(\.format) == [0, 1])
      #expect(try json(c).contains(#"{"type":"text","format":1,"text":"world"}"#))
      // A caret after the bold text takes its format, so typing stays bold.
      try c.setSelectedRange(location: 11, length: 0)
      try type(c, "!")
      #expect(try json(c).contains(#"{"type":"text","format":1,"text":"world!"}"#))
      try c.setSelectedRange(location: 0, length: 5)
      try c.beforeInput("formatItalic")
      #expect(try json(c).contains(#"{"type":"text","format":2,"text":"Hello"}"#))
    }
  }

  @Test func attachesAChangedDecoratorAgain() throws {
    let editor = try createTestEditor()
    _ = registerRichText(editor)
    let c = TextKitInputController(editor: editor)
    // The editor decorates only while it has a root element, which a view
    // is.
    let root = NSObject()
    try editor.setRootElement(root)
    var update = TextKitUpdate()
    c.onRender = { update = $0 }
    var key: NodeKey = ""
    try editor.update(discrete: true) {
      let decorator = try createTestDecoratorNode()
      key = decorator.getKey()
      try getRoot().append(
        createParagraphNode().append(createTextNode("a"), decorator),
        createParagraphNode().append(createTextNode("b")))
    }
    #expect(update.decorators.map(\.key) == [key])
    // Its character is the same, but a view shows what it decorates now.
    try editor.update(discrete: true) { try getNodeByKey(key)?.markDirty() }
    #expect(update.edits.isEmpty)
    #expect(update.decorators.map(\.key) == [key])
    // A commit that leaves it alone does not.
    try editor.update(discrete: true) {
      try (getRoot().getLastChild() as? ElementNode)?.getFirstChild()?.markDirty()
    }
    #expect(update.decorators.isEmpty)
  }

  @Test func deletesThroughBeforeInput() throws {
    let c = try makeController()
    try type(c, "ab😀c")
    try c.beforeInput("deleteContentBackward")
    #expect(c.document.string == "ab😀")
    // The emoji is one grapheme of two UTF-16 units.
    try c.beforeInput("deleteContentBackward")
    #expect(c.document.string == "ab")
    try c.setSelectedRange(location: 0, length: 0)
    try c.beforeInput("deleteContentForward")
    #expect(c.document.string == "b")
    try c.setSelectedRange(location: 1, length: 0)
    try c.insertText(" one two")
    try c.beforeInput("deleteWordBackward")
    #expect(c.document.string == "b one ")
    try c.beforeInput("deleteSoftLineBackward")
    #expect(c.document.string == "")
  }

  @Test func mergesParagraphsOnBackspace() throws {
    let c = try makeController()
    try type(c, "a")
    try c.beforeInput("insertParagraph")
    try type(c, "b")
    try c.setSelectedRange(location: 2, length: 0)
    try c.beforeInput("deleteContentBackward")
    #expect(c.document.string == "ab")
    #expect(try c.selectedRange() == TextKitRange(location: 1, length: 0))
  }

  @Test func replacesARange() throws {
    let c = try makeController()
    try type(c, "teh cat")
    // Autocorrection replaces a range other than the selection.
    try c.insertText("the", replacementRange: TextKitRange(location: 0, length: 3))
    #expect(c.document.string == "the cat")
    try c.setSelectedRange(location: 4, length: 3)
    try c.insertText("dog")
    #expect(c.document.string == "the dog")
  }

  @Test func composesMarkedText() throws {
    let clock = Clock()
    let c = try makeController(clock: clock)
    try type(c, "a")
    clock.now = 1000
    try c.setMarkedText("k", selectedRange: TextKitRange(location: 1, length: 0))
    #expect(c.editor.isComposing())
    #expect(c.markedRange == TextKitRange(location: 1, length: 1))
    try c.setMarkedText("か", selectedRange: TextKitRange(location: 1, length: 0))
    try c.setMarkedText("かn", selectedRange: TextKitRange(location: 2, length: 0))
    #expect(c.document.string == "aかn")
    #expect(try c.selectedRange() == TextKitRange(location: 3, length: 0))
    try c.insertText("課")
    #expect(!c.editor.isComposing())
    #expect(c.markedRange == nil)
    #expect(c.document.string == "a課")
    #expect(try c.selectedRange() == TextKitRange(location: 2, length: 0))
    // The composition is one undo step.
    try c.beforeInput("historyUndo")
    #expect(c.document.string == "a")
    try c.beforeInput("historyRedo")
    #expect(c.document.string == "a課")
  }

  @Test func cancelsAndCommitsCompositionAsIs() throws {
    let c = try makeController()
    try type(c, "x")
    try c.setMarkedText("ni", selectedRange: TextKitRange(location: 2, length: 0))
    try c.setMarkedText("", selectedRange: TextKitRange(location: 0, length: 0))
    #expect(!c.editor.isComposing())
    #expect(c.document.string == "x")
    try c.setMarkedText("ni", selectedRange: TextKitRange(location: 2, length: 0))
    try c.unmarkText()
    #expect(!c.editor.isComposing())
    #expect(c.document.string == "xni")
    #expect(try c.selectedRange() == TextKitRange(location: 3, length: 0))
  }

  @Test func showsASuffixOutsideTheModel() throws {
    try withIsApple(true) {
      let c = try makeController()
      try type(c, "He ")
      #expect(try c.keyDown(key("b", code: "KeyB", meta: true)))
      try type(c, "b")
      try c.beforeInput("insertParagraph")
      try type(c, "next")
      try c.setSelectedRange(location: 3, length: 0)
      let before = c.editor.getEditorState()
      let model = try json(c)
      // The view's text, built from the edits it is sent.
      var shown = c.document.string
      c.onRender = { update in
        for edit in update.edits.map(\.edit) {
          let utf16 = shown.utf16
          let lower = utf16.index(utf16.startIndex, offsetBy: edit.location)
          shown.replaceSubrange(
            lower..<utf16.index(lower, offsetBy: edit.length), with: edit.string)
        }
      }
      let key = c.document.runs[0].key
      try c.setSuffix(TextKitSuffix(key: key, text: "llo", type: "autocomplete"))
      // The text after it moves aside: the bold node and the next block.
      #expect(c.document.string == "He llob\nnext")
      #expect(shown == c.document.string)
      #expect(c.document.runs.map(\.length) == [3, 3, 1, 4])
      #expect(c.document.runs[1].type == "autocomplete")
      #expect(c.document.location(c.document.runs[2].key, 0, .text) == 6)
      // The model, its selection and the history don't see it.
      #expect(c.editor.getEditorState() === before)
      #expect(try json(c) == model)
      #expect(try c.selectedRange() == TextKitRange(location: 3, length: 0))
      // A caret can't stop in it: a click there lands at the node's end,
      // and the arrows cross it in one step.
      try c.setSelectedRange(location: 5, length: 0)
      #expect(try c.selectedRange() == TextKitRange(location: 3, length: 0))
      try c.setSelectedRange(location: 6, length: 0)
      #expect(try c.selectedRange() == TextKitRange(location: 6, length: 0))
      for (location, isBackward, target) in [
        (3, false, 6), (6, true, 3), (7, true, 6), (2, false, 3),
      ] {
        #expect(
          c.document.move(location, isBackward: isBackward, granularity: .character) == target)
      }
      // Replacing it edits only it.
      try c.setSuffix(TextKitSuffix(key: key, text: "llo world", type: "autocomplete"))
      #expect(c.document.string == "He llo worldb\nnext")
      #expect(shown == c.document.string)
      // Typing into its node drops it, and the history has only the typing.
      try c.setSelectedRange(location: 3, length: 0)
      try type(c, "l")
      #expect(c.document.string == "He lb\nnext")
      #expect(c.document.suffix == nil)
      #expect(shown == c.document.string)
      try c.beforeInput("historyUndo")
      #expect(try json(c) == model)
      // Accepted: the same characters, now the node's own, at its look.
      try c.setSuffix(TextKitSuffix(key: key, text: "llo", type: "autocomplete"))
      try c.editor.update(discrete: true) {
        let node = try getNodeByKey(key) as! TextNode
        _ = try node.spliceText(3, 0, "llo", moveSelection: true)
      }
      #expect(c.document.string == "He llob\nnext")
      #expect(c.document.suffix == nil)
      #expect(c.document.runs.map(\.length) == [6, 1, 4])
      #expect(c.document.runs.allSatisfy { $0.type == nil })
      #expect(try c.selectedRange() == TextKitRange(location: 6, length: 0))
      try c.beforeInput("historyUndo")
      #expect(try json(c) == model)
      shown = c.document.string
      // Shown again, then removed.
      try c.setSuffix(TextKitSuffix(key: key, text: "llo", type: "autocomplete"))
      try c.setSuffix(nil)
      #expect(c.document.string == "He b\nnext")
      #expect(shown == c.document.string)
    }
  }

  @Test func endsTheMarkedTextWhenTheModelEndsTheComposition() throws {
    let c = try makeController()
    try type(c, "a ")
    try c.setMarkedText("사", selectedRange: TextKitRange(location: 1, length: 0))
    try c.setMarkedText("사요", selectedRange: TextKitRange(location: 2, length: 0))
    var ended: [Bool] = []
    c.onRender = { ended.append($0.endsComposition) }
    // A suggestion accepted mid-composition, as the playground's
    // autocomplete does it.
    try c.editor.update {
      let node = try getRoot().getFirstDescendant() as! TextNode
      try node.setTextContent(try node.getTextContent() + "약")
      try setCompositionKey(nil)
      _ = try node.select()
    }
    #expect(ended.first == true)
    #expect(c.markedRange == nil)
    #expect(!c.editor.isComposing())
    #expect(c.document.string == "a 사요약")
    // The IME's next marked text starts a composition of its own after it.
    try c.setMarkedText("ㄱ", selectedRange: TextKitRange(location: 1, length: 0))
    #expect(c.editor.isComposing())
    #expect(c.markedRange == TextKitRange(location: 5, length: 1))
    try c.unmarkText()
    #expect(c.document.string == "a 사요약ㄱ")
    #expect(try c.selectedRange() == TextKitRange(location: 6, length: 0))
  }

  @Test func undoesAndRedoes() throws {
    let clock = Clock()
    let c = try makeController(clock: clock)
    try type(c, "Hello")
    clock.now = 1000
    try c.beforeInput("insertParagraph")
    try type(c, "World")
    #expect(c.document.string == "Hello\nWorld")
    // The paragraph and the typing after it are separate steps, as in JS.
    try c.beforeInput("historyUndo")
    #expect(c.document.string == "Hello\n")
    try c.beforeInput("historyUndo")
    #expect(c.document.string == "Hello")
    try c.beforeInput("historyRedo")
    try c.beforeInput("historyRedo")
    #expect(c.document.string == "Hello\nWorld")
  }

  @Test func copiesCutsAndPastes() throws {
    let c = try makeController()
    try type(c, "Hello world")
    try c.setSelectedRange(location: 0, length: 5)
    let copied = try #require(try c.copy())
    #expect(copied.getData("text/plain") == "Hello")
    #expect(copied.getData("text/html").contains("Hello"))
    #expect(copied.getData("application/x-lexical-editor").contains(#""text":"Hello""#))
    let cut = try #require(try c.cut())
    #expect(cut.getData("text/plain") == "Hello")
    #expect(c.document.string == " world")
    // Lexical's own format wins over the others.
    try c.setSelectedRange(location: 6, length: 0)
    try c.paste(copied)
    #expect(c.document.string == " worldHello")
  }

  @Test func pastesHTML() throws {
    let c = try makeController()
    var update = TextKitUpdate()
    c.onRender = { update = $0 }
    let data = DataTransfer()
    data.setData("text/html", "<h1>Title</h1><p>Some <b>bold</b> text</p>")
    data.setData("text/plain", "Title\nSome bold text")
    try c.paste(data)
    #expect(c.document.string == "Title\nSome bold text")
    let state = try json(c)
    #expect(state.contains(#""type":"heading""#))
    #expect(state.contains(#"{"type":"text","format":1,"text":"bold"}"#))
    let heading = try #require(update.paragraphs.first { $0.range.location == 0 })
    #expect(heading.text.fontSize == 24)
    #expect(update.edits.flatMap(\.runs).first?.attributes.fontSize == 24)
  }

  @Test func plainTextPastesText() throws {
    let c = try makeController(plainText: true)
    let data = DataTransfer()
    data.setData("text/html", "<b>bold</b>")
    data.setData("text/plain", "bold")
    try c.paste(data)
    #expect(try !json(c).contains(#""format":1"#))
    #expect(c.document.string == "bold")
  }

  @Test func lineBreaksAndLocations() throws {
    let c = try makeController()
    try type(c, "ab")
    try c.beforeInput("insertLineBreak")
    try type(c, "cd")
    #expect(c.document.string == "ab\u{2028}cd")
    try c.beforeInput("insertParagraph")
    let document = c.document
    try c.editor.read { () throws in
      let paragraphs = try getRoot().getChildren()
      let first = try #require(try (paragraphs[0] as? ElementNode)?.getFirstChild())
      #expect(document.point(at: 1) == (first.getKey(), 1, .text))
      #expect(document.location(paragraphs[1].getKey(), 0, .element) == 6)
      #expect(document.location("root", 2, .element) == 6)
    }
  }

  @Test func resolvesCaretsAndNodes() throws {
    let c = try makeController()
    try type(c, "ab")
    try c.beforeInput("insertParagraph")
    try c.editor.read { () throws in
      let caret = try #require(try c.caret(at: 1))
      #expect(caret.isText && caret.offset == 1)
      let empty = try #require(try c.caret(at: 3))
      #expect(empty.isChild)
      #expect(try empty.origin.is(getRoot().getLastChild()))
    }
    #expect(c.nodeKey(at: 0) != "root")
  }
}

@Suite struct TextKitCheckboxTests {
  private func makeListController(_ listType: ListType) throws -> TextKitInputController {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "test", dependencies: [RichTextExtension, CheckListExtension]))
    let controller = TextKitInputController(editor: editor)
    try controller.setSelectedRange(location: 0, length: 0)
    try controller.insertText("a")
    try editor.update(discrete: true) { try insertList(listType) }
    return controller
  }

  @Test func aClickInTheBoxColumnTogglesTheItem() throws {
    let c = try makeListController("check")
    let item = try #require(c.listMarkers.first?.key)
    // The default theme: a 16pt item indented 32, so its 14.4pt box starts 8 in.
    #expect(c.listMarkers[item]?.marker == .checkbox(checked: false))
    #expect(c.checkboxItem(at: 9, location: 0, lineWidth: 600) == item)
    #expect(c.checkboxItem(at: 23, location: 0, lineWidth: 600) == nil)
    #expect(c.checkboxItem(at: 7, location: 0, lineWidth: 600) == nil)
    var restyled: [TextKitStyledRun] = []
    c.onRender = { restyled = $0.restyled }
    try c.toggleChecked(item)
    #expect(c.listMarkers[item]?.marker == .checkbox(checked: true))
    // The item's text, left in place, takes the checked item's strikethrough.
    #expect(restyled.map(\.range) == [TextKitRange(location: 0, length: 1)])
    #expect(restyled.first?.attributes.strikethrough == true)
    #expect(
      try c.editor.read(.latest) { try (getNodeByKey(item) as? ListItemNode)?.getChecked() }
        == true)
    try c.toggleChecked(item)
    #expect(c.listMarkers[item]?.marker == .checkbox(checked: false))
    #expect(restyled.first?.attributes.strikethrough == false)
  }

  @Test func aClickTogglesTheInnermostItem() throws {
    let c = try makeListController("check")
    _ = try c.keyDown(KeyboardEvent(key: "Enter"))
    try c.insertText("b")
    try c.editor.update(discrete: true) {
      _ = try dispatchCommand(c.editor, INDENT_CONTENT_COMMAND, ())
    }
    // "a\nb": the nested item's box is one indent level further in.
    let nested = try #require(c.listMarkers.first { $0.value.headIndent > 32 }?.key)
    let indent = try #require(c.listMarkers[nested]?.headIndent) - 32
    #expect(c.checkboxItem(at: 9, location: 2, lineWidth: 600) == nil)
    #expect(c.checkboxItem(at: 9 + indent, location: 2, lineWidth: 600) == nested)
    try c.toggleChecked(nested)
    let checked = c.listMarkers.values.map(\.marker).filter { $0 == .checkbox(checked: true) }
    #expect(checked.count == 1)
    #expect(c.listMarkers[nested]?.marker == .checkbox(checked: true))
  }

  @Test func aBulletHasNoBox() throws {
    let c = try makeListController("bullet")
    #expect(c.checkboxItem(at: 9, location: 0, lineWidth: 600) == nil)
  }
}
