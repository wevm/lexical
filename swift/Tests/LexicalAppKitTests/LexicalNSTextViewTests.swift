#if os(macOS)
  import AppKit
  import Lexical
  import LexicalAppKit
  import LexicalTextKit
  import Testing

  @MainActor
  private func makeEditor() throws -> Editor {
    let editor = try createEditor(CreateEditorArgs(namespace: "", onError: { throw $0 }))
    registerTextKitSliceEditing(editor)
    return editor
  }

  @MainActor
  @Suite struct LexicalNSTextViewTests {
    @Test func typesThroughTheModel() throws {
      let view = LexicalNSTextView(editor: try makeEditor())
      #expect(view.textLayoutManager != nil, "expected TextKit 2")
      view.insertText("Hello", replacementRange: NSRange(location: NSNotFound, length: 0))
      view.insertNewline(nil)
      view.insertText("world", replacementRange: NSRange(location: NSNotFound, length: 0))
      view.deleteBackward(nil)
      #expect(view.string == "Hello\nworl")
      #expect(view.selectedRange() == NSRange(location: 10, length: 0))
      view.setSelectedRange(NSRange(location: 0, length: 5))
      view.insertText("Bye", replacementRange: NSRange(location: NSNotFound, length: 0))
      #expect(view.string == "Bye\nworl")
      #expect(view.controller.document.string == view.string)
    }
  }
#endif
