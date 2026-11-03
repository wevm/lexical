#if canImport(UIKit)
  import Lexical
  import LexicalTextKit
  import UIKit

  // The UIKit editor view: a TextKit 2 `UITextView` that never edits its own
  // storage. Input goes to `TextKitInputController` as commands, and the view
  // shows what the model renders (Q11, Q12, Q28, Q40).

  /// The attributed string a `TextKitDocument` shows.
  @MainActor
  func renderedAttributedString(_ document: TextKitDocument, size: CGFloat) -> NSAttributedString {
    renderedAttributedString(document.string, runs: document.runs, at: 0, size: size)
  }

  /// The attributed string of `string`, which starts at `location` and holds
  /// `runs` (a `TextKitEdit`'s text).
  @MainActor
  func renderedAttributedString(
    _ string: String, runs: [TextKitRun], at location: Int, size: CGFloat
  ) -> NSAttributedString {
    let result = NSMutableAttributedString(
      string: string,
      attributes: [.font: UIFont.systemFont(ofSize: size), .foregroundColor: UIColor.label])
    for run in runs where run.length > 0 {
      let range = NSRange(location: run.location - location, length: run.length)
      let bold = run.format & IS_BOLD != 0
      var font = UIFont.systemFont(ofSize: size, weight: bold ? .bold : .regular)
      if run.format & IS_CODE != 0 {
        font = UIFont.monospacedSystemFont(ofSize: size, weight: bold ? .bold : .regular)
      }
      if run.format & IS_ITALIC != 0,
        let descriptor = font.fontDescriptor.withSymbolicTraits(
          font.fontDescriptor.symbolicTraits.union(.traitItalic))
      {
        font = UIFont(descriptor: descriptor, size: size)
      }
      result.addAttribute(.font, value: font, range: range)
      if run.format & IS_UNDERLINE != 0 {
        result.addAttribute(.underlineStyle, value: NSUnderlineStyle.single.rawValue, range: range)
      }
      if run.format & IS_STRIKETHROUGH != 0 {
        result.addAttribute(
          .strikethroughStyle, value: NSUnderlineStyle.single.rawValue, range: range)
      }
    }
    return result
  }

  /// A `UITextView` (TextKit 2) driven by a Lexical editor.
  @MainActor
  public final class LexicalUITextView: UITextView, UITextViewDelegate {
    public let controller: TextKitInputController
    private var isRendering = false
    public var fontSize: CGFloat = UIFont.systemFontSize

    public init(editor: Editor, frame: CGRect = .zero) {
      controller = TextKitInputController(editor: editor)
      super.init(frame: frame, textContainer: nil)
      delegate = self
      controller.onRender = { [weak self] edits, range in
        MainActor.assumeIsolated { self?.apply(edits, range) }
      }
      show(controller.document, try? controller.selectedRange())
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    private func show(_ document: TextKitDocument, _ range: (location: Int, length: Int)?) {
      isRendering = true
      defer { isRendering = false }
      attributedText = renderedAttributedString(document, size: fontSize)
      if let range { selectedRange = NSRange(location: range.location, length: range.length) }
    }

    /// Applies the reconciler's edits to the text storage.
    private func apply(_ edits: [TextKitEdit], _ range: (location: Int, length: Int)?) {
      isRendering = true
      defer { isRendering = false }
      if !edits.isEmpty {
        textStorage.beginEditing()
        for edit in edits {
          textStorage.replaceCharacters(
            in: NSRange(location: edit.location, length: edit.length),
            with: renderedAttributedString(
              edit.string, runs: edit.runs, at: edit.location, size: fontSize))
        }
        textStorage.endEditing()
      }
      if let range { selectedRange = NSRange(location: range.location, length: range.length) }
    }

    private func perform(_ body: () throws -> Void) {
      do { try body() } catch {}
    }

    public override func insertText(_ text: String) {
      perform { try controller.insertText(text) }
    }

    public override func deleteBackward() { perform { try controller.deleteBackward() } }

    public func textViewDidChangeSelection(_ textView: UITextView) {
      guard !isRendering else { return }
      let range = selectedRange
      perform { try controller.setSelectedRange(location: range.location, length: range.length) }
    }

    // The view never edits its own storage; every change is a command.
    public func textView(
      _ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String
    ) -> Bool {
      perform {
        try controller.setSelectedRange(location: range.location, length: range.length)
        if text.isEmpty {
          if range.length > 0 { try controller.deleteBackward() }
        } else {
          try controller.insertText(text)
        }
      }
      return false
    }
  }
#endif
