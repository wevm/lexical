#if os(macOS)
  import AppKit
  import Lexical
  import LexicalTextKit

  // The AppKit editor view: a TextKit 2 `NSTextView` that never edits its
  // own storage (Q11, Q12, Q28, Q40). Key presses go to `KEY_DOWN_COMMAND`
  // first; what a handler leaves to the platform reaches `NSTextView` as
  // usual, which moves the selection itself (as a browser does) and sends
  // editing as `NSTextInputClient` calls and NSResponder selectors, which
  // become the `beforeinput` the browser would send. Any edit that still
  // reaches the storage is refused in `shouldChangeText(in:replacementString:)`
  // and sent as a replacement instead.

  /// An `NSTextView` (TextKit 2) driven by a Lexical editor, which it is the
  /// root element of.
  @MainActor
  public final class LexicalNSTextView: NSTextView {
    public let controller: TextKitInputController
    public var editor: Editor { controller.editor }
    /// The pasteboard copy, cut and paste use.
    public var pasteboard: NSPasteboard = .general

    private var isRendering = false
    /// The last key equivalent sent to the editor, which AppKit may send
    /// again as a `keyDown` when nothing else handled it.
    private var keyEquivalentEvent: NSEvent?
    private var canUndo = false
    private var canRedo = false
    private var removeListeners: [() -> Void] = []
    /// Set while a drag that started here is dropped back here.
    private var isDropFromSelf = false
    let decorators = DecoratorAttachments()

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
      importsGraphics = false
      allowsUndo = false
      usesFontPanel = false
      textContainerInset = NSSize(width: 8, height: 8)
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

    // MARK: Rendering

    /// Applies a commit's edits and styles to the text storage, and the
    /// model's selection to the view.
    private func apply(_ update: TextKitUpdate) {
      isRendering = true
      defer { isRendering = false }
      guard let textStorage else { return }
      textStorage.beginEditing()
      let base = update.base.nsAttributes
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
          .paragraphStyle, value: paragraph.attributes.nsParagraphStyle, range: range)
        // The separator after the text takes the block's own look, so an
        // empty block's line has its height.
        if range.length > 0,
          (textStorage.string as NSString).character(at: range.upperBound - 1) == 0x0A
        {
          textStorage.addAttributes(
            paragraph.text.nsAttributes, range: NSRange(location: range.upperBound - 1, length: 1))
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
        if selectedRange() != selected { setSelectedRange(selected) }
      }
      if update.tags.contains(FOCUS_TAG), !hasFocus {
        focus()
      }
      needsDisplay = true
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

    /// Quote bars, behind the text.
    public override func drawBackground(in rect: NSRect) {
      super.drawBackground(in: rect)
      guard let layoutManager = textLayoutManager else { return }
      let origin = textContainerOrigin
      let padding = textContainer?.lineFragmentPadding ?? 0
      let document = controller.document
      for (key, bars) in controller.blockBars {
        guard let span = document.span(key),
          let frame = segmentsFrame(
            layoutManager, NSRange(location: span.location, length: max(span.length, 1)))
        else { continue }
        for bar in bars {
          bar.color.nsColor.setFill()
          NSRect(
            x: origin.x + padding + bar.offset, y: origin.y + frame.minY, width: bar.width,
            height: frame.height
          ).fill()
        }
      }
    }

    /// The union of the line fragments of `range`, in container
    /// coordinates.
    private func segmentsFrame(_ layoutManager: NSTextLayoutManager, _ range: NSRange) -> NSRect? {
      guard let textRange = textRange(range) else { return nil }
      var frame: NSRect?
      layoutManager.enumerateTextSegments(in: textRange, type: .standard, options: []) {
        _, segment, _, _ in
        frame = frame.map { $0.union(segment) } ?? segment
        return true
      }
      return frame
    }

    private func textRange(_ range: NSRange) -> NSTextRange? {
      let documentRange = contentStorage.documentRange
      guard
        let start = contentStorage.location(documentRange.location, offsetBy: range.location),
        let end = contentStorage.location(start, offsetBy: range.length)
      else { return nil }
      return NSTextRange(location: start, end: end)
    }

    private func perform(_ body: () throws -> Void) {
      do { try body() } catch {
        try? editor._onError(error)
      }
    }

    // MARK: Keys

    public override func keyDown(with event: NSEvent) {
      if event === keyEquivalentEvent {
        keyEquivalentEvent = nil
        super.keyDown(with: event)
        return
      }
      if !hasMarkedText(), let keyboardEvent = KeyboardEvent(event) {
        var handled = false
        perform { handled = try controller.keyDown(keyboardEvent) }
        if handled { return }
      }
      super.keyDown(with: event)
    }

    /// Command and Control shortcuts reach the editor's `KEY_DOWN_COMMAND`
    /// before the menus, as a web page gets them before the browser's.
    public override func performKeyEquivalent(with event: NSEvent) -> Bool {
      guard window?.firstResponder === self, !hasMarkedText(),
        event.modifierFlags.contains(.command) || event.modifierFlags.contains(.control),
        let keyboardEvent = KeyboardEvent(event)
      else { return super.performKeyEquivalent(with: event) }
      var handled = false
      perform { handled = try controller.keyDown(keyboardEvent) }
      if handled { return true }
      keyEquivalentEvent = event
      return super.performKeyEquivalent(with: event)
    }

    // MARK: Text input

    private func beforeInput(_ inputType: String, data: String? = nil) {
      perform { try controller.beforeInput(inputType, data: data) }
    }

    private static func string(_ value: Any) -> String {
      (value as? NSAttributedString)?.string ?? (value as? String) ?? ""
    }

    private static func range(_ range: NSRange) -> TextKitRange? {
      range.location == NSNotFound
        ? nil : TextKitRange(location: range.location, length: range.length)
    }

    public override func insertText(_ string: Any, replacementRange: NSRange) {
      let text = Self.string(string)
      perform {
        try controller.insertText(text, replacementRange: Self.range(replacementRange))
      }
    }

    public override func setMarkedText(
      _ string: Any, selectedRange: NSRange, replacementRange: NSRange
    ) {
      perform {
        try controller.setMarkedText(
          Self.string(string),
          selectedRange: TextKitRange(
            location: selectedRange.location, length: selectedRange.length),
          replacementRange: Self.range(replacementRange))
      }
    }

    public override func unmarkText() {
      perform { try controller.unmarkText() }
      inputContext?.discardMarkedText()
    }

    public override func hasMarkedText() -> Bool { controller.markedRange != nil }

    public override func markedRange() -> NSRange {
      guard let marked = controller.markedRange else {
        return NSRange(location: NSNotFound, length: 0)
      }
      return NSRange(location: marked.location, length: marked.length)
    }

    /// The backstop for edits that reach the storage another way
    /// (autocorrection, Writing Tools, services, menu actions): they become
    /// a replacement of that range, and the storage is left alone.
    public override func shouldChangeText(
      in affectedCharRange: NSRange, replacementString: String?
    ) -> Bool {
      if isRendering { return true }
      if let replacementString {
        perform {
          if replacementString.isEmpty {
            try controller.beforeInput(
              "deleteContent", targetRange: Self.range(affectedCharRange))
          } else {
            try controller.insertText(
              replacementString, replacementRange: Self.range(affectedCharRange))
          }
        }
      }
      return false
    }

    public override func deleteBackward(_ sender: Any?) { beforeInput("deleteContentBackward") }
    public override func deleteBackwardByDecomposingPreviousCharacter(_ sender: Any?) {
      beforeInput("deleteContentBackward")
    }
    public override func deleteForward(_ sender: Any?) { beforeInput("deleteContentForward") }
    public override func deleteWordBackward(_ sender: Any?) { beforeInput("deleteWordBackward") }
    public override func deleteWordForward(_ sender: Any?) { beforeInput("deleteWordForward") }
    public override func deleteToBeginningOfLine(_ sender: Any?) {
      beforeInput("deleteSoftLineBackward")
    }
    public override func deleteToEndOfLine(_ sender: Any?) { beforeInput("deleteSoftLineForward") }
    public override func deleteToBeginningOfParagraph(_ sender: Any?) {
      beforeInput("deleteHardLineBackward")
    }
    public override func deleteToEndOfParagraph(_ sender: Any?) {
      beforeInput("deleteHardLineForward")
    }
    public override func insertNewline(_ sender: Any?) { beforeInput("insertParagraph") }
    public override func insertParagraphSeparator(_ sender: Any?) { beforeInput("insertParagraph") }
    public override func insertNewlineIgnoringFieldEditor(_ sender: Any?) {
      beforeInput("insertParagraph")
    }
    public override func insertLineBreak(_ sender: Any?) { beforeInput("insertLineBreak") }
    public override func insertTabIgnoringFieldEditor(_ sender: Any?) {
      beforeInput("insertText", data: "\t")
    }

    /// Tab that no `KEY_TAB_COMMAND` handler took moves the focus, as it
    /// does out of a `contenteditable`.
    public override func insertTab(_ sender: Any?) { window?.selectNextKeyView(self) }
    public override func insertBacktab(_ sender: Any?) { window?.selectPreviousKeyView(self) }

    /// Escape reached the editor as `KEY_ESCAPE_COMMAND` with its key press.
    public override func cancelOperation(_ sender: Any?) {}

    public override func selectAll(_ sender: Any?) {
      perform {
        _ = try editor.update(discrete: true) { [editor] in
          try editor.dispatchCommand(SELECT_ALL_COMMAND, KeyboardEvent(key: "a", metaKey: true))
        }
      }
    }

    @objc public func undo(_ sender: Any?) { beforeInput("historyUndo") }
    @objc public func redo(_ sender: Any?) { beforeInput("historyRedo") }

    public override func validateUserInterfaceItem(
      _ item: any NSValidatedUserInterfaceItem
    )
      -> Bool
    {
      switch item.action {
      case #selector(undo(_:)): canUndo
      case #selector(redo(_:)): canRedo
      default: super.validateUserInterfaceItem(item)
      }
    }

    // MARK: Selection

    public override func setSelectedRanges(
      _ ranges: [NSValue], affinity: NSSelectionAffinity, stillSelecting: Bool
    ) {
      super.setSelectedRanges(ranges, affinity: affinity, stillSelecting: stillSelecting)
      guard !isRendering, !stillSelecting, let range = ranges.first?.rangeValue else { return }
      if let current = try? controller.selectedRange(),
        current == TextKitRange(location: range.location, length: range.length)
      {
        return
      }
      perform { try controller.setSelectedRange(location: range.location, length: range.length) }
    }

    public override func mouseDown(with event: NSEvent) {
      super.mouseDown(with: event)
      let point = convert(event.locationInWindow, from: nil)
      let mouseEvent = MouseEvent(
        "click", clientX: point.x, clientY: point.y, detail: event.clickCount,
        altKey: event.modifierFlags.contains(.option),
        ctrlKey: event.modifierFlags.contains(.control),
        metaKey: event.modifierFlags.contains(.command),
        shiftKey: event.modifierFlags.contains(.shift), targetKey: targetKey(at: point))
      perform { try controller.click(mouseEvent) }
    }

    /// The node under `point`, `"root"` past the last block.
    func targetKey(at point: NSPoint) -> NodeKey {
      if let layoutManager = textLayoutManager,
        point.y > textContainerOrigin.y + layoutManager.usageBoundsForTextContainer.maxY
      {
        return "root"
      }
      return controller.nodeKey(at: characterIndexForInsertion(at: point))
    }

    // MARK: Focus

    public override func becomeFirstResponder() -> Bool {
      guard super.becomeFirstResponder() else { return false }
      perform {
        if try editor.read(.latest, { try getSelection() }) == nil {
          let range = selectedRange()
          try controller.setSelectedRange(location: range.location, length: range.length)
        }
        try controller.focusChanged(true)
      }
      return true
    }

    public override func resignFirstResponder() -> Bool {
      guard super.resignFirstResponder() else { return false }
      perform {
        if hasMarkedText() { try controller.unmarkText() }
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

    public override func pasteAsPlainText(_ sender: Any?) {
      perform { try controller.paste(pasteboard.lexicalDataTransfer(plainTextOnly: true)) }
    }

    public override func pasteAsRichText(_ sender: Any?) { paste(sender) }

    // MARK: Drag and drop

    public override var readablePasteboardTypes: [NSPasteboard.PasteboardType] {
      TextKitPasteboard.types.map { NSPasteboard.PasteboardType($0.type) } + [
        .rtf, .rtfd, .fileURL, .png, .tiff,
      ]
    }

    public override func dragSelection(
      with event: NSEvent, offset mouseOffset: NSSize, slideBack: Bool
    ) -> Bool {
      let point = convert(event.locationInWindow, from: nil)
      guard let data = controller.report({ try controller.dragStart(x: point.x, y: point.y) }),
        let data
      else { return false }
      let item = NSDraggingItem(pasteboardWriter: data.pasteboardItem())
      let range = selectedRange()
      let frame =
        textLayoutManager.flatMap { segmentsFrame($0, range) }?.offsetBy(
          dx: textContainerOrigin.x, dy: textContainerOrigin.y)
        ?? NSRect(origin: point, size: .zero)
      item.setDraggingFrame(frame, contents: nil)
      beginDraggingSession(with: [item], event: event, source: self)
      return true
    }

    public override func draggingSession(
      _ session: NSDraggingSession, sourceOperationMaskFor context: NSDraggingContext
    ) -> NSDragOperation {
      context == .withinApplication ? [.copy, .move] : .copy
    }

    public override func draggingSession(
      _ session: NSDraggingSession, endedAt screenPoint: NSPoint, operation: NSDragOperation
    ) {
      let isMoveToOtherEditor = operation.contains(.move) && !isDropFromSelf
      isDropFromSelf = false
      perform { try controller.dragEnd(isMoveToOtherEditor: isMoveToOtherEditor) }
    }

    public override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
      let operation = super.draggingUpdated(sender)
      let point = convert(sender.draggingLocation, from: nil)
      let data = sender.draggingPasteboard.lexicalDataTransfer()
      var accepted = false
      perform { accepted = try controller.dragOver(data, x: point.x, y: point.y) }
      if !operation.isEmpty { return operation }
      return accepted ? .copy : []
    }

    public override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
      let point = convert(sender.draggingLocation, from: nil)
      let data = sender.draggingPasteboard.lexicalDataTransfer()
      isDropFromSelf = (sender.draggingSource as AnyObject?) === self
      var handled = false
      perform { handled = try controller.drop(data, x: point.x, y: point.y) }
      cleanUpAfterDragOperation()
      return handled
    }
  }

  extension LexicalNSTextView: @MainActor LexicalRootElement {
    public var isConnected: Bool { window != nil }

    public var hasFocus: Bool { window?.firstResponder === self }

    public func focus() {
      window?.makeFirstResponder(self)
    }

    public func blur() {
      if hasFocus { window?.makeFirstResponder(nil) }
    }

    /// The caret at `(x, y)` of the view, hit-tested by TextKit.
    public func caretFromPoint(_ x: Double, _ y: Double) throws -> PointCaret? {
      try controller.caret(at: characterIndexForInsertion(at: NSPoint(x: x, y: y)))
    }
  }
#endif
