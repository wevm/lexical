#if canImport(UIKit)
  import Lexical
  import UIKit

  // The rects of model ranges, which floating UI (typeahead menus, toolbars)
  // positions itself at, as JS reads `range.getBoundingClientRect()`.

  extension LexicalUITextView {
    /// The bounding rect, in this view's coordinates, of the rendered text
    /// from `start` to `end`, or `nil` when either point isn't shown. A
    /// collapsed range is the caret's rect.
    public func rect(from start: Point, to end: Point) -> CGRect? {
      let document = controller.document
      guard let a = document.location(for: start), let b = document.location(for: end),
        let startPosition = position(from: beginningOfDocument, offset: min(a, b)),
        let endPosition = position(from: beginningOfDocument, offset: max(a, b)),
        let range = textRange(from: startPosition, to: endPosition)
      else { return nil }
      if a == b {
        let caret = caretRect(for: startPosition)
        return caret.isNull || caret.isInfinite ? nil : caret
      }
      let rects = selectionRects(for: range).map(\.rect).filter { !$0.isNull && !$0.isEmpty }
      guard let first = rects.first else { return nil }
      return rects.dropFirst().reduce(first) { $0.union($1) }
    }

    /// The rect, in this view's coordinates, of the rendered block `key`:
    /// its box (the background and padding of a code block) where it has
    /// one, otherwise its lines across the text's width, as JS reads the
    /// block element's `getBoundingClientRect()`. `nil` when the block isn't
    /// shown.
    public func blockRect(for key: NodeKey) -> CGRect? {
      guard let layoutManager = textLayoutManager, let span = controller.document.span(key),
        let frame = blockLinesFrame(layoutManager, span)
      else { return nil }
      let padding = textContainer.lineFragmentPadding
      let lines = CGRect(
        x: textContainerInset.left + padding, y: textContainerInset.top + frame.minY,
        width: textContainer.size.width - 2 * padding, height: frame.height)
      guard let layout = controller.blockBoxes[key] else { return lines }
      let box = layout.box
      return CGRect(
        x: layout.isRightToLeft ? lines.minX : lines.minX + box.offset,
        y: lines.minY - box.paddingTop, width: lines.width - box.offset,
        height: lines.height + box.paddingTop + box.paddingBottom)
    }

    /// `rect(from:to:)` in the coordinates of the screen the view is on,
    /// or `nil` while it is not in a window.
    public func screenRect(from start: Point, to end: Point) -> CGRect? {
      guard let window, let rect = rect(from: start, to: end) else { return nil }
      return window.convert(convert(rect, to: window), to: window.screen.coordinateSpace)
    }
  }
#endif
