#if canImport(SwiftUI)
  import Lexical
  import SwiftUI
  #if os(macOS)
    import AppKit
    import LexicalAppKit
  #elseif canImport(UIKit)
    import UIKit
    import LexicalUIKit
  #endif

  /// A SwiftUI editor for a Lexical `Editor`, such as one built with
  /// `buildEditorFromExtensions`: the platform text view (`LexicalNSTextView`
  /// on macOS, `LexicalUITextView` on iOS) bound to it (Q11, Q12).
  ///
  /// ```swift
  /// let editor = try buildEditorFromExtensions(
  ///   defineExtension(name: "app", dependencies: [RichTextExtension, HistoryExtension]))
  /// LexicalEditor(editor, placeholder: "Enter some rich text...")
  /// ```
  @MainActor
  public struct LexicalEditor {
    public let editor: Editor
    /// Shown while the editor is empty, as upstream `ContentEditable`'s
    /// `placeholder` (the text view's `placeholder`).
    public var placeholder: String?

    public init(_ editor: Editor) {
      self.editor = editor
    }

    public init(_ editor: Editor, placeholder: String?) {
      self.editor = editor
      self.placeholder = placeholder
    }
  }

  #if os(macOS)
    extension LexicalEditor: NSViewRepresentable {
      public func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        scrollView.drawsBackground = false
        let textView = LexicalNSTextView(editor: editor)
        textView.minSize = .zero
        textView.maxSize = NSSize(
          width: CGFloat.greatestFiniteMagnitude, height: .greatestFiniteMagnitude)
        textView.isVerticallyResizable = true
        textView.isHorizontallyResizable = false
        textView.autoresizingMask = [.width]
        scrollView.documentView = textView
        return scrollView
      }

      public func updateNSView(_ scrollView: NSScrollView, context: Context) {
        (scrollView.documentView as? LexicalNSTextView)?.placeholder = placeholder
      }
    }
  #elseif canImport(UIKit)
    extension LexicalEditor: UIViewRepresentable {
      public func makeUIView(context: Context) -> LexicalUITextView {
        LexicalUITextView(editor: editor)
      }

      public func updateUIView(_ textView: LexicalUITextView, context: Context) {
        textView.placeholder = placeholder
      }
    }
  #endif
#endif
