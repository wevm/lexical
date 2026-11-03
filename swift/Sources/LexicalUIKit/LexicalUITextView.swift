#if canImport(UIKit)
  import Lexical
  import LexicalA11y
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
    /// The editor's live region, while this view hosts it
    /// (`AriaLiveRegionExtension`); its announcements are spoken.
    public internal(set) var liveRegion: LiveRegion?
    private let history = HistoryUndoManager()
    public override var undoManager: UndoManager? { history }
    private let blockBarsView = BlockBarsView()
    /// Character edits the text view made to its own text, bypassing
    /// UITextInput (the Korean keyboard merges jamo into a syllable that
    /// way), oldest first: the range each replaced and its text.
    private var foreignEdits: [(range: NSRange, text: String)] = []

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
      let storageObserver = NotificationCenter.default.addObserver(
        forName: NSTextStorage.didProcessEditingNotification, object: textStorage, queue: nil
      ) { [weak self] _ in
        MainActor.assumeIsolated { self?.recordForeignEdit() }
      }
      removeListeners = [
        { NotificationCenter.default.removeObserver(storageObserver) },
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
      history.canUndoEdit = { [weak self] in
        MainActor.assumeIsolated { self?.canUndo ?? false }
      }
      history.canRedoEdit = { [weak self] in
        MainActor.assumeIsolated { self?.canRedo ?? false }
      }
      history.onUndo = { [weak self] in MainActor.assumeIsolated { self?.undo() } }
      history.onRedo = { [weak self] in MainActor.assumeIsolated { self?.redo() } }
      decorators.onClick = { [weak self] key in
        self?.perform { try self?.controller.click(MouseEvent("click", targetKey: key)) }
      }
      // Attaching commits what is pending; the view starts from the full
      // render below, so it renders nothing before then.
      controller.report { try editor.setRootElement(self) }
      controller.onRender = { [weak self] update in
        MainActor.assumeIsolated { self?.apply(update) }
      }
      controller.report { try controller.renderAll(replacing: textStorage.length) }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    isolated deinit {
      removeListeners.forEach { $0() }
    }

    private func perform(_ body: () throws -> Void) {
      do {
        try replayForeignEdits()
        try body()
      } catch {
        try? editor._onError(error)
      }
    }

    private func recordForeignEdit() {
      guard !isRendering, textStorage.editedMask.contains(.editedCharacters) else { return }
      let edited = textStorage.editedRange
      foreignEdits.append(
        (
          NSRange(location: edited.location, length: edited.length - textStorage.changeInLength),
          (textStorage.string as NSString).substring(with: edited)
        ))
    }

    /// Brings the model up to the edits the text view made on its own, before
    /// the next input reaches it: the model's text is shown again, and the
    /// edits replay as input.
    private func replayForeignEdits() throws {
      guard !foreignEdits.isEmpty else { return }
      let edits = foreignEdits
      foreignEdits = []
      try controller.renderAll(replacing: textStorage.length)
      for edit in edits {
        let target = TextKitRange(location: edit.range.location, length: edit.range.length)
        if edit.text.isEmpty {
          try controller.beforeInput("deleteContent", targetRange: target)
        } else {
          try controller.insertText(edit.text, replacementRange: target)
        }
      }
    }

    // MARK: Rendering

    /// Applies a commit's edits and styles to the text storage, and the
    /// model's selection to the view.
    private func apply(_ update: TextKitUpdate) {
      // A commit that did not come from input finds the text edited on its
      // own: those edits are lost, and the model's text is shown again.
      if !foreignEdits.isEmpty, !update.isFull {
        foreignEdits = []
        controller.report { try controller.renderAll(replacing: textStorage.length) }
        return
      }
      foreignEdits = []
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
    /// range, and the text is left alone. The keyboard asks first for what
    /// it types over the selection or the marked text, and then calls
    /// `insertText(_:)` or `setMarkedText(_:selectedRange:)`, so that
    /// change, and any while it composes, is let through to them: refusing
    /// it would cut the IME's composition short.
    public func textView(
      _ textView: UITextView, shouldChangeTextIn range: NSRange, replacementText text: String
    ) -> Bool {
      if isRendering { return true }
      // While composing, the keyboard asks with the caret and the key it
      // typed, not the marked text.
      if controller.markedRange != nil || (!text.isEmpty && range == selectedRange) {
        return true
      }
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

    /// The text view edited its own text: the edits replay as input.
    public func textViewDidChange(_ textView: UITextView) {
      if isRendering { return }
      perform {}
    }

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
