#if canImport(UIKit)
  import Lexical
  import LexicalTextKit
  import LexicalUIKit
  import Testing
  import UIKit

  @MainActor
  private func makeEditor() throws -> Editor {
    let editor = try createEditor(CreateEditorArgs(namespace: "", onError: { throw $0 }))
    registerTextKitSliceEditing(editor)
    return editor
  }

  @MainActor
  @Suite struct LexicalUITextViewTests {
    @Test func typesThroughTheModel() throws {
      let view = LexicalUITextView(editor: try makeEditor())
      #expect(view.textLayoutManager != nil, "expected TextKit 2")
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("world")
      view.deleteBackward()
      #expect(view.text == "Hello\nworl")
      #expect(view.selectedRange == NSRange(location: 10, length: 0))
      #expect(view.controller.document.string == view.text)
    }
  }
#endif
