import Lexical

// The text a TextKit view shows for an editor state, and the map between its
// UTF-16 locations and model points. This is the thin M1 slice of the
// reconciler (Q11, Q12): it renders the whole document each time, which the
// incremental reconciler replaces later. It holds no UI types, so it builds
// and tests on Linux.
//
// Blocks are separated by "\n", a LineBreakNode is U+2028 (a line break
// within a paragraph, as TextKit reads it), a decorator is U+FFFC, and text
// is its text content.

/// One node's span of the rendered string.
public struct TextKitSpan: Equatable, Sendable {
  public let location: Int
  public let length: Int
  public var end: Int { location + length }
}

/// One text node's run: where it is and the formats it shows.
public struct TextKitRun: Equatable, Sendable {
  public let key: NodeKey
  public let location: Int
  public let length: Int
  public let format: Int
  public let style: String
}

public struct TextKitDocument: Sendable {
  /// The rendered text.
  public private(set) var string = ""
  /// The text nodes, in document order.
  public private(set) var runs: [TextKitRun] = []
  /// Every node's span, its descendants and separators included.
  public private(set) var spans: [NodeKey: TextKitSpan] = [:]
  private var children: [NodeKey: [NodeKey]] = [:]
  private var textKeys: Set<NodeKey> = []
  private var length = 0

  public init() {}

  /// The document of `root` (call it in a read or update).
  public init(root: RootNode) throws {
    try append(root)
  }

  private mutating func add(_ text: String) {
    string += text
    length += text.utf16.count
  }

  private mutating func append(_ node: LexicalNode) throws {
    let key = node.getKey()
    let start = length
    if let text = node as? TextNode {
      let content = try text.getTextContent()
      add(content)
      textKeys.insert(key)
      runs.append(
        TextKitRun(
          key: key, location: start, length: content.utf16.count, format: try text.getFormat(),
          style: try text.getStyle()))
    } else if node is LineBreakNode {
      add("\u{2028}")
    } else if let element = node as? ElementNode {
      let kids = try element.getChildren()
      children[key] = kids.map { $0.getKey() }
      var previous: LexicalNode?
      for child in kids {
        if let previous, try isBlock(previous) || isBlock(child) {
          add("\n")
        }
        try append(child)
        previous = child
      }
    } else {
      add("\u{FFFC}")
    }
    spans[key] = TextKitSpan(location: start, length: length - start)
  }

  private func isBlock(_ node: LexicalNode) throws -> Bool {
    try (node is ElementNode || node is DecoratorNode) && !node.isInline()
  }

  /// The location of a model point, or `nil` when its node isn't shown.
  public func location(_ key: NodeKey, _ offset: Int, _ type: PointType) -> Int? {
    guard let span = spans[key] else { return nil }
    if type == .text {
      return span.location + min(offset, span.length)
    }
    let kids = children[key] ?? []
    if offset < kids.count {
      return spans[kids[offset]]?.location
    }
    return kids.last.flatMap { spans[$0]?.end } ?? span.location
  }

  public func location(for point: Point) -> Int? {
    location(point.key, point.offset, point.type)
  }

  /// The model point at `location`: in a text node where one contains it
  /// (the earlier one at a boundary), otherwise an element point.
  public func point(at location: Int) -> (NodeKey, Int, PointType) {
    let location = max(0, min(location, length))
    var element: NodeKey = "root"
    while true {
      let kids = children[element] ?? []
      let containing = kids.filter { key in
        spans[key].map { $0.location <= location && location <= $0.end } ?? false
      }
      if let text = containing.first(where: textKeys.contains) {
        return (text, location - spans[text]!.location, .text)
      }
      if let child = containing.first(where: { children[$0] != nil }) {
        element = child
        continue
      }
      let index =
        kids.firstIndex { (spans[$0]?.location ?? 0) >= location } ?? kids.count
      return (element, index, .element)
    }
  }

  /// The location one `granularity` unit before or after `location`.
  public func move(
    _ location: Int, isBackward: Bool, granularity: SelectionGranularity
  ) -> Int {
    let units = Array(string.utf16)
    let isBreak: (UInt16) -> Bool = { $0 == 0x0A || $0 == 0x2028 }
    switch granularity {
    case .character:
      // Grapheme boundaries in UTF-16 offsets.
      var boundaries: [Int] = []
      var offset = 0
      for character in string {
        boundaries.append(offset)
        offset += character.utf16.count
      }
      boundaries.append(offset)
      return isBackward
        ? boundaries.last { $0 < location } ?? location
        : boundaries.first { $0 > location } ?? location
    case .word:
      let isSpace: (UInt16) -> Bool = { $0 == 0x20 || $0 == 0x09 || isBreak($0) }
      var i = location
      if isBackward {
        while i > 0, isSpace(units[i - 1]) { i -= 1 }
        while i > 0, !isSpace(units[i - 1]) { i -= 1 }
      } else {
        while i < units.count, isSpace(units[i]) { i += 1 }
        while i < units.count, !isSpace(units[i]) { i += 1 }
      }
      return i
    case .lineboundary:
      var i = location
      if isBackward {
        while i > 0, !isBreak(units[i - 1]) { i -= 1 }
      } else {
        while i < units.count, !isBreak(units[i]) { i += 1 }
      }
      return i
    }
  }
}

/// `Editor.selectionMovement` over the rendered text: characters are
/// graphemes, words are separated by spaces and line breaks, and a line
/// boundary is a paragraph or line break edge. The views replace the line
/// step with visual lines when they lay text out.
public final class TextKitMovement: SelectionMovement {
  public init() {}

  public func landedPoint(
    from point: Point, isBackward: Bool, granularity: SelectionGranularity
  ) throws -> (NodeKey, Int, PointType)? {
    let document = try TextKitDocument(root: getRoot())
    guard let location = document.location(for: point) else { return nil }
    let target = document.move(location, isBackward: isBackward, granularity: granularity)
    return target == location ? nil : document.point(at: target)
  }
}

/// Sets a range selection from two `(key, offset, type)` points (call it in
/// an update).
public func selectPoints(
  _ anchor: (NodeKey, Int, PointType), _ focus: (NodeKey, Int, PointType)
) throws {
  let selection = createRangeSelection()
  try selection.anchor.set(anchor.0, anchor.1, anchor.2)
  try selection.focus.set(focus.0, focus.1, focus.2)
  try setSelection(selection)
}
