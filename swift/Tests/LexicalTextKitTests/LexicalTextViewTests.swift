#if canImport(AppKit) || canImport(UIKit)
  import Lexical
  import LexicalTextKit
  import Testing
  #if canImport(AppKit)
    import AppKit
  #else
    import UIKit
  #endif

  // The native views of the typeable slice, on the platform's text system.

  @MainActor
  private func makeEditor() throws -> Editor {
    let editor = try createEditor(CreateEditorArgs(namespace: "", onError: { throw $0 }))
    registerTextKitSliceEditing(editor)
    return editor
  }

  @MainActor
  @Suite struct LexicalTextViewTests {
    #if canImport(AppKit)
      @Test func nsTextViewTypesThroughTheModel() throws {
        let view = try LexicalNSTextView(editor: makeEditor())
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
    #else
      @Test func uiTextViewTypesThroughTheModel() throws {
        let view = try LexicalUITextView(editor: makeEditor())
        #expect(view.textLayoutManager != nil, "expected TextKit 2")
        view.insertText("Hello")
        view.insertText("\n")
        view.insertText("world")
        view.deleteBackward()
        #expect(view.text == "Hello\nworl")
        #expect(view.selectedRange == NSRange(location: 10, length: 0))
        #expect(view.controller.document.string == view.text)
      }
    #endif
  }
#endif
