#if os(macOS)
  import AppKit
  import Lexical
  import LexicalTextKit

  // The AppKit editor view: a TextKit 2 `NSTextView` that never edits its
  // own storage. Input goes to `TextKitInputController` as commands, and the
  // view shows what the model renders (Q11, Q12, Q28, Q40).

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
      attributes: [.font: NSFont.systemFont(ofSize: size), .foregroundColor: NSColor.textColor])
    for run in runs where run.length > 0 {
      let range = NSRange(location: run.location - location, length: run.length)
      let bold = run.format & IS_BOLD != 0
      var font = NSFont.systemFont(ofSize: size, weight: bold ? .bold : .regular)
      if run.format & IS_CODE != 0 {
        font = NSFont.monospacedSystemFont(ofSize: size, weight: bold ? .bold : .regular)
      }
      if run.format & IS_ITALIC != 0 {
        font = NSFontManager.shared.convert(font, toHaveTrait: .italicFontMask)
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

  /// An `NSTextView` (TextKit 2) driven by a Lexical editor.
  @MainActor
  public final class LexicalNSTextView: NSTextView {
    public let controller: TextKitInputController
    private var isRendering = false
    public var fontSize: CGFloat = NSFont.systemFontSize

    /// Kept here because the layout manager refers to it weakly.
    private let contentStorage: NSTextContentStorage

    public init(editor: Editor, frame: NSRect = .zero) {
      controller = TextKitInputController(editor: editor)
      // `init(frame:textContainer: nil)` leaves the view without a text
      // system, so the TextKit 2 stack is built here.
      contentStorage = NSTextContentStorage()
      let layoutManager = NSTextLayoutManager()
      contentStorage.addTextLayoutManager(layoutManager)
      let container = NSTextContainer(
        size: NSSize(width: frame.width, height: CGFloat.greatestFiniteMagnitude))
      container.widthTracksTextView = true
      layoutManager.textContainer = container
      super.init(frame: frame, textContainer: container)
      isRichText = true
      allowsUndo = false
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
      textStorage?.setAttributedString(renderedAttributedString(document, size: fontSize))
      if let range { setSelectedRange(NSRange(location: range.location, length: range.length)) }
    }

    /// Applies the reconciler's edits to the text storage.
    private func apply(_ edits: [TextKitEdit], _ range: (location: Int, length: Int)?) {
      isRendering = true
      defer { isRendering = false }
      if let textStorage, !edits.isEmpty {
        textStorage.beginEditing()
        for edit in edits {
          textStorage.replaceCharacters(
            in: NSRange(location: edit.location, length: edit.length),
            with: renderedAttributedString(
              edit.string, runs: edit.runs, at: edit.location, size: fontSize))
        }
        textStorage.endEditing()
      }
      if let range { setSelectedRange(NSRange(location: range.location, length: range.length)) }
    }

    private func perform(_ body: () throws -> Void) {
      do { try body() } catch { NSSound.beep() }
    }

    public override func insertText(_ string: Any, replacementRange: NSRange) {
      let text = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
      perform {
        if replacementRange.location != NSNotFound {
          try controller.setSelectedRange(
            location: replacementRange.location, length: replacementRange.length)
        }
        try controller.insertText(text)
      }
    }

    public override func deleteBackward(_ sender: Any?) {
      perform { try controller.deleteBackward() }
    }
    public override func deleteForward(_ sender: Any?) {
      perform { try controller.deleteForward() }
    }
    public override func deleteWordBackward(_ sender: Any?) {
      perform { try controller.deleteWordBackward() }
    }
    public override func deleteToBeginningOfLine(_ sender: Any?) {
      perform { try controller.deleteToBeginningOfLine() }
    }
    public override func insertNewline(_ sender: Any?) {
      perform { try controller.insertNewline() }
    }
    public override func insertLineBreak(_ sender: Any?) {
      perform { try controller.insertLineBreak() }
    }
    public override func insertTab(_ sender: Any?) { perform { try controller.insertText("\t") } }

    public override func setSelectedRanges(
      _ ranges: [NSValue], affinity: NSSelectionAffinity, stillSelecting: Bool
    ) {
      super.setSelectedRanges(ranges, affinity: affinity, stillSelecting: stillSelecting)
      guard !isRendering, !stillSelecting, let range = ranges.first?.rangeValue else { return }
      perform { try controller.setSelectedRange(location: range.location, length: range.length) }
    }

    public override func paste(_ sender: Any?) {
      if let text = NSPasteboard.general.string(forType: .string) {
        perform { try controller.insertText(text) }
      }
    }
  }
#endif
