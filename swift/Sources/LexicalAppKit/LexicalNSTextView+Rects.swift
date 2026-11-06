#if os(macOS)
  import AppKit
  import Lexical

  // The rects of model ranges, which floating UI (typeahead menus, toolbars)
  // positions itself at, as JS reads `range.getBoundingClientRect()`.

  extension LexicalNSTextView {
    /// The bounding rect, in this view's coordinates, of the rendered text
    /// from `start` to `end`, or `nil` when either point isn't shown or
    /// the view has no layout. A collapsed range is the caret's line box.
    public func rect(from start: Point, to end: Point) -> CGRect? {
      let document = controller.document
      guard let a = document.location(for: start), let b = document.location(for: end),
        let layoutManager = textLayoutManager, let contentManager = layoutManager.textContentManager
      else { return nil }
      let range = NSRange(location: min(a, b), length: abs(b - a))
      guard
        let startLocation = contentManager.location(
          contentManager.documentRange.location, offsetBy: range.location),
        let endLocation = contentManager.location(startLocation, offsetBy: range.length),
        let textRange = NSTextRange(location: startLocation, end: endLocation)
      else { return nil }
      layoutManager.ensureLayout(for: textRange)
      var union: CGRect?
      layoutManager.enumerateTextSegments(
        in: textRange, type: range.length == 0 ? .standard : .selection, options: []
      ) { _, frame, _, _ in
        union = union.map { $0.union(frame) } ?? frame
        return true
      }
      guard let union else { return nil }
      let origin = textContainerOrigin
      return union.offsetBy(dx: origin.x, dy: origin.y)
    }

    /// The rect, in this view's coordinates, of the rendered block `key`:
    /// its box (the background and padding of a code block) where it has
    /// one, otherwise its lines across the text's width, as JS reads the
    /// block element's `getBoundingClientRect()`. `nil` when the block isn't
    /// shown or the view has no layout.
    public func blockRect(for key: NodeKey) -> CGRect? {
      guard let layoutManager = textLayoutManager, let span = controller.document.span(key),
        let frame = blockLinesFrame(layoutManager, span)
      else { return nil }
      let origin = textContainerOrigin
      let padding = textContainer?.lineFragmentPadding ?? 0
      let lines = CGRect(
        x: origin.x + padding, y: origin.y + frame.minY,
        width: (textContainer?.size.width ?? bounds.width) - 2 * padding, height: frame.height)
      guard let layout = controller.blockBoxes[key] else { return lines }
      let box = layout.box
      return CGRect(
        x: layout.isRightToLeft ? lines.minX : lines.minX + box.offset,
        y: lines.minY - box.paddingTop, width: lines.width - box.offset,
        height: lines.height + box.paddingTop + box.paddingBottom)
    }

    /// `rect(from:to:)` in screen coordinates, or `nil` while the view is
    /// not in a window.
    public func screenRect(from start: Point, to end: Point) -> CGRect? {
      guard let window, let rect = rect(from: start, to: end) else { return nil }
      return window.convertToScreen(convert(rect, to: nil))
    }
  }
#endif
