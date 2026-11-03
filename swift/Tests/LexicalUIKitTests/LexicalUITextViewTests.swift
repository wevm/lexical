#if canImport(UIKit)
  import Lexical
  import LexicalExtension
  import LexicalHistory
  import LexicalRichText
  import LexicalTextKit
  import LexicalUIKit
  import Testing
  import UIKit

  // The UIKit view on the real text system: UIKeyInput and UITextInput
  // calls, marked text, the pasteboard and hit-testing, each checked against
  // the model the editor's extensions produce.

  @MainActor
  private func makeView(delay: Double = 300) throws -> (LexicalUITextView, UIWindow) {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test",
        dependencies: [
          RichTextExtension,
          configExtension(HistoryExtension) { $0.delay = delay },
        ]))
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
    let view = LexicalUITextView(editor: editor, frame: window.bounds)
    window.addSubview(view)
    window.makeKeyAndVisible()
    _ = view.becomeFirstResponder()
    view.pasteboard = UIPasteboard(name: UIPasteboard.Name("lexical-test-\(UUID())"), create: true)!
    return (view, window)
  }

  @MainActor
  private func json(_ view: LexicalUITextView) throws -> String {
    try view.editor.getEditorState().toJSON(true).stringify()
  }

  @MainActor
  @Suite struct LexicalUITextViewTests {
    @Test func usesTextKit2AndIsTheRootElement() throws {
      let (view, window) = try makeView()
      #expect(view.textLayoutManager != nil, "expected TextKit 2")
      #expect(view.editor.getRootElement() === view)
      #expect(view.isConnected)
      view.blur()
      #expect(!view.hasFocus)
      view.focus()
      #expect(view.hasFocus)
      _ = window
    }

    @Test func typesDeletesAndEnters() throws {
      let (view, window) = try makeView()
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("world")
      view.deleteBackward()
      #expect(view.text == "Hello\nworl")
      #expect(view.selectedRange == NSRange(location: 10, length: 0))
      #expect(try json(view).contains(#""text":"worl""#))
      view.selectedRange = NSRange(location: 0, length: 5)
      view.insertText("Bye")
      #expect(view.text == "Bye\nworl")
      // Autocorrection replaces a range through UITextInput.
      let start = try #require(view.position(from: view.beginningOfDocument, offset: 4))
      let end = try #require(view.position(from: start, offset: 4))
      view.replace(try #require(view.textRange(from: start, to: end)), withText: "world")
      #expect(view.text == "Bye\nworld")
      // The keyboard's Return replaces the selection with a newline: Enter.
      let caret = try #require(view.selectedTextRange)
      view.replace(caret, withText: "\n")
      #expect(view.text == "Bye\nworld\n")
      #expect(try view.editor.getEditorState().read { try getRoot().getChildrenSize() } == 3)
      // The system's undo (Command-Z, shake) goes to the editor's history.
      view.undoManager?.undo()
      #expect(view.text == "Bye\nworld")
      #expect(view.undoManager?.canRedo == true)
      view.undoManager?.redo()
      #expect(view.text == "Bye\nworld\n")
      _ = window
    }

    @Test func formatsWithTheEditMenu() throws {
      let (view, window) = try makeView()
      view.insertText("Hello world")
      view.selectedRange = NSRange(location: 6, length: 5)
      view.toggleBoldface(nil)
      #expect(try json(view).contains(#"{"type":"text","format":1,"text":"world"}"#))
      let font = try #require(
        view.textStorage.attribute(.font, at: 7, effectiveRange: nil) as? UIFont)
      #expect(font.fontDescriptor.symbolicTraits.contains(.traitBold))
      _ = window
    }

    @Test func composesMarkedTextThroughTheModel() throws {
      let (view, window) = try makeView()
      view.insertText("a")
      view.setMarkedText("k", selectedRange: NSRange(location: 1, length: 0))
      #expect(view.markedTextRange != nil)
      #expect(view.editor.isComposing())
      view.setMarkedText("かな", selectedRange: NSRange(location: 2, length: 0))
      #expect(view.text == "aかな")
      #expect(view.selectedRange == NSRange(location: 3, length: 0))
      view.insertText("仮名")
      #expect(view.markedTextRange == nil)
      #expect(!view.editor.isComposing())
      #expect(view.text == "a仮名")
      view.setMarkedText("x", selectedRange: NSRange(location: 1, length: 0))
      view.unmarkText()
      #expect(view.text == "a仮名x")
      #expect(!view.editor.isComposing())
      _ = window
    }

    @Test func undoesThroughHistory() throws {
      let (view, window) = try makeView(delay: 0)
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("World")
      view.undo()
      #expect(view.text == "Hello\n")
      view.undo()
      #expect(view.text == "Hello")
      view.redo()
      view.redo()
      #expect(view.text == "Hello\nWorld")
      _ = window
    }

    @Test func replaysEditsTheTextViewMakesToItsOwnText() throws {
      let (view, window) = try makeView()
      view.insertText("ㅎ")
      // The Korean keyboard deletes the jamo in the text storage itself,
      // then inserts the syllable.
      view.textStorage.replaceCharacters(in: NSRange(location: 0, length: 1), with: "")
      view.insertText("하")
      #expect(view.text == "하")
      #expect(try json(view).contains(#""text":"하""#))
      _ = window
    }

    @Test func mergesKeysTypedAsReplacementsOfTheSelection() throws {
      let (view, window) = try makeView()
      // The iOS keyboard types each key by replacing the selection.
      for key in ["O", "n", "e"] {
        view.replace(try #require(view.selectedTextRange), withText: key)
      }
      #expect(view.text == "One")
      #expect(view.selectedRange == NSRange(location: 3, length: 0))
      view.undoManager?.undo()
      #expect(view.text == "")
      _ = window
    }

    @Test func copiesAndPastesThroughThePasteboard() throws {
      let (view, window) = try makeView()
      view.insertText("Hello world")
      view.selectedRange = NSRange(location: 0, length: 5)
      view.copy(nil)
      #expect(view.pasteboard.types.contains("dev.lexical.editor"))
      #expect(view.pasteboard.types.contains("public.html"))
      view.cut(nil)
      #expect(view.text == " world")
      view.selectedRange = NSRange(location: 6, length: 0)
      view.paste(nil)
      #expect(view.text == " worldHello")
      _ = window
    }

    @Test func pastesHTML() throws {
      let (view, window) = try makeView()
      view.pasteboard.setItems([
        [
          "public.html":
            "<h1>Title</h1><blockquote>Quoted</blockquote><p>Some <b>bold</b> text</p>",
          "public.utf8-plain-text": "Title\nQuoted\nSome bold text",
        ]
      ])
      view.paste(nil)
      #expect(view.text == "Title\nQuoted\nSome bold text")
      let state = try json(view)
      #expect(state.contains(#""type":"heading""#))
      #expect(state.contains(#""type":"quote""#))
      #expect(state.contains(#"{"type":"text","format":1,"text":"bold"}"#))
      let heading = try #require(
        view.textStorage.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
      #expect(heading.pointSize == 24)
      _ = window
    }

    @Test func hitTestsCarets() throws {
      let (view, window) = try makeView()
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("world")
      view.layoutIfNeeded()
      let start = try #require(view.position(from: view.beginningOfDocument, offset: 6))
      let rect = view.caretRect(for: start)
      try view.editor.update(discrete: true) {
        let caret = try #require(try view.caretFromPoint(rect.midX + 1, rect.midY))
        #expect(caret.isText)
        #expect(try caret.origin.getTextContent() == "world")
        #expect(caret.offset == 0)
      }
      _ = window
    }
  }
#endif
