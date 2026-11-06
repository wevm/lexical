#if canImport(UIKit)
  import Lexical
  import LexicalA11y
  import LexicalTextKit
  import UIKit

  /// A block separator and a LineBreakNode's character.
  private let lineBreaks = CharacterSet(charactersIn: "\n\u{2028}")

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
    /// Menu elements the edit menu shows for a selection, before the
    /// system's (see `LexicalUITextView+EditMenu.swift`).
    public var editMenuElements: ((_ range: NSRange) -> [UIMenuElement])?

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
    /// The text and paragraph look of the last block.
    private var lastBlockAttributes: [NSAttributedString.Key: Any]?
    /// Whether the selection is text, which shows the caret, the highlight
    /// and the handles. A node selection shows only its decorators'
    /// outlines, as on the web, where it has no DOM range.
    private var showsTextSelection = true {
      didSet { showSelectionDisplay() }
    }
    /// Shown while the editor is empty (`canShowPlaceholder`), over its
    /// first line in the theme's `placeholder` style, and spoken as the
    /// view's value then, as an empty text field's placeholder is: upstream
    /// `ContentEditable`'s `placeholder` and `aria-placeholder`.
    public var placeholder: String? {
      didSet {
        placeholderLabel.text = placeholder
        setNeedsLayout()
      }
    }
    /// When set, the view's intrinsic height is its content's
    /// (`contentHeight`) kept within the range, so it grows with its text
    /// up to the upper bound, as a chat composer does, and scrolls past it.
    /// `nil` (the default) leaves the intrinsic size to `UITextView`.
    public var heightRange: ClosedRange<CGFloat>? {
      didSet { invalidateIntrinsicContentSize() }
    }

    /// The height of the laid-out text with the container inset above and
    /// below it.
    public var contentHeight: CGFloat {
      let insets = textContainerInset.top + textContainerInset.bottom
      guard let layoutManager = textLayoutManager else { return insets }
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      return ceil(layoutManager.usageBoundsForTextContainer.height + insets)
    }

    public override var intrinsicContentSize: CGSize {
      guard let heightRange else { return super.intrinsicContentSize }
      return CGSize(
        width: UIView.noIntrinsicMetric,
        height: min(max(contentHeight, heightRange.lowerBound), heightRange.upperBound))
    }

    private var lastLaidOutWidth: CGFloat = 0

    /// Whether the editor's state can show the placeholder, as of the last
    /// commit.
    public private(set) var canShowPlaceholder = false
    private var showsPlaceholder: Bool { canShowPlaceholder && placeholder?.isEmpty == false }
    private let placeholderLabel = UILabel()
    /// The layout manager's delegate, kept here because it is held weakly.
    private let layoutDelegate = CenteredLineLayoutDelegate()
    /// A control over each check list item's box, which toggles the item:
    /// a tap on a control does not reach the text view's own taps, so the
    /// caret stays where it was, as checkList.ts leaves it.
    private var checkboxes: [NodeKey: CheckboxControl] = [:]
    /// Character edits the text view made to its own text, bypassing
    /// UITextInput (the Korean keyboard merges jamo into a syllable that
    /// way), oldest first: the range each replaced and its text.
    private var foreignEdits: [(range: NSRange, text: String)] = []
    /// Set while a shifted hardware key the text view handles is pressed.
    private var isExtendingSelection = false
    /// Set while hardware key presses no handler took reach the text view,
    /// which have had their `keydown` already.
    private var isPassingPresses = false

    public init(editor: Editor, frame: CGRect = .zero) {
      controller = TextKitInputController(editor: editor)
      super.init(frame: frame, textContainer: nil)
      delegate = self
      textLayoutManager?.delegate = layoutDelegate
      allowsEditingTextAttributes = false
      textDragDelegate = self
      for interaction in interactions where interaction is UIDropInteraction {
        removeInteraction(interaction)
      }
      addInteraction(UIDropInteraction(delegate: self))
      blockBarsView.isUserInteractionEnabled = false
      blockBarsView.backgroundColor = .clear
      insertSubview(blockBarsView, at: 0)
      placeholderLabel.isUserInteractionEnabled = false
      placeholderLabel.isAccessibilityElement = false
      placeholderLabel.lineBreakMode = .byTruncatingTail
      placeholderLabel.isHidden = true
      addSubview(placeholderLabel)
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
      decorators.dragItems = { [weak self] key, view in
        self?.dragItems(forDecorator: key, view: view) ?? []
      }
      decorators.onDragEnd = { [weak self] operation in
        guard let self else { return }
        let isMoveToOtherEditor = operation == .move && !isDropFromSelf
        isDropFromSelf = false
        perform { try controller.dragEnd(isMoveToOtherEditor: isMoveToOtherEditor) }
      }
      // A triple (or later) tap is a click with the tap count as `detail`,
      // after the text view has applied the paragraph selection it makes,
      // as AppKit's `mouseDown(with:)` reports one after tracking.
      let tapCount = TapCountRecognizer()
      tapCount.onTap = { [weak self] count, location in
        DispatchQueue.main.async { self?.click(count: count, at: location) }
      }
      addGestureRecognizer(tapCount)
      // Attaching commits what is pending; the view starts from the full
      // render below, so it renders nothing before then.
      controller.report { try editor.setRootElement(self) }
      controller.onHighlightsChange = { [weak self] in
        MainActor.assumeIsolated { self?.showHighlights() }
      }
      controller.onRender = { [weak self] update in
        MainActor.assumeIsolated { self?.apply(update) }
      }
      controller.report { try controller.renderAll(replacing: textStorage.length) }
      // Dynamic Type: the theme's fonts scale with the content size
      // category, so a change renders the document again.
      registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) {
        (self: Self, _: UITraitCollection) in
        self.controller.report { try self.controller.renderAll(replacing: self.textStorage.length) }
      }
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
      let contentSize = traitCollection.preferredContentSizeCategory
      let base = update.base.uiAttributes(contentSize)
      for edit in update.edits {
        textStorage.replaceCharacters(
          in: NSRange(location: edit.edit.location, length: edit.edit.length),
          with: edit.attributedString(base: base, contentSize: contentSize))
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
          .baselineOffset,
        ] {
          textStorage.removeAttribute(key, range: range)
        }
        textStorage.addAttributes(run.attributes.uiAttributes(contentSize), range: range)
      }
      for paragraph in update.paragraphs {
        let range = NSIntersectionRange(
          NSRange(location: paragraph.range.location, length: paragraph.range.length),
          NSRange(location: 0, length: length))
        let style = paragraph.attributes.uiParagraphStyle(font: paragraph.text.uiFont(contentSize))
        textStorage.addAttribute(.paragraphStyle, value: style, range: range)
        // The last block, unless it ends before an empty one.
        if paragraph.range.end == length,
          range.length == 0 || (textStorage.string as NSString).character(at: length - 1) != 0x0A
        {
          var attributes = paragraph.text.uiAttributes(contentSize)
          attributes[.paragraphStyle] = style
          lastBlockAttributes = attributes
        }
        // The separator after the text, and each line break in it, take
        // the block's own look, so an empty block's line, and an empty
        // line after a break, have its height and caret, as the web's
        // `<br>` takes its block's font.
        let text = textStorage.string as NSString
        var start = range.location
        while start < range.upperBound {
          let found = text.rangeOfCharacter(
            from: lineBreaks, range: NSRange(location: start, length: range.upperBound - start))
          guard found.location != NSNotFound else { break }
          if text.character(at: found.location) == 0x2028 || found.upperBound == range.upperBound {
            textStorage.addAttributes(paragraph.text.uiAttributes(contentSize), range: found)
          }
          start = found.upperBound
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
      setNeedsLayout()
      // The model ended the composition: the keyboard starts afresh, as
      // after any change to the text it didn't make.
      if update.endsComposition {
        inputDelegate?.textWillChange(self)
        inputDelegate?.textDidChange(self)
      }
      showMarkedText()
      showHighlights()
      if let range = update.selectedRange {
        let selected = NSRange(location: range.location, length: range.length)
        if selectedRange != selected { selectedRange = selected }
        // `updateDOMSelection`'s `scrollIntoViewIfNeeded`: a collapsed
        // caret in the focused editor is kept in view, as when typing at the
        // end of a scrolled composer.
        if selected.length == 0, hasFocus, !update.tags.contains(SKIP_SCROLL_INTO_VIEW_TAG) {
          scrollRangeToVisible(selected)
        }
      }
      showsTextSelection = update.selectedNodes.isEmpty
      showLastBlockAtCaret()
      if update.tags.contains(FOCUS_TAG), !hasFocus {
        focus()
      }
      setNeedsLayout()
      if heightRange != nil { invalidateIntrinsicContentSize() }
    }

    /// An empty last block's line is TextKit's extra line fragment, which
    /// no character carries: it, and the caret on it, take their look from
    /// the typing attributes, which are the last block's while the caret is
    /// there. Otherwise an empty document's line has the default font's
    /// height rather than the root's `line-height`, until something is
    /// typed.
    private func showLastBlockAtCaret() {
      guard let lastBlockAttributes,
        selectedRange == NSRange(location: textStorage.length, length: 0)
      else { return }
      let text = textStorage.string as NSString
      guard text.length == 0 || [0x0A, 0x2028].contains(text.character(at: text.length - 1)),
        !NSDictionary(dictionary: typingAttributes).isEqual(to: lastBlockAttributes)
      else { return }
      typingAttributes = lastBlockAttributes
      // TextKit keeps a line it already measured until it is laid out
      // again.
      if let layoutManager = textLayoutManager {
        layoutManager.enumerateTextLayoutFragments(
          from: layoutManager.documentRange.endLocation, options: [.reverse]
        ) { fragment in
          layoutManager.invalidateLayout(for: fragment.rangeInElement)
          return false
        }
      }
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

    /// Draws the controller's highlights again: `BlockBarsView` paints
    /// them behind the text, so the storage keeps the model's attributes.
    private func showHighlights() {
      setNeedsLayout()
    }

    /// The rects of the controller's highlights in content coordinates,
    /// with their colors.
    private func highlightRects(_ layoutManager: NSTextLayoutManager) -> [(CGRect, UIColor)] {
      var rects: [(CGRect, UIColor)] = []
      for (range, highlight) in controller.highlightedRanges() {
        guard let color = highlight.backgroundColor?.uiColor,
          let textRange = textRange(NSRange(location: range.location, length: range.length))
        else { continue }
        layoutManager.enumerateTextSegments(in: textRange, type: .highlight, options: []) {
          _, frame, _, _ in
          rects.append(
            (frame.offsetBy(dx: textContainerInset.left, dy: textContainerInset.top), color))
          return true
        }
      }
      return rects
    }

    /// Puts a `CheckboxControl` over each check box in `markers`, and
    /// removes those of items that no longer have one.
    private func layoutCheckboxes(_ markers: [(NodeKey, CGRect, TextKitListMarkerLayout)]) {
      var keep: Set<NodeKey> = []
      for (key, line, layout) in markers {
        guard case .checkbox = layout.marker else { continue }
        let (start, side) = layout.checkboxOffset(fontSize: layout.text.fontSize)
        let box = CGRect(
          x: layout.isRightToLeft ? line.maxX - start - side : line.minX + start,
          y: line.minY, width: side, height: line.height)
        let target = max(44, side)
        let control = checkboxes[key] ?? CheckboxControl()
        if checkboxes[key] == nil {
          control.onToggle = { [weak self] key in
            self?.perform { try self?.controller.toggleChecked(key) }
          }
          checkboxes[key] = control
          addSubview(control)
        }
        control.key = key
        control.frame = box.insetBy(dx: (side - target) / 2, dy: min(0, (line.height - target) / 2))
        keep.insert(key)
      }
      for (key, control) in checkboxes where !keep.contains(key) {
        control.removeFromSuperview()
        checkboxes[key] = nil
      }
    }

    /// Lays out the quote bars and list markers behind the text.
    public override func layoutSubviews() {
      super.layoutSubviews()
      // Text wraps differently at another width.
      if heightRange != nil, bounds.width != lastLaidOutWidth {
        lastLaidOutWidth = bounds.width
        invalidateIntrinsicContentSize()
      }
      // The bars are in content coordinates, so the view starts at the
      // content's origin even while the content is pulled past its top.
      blockBarsView.frame = CGRect(
        x: 0, y: 0, width: max(contentSize.width, bounds.maxX),
        height: max(contentSize.height, bounds.maxY))
      guard let layoutManager = textLayoutManager else { return }
      let document = controller.document
      let padding = textContainer.lineFragmentPadding
      var markers: [(NodeKey, CGRect, TextKitListMarkerLayout)] = []
      for (key, layout) in controller.listMarkers {
        guard let span = document.span(key),
          let range = textRange(
            NSRange(
              location: span.location,
              length: min(max(span.length, 1), document.length - span.location)))
        else { continue }
        var line: CGRect?
        layoutManager.enumerateTextSegments(in: range, type: .standard, options: []) {
          _, segment, _, _ in
          if let first = line, segment.minY >= first.maxY { return false }
          line = line.map { $0.union(segment) } ?? segment
          return true
        }
        guard let line else { continue }
        // Around the line's text as drawn: the marker is on the text's
        // baseline, which a tall inline decorator puts low in its line.
        let midY = layoutManager.firstLineTextMidY(at: range.location) ?? line.midY
        markers.append(
          (
            key,
            CGRect(
              x: textContainerInset.left + padding,
              y: textContainerInset.top + midY - line.height / 2,
              width: textContainer.size.width - 2 * padding, height: line.height),
            layout
          ))
      }
      blockBarsView.contentSizeCategory = traitCollection.preferredContentSizeCategory
      blockBarsView.markers = markers.map { ($0.1, $0.2) }
      layoutCheckboxes(markers)
      var bars: [(CGRect, UIColor)] = []
      for (key, blockBars) in controller.blockBars {
        guard let span = document.span(key), let frame = blockLinesFrame(layoutManager, span)
        else { continue }
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
      blockBarsView.boxes = controller.blockBoxes.compactMap { key, layout in
        guard let span = document.span(key), let frame = blockLinesFrame(layoutManager, span)
        else { return nil }
        return blockBox(
          layout, span: span, layoutManager,
          lines: CGRect(
            x: textContainerInset.left + padding, y: textContainerInset.top + frame.minY,
            width: textContainer.size.width - 2 * padding, height: frame.height))
      }
      blockBarsView.highlights = highlightRects(layoutManager)
      sendSubviewToBack(blockBarsView)
      layoutPlaceholder(layoutManager)
    }

    /// Puts the placeholder on the first line, centered in it as the
    /// line's own text is, and as wide as the lines (upstream's is a line
    /// of the root's text over the first).
    private func layoutPlaceholder(_ layoutManager: NSTextLayoutManager) {
      var line: CGRect?
      if showsPlaceholder, let range = textRange(NSRange(location: 0, length: 0)) {
        layoutManager.enumerateTextSegments(in: range, type: .standard, options: []) {
          _, segment, _, _ in
          line = segment
          return false
        }
      }
      guard let line else {
        placeholderLabel.isHidden = true
        return
      }
      let attributes = controller.theme.placeholderAttributes
      let font = attributes.uiFont(traitCollection.preferredContentSizeCategory)
      let height = font.ascender - font.descender
      // An empty document's line is TextKit's extra line fragment, which
      // has the font's own height here; text typed there gets the root's
      // `line-height`, which the placeholder is centered in as on the web.
      let lineHeight = max(
        line.height, (controller.theme["root"]?.lineHeightMultiple ?? 0) * font.pointSize)
      let padding = textContainer.lineFragmentPadding
      placeholderLabel.font = font
      placeholderLabel.textColor = attributes.foregroundColor.uiColor
      placeholderLabel.frame = CGRect(
        x: textContainerInset.left + padding,
        y: textContainerInset.top + line.minY + (lineHeight - height) / 2,
        width: textContainer.size.width - 2 * padding, height: ceil(height))
      placeholderLabel.isHidden = false
    }

    public override var accessibilityValue: String? {
      get { showsPlaceholder ? placeholder : super.accessibilityValue }
      set { super.accessibilityValue = newValue }
    }

    /// The lines of the block at `span`, in container coordinates. The
    /// line a block ends on holds none of its characters when the block is
    /// empty or ends with a line break: it is the next block's separator's
    /// line, or TextKit's extra line fragment at the end of the text. It is
    /// the block's line all the same, as a trailing `<br>` gives the web's
    /// block an empty line box, so the line at the block's end joins it.
    func blockLinesFrame(
      _ layoutManager: NSTextLayoutManager, _ span: TextKitSpan
    )
      -> CGRect?
    {
      let text = textStorage.string as NSString
      let end = min(span.end, text.length)
      func frame(_ range: NSRange) -> CGRect? {
        guard let range = textRange(range) else { return nil }
        var frame: CGRect?
        layoutManager.enumerateTextSegments(in: range, type: .standard, options: []) {
          _, segment, _, _ in
          frame = frame.map { $0.union(segment) } ?? segment
          return true
        }
        return frame
      }
      let lines = frame(NSRange(location: span.location, length: end - span.location))
      guard end == span.location || text.character(at: end - 1) == 0x2028,
        let last = frame(NSRange(location: end, length: 0))
      else { return lines }
      return lines.map { $0.union(last) } ?? last
    }

    /// `layout`'s box behind the block's lines, `lines` (starting where line
    /// fragments do), and its gutter with a number on the first line of each
    /// of the block's lines (the text after each line break).
    private func blockBox(
      _ layout: TextKitBlockBoxLayout, span: TextKitSpan, _ layoutManager: NSTextLayoutManager,
      lines: CGRect
    ) -> BlockBarsView.Box {
      let box = layout.box
      let isRTL = layout.isRightToLeft
      let frame = CGRect(
        x: isRTL ? lines.minX : lines.minX + box.offset, y: lines.minY - box.paddingTop,
        width: lines.width - box.offset, height: lines.height + box.paddingTop + box.paddingBottom)
      var drawing = BlockBarsView.Box(frame: frame, color: box.color.uiColor)
      guard let gutter = box.gutter else { return drawing }
      let gutterFrame = CGRect(
        x: isRTL ? frame.maxX - gutter.width : frame.minX, y: frame.minY, width: gutter.width,
        height: frame.height)
      drawing.gutter = (gutterFrame, gutter.backgroundColor.uiColor)
      drawing.border = (
        CGRect(
          x: isRTL ? gutterFrame.minX - gutter.borderWidth : gutterFrame.maxX, y: frame.minY,
          width: gutter.borderWidth, height: frame.height),
        gutter.borderColor.uiColor
      )
      let font = layout.text.uiFont(traitCollection.preferredContentSizeCategory)
      let attributes: [NSAttributedString.Key: Any] = [
        .font: font, .foregroundColor: gutter.foregroundColor.uiColor,
      ]
      let text = textStorage.string as NSString
      var start = span.location
      var number = 1
      while start <= span.end {
        let end = text.range(
          of: "\u{2028}", range: NSRange(location: start, length: span.end - start)
        ).location
        var line: CGRect?
        if let range = textRange(
          NSRange(location: start, length: min(1, max(0, text.length - start))))
        {
          layoutManager.enumerateTextSegments(in: range, type: .standard, options: []) {
            _, segment, _, _ in
            line = segment
            return false
          }
        }
        if let line {
          let string = NSAttributedString(string: "\(number)", attributes: attributes)
          let width = string.size().width
          let y = textContainerInset.top + line.midY - (font.ascender - font.descender) / 2
          let x =
            isRTL ? gutterFrame.minX + gutter.padding : gutterFrame.maxX - gutter.padding - width
          drawing.numbers.append((string, CGPoint(x: x, y: y)))
        }
        guard end != NSNotFound else { break }
        start = end + 1
        number += 1
      }
      return drawing
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
      if !unhandled.isEmpty {
        // A shifted key the text view handles (a shift-arrow) extends the
        // selection from the model's anchor.
        isExtendingSelection = unhandled.contains { $0.key?.modifierFlags.contains(.shift) == true }
        isPassingPresses = true
        defer {
          isExtendingSelection = false
          isPassingPresses = false
        }
        super.pressesBegan(unhandled, with: event)
      }
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
      // The software keyboard's Return has no key press. Safari sends a
      // `keydown` of Enter for it, which a menu open in the editor takes.
      if text == "\n", !isPassingPresses, controller.markedRange == nil {
        var handled = false
        perform {
          handled = try controller.keyDown(KeyboardEvent(key: "Enter", code: "Enter"))
        }
        if handled { return }
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

    /// The caret is as tall as the font before it, on its line's centered
    /// text, as a browser draws it: `UITextView` makes it as tall as the
    /// line, from where TextKit put the text.
    public override func caretRect(for position: UITextPosition) -> CGRect {
      let caret = super.caretRect(for: position)
      let offset = self.offset(from: beginningOfDocument, to: position)
      // The end of the text has no layout fragment of its own: the caret
      // there is on the last character's (or on the extra line TextKit
      // adds to it after a final newline).
      let length = textStorage.length
      guard let layoutManager = textLayoutManager,
        let location = textRange(
          NSRange(location: offset == length && length > 0 ? length - 1 : offset, length: 0))?
          .location,
        let fragment = layoutManager.textLayoutFragment(for: location)
      else { return caret }
      let frame = fragment.layoutFragmentFrame
      let top = textContainerInset.top + frame.minY
      // The line the caret is on, which `UITextView` put it beside.
      guard
        let line = fragment.textLineFragments.first(where: { line in
          let bounds = line.typographicBounds
          return top + bounds.minY <= caret.midY && caret.midY < top + bounds.maxY
        }) ?? fragment.textLineFragments.last
      else { return caret }
      // The text before the caret, or at the start of a line the text after.
      let string = textStorage.string as NSString
      let index = offset > 0 && string.character(at: offset - 1) != 0x0A ? offset - 1 : offset
      let font =
        index < string.length
        ? textStorage.attribute(.font, at: index, effectiveRange: nil) as? UIFont
        : typingAttributes[.font] as? UIFont
      let ascent = font?.ascender ?? line.fontExtents?.ascent ?? 0
      let descent = font.map { -$0.descender } ?? line.fontExtents?.descent ?? 0
      guard ascent + descent > 0 else { return caret }
      let textOffset =
        line.fontExtents == nil
        ? line.textOffset(ascent: ascent, descent: descent) : line.textOffset
      let baseline = top + line.typographicBounds.minY + line.glyphOrigin.y - textOffset
      return CGRect(
        x: caret.minX, y: baseline - ascent, width: caret.width, height: ascent + descent)
    }

    /// The selection covers only each line's text, as a browser
    /// highlights it: `UITextView`'s rects are whole line boxes, from the
    /// container's edge on each line the selection runs through.
    public override func selectionRects(for range: UITextRange) -> [UITextSelectionRect] {
      let rects = super.selectionRects(for: range)
      guard let layoutManager = textLayoutManager else { return rects }
      let origin = CGPoint(x: textContainerInset.left, y: textContainerInset.top)
      return rects.compactMap { rect in
        let segment = rect.rect.offsetBy(dx: -origin.x, dy: -origin.y)
        // A selected horizontal rule is outlined, without handles: a node
        // selection has no DOM range for Safari to put them on.
        if layoutManager.isHorizontalRule(at: CGPoint(x: segment.minX, y: segment.midY)) {
          return nil
        }
        let trimmed =
          layoutManager.selectionSegment(trimming: segment)
          // The start and the end keep a rect, for their handles.
          ?? ((rect.containsStart || rect.containsEnd)
            ? CGRect(x: segment.minX, y: segment.minY, width: 0, height: segment.height) : nil)
        guard let trimmed else { return nil }
        return TrimmedSelectionRect(rect, trimmed.offsetBy(dx: origin.x, dy: origin.y))
      }
    }

    /// The text view edited its own text: the edits replay as input.
    public func textViewDidChange(_ textView: UITextView) {
      if isRendering { return }
      perform {}
    }

    private func click(count: Int, at point: CGPoint) {
      let targetKey =
        point.y > textContainerInset.top
          + (textLayoutManager?.usageBoundsForTextContainer.maxY ?? 0)
        ? "root"
        : closestPosition(to: point).map {
          controller.nodeKey(at: offset(from: beginningOfDocument, to: $0))
        } ?? "root"
      perform {
        try controller.click(
          MouseEvent(
            "click", clientX: point.x, clientY: point.y, detail: count, targetKey: targetKey))
      }
    }

    public func textViewDidChangeSelection(_ textView: UITextView) {
      showLastBlockAtCaret()
      guard !isRendering else { return }
      let range = selectedRange
      if let current = try? controller.selectedRange(),
        current == TextKitRange(location: range.location, length: range.length)
      {
        return
      }
      perform {
        try controller.setSelectedRange(
          location: range.location, length: range.length, keepsAnchor: isExtendingSelection)
      }
    }

    // MARK: Focus

    /// Turns UIKit's caret, highlight and handles off for a node
    /// selection, and back on (while focused) for text.
    private func showSelectionDisplay() {
      guard
        let display = interactions.lazy.compactMap({ $0 as? UITextSelectionDisplayInteraction })
          .first
      else { return }
      if !showsTextSelection {
        if display.isActivated { display.isActivated = false }
      } else if isFirstResponder, !display.isActivated {
        display.isActivated = true
      }
    }

    public override func becomeFirstResponder() -> Bool {
      guard super.becomeFirstResponder() else { return false }
      showSelectionDisplay()
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

    /// The drag of the selected decorator `key`'s view (`view`): what
    /// `DRAGSTART_COMMAND`'s handlers put in the drag, as JS drags a
    /// `draggable` element; none leaves the decorator where it is.
    fileprivate func dragItems(forDecorator key: NodeKey, view: UIView) -> [UIDragItem] {
      guard editor.isEditable() else { return [] }
      let frame = convert(view.bounds, from: view)
      guard
        let data = controller.report({ try controller.dragStart(x: frame.midX, y: frame.midY) }),
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

  /// The tap target over a check list item's box (`BlockBarsView` draws
  /// the box): at least 44 points square, as the HIG asks of a control.
  final class CheckboxControl: UIControl {
    var key: NodeKey = ""
    var onToggle: ((NodeKey) -> Void)?

    override init(frame: CGRect) {
      super.init(frame: frame)
      addTarget(self, action: #selector(toggle), for: .touchUpInside)
      isAccessibilityElement = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    @objc private func toggle() { onToggle?(key) }
  }

  /// A selection rect of `UITextView`'s with another frame.
  private final class TrimmedSelectionRect: UITextSelectionRect {
    private let base: UITextSelectionRect
    private let frame: CGRect

    init(_ base: UITextSelectionRect, _ frame: CGRect) {
      self.base = base
      self.frame = frame
    }

    override var rect: CGRect { frame }
    override var writingDirection: NSWritingDirection { base.writingDirection }
    override var containsStart: Bool { base.containsStart }
    override var containsEnd: Bool { base.containsEnd }
    override var isVertical: Bool { base.isVertical }
  }

  /// Draws quote bars and list markers behind the text.
  final class BlockBarsView: UIView {
    /// A block's box, with its gutter, its border and the gutter's numbers.
    struct Box {
      var frame: CGRect
      var color: UIColor
      var gutter: (CGRect, UIColor)?
      var border: (CGRect, UIColor)?
      var numbers: [(NSAttributedString, CGPoint)] = []
    }

    var boxes: [Box] = [] {
      didSet { setNeedsDisplay() }
    }
    var bars: [(CGRect, UIColor)] = [] {
      didSet { setNeedsDisplay() }
    }
    /// Each list item's first line, with its marker.
    var markers: [(CGRect, TextKitListMarkerLayout)] = [] {
      didSet { setNeedsDisplay() }
    }
    /// The highlights' rects, over the boxes and bars.
    var highlights: [(CGRect, UIColor)] = [] {
      didSet { setNeedsDisplay() }
    }
    var contentSizeCategory = UIContentSizeCategory.large

    override func draw(_ rect: CGRect) {
      for box in boxes where box.frame.intersects(rect) {
        box.color.setFill()
        UIRectFill(box.frame)
        for (frame, color) in [box.gutter, box.border].compactMap({ $0 }) {
          color.setFill()
          UIRectFill(frame)
        }
        for (string, point) in box.numbers { string.draw(at: point) }
      }
      for (frame, color) in bars where frame.intersects(rect) {
        color.setFill()
        UIRectFill(frame)
      }
      for (line, layout) in markers where line.intersects(rect) {
        draw(layout, line: line)
      }
      for (frame, color) in highlights where frame.intersects(rect) {
        color.setFill()
        UIRectFillUsingBlendMode(frame, .normal)
      }
    }

    /// Where text in `font` sits in its first line `line`, centered on the
    /// text as `CenteredLineLayoutFragment` draws it.
    private func glyphBox(_ font: UIFont, _ line: CGRect) -> CGRect {
      let height = font.ascender - font.descender
      return CGRect(x: line.minX, y: line.midY - height / 2, width: line.width, height: height)
    }

    /// `layout`'s marker on the item's first line, `line`: a text marker
    /// in the item's font ending `TextKitListMarker.gap` before the text,
    /// or the playground theme's check box.
    private func draw(_ layout: TextKitListMarkerLayout, line: CGRect) {
      let isRTL = layout.isRightToLeft
      // The marker's font scales with Dynamic Type as the text's does; its
      // place does not, as the paragraph's indent does not.
      let font = layout.text.uiFont(contentSizeCategory)
      switch layout.marker {
      case .text(let marker):
        let string = NSAttributedString(
          string: marker,
          attributes: [.font: font, .foregroundColor: layout.text.foregroundColor.uiColor])
        let width = string.size().width
        let end = layout.headIndent - TextKitListMarker.gap
        let x = isRTL ? line.maxX - end : line.minX + end - width
        string.draw(at: CGPoint(x: x, y: glyphBox(font, line).minY))
      case .checkbox(let checked):
        let (start, side) = layout.checkboxOffset(fontSize: layout.text.fontSize)
        let box = CGRect(
          x: isRTL ? line.maxX - start - side : line.minX + start,
          y: glyphBox(font, line).midY - side / 2, width: side, height: side)
        if checked {
          ListMarkerColors.checked.setFill()
          UIBezierPath(roundedRect: box, cornerRadius: 2).fill()
          let mark = UIBezierPath()
          mark.move(to: CGPoint(x: box.minX + 0.27 * box.width, y: box.minY + 0.52 * box.height))
          mark.addLine(to: CGPoint(x: box.minX + 0.43 * box.width, y: box.minY + 0.68 * box.height))
          mark.addLine(to: CGPoint(x: box.minX + 0.74 * box.width, y: box.minY + 0.32 * box.height))
          mark.lineWidth = max(1.5, 0.12 * box.width)
          mark.lineCapStyle = .round
          mark.lineJoinStyle = .round
          UIColor.white.setStroke()
          mark.stroke()
        } else {
          ListMarkerColors.unchecked.setStroke()
          let border = UIBezierPath(roundedRect: box.insetBy(dx: 0.5, dy: 0.5), cornerRadius: 2)
          border.lineWidth = 1
          border.stroke()
        }
      }
    }
  }
#endif
