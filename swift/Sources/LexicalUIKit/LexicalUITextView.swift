#if canImport(UIKit)
  import Lexical
  import LexicalTextKit
  import UIKit

  // The UIKit editor view: a TextKit 2 `UITextView` that never edits its own
  // storage (Q11, Q12, Q28, Q40). Hardware key presses go to
  // `KEY_DOWN_COMMAND` first; what a handler leaves to the platform reaches
  // `UITextView` as usual, which moves the selection itself (as Safari does)
  // and sends editing through `UIKeyInput` and `UITextInput`, which become
  // the `beforeinput` Safari would send. Any edit that still reaches the
  // text is refused in the delegate's `shouldChangeTextIn` and sent as a
  // replacement instead.

  /// A `UITextView` (TextKit 2) driven by a Lexical editor, which it is the
  /// root element of. It is its own delegate.
  @MainActor
  public final class LexicalUITextView: UITextView, UITextViewDelegate {
    public let controller: TextKitInputController
    public var editor: Editor { controller.editor }
    /// The pasteboard copy, cut and paste use.
    public var pasteboard: UIPasteboard = .general

    private var isRendering = false
    private var removeListeners: [() -> Void] = []
    private var canUndo = false
    private var canRedo = false
    /// Set while a drag that started here is dropped back here.
    private var isDropFromSelf = false
    let decorators = DecoratorAttachments()
    private let blockBarsView = BlockBarsView()

    public init(editor: Editor, frame: CGRect = .zero) {
      controller = TextKitInputController(editor: editor)
      super.init(frame: frame, textContainer: nil)
      delegate = self
      allowsEditingTextAttributes = false
      textDragDelegate = self
      for interaction in interactions where interaction is UIDropInteraction {
        removeInteraction(interaction)
      }
      addInteraction(UIDropInteraction(delegate: self))
      blockBarsView.isUserInteractionEnabled = false
      blockBarsView.backgroundColor = .clear
      insertSubview(blockBarsView, at: 0)
      MicrotaskPump.install()
      removeListeners = [
        editor.registerCommand(
          CAN_UNDO_COMMAND,
          { [weak self] canUndo, _ in
            MainActor.assumeIsolated { self?.canUndo = canUndo }
            return false
          }, COMMAND_PRIORITY_CRITICAL),
        editor.registerCommand(
          CAN_REDO_COMMAND,
          { [weak self] canRedo, _ in
            MainActor.assumeIsolated { self?.canRedo = canRedo }
            return false
          }, COMMAND_PRIORITY_CRITICAL),
      ]
      decorators.onClick = { [weak self] key in
        self?.perform { try self?.controller.click(MouseEvent("click", targetKey: key)) }
      }
      controller.onRender = { [weak self] update in
        MainActor.assumeIsolated { self?.apply(update) }
      }
      controller.report { try editor.setRootElement(self) }
      controller.report { try controller.renderAll() }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    isolated deinit {
      removeListeners.forEach { $0() }
    }

    private func perform(_ body: () throws -> Void) {
      do { try body() } catch {
        try? editor._onError(error)
      }
    }

    // MARK: Rendering

    /// Applies a commit's edits and styles to the text storage, and the
    /// model's selection to the view.
    private func apply(_ update: TextKitUpdate) {
      isRendering = true
      defer { isRendering = false }
      textStorage.beginEditing()
      let base = update.base.uiAttributes
      for edit in update.edits {
        textStorage.replaceCharacters(
          in: NSRange(location: edit.edit.location, length: edit.edit.length),
          with: edit.attributedString(base: base))
      }
      let length = textStorage.length
      for paragraph in update.paragraphs {
        let range = NSIntersectionRange(
          NSRange(location: paragraph.range.location, length: paragraph.range.length),
          NSRange(location: 0, length: length))
        textStorage.addAttribute(
          .paragraphStyle, value: paragraph.attributes.uiParagraphStyle, range: range)
        if range.length > 0,
          (textStorage.string as NSString).character(at: range.upperBound - 1) == 0x0A
        {
          textStorage.addAttributes(
            paragraph.text.uiAttributes, range: NSRange(location: range.upperBound - 1, length: 1))
        }
      }
      for decorator in update.decorators where decorator.location < length {
        if let attachment = decorators.attachment(for: decorator, editor: editor) {
          textStorage.addAttribute(
            .attachment, value: attachment, range: NSRange(location: decorator.location, length: 1))
        }
      }
      textStorage.endEditing()
      if update.isFull || !update.decorators.isEmpty {
        decorators.retain(Set(editor.getDecorators().keys))
      }
      showMarkedText()
      if let range = update.selectedRange {
        let selected = NSRange(location: range.location, length: range.length)
        if selectedRange != selected { selectedRange = selected }
      }
      if update.tags.contains(FOCUS_TAG), !hasFocus {
        focus()
      }
      setNeedsLayout()
    }

    private func textRange(_ range: NSRange) -> NSTextRange? {
      guard let contentManager = textLayoutManager?.textContentManager,
        let start = contentManager.location(
          contentManager.documentRange.location, offsetBy: range.location),
        let end = contentManager.location(start, offsetBy: range.length)
      else { return nil }
      return NSTextRange(location: start, end: end)
    }

    /// Underlines the marked text, as rendering attributes so the storage
    /// keeps the model's.
    private func showMarkedText() {
      guard let layoutManager = textLayoutManager else { return }
      layoutManager.removeRenderingAttribute(.underlineStyle, for: layoutManager.documentRange)
      if let marked = controller.markedRange,
        let range = textRange(NSRange(location: marked.location, length: marked.length))
      {
        layoutManager.addRenderingAttribute(
          .underlineStyle, value: NSUnderlineStyle.single.rawValue, for: range)
      }
    }

    /// Lays out the quote bars behind the text.
    public override func layoutSubviews() {
      super.layoutSubviews()
      blockBarsView.frame = CGRect(origin: .zero, size: contentSize).union(bounds)
      guard let layoutManager = textLayoutManager else { return }
      let document = controller.document
      var bars: [(CGRect, UIColor)] = []
      for (key, blockBars) in controller.blockBars {
        guard let span = document.span(key),
          let range = textRange(NSRange(location: span.location, length: max(span.length, 1)))
        else { continue }
        var frame: CGRect?
        layoutManager.enumerateTextSegments(in: range, type: .standard, options: []) {
          _, segment, _, _ in
          frame = frame.map { $0.union(segment) } ?? segment
          return true
        }
        guard let frame else { continue }
        for bar in blockBars {
          bars.append(
            (
              CGRect(
                x: textContainerInset.left + textContainer.lineFragmentPadding + bar.offset,
                y: textContainerInset.top + frame.minY, width: bar.width, height: frame.height),
              bar.color.uiColor
            ))
        }
      }
      blockBarsView.bars = bars
      sendSubviewToBack(blockBarsView)
    }

    // MARK: Keys

    public override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
      var unhandled: Set<UIPress> = []
      for press in presses {
        guard let key = press.key, controller.markedRange == nil else {
          unhandled.insert(press)
          continue
        }
        var handled = false
        perform {
          handled = try controller.keyDown(KeyboardEvent(key, timeStamp: press.timestamp * 1000))
        }
        if !handled { unhandled.insert(press) }
      }
      if !unhandled.isEmpty { super.pressesBegan(unhandled, with: event) }
    }

    // MARK: Text input

    private func beforeInput(_ inputType: String, data: String? = nil) {
      perform { try controller.beforeInput(inputType, data: data) }
    }

    private func nsRange(_ range: UITextRange) -> NSRange {
      let location = offset(from: beginningOfDocument, to: range.start)
      return NSRange(location: location, length: offset(from: range.start, to: range.end))
    }

    private func textKitRange(_ range: UITextRange) -> TextKitRange {
      let range = nsRange(range)
      return TextKitRange(location: range.location, length: range.length)
    }

    public override func insertText(_ text: String) {
      // Return on the software keyboard is Enter, as Safari reports it.
      if text == "\n", controller.markedRange == nil {
        beforeInput("insertParagraph")
        return
      }
      perform { try controller.insertText(text) }
    }

    public override func deleteBackward() { beforeInput("deleteContentBackward") }

    public override var hasText: Bool { controller.document.length > 0 }

    public override func replace(_ range: UITextRange, withText text: String) {
      perform { try controller.insertText(text, replacementRange: textKitRange(range)) }
    }

    public override func setMarkedText(_ markedText: String?, selectedRange: NSRange) {
      perform {
        try controller.setMarkedText(
          markedText ?? "",
          selectedRange: TextKitRange(
            location: selectedRange.location, length: selectedRange.length))
      }
    }

    public override func setAttributedMarkedText(
      _ markedText: NSAttributedString?, selectedRange: NSRange
    ) {
      setMarkedText(markedText?.string, selectedRange: selectedRange)
    }

    public override func unmarkText() {
      perform { try controller.unmarkText() }
    }

    public override var markedTextRange: UITextRange? {
      guard let marked = controller.markedRange,
        let start = position(from: beginningOfDocument, offset: marked.location),
        let end = position(from: start, offset: marked.length)
      else { return nil }
      return textRange(from: start, to: end)
    }

    /// The backstop for edits that reach the text another way (dictation,
    /// Writing Tools, menu actions): they become a replacement of that
    /// range, and the text is left alone.
    public func textView(
      _ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String
    ) -> Bool {
      if isRendering { return true }
      perform {
        let target = TextKitRange(location: range.location, length: range.length)
        if text.isEmpty {
          try controller.beforeInput("deleteContent", targetRange: target)
        } else {
          try controller.insertText(text, replacementRange: target)
        }
      }
      return false
    }

    public override func toggleBoldface(_ sender: Any?) { beforeInput("formatBold") }
    public override func toggleItalics(_ sender: Any?) { beforeInput("formatItalic") }
    public override func toggleUnderline(_ sender: Any?) { beforeInput("formatUnderline") }

    /// `historyUndo`: undoes through the editor's history.
    public func undo() { beforeInput("historyUndo") }
    /// `historyRedo`: redoes through the editor's history.
    public func redo() { beforeInput("historyRedo") }
    public var editorCanUndo: Bool { canUndo }
    public var editorCanRedo: Bool { canRedo }

    public override func selectAll(_ sender: Any?) {
      perform {
        _ = try editor.update(discrete: true) { [editor] in
          try editor.dispatchCommand(SELECT_ALL_COMMAND, KeyboardEvent(key: "a", metaKey: true))
        }
      }
    }

    // MARK: Selection

    public func textViewDidChangeSelection(_ textView: UITextView) {
      guard !isRendering else { return }
      let range = selectedRange
      if let current = try? controller.selectedRange(),
        current == TextKitRange(location: range.location, length: range.length)
      {
        return
      }
      perform { try controller.setSelectedRange(location: range.location, length: range.length) }
    }

    // MARK: Focus

    public override func becomeFirstResponder() -> Bool {
      guard super.becomeFirstResponder() else { return false }
      perform {
        if try editor.read(.latest, { try getSelection() }) == nil {
          let range = selectedRange
          try controller.setSelectedRange(location: range.location, length: range.length)
        }
        try controller.focusChanged(true)
      }
      return true
    }

    public override func resignFirstResponder() -> Bool {
      guard super.resignFirstResponder() else { return false }
      perform {
        if controller.markedRange != nil { try controller.unmarkText() }
        try controller.focusChanged(false)
      }
      return true
    }

    // MARK: Clipboard

    public override func copy(_ sender: Any?) {
      perform {
        if let data = try controller.copy() { pasteboard.write(data) }
      }
    }

    public override func cut(_ sender: Any?) {
      perform {
        if let data = try controller.cut() { pasteboard.write(data) }
      }
    }

    public override func paste(_ sender: Any?) {
      perform { try controller.paste(pasteboard.lexicalDataTransfer()) }
    }

    public override func pasteAndMatchStyle(_ sender: Any?) {
      perform { try controller.paste(pasteboard.lexicalDataTransfer(plainTextOnly: true)) }
    }

    public override func canPerformAction(_ action: Selector, withSender sender: Any?) -> Bool {
      switch action {
      case #selector(paste(_:)), #selector(pasteAndMatchStyle(_:)): pasteboard.numberOfItems > 0
      default: super.canPerformAction(action, withSender: sender)
      }
    }
  }

  extension LexicalUITextView: @MainActor LexicalRootElement {
    public var isConnected: Bool { window != nil }

    public var hasFocus: Bool { isFirstResponder }

    public func focus() {
      _ = becomeFirstResponder()
    }

    public func blur() {
      if isFirstResponder { _ = resignFirstResponder() }
    }

    /// The caret at `(x, y)` of the view, hit-tested by TextKit.
    public func caretFromPoint(_ x: Double, _ y: Double) throws -> PointCaret? {
      guard let position = closestPosition(to: CGPoint(x: x, y: y)) else { return nil }
      return try controller.caret(at: offset(from: beginningOfDocument, to: position))
    }
  }

  // MARK: Drag and drop

  extension LexicalUITextView: UITextDragDelegate, UIDropInteractionDelegate {
    public func textDraggableView(
      _ textDraggableView: any UIView & UITextDraggable,
      itemsForDrag dragRequest: any UITextDragRequest
    ) -> [UIDragItem] {
      let point = dragRequest.dragSession.location(in: self)
      guard let data = controller.report({ try controller.dragStart(x: point.x, y: point.y) }),
        let data
      else { return [] }
      let item = UIDragItem(itemProvider: data.itemProvider())
      item.localObject = self
      return [item]
    }

    public func textDraggableView(
      _ textDraggableView: any UIView & UITextDraggable,
      dragSessionDidEnd session: any UIDragSession,
      with operation: UIDropOperation
    ) {
      let isMoveToOtherEditor = operation == .move && !isDropFromSelf
      isDropFromSelf = false
      perform { try controller.dragEnd(isMoveToOtherEditor: isMoveToOtherEditor) }
    }

    public func dropInteraction(
      _ interaction: UIDropInteraction, canHandle session: any UIDropSession
    ) -> Bool {
      session.hasItemsConforming(
        toTypeIdentifiers: TextKitPasteboard.types.map(\.type) + ["public.image"])
    }

    public func dropInteraction(
      _ interaction: UIDropInteraction, sessionDidUpdate session: any UIDropSession
    ) -> UIDropProposal {
      let isFromSelf = session.localDragSession?.items.contains {
        $0.localObject as? LexicalUITextView === self
      }
      return UIDropProposal(operation: isFromSelf == true ? .move : .copy)
    }

    public func dropInteraction(
      _ interaction: UIDropInteraction, performDrop session: any UIDropSession
    ) {
      let point = session.location(in: self)
      isDropFromSelf =
        session.localDragSession?.items.contains { $0.localObject as? LexicalUITextView === self }
        ?? false
      DataTransfer.load(session.items.map(\.itemProvider)) { [weak self] data in
        self?.perform { _ = try self?.controller.drop(data, x: point.x, y: point.y) }
      }
    }
  }

  /// Draws quote bars behind the text.
  final class BlockBarsView: UIView {
    var bars: [(CGRect, UIColor)] = [] {
      didSet { setNeedsDisplay() }
    }

    override func draw(_ rect: CGRect) {
      for (frame, color) in bars where frame.intersects(rect) {
        color.setFill()
        UIRectFill(frame)
      }
    }
  }
#endif
