#if canImport(SwiftUI)
  import Lexical
  import LexicalTextKit
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
    /// The look of the rendered text (`theme(_:)`); `nil` keeps
    /// `TextKitTheme.default`.
    public var textKitTheme: TextKitTheme?
    /// Whether the text view paints its own background (`editorBackground(_:)`);
    /// `false` shows what is behind it, such as a message bubble.
    public var drawsBackground = true
    /// What `configure(_:)` was given, in order.
    var configurations: [@MainActor (TextView) -> Void] = []

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

    /// Whether the editor paints the text view's background (the default)
    /// or is transparent, showing the views behind it, as a chat bubble or
    /// a composer with its own background needs.
    public func editorBackground(_ visible: Bool) -> Self {
      var editor = self
      editor.drawsBackground = visible
      return editor
    }

    /// Draws the text with `theme` (the text view's `theme`), such as
    /// `TextKitTheme.default` with styles for an app's own node types under
    /// their theme keys. The text is drawn again when the theme changes.
    ///
    /// On iOS the theme's sizes scale with Dynamic Type; to keep them at
    /// the theme's points, pin the size with `.dynamicTypeSize(.large)`.
    public func theme(_ theme: TextKitTheme) -> Self {
      var editor = self
      editor.textKitTheme = theme
      return editor
    }

    /// Sets up the text view with `body` each time SwiftUI updates the
    /// editor, after the editor's own settings, as upstream `ContentEditable`
    /// passes its props to the root element on each render. Use it for what
    /// `LexicalEditor` has no modifier for, such as the text container's
    /// inset and padding:
    ///
    /// ```swift
    /// LexicalEditor(editor, placeholder: "Message")
    ///   .sizedToContent(minHeight: 36, maxHeight: 160)
    ///   .configure { textView in
    ///     textView.textContainerInset = UIEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)
    ///     textView.textContainer.lineFragmentPadding = 0
    ///   }
    /// ```
    ///
    /// `body` may read SwiftUI state, so it runs again when that changes and
    /// must set the same values each time. It is no place for registrations:
    /// commands and listeners belong to the editor, in an extension's
    /// `register` or in whatever owns the editor, as upstream plugins
    /// register on the editor. Editability is the editor's too
    /// (`editor.setEditable(_:)`), which the text view follows. Each call adds
    /// to the ones before, which run first.
    public func configure(_ body: @escaping @MainActor (TextView) -> Void) -> Self {
      var editor = self
      editor.configurations.append(body)
      return editor
    }

    /// The height of the text view's content at `width`, within
    /// `heightRange`, measured without resizing the view SwiftUI is
    /// measuring.
    fileprivate func fittingHeight(_ textView: TextView, width: CGFloat) -> CGFloat? {
      guard let heightRange else { return nil }
      let height = textView.contentHeight(atWidth: width)
      return min(max(height, heightRange.lowerBound), heightRange.upperBound)
    }
  }

  extension LexicalEditor {
    #if os(macOS)
      /// The text view `configure(_:)` sets up.
      public typealias TextView = LexicalNSTextView
    #elseif canImport(UIKit)
      /// The text view `configure(_:)` sets up.
      public typealias TextView = LexicalUITextView
    #endif
  }

  #if os(macOS)
    extension LexicalEditor: NSViewRepresentable {
      public func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        scrollView.hasVerticalScroller = true
        // Only while the content overflows, as when sized to it.
        scrollView.autohidesScrollers = true
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
        textView.drawsBackground = drawsBackground
        textView.theme = textKitTheme ?? .default
        for configure in configurations { configure(textView) }
      }

      public func sizeThatFits(
        _ proposal: ProposedViewSize, nsView scrollView: NSScrollView, context: Context
      ) -> CGSize? {
        guard heightRange != nil, let textView = scrollView.documentView as? LexicalNSTextView
        else { return nil }
        // The text wraps at the width the scroll view gives its content.
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? scrollView.frame.width
        let textWidth = width - (scrollView.frame.width - scrollView.contentSize.width)
        return fittingHeight(textView, width: textWidth).map { CGSize(width: width, height: $0) }
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
        if drawsBackground {
          if textView.backgroundColor == .clear { textView.backgroundColor = .systemBackground }
        } else {
          textView.backgroundColor = .clear
        }
        textView.theme = textKitTheme ?? .default
        for configure in configurations { configure(textView) }
      }

      public func sizeThatFits(
        _ proposal: ProposedViewSize, uiView textView: LexicalUITextView, context: Context
      ) -> CGSize? {
        // The text wraps at the width offered.
        let width = proposal.width.flatMap { $0.isFinite ? $0 : nil } ?? textView.bounds.width
        return fittingHeight(textView, width: width).map { CGSize(width: width, height: $0) }
      }
    }
  #endif
#endif
