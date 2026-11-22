#if os(macOS)
  import AppKit
  import Lexical
  import LexicalSelection

  // The rects of model ranges, which floating UI (typeahead menus, toolbars)
  // positions itself at, as JS reads `range.getBoundingClientRect()`.

  extension LexicalNSTextView {
    /// The bounding rect, in this view's coordinates, of the rendered text
    /// from `start` to `end`, or `nil` when either point isn't shown or
    /// the view has no layout. A collapsed range is the caret's line box.
    public func rect(from start: Point, to end: Point) -> CGRect? {
      let document = controller.document
      guard let a = document.location(for: start), let b = document.location(for: end) else {
        return nil
      }
      return rect(for: NSRange(location: min(a, b), length: abs(b - a)))
    }

    /// The bounding rect, in this view's coordinates, of the rendered text
    /// in `range`, or the caret's rect for an empty one.
    func rect(for range: NSRange) -> CGRect? {
      guard let layoutManager = textLayoutManager,
        let contentManager = layoutManager.textContentManager,
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
  /// The rects `@lexical/table`'s arrow keys measure (JS:
  /// `getBoundingClientRect`), from whichever of the editor's views shows
  /// the node, the root's or a table cell's, in this view's coordinates.
  extension LexicalNSTextView: @MainActor SelectionLayout {
    public func selectionRect(_ selection: RangeSelection) -> SelectionRect? {
      guard let view = editorView(showing: selection.focus.key),
        let rect = view.rect(from: selection.focus, to: selection.focus)
      else { return nil }
      return SelectionRect(convert(rect, from: view))
    }

    public func nodeRect(_ key: NodeKey) -> SelectionRect? {
      guard let view = editorView(showing: key), let rect = view.blockRect(for: key) else {
        return nil
      }
      return SelectionRect(convert(rect, from: view))
    }
  }

  extension SelectionRect {
    init(_ rect: CGRect) {
      self.init(
        left: rect.minX, top: rect.minY, width: rect.width, height: rect.height)
    }
  }
#endif
