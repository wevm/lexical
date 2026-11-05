#if os(macOS)
  import AppKit
  import Lexical
  import LexicalA11y
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
    /// Set while a shifted key the text view handles is pressed.
    private var isExtendingSelection = false
    /// Set while the mouse tracks a triple (or later) click.
    private var isSelectingParagraphs = false
    /// The inline element (a link) under the pointer, underlined as CSS
    /// `:hover` would: view state that never reaches the editor.
    private var hoveredKey: NodeKey?
    /// The runs the hover underlined, which take it off again.
    private var hoverUnderlines: [NSRange] = []
    /// The editor's live region, while this view hosts it
    /// (`AriaLiveRegionExtension`); its announcements are spoken.
    public internal(set) var liveRegion: LiveRegion?

    /// Shown while the editor is empty (`canShowPlaceholder`), over its
    /// first line in the theme's `placeholder` style, and given to
    /// accessibility as the placeholder value: upstream `ContentEditable`'s
    /// `placeholder` and `aria-placeholder`.
    public var placeholder: String? {
      didSet { needsDisplay = true }
    }
    /// Whether the editor's state can show the placeholder, as of the last
    /// commit.
    public private(set) var canShowPlaceholder = false
    private var showsPlaceholder: Bool { canShowPlaceholder && placeholder?.isEmpty == false }

    /// The text and paragraph look of the last block.
    private var lastBlockAttributes: [NSAttributedString.Key: Any]?

    /// Kept here because the layout manager refers to it weakly.
    private let contentStorage: NSTextContentStorage
    /// The layout manager's delegate, kept for the same reason.
    private let layoutDelegate = CenteredLineLayoutDelegate()
    /// The caret view, while its frame changes are observed.
    private var observedInsertionIndicator: NSTextInsertionIndicator?
    private var isFittingInsertionIndicator = false

    public init(editor: Editor, frame: NSRect = .zero) {
      controller = TextKitInputController(editor: editor)
      // `init(frame:textContainer: nil)` leaves the view without a text
      // system, so the TextKit 2 stack is built here.
      contentStorage = NSTextContentStorage()
      let layoutManager = TextBoxLayoutManager()
      contentStorage.addTextLayoutManager(layoutManager)
      let container = NSTextContainer(
        size: NSSize(width: frame.width, height: CGFloat.greatestFiniteMagnitude))
      container.widthTracksTextView = true
      layoutManager.textContainer = container
      super.init(frame: frame, textContainer: container)
      layoutManager.delegate = layoutDelegate
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
      // Attaching commits what is pending; the view starts from the full
      // render below, so it renders nothing before then.
      controller.report { try editor.setRootElement(self) }
      controller.onRender = { [weak self] update in
        MainActor.assumeIsolated { self?.apply(update) }
      }
      controller.report { try controller.renderAll(replacing: textStorage?.length ?? 0) }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    isolated deinit {
      removeListeners.forEach { $0() }
      NotificationCenter.default.removeObserver(self)
    }

    // MARK: Rendering

    /// Applies a commit's edits and styles to the text storage, and the
    /// model's selection to the view.
    private func apply(_ update: TextKitUpdate) {
      isRendering = true
      defer { isRendering = false }
      guard let textStorage else { return }
      // The hover underline comes off the old text and goes on the new.
      showHoverUnderline(false)
      defer { showHoverUnderline(true) }
      textStorage.beginEditing()
      let base = update.base.nsAttributes
      for edit in update.edits {
        textStorage.replaceCharacters(
          in: NSRange(location: edit.edit.location, length: edit.edit.length),
          with: edit.attributedString(base: base))
      }
      let length = textStorage.length
      // Text whose block changed what it inherits: the theme's keys are
      // replaced, so a look the block no longer gives goes away.
      for run in update.restyled {
        let range = NSIntersectionRange(
          NSRange(location: run.range.location, length: run.range.length),
          NSRange(location: 0, length: length))
        for key: NSAttributedString.Key in [
          .font, .foregroundColor, .backgroundColor, .underlineStyle, .strikethroughStyle,
          .baselineOffset, .lexicalHoverUnderline,
        ] {
          textStorage.removeAttribute(key, range: range)
        }
        textStorage.addAttributes(run.attributes.nsAttributes, range: range)
      }
      for paragraph in update.paragraphs {
        let range = NSIntersectionRange(
          NSRange(location: paragraph.range.location, length: paragraph.range.length),
          NSRange(location: 0, length: length))
        let style = paragraph.attributes.nsParagraphStyle(font: paragraph.text.nsFont)
        textStorage.addAttribute(.paragraphStyle, value: style, range: range)
        // The last block, unless it ends before an empty one.
        if paragraph.range.end == length,
          range.length == 0 || (textStorage.string as NSString).character(at: length - 1) != 0x0A
        {
          var attributes = paragraph.text.nsAttributes
          attributes[.paragraphStyle] = style
          lastBlockAttributes = attributes
        }
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
        decorators.retain(Set(editor.getDecorators().keys), document: controller.document)
      }
      decorators.select(update.selectedNodes)
      canShowPlaceholder = update.canShowPlaceholder
      showMarkedText()
      if let range = update.selectedRange {
        let selected = NSRange(location: range.location, length: range.length)
        if selectedRange() != selected { setSelectedRange(selected) }
      }
      showLastBlockAtCaret()
      if update.tags.contains(FOCUS_TAG), !hasFocus {
        focus()
      }
      needsDisplay = true
    }

    /// An empty last block's line is TextKit's extra line fragment, which
    /// no character carries: it, and the caret on it, take their look from
    /// the typing attributes. With the caret there, they are the last
    /// block's, so the caret moves when only the block's look changes (a
    /// list item that becomes a paragraph), and not once something is typed.
    private func showLastBlockAtCaret() {
      guard let lastBlockAttributes, let textStorage,
        selectedRange() == NSRange(location: textStorage.length, length: 0)
      else { return }
      let changed = !NSDictionary(dictionary: typingAttributes).isEqual(to: lastBlockAttributes)
      typingAttributes = lastBlockAttributes
      // TextKit takes a new look for that line from the typing attributes
      // but keeps a line it already measured where it was, without the
      // block's paragraph spacing (an empty paragraph that becomes a code
      // block stays against the line above), until the fragment is laid
      // out again.
      let text = textStorage.string as NSString
      guard changed,
        text.length == 0 || [0x0A, 0x2028].contains(text.character(at: text.length - 1)),
        let layoutManager = textLayoutManager
      else { return }
      layoutManager.enumerateTextLayoutFragments(
        from: layoutManager.documentRange.endLocation, options: [.reverse]
      ) { fragment in
        layoutManager.invalidateLayout(for: fragment.rangeInElement)
        return false
      }
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

    /// Quote bars and list markers, behind the text.
    public override func drawBackground(in rect: NSRect) {
      super.drawBackground(in: rect)
      guard let layoutManager = textLayoutManager else { return }
      let origin = textContainerOrigin
      let padding = textContainer?.lineFragmentPadding ?? 0
      let document = controller.document
      let lineWidth = (textContainer?.size.width ?? bounds.width) - 2 * padding
      for (key, layout) in controller.blockBoxes {
        guard let span = document.span(key), let frame = blockLinesFrame(layoutManager, span)
        else { continue }
        drawBlockBox(
          layout, span: span, layoutManager,
          lines: NSRect(
            x: origin.x + padding, y: origin.y + frame.minY, width: lineWidth,
            height: frame.height))
      }
      for (key, layout) in controller.listMarkers {
        guard let span = document.span(key),
          let line = firstLineFrame(
            layoutManager,
            NSRange(
              location: span.location,
              length: min(max(span.length, 1), document.length - span.location)))
        else { continue }
        // The line in view coordinates, starting where line fragments do.
        drawListMarker(
          layout,
          line: NSRect(
            x: origin.x + padding, y: origin.y + line.minY, width: lineWidth,
            height: line.height))
      }
      for (key, bars) in controller.blockBars {
        guard let span = document.span(key), let frame = blockLinesFrame(layoutManager, span)
        else { continue }
        for bar in bars {
          bar.color.nsColor.setFill()
          NSRect(
            x: origin.x + padding + bar.offset, y: origin.y + frame.minY, width: bar.width,
            height: frame.height
          ).fill()
        }
      }
      if let frame = placeholderFrame, let placeholder {
        placeholderString(placeholder).draw(
          with: frame, options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine])
      }
    }

    /// Where the placeholder's text is drawn, while it shows: on the first
    /// line, centered in it as the line's own text is, and as wide as the
    /// lines (upstream's is a line of the root's text over the first).
    private var placeholderFrame: NSRect? {
      guard showsPlaceholder, let layoutManager = textLayoutManager,
        let line = firstLineFrame(layoutManager, NSRange(location: 0, length: 0))
      else { return nil }
      let font = controller.theme.placeholderAttributes.nsFont
      let height = font.ascender - font.descender
      let padding = textContainer?.lineFragmentPadding ?? 0
      return NSRect(
        x: textContainerOrigin.x + padding, y: textContainerOrigin.y + line.midY - height / 2,
        width: (textContainer?.size.width ?? bounds.width) - 2 * padding, height: ceil(height))
    }

    /// The placeholder in the theme's look, on one line cut short with an
    /// ellipsis, as `white-space: nowrap; text-overflow: ellipsis` does.
    private func placeholderString(_ placeholder: String) -> NSAttributedString {
      var attributes = controller.theme.placeholderAttributes.nsAttributes
      let style = NSMutableParagraphStyle()
      style.lineBreakMode = .byTruncatingTail
      attributes[.paragraphStyle] = style
      return NSAttributedString(string: placeholder, attributes: attributes)
    }

    public override func accessibilityPlaceholderValue() -> String? {
      showsPlaceholder ? placeholder : nil
    }

    /// Fills `layout`'s box behind the block's lines, `lines` (in view
    /// coordinates, starting where line fragments do), and draws its
    /// gutter with a number on the first line of each of the block's lines
    /// (the text after each line break).
    private func drawBlockBox(
      _ layout: TextKitBlockBoxLayout, span: TextKitSpan, _ layoutManager: NSTextLayoutManager,
      lines: NSRect
    ) {
      let box = layout.box
      let isRTL = layout.isRightToLeft
      let frame = NSRect(
        x: isRTL ? lines.minX : lines.minX + box.offset, y: lines.minY - box.paddingTop,
        width: lines.width - box.offset, height: lines.height + box.paddingTop + box.paddingBottom)
      box.color.nsColor.setFill()
      frame.fill()
      guard let gutter = box.gutter else { return }
      let gutterFrame = NSRect(
        x: isRTL ? frame.maxX - gutter.width : frame.minX, y: frame.minY, width: gutter.width,
        height: frame.height)
      gutter.backgroundColor.nsColor.setFill()
      gutterFrame.fill()
      gutter.borderColor.nsColor.setFill()
      NSRect(
        x: isRTL ? gutterFrame.minX - gutter.borderWidth : gutterFrame.maxX, y: frame.minY,
        width: gutter.borderWidth, height: frame.height
      ).fill()
      let font = layout.text.nsFont
      let attributes: [NSAttributedString.Key: Any] = [
        .font: font, .foregroundColor: gutter.foregroundColor.nsColor,
      ]
      let text = textStorage?.string as NSString? ?? ""
      var start = span.location
      var number = 1
      while start <= span.end {
        let end = text.range(
          of: "\u{2028}", range: NSRange(location: start, length: span.end - start)
        ).location
        if let line = firstLineFrame(
          layoutManager,
          NSRange(location: start, length: min(1, max(0, text.length - start))))
        {
          let string = NSAttributedString(string: "\(number)", attributes: attributes)
          let width = string.size().width
          let y = textContainerOrigin.y + line.midY - (font.ascender - font.descender) / 2
          let x =
            isRTL ? gutterFrame.minX + gutter.padding : gutterFrame.maxX - gutter.padding - width
          string.draw(at: NSPoint(x: x, y: y))
        }
        guard end != NSNotFound else { break }
        start = end + 1
        number += 1
      }
    }

    /// The first line of `range`, in container coordinates.
    private func firstLineFrame(_ layoutManager: NSTextLayoutManager, _ range: NSRange) -> NSRect? {
      guard let textRange = textRange(range) else { return nil }
      var frame: NSRect?
      layoutManager.enumerateTextSegments(in: textRange, type: .standard, options: []) {
        _, segment, _, _ in
        if let first = frame, segment.minY >= first.maxY { return false }
        frame = frame.map { $0.union(segment) } ?? segment
        return true
      }
      return frame
    }

    /// Where the item's text sits in its first line `line`: centered, as
    /// `CenteredLineLayoutFragment` draws it.
    private func glyphBox(_ layout: TextKitListMarkerLayout, _ line: NSRect) -> NSRect {
      let font = layout.text.nsFont
      let height = font.ascender - font.descender
      return NSRect(x: line.minX, y: line.midY - height / 2, width: line.width, height: height)
    }

    // MARK: Caret

    public override func layout() {
      super.layout()
      fitInsertionIndicator()
    }

    /// Makes the caret as tall as the font at it, on its line's centered
    /// text, as a browser draws it: `NSTextView` makes it as tall as the
    /// line, from where TextKit put the text. The caret view's frame is
    /// fitted again whenever `NSTextView` moves it.
    private func fitInsertionIndicator() {
      guard !isFittingInsertionIndicator,
        let indicator = subviews.lazy.compactMap({ $0 as? NSTextInsertionIndicator }).first
      else { return }
      if indicator !== observedInsertionIndicator {
        if let observed = observedInsertionIndicator {
          NotificationCenter.default.removeObserver(
            self, name: NSView.frameDidChangeNotification, object: observed)
        }
        observedInsertionIndicator = indicator
        indicator.postsFrameChangedNotifications = true
        NotificationCenter.default.addObserver(
          self, selector: #selector(insertionIndicatorMoved(_:)),
          name: NSView.frameDidChangeNotification, object: indicator)
      }
      // The end of the text has no layout fragment of its own: the caret
      // there is on the last character's (or on the extra line TextKit
      // adds to it after a final newline).
      let length = (string as NSString).length
      let offset = selectedRange().location
      guard let layoutManager = textLayoutManager,
        let location = textRange(
          NSRange(location: offset == length && length > 0 ? length - 1 : offset, length: 0))?
          .location,
        let fragment = layoutManager.textLayoutFragment(for: location)
      else { return }
      let origin = textContainerOrigin
      let frame = fragment.layoutFragmentFrame
      let caret = indicator.frame
      // The line the caret is on, which `NSTextView` put it beside.
      guard
        let line = fragment.textLineFragments.first(where: { line in
          let bounds = line.typographicBounds.offsetBy(
            dx: frame.minX + origin.x, dy: frame.minY + origin.y)
          return bounds.minY <= caret.midY && caret.midY < bounds.maxY
        }) ?? fragment.textLineFragments.last
      else { return }
      let font = typingAttributes[.font] as? NSFont
      let ascent = font?.ascender ?? line.fontExtents?.ascent ?? 0
      let descent = font.map { -$0.descender } ?? line.fontExtents?.descent ?? 0
      let textOffset =
        line.fontExtents == nil
        ? line.textOffset(ascent: ascent, descent: descent) : line.textOffset
      let baseline =
        origin.y + frame.minY + line.typographicBounds.minY + line.glyphOrigin.y - textOffset
      let fitted = NSRect(
        x: caret.minX, y: baseline - ascent, width: caret.width, height: ascent + descent)
      guard ascent + descent > 0, fitted != caret else { return }
      isFittingInsertionIndicator = true
      indicator.frame = fitted
      isFittingInsertionIndicator = false
    }

    @objc private func insertionIndicatorMoved(_ notification: Notification) {
      fitInsertionIndicator()
    }

    /// Draws `layout`'s marker on the item's first line, `line`: a text
    /// marker in the item's font ending `TextKitListMarker.gap` before the
    /// text, or the playground theme's check box.
    private func drawListMarker(_ layout: TextKitListMarkerLayout, line: NSRect) {
      let isRTL = layout.isRightToLeft
      switch layout.marker {
      case .text(let marker):
        var attributes = layout.text.nsAttributes
        attributes[.strikethroughStyle] = nil
        attributes[.underlineStyle] = nil
        attributes[.backgroundColor] = nil
        let string = NSAttributedString(string: marker, attributes: attributes)
        let width = string.size().width
        let end = layout.headIndent - TextKitListMarker.gap
        let x = isRTL ? line.maxX - end : line.minX + end - width
        string.draw(at: NSPoint(x: x, y: glyphBox(layout, line).minY))
      case .checkbox(let checked):
        let (start, side) = layout.checkboxOffset(fontSize: layout.text.fontSize)
        let box = NSRect(
          x: isRTL ? line.maxX - start - side : line.minX + start,
          y: glyphBox(layout, line).midY - side / 2, width: side, height: side)
        let path = NSBezierPath(roundedRect: box, xRadius: 2, yRadius: 2)
        if checked {
          ListMarkerColors.checked.setFill()
          path.fill()
          let mark = NSBezierPath()
          mark.move(to: NSPoint(x: box.minX + 0.27 * side, y: box.minY + 0.52 * side))
          mark.line(to: NSPoint(x: box.minX + 0.43 * side, y: box.minY + 0.68 * side))
          mark.line(to: NSPoint(x: box.minX + 0.74 * side, y: box.minY + 0.32 * side))
          mark.lineWidth = max(1.5, 0.12 * side)
          mark.lineCapStyle = .round
          mark.lineJoinStyle = .round
          NSColor.white.setStroke()
          mark.stroke()
        } else {
          ListMarkerColors.unchecked.setStroke()
          let inset = NSBezierPath(
            roundedRect: box.insetBy(dx: 0.5, dy: 0.5), xRadius: 2, yRadius: 2)
          inset.lineWidth = 1
          inset.stroke()
        }
      }
    }

    /// The lines of the block at `span`, in container coordinates. The
    /// line a block ends on holds none of its characters when the block is
    /// empty or ends with a line break: it is the next block's separator's
    /// line, or TextKit's extra line fragment at the end of the text. It is
    /// the block's line all the same, as a trailing `<br>` gives the web's
    /// block an empty line box, so the line at the block's end joins it.
    private func blockLinesFrame(
      _ layoutManager: NSTextLayoutManager, _ span: TextKitSpan
    )
      -> NSRect?
    {
      let text = textStorage?.string as NSString? ?? ""
      let end = min(span.end, text.length)
      let lines = segmentsFrame(
        layoutManager, NSRange(location: span.location, length: end - span.location))
      guard end == span.location || text.character(at: end - 1) == 0x2028,
        let last = segmentsFrame(layoutManager, NSRange(location: end, length: 0))
      else { return lines }
      return lines.map { $0.union(last) } ?? last
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
      // A shifted key the text view handles (a shift-arrow) extends the
      // selection from the model's anchor.
      isExtendingSelection = event.modifierFlags.contains(.shift)
      defer { isExtendingSelection = false }
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
      var ranges = ranges
      // A triple click's paragraph selection stops before the paragraph's
      // newline, as `UITextView`'s does. `NSTextView` draws it while it
      // tracks the mouse, so one that took the newline would cover the
      // line's whole width until the click is reported and
      // `NormalizeTripleClickSelectionExtension` trims it.
      if isSelectingParagraphs, let last = ranges.last?.rangeValue, last.length > 1,
        (string as NSString).character(at: last.upperBound - 1) == 0x0A
      {
        ranges[ranges.count - 1] = NSValue(
          range: NSRange(location: last.location, length: last.length - 1))
      }
      super.setSelectedRanges(ranges, affinity: affinity, stillSelecting: stillSelecting)
      if stillSelecting { takeTypingAttributes() }
      showLastBlockAtCaret()
      // Each step of a click or a drag reaches the model as it happens, as
      // a browser fires `selectionchange` from the press on, not only once
      // `NSTextView` stops tracking the mouse.
      guard !isRendering, let range = ranges.first?.rangeValue else { return }
      if let current = try? controller.selectedRange(),
        current == TextKitRange(location: range.location, length: range.length)
      {
        return
      }
      // Only a shift-arrow (or another shifted key) extends from the model's
      // anchor; a click, a drag or a program selects forward.
      perform {
        try controller.setSelectedRange(
          location: range.location, length: range.length, keepsAnchor: isExtendingSelection)
      }
    }

    /// The typing attributes `NSTextView` gives a selection once the mouse
    /// is released, which it leaves as they were while it tracks it: those
    /// of the character before the caret, or after it at a line's start,
    /// and of a range's first character. The caret is fitted to their font.
    private func takeTypingAttributes() {
      guard let textStorage, textStorage.length > 0 else { return }
      let text = textStorage.string as NSString
      let range = selectedRange()
      var index = range.location
      if range.length == 0, index > 0,
        index == text.length || ![0x0A, 0x2028].contains(text.character(at: index - 1))
      {
        index -= 1
      }
      guard index < text.length else { return }
      var attributes = textStorage.attributes(at: index, effectiveRange: nil)
      attributes[.attachment] = nil
      typingAttributes = attributes
      fitInsertionIndicator()
    }

    public override func mouseDown(with event: NSEvent) {
      let point = convert(event.locationInWindow, from: nil)
      // A click in a check list item's box toggles it and leaves the
      // selection alone, as checkList.ts's pointer handlers do.
      let padding = textContainer?.lineFragmentPadding ?? 0
      if event.clickCount == 1,
        let item = controller.checkboxItem(
          at: point.x - textContainerOrigin.x - padding,
          location: characterIndexForInsertion(at: point),
          lineWidth: (textContainer?.size.width ?? bounds.width) - 2 * padding)
      {
        perform { try controller.toggleChecked(item) }
        return
      }
      isSelectingParagraphs = event.clickCount >= 3
      super.mouseDown(with: event)
      isSelectingParagraphs = false
      let mouseEvent = MouseEvent(
        "click", clientX: point.x, clientY: point.y, detail: event.clickCount,
        altKey: event.modifierFlags.contains(.option),
        ctrlKey: event.modifierFlags.contains(.control),
        metaKey: event.modifierFlags.contains(.command),
        shiftKey: event.modifierFlags.contains(.shift), targetKey: targetKey(at: point))
      perform { try controller.click(mouseEvent) }
    }

    // MARK: Hover

    public override func updateTrackingAreas() {
      super.updateTrackingAreas()
      if !trackingAreas.contains(where: { $0.owner === self && $0.userInfo?["hover"] != nil }) {
        addTrackingArea(
          NSTrackingArea(
            rect: .zero,
            options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow, .inVisibleRect],
            owner: self, userInfo: ["hover": true]))
      }
    }

    public override func mouseMoved(with event: NSEvent) {
      super.mouseMoved(with: event)
      hover(at: convert(event.locationInWindow, from: nil))
    }

    public override func mouseExited(with event: NSEvent) {
      super.mouseExited(with: event)
      hover(at: nil)
    }

    /// Underlines the inline element whose text is under `point` when its
    /// theme style has `hoverUnderline`, and takes the underline off the one
    /// the pointer left.
    private func hover(at point: NSPoint?) {
      let key = point.flatMap(hoverTarget)
      guard key != hoveredKey else { return }
      showHoverUnderline(false)
      hoveredKey = key
      showHoverUnderline(true)
    }

    /// Underlines the hovered element's text that isn't underlined already,
    /// or takes off the underline it added. The storage's attributes are
    /// the view's, so this sends no update. (TextKit 2's `NSTextView` draws
    /// no rendering attributes, so they can't hold it.)
    private func showHoverUnderline(_ shows: Bool) {
      guard let textStorage else { return }
      if !shows {
        for range in hoverUnderlines where range.upperBound <= textStorage.length {
          textStorage.removeAttribute(.underlineStyle, range: range)
        }
        hoverUnderlines = []
        return
      }
      guard let key = hoveredKey, let span = controller.document.span(key) else { return }
      let range = NSIntersectionRange(
        NSRange(location: span.location, length: span.length),
        NSRange(location: 0, length: textStorage.length))
      textStorage.enumerateAttribute(.underlineStyle, in: range) { value, run, _ in
        guard value == nil else { return }
        hoverUnderlines.append(run)
      }
      for run in hoverUnderlines {
        textStorage.addAttribute(
          .underlineStyle, value: NSUnderlineStyle.single.rawValue, range: run)
      }
    }

    /// The inline element around the character under `point`, if that
    /// character is hover-underlined.
    private func hoverTarget(_ point: NSPoint) -> NodeKey? {
      guard let textStorage, let layoutManager = textLayoutManager else { return nil }
      let insertion = characterIndexForInsertion(at: point)
      let origin = textContainerOrigin
      // The character the pointer is on: the one after the insertion point
      // or the one before it, whichever's glyphs it is over.
      guard
        let index = [insertion, insertion - 1].first(where: { index in
          guard index >= 0, index < textStorage.length,
            textStorage.attribute(.lexicalHoverUnderline, at: index, effectiveRange: nil) != nil,
            let frame = segmentsFrame(layoutManager, NSRange(location: index, length: 1))
          else { return false }
          return frame.offsetBy(dx: origin.x, dy: origin.y).contains(point)
        })
      else { return nil }
      // The widest inline element around it that is underlined throughout:
      // the link, not a mark inside it or a block outside it.
      var underlined = NSRange()
      _ = textStorage.attribute(
        .lexicalHoverUnderline, at: index, longestEffectiveRange: &underlined,
        in: NSRange(location: 0, length: textStorage.length))
      let key = controller.nodeKey(at: index)
      let document = controller.document
      return try? editor.read(.latest) { () throws -> NodeKey? in
        // The key is the text node's, or the link's when the two have the
        // same span.
        var target: NodeKey?
        var node = try getNodeByKey(key)
        while let current = node {
          if let element = current as? ElementNode {
            guard try element.isInline(), let span = document.span(element.getKey()),
              NSRange(location: span.location, length: span.length).isWithin(underlined)
            else { break }
            target = element.getKey()
          }
          node = try current.getParent()
        }
        return target
      }
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

  extension NSRange {
    fileprivate func isWithin(_ other: NSRange) -> Bool {
      location >= other.location && upperBound <= other.upperBound
    }
  }
#endif
