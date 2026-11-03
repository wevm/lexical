/// A rectangle as `DOMRect` describes it: the edges and the size.
public struct SelectionRect: Equatable, Sendable {
  public var left: Double
  public var top: Double
  public var width: Double
  public var height: Double
  public var right: Double
  public var bottom: Double

  public init(left: Double, top: Double, width: Double, height: Double) {
    self.init(
      left: left, top: top, right: left + width, bottom: top + height, width: width, height: height)
  }

  /// A rect whose edges and size are given separately, as a measured
  /// `DOMRect`'s may disagree by rounding.
  public init(
    left: Double, top: Double, right: Double, bottom: Double, width: Double, height: Double
  ) {
    self.left = left
    self.top = top
    self.right = right
    self.bottom = bottom
    self.width = width
    self.height = height
  }
}

/// The filtering half of `createRectsFromDOMRange`: sorts a range's client
/// rects from top left to bottom right (rows within 3 points), and drops each
/// rect contained in the one kept before it and each that spans the root.
/// The views pass their TextKit selection rects, the root's width and its
/// horizontal padding.
public func filterSelectionRects(
  _ rects: [SelectionRect], rootWidth: Double, rootPadding: Double
) -> [SelectionRect] {
  var selectionRects = rects.enumerated().sorted { a, b in
    let top = a.element.top - b.element.top
    if abs(top) <= 3 {
      return a.element.left != b.element.left
        ? a.element.left < b.element.left : a.offset < b.offset
    }
    return top < 0 || (top == 0 && a.offset < b.offset)
  }.map(\.element)
  var prevRect: SelectionRect?
  var i = 0
  while i < selectionRects.count {
    let selectionRect = selectionRects[i]
    let isContainedRect =
      prevRect.map {
        $0.top <= selectionRect.top && $0.bottom >= selectionRect.bottom
          && $0.left <= selectionRect.left && $0.right >= selectionRect.right
      } ?? false
    let selectionSpansElement = selectionRect.width + rootPadding == rootWidth
    if isContainedRect || selectionSpansElement {
      selectionRects.remove(at: i)
      continue
    }
    prevRect = selectionRect
    i += 1
  }
  return selectionRects
}
