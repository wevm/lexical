import Lexical

// Highlights: ranges of the rendered text a view paints without changing
// the text storage, the way a web page paints CSS custom highlights
// (`::highlight()`) over the DOM. They are model ranges, so they follow the
// text through every commit.

/// A model range a highlight covers: from one point to another, in either
/// order.
public struct TextKitHighlightRange: Hashable, Sendable {
  public var anchorKey: NodeKey
  public var anchorOffset: Int
  public var anchorType: PointType
  public var focusKey: NodeKey
  public var focusOffset: Int
  public var focusType: PointType

  public init(
    anchorKey: NodeKey, anchorOffset: Int, anchorType: PointType = .text, focusKey: NodeKey,
    focusOffset: Int, focusType: PointType = .text
  ) {
    self.anchorKey = anchorKey
    self.anchorOffset = anchorOffset
    self.anchorType = anchorType
    self.focusKey = focusKey
    self.focusOffset = focusOffset
    self.focusType = focusType
  }

  /// The whole text content of the node `key`, as a range over its element
  /// (or text) points.
  public static func node(_ key: NodeKey, isText: Bool, length: Int) -> TextKitHighlightRange {
    TextKitHighlightRange(
      anchorKey: key, anchorOffset: 0, anchorType: isText ? .text : .element, focusKey: key,
      focusOffset: length, focusType: isText ? .text : .element)
  }
}

/// A set of ranges painted with one background color, behind the text.
/// Where highlights overlap, the one with the higher `priority` is painted
/// last, over the others.
public struct TextKitHighlight: Hashable, Sendable {
  public var ranges: [TextKitHighlightRange]
  public var backgroundColor: TextKitColor?
  public var priority: Int

  public init(
    ranges: [TextKitHighlightRange], backgroundColor: TextKitColor? = nil, priority: Int = 0
  ) {
    self.ranges = ranges
    self.backgroundColor = backgroundColor
    self.priority = priority
  }
}

extension TextKitInputController {
  /// The rendered ranges of `highlights` in `document`, lowest priority
  /// first, so a view that paints them in order shows the highest on top.
  /// Ranges whose points are not shown are left out.
  public func highlightedRanges() -> [(range: TextKitRange, highlight: TextKitHighlight)] {
    let document = self.document
    return highlights.sorted { ($0.value.priority, $0.key) < ($1.value.priority, $1.key) }
      .flatMap { _, highlight in
        highlight.ranges.compactMap { range -> (TextKitRange, TextKitHighlight)? in
          guard let a = document.location(range.anchorKey, range.anchorOffset, range.anchorType),
            let b = document.location(range.focusKey, range.focusOffset, range.focusType),
            a != b
          else { return nil }
          return (TextKitRange(location: min(a, b), length: abs(a - b)), highlight)
        }
      }
  }
}
