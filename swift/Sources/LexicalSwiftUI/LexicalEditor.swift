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
    /// When set, the editor is as tall as its content within the range and
    /// scrolls past it (`sizedToContent(minHeight:maxHeight:)`); `nil`
    /// fills the height it is offered.
    public var heightRange: ClosedRange<CGFloat>?

    public init(_ editor: Editor) {
      self.editor = editor
    }

    public init(_ editor: Editor, placeholder: String?) {
      self.editor = editor
      self.placeholder = placeholder
    }

    /// Sizes the editor to its content, between `minHeight` and
    /// `maxHeight`, past which it scrolls, as a chat composer does. Without
    /// it the editor takes the height it is offered.
    public func sizedToContent(minHeight: CGFloat = 0, maxHeight: CGFloat = .infinity) -> Self {
      var editor = self
      editor.heightRange = minHeight...max(minHeight, maxHeight)
      return editor
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
        guard let textView = scrollView.documentView as? LexicalNSTextView else { return }
        textView.placeholder = placeholder
        textView.heightRange = heightRange
      }

      public func sizeThatFits(
        _ proposal: ProposedViewSize, nsView scrollView: NSScrollView, context: Context
      ) -> CGSize? {
        guard let heightRange, let textView = scrollView.documentView as? LexicalNSTextView else {
          return nil
        }
        textView.heightRange = heightRange
        // The text wraps at the width the scroll view gives its content.
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? scrollView.frame.width
        let textWidth = width - (scrollView.frame.width - scrollView.contentSize.width)
        if textWidth > 0, textView.frame.width != textWidth {
          textView.setFrameSize(NSSize(width: textWidth, height: textView.frame.height))
        }
        return CGSize(width: width, height: textView.intrinsicContentSize.height)
      }
    }
  #elseif canImport(UIKit)
    extension LexicalEditor: UIViewRepresentable {
      public func makeUIView(context: Context) -> LexicalUITextView {
        LexicalUITextView(editor: editor)
      }

      public func updateUIView(_ textView: LexicalUITextView, context: Context) {
        textView.placeholder = placeholder
        textView.heightRange = heightRange
      }

      public func sizeThatFits(
        _ proposal: ProposedViewSize, uiView textView: LexicalUITextView, context: Context
      ) -> CGSize? {
        guard let heightRange else { return nil }
        textView.heightRange = heightRange
        // The text wraps at the width offered.
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? textView.bounds.width
        if width > 0, textView.bounds.width != width {
          textView.bounds.size.width = width
          textView.layoutIfNeeded()
        }
        return CGSize(width: width, height: textView.intrinsicContentSize.height)
      }
    }
  #endif
#endif
