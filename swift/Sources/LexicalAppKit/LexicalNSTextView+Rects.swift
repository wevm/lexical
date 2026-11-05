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

    /// `rect(from:to:)` in screen coordinates, or `nil` while the view is
    /// not in a window.
    public func screenRect(from start: Point, to end: Point) -> CGRect? {
      guard let window, let rect = rect(from: start, to: end) else { return nil }
      return window.convertToScreen(convert(rect, to: nil))
    }
  }
#endif
