import Lexical

// The text a TextKit view shows for an editor state, and the map between its
// UTF-16 locations and model points (Q11, Q12). `TextKitReconciler` keeps it
// up to date edit by edit; `init(root:)` renders a whole state. It holds no
// UI types, so it builds and tests on Linux.
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
  /// What a node renders as.
  enum Kind: Sendable {
    case text, lineBreak, decorator, element
  }

  /// One child of an element: where it starts, relative to the element, and
  /// whether it is a block (a block is set apart from its siblings by "\n").
  struct Kid: Sendable {
    var key: NodeKey
    var start: Int
    var isBlock: Bool
  }

  /// A rendered node. Children are located relative to their parent, so an
  /// edit moves only the later siblings of the edited node's ancestors.
  struct Entry: Sendable {
    var kind: Kind
    var parent: NodeKey?
    /// The index in the parent's `kids`.
    var index: Int
    var length: Int
    var format = 0
    var style = ""
    var kids: [Kid] = []
  }

  /// The rendered text.
  public internal(set) var string = ""
  /// Every rendered node, by key.
  var entries: [NodeKey: Entry] = [:]

  public init() {}

  /// The document of `root` (call it in a read or update).
  public init(root: RootNode) throws {
    var text = ""
    var runs: [TextKitRun] = []
    try build(root, parent: nil, index: 0, at: 0, into: &text, runs: &runs)
    string = text
  }

  /// The rendered text's length in UTF-16 code units.
  public var length: Int { entries["root"]?.length ?? 0 }

  /// The text nodes, in document order.
  public var runs: [TextKitRun] {
    var runs: [TextKitRun] = []
    walk { key, entry, location in
      if entry.kind == .text {
        runs.append(
          TextKitRun(
            key: key, location: location, length: entry.length, format: entry.format,
            style: entry.style))
      }
    }
    return runs
  }

  /// The text nodes under the node at `key`, in document order, or none
  /// when it isn't shown. Walks only that subtree.
  func runs(under key: NodeKey) -> [TextKitRun] {
    guard let location = start(of: key) else { return [] }
    var runs: [TextKitRun] = []
    walk(from: key, at: location) { key, entry, location in
      if entry.kind == .text {
        runs.append(
          TextKitRun(
            key: key, location: location, length: entry.length, format: entry.format,
            style: entry.style))
      }
    }
    return runs
  }

  /// Every node's span, its descendants and separators included.
  public var spans: [NodeKey: TextKitSpan] {
    var spans: [NodeKey: TextKitSpan] = [:]
    walk { key, entry, location in
      spans[key] = TextKitSpan(location: location, length: entry.length)
    }
    return spans
  }

  /// One node's span, found through its ancestors, or `nil` when it isn't
  /// shown.
  public func span(_ key: NodeKey) -> TextKitSpan? {
    guard let entry = entries[key], let location = start(of: key) else { return nil }
    return TextKitSpan(location: location, length: entry.length)
  }

  private func walk(
    from key: NodeKey = "root", at location: Int = 0, _ visit: (NodeKey, Entry, Int) -> Void
  ) {
    func visitNode(_ key: NodeKey, _ location: Int) {
      guard let entry = entries[key] else { return }
      visit(key, entry, location)
      for kid in entry.kids {
        visitNode(kid.key, location + kid.start)
      }
    }
    visitNode(key, location)
  }

  /// Where the node at `key` starts.
  func start(of key: NodeKey) -> Int? {
    guard var entry = entries[key] else { return nil }
    var key = key
    var location = 0
    while let parentKey = entry.parent {
      guard let parent = entries[parentKey], entry.index < parent.kids.count,
        parent.kids[entry.index].key == key
      else { return nil }
      location += parent.kids[entry.index].start
      key = parentKey
      entry = parent
    }
    return key == "root" ? location : nil
  }

  static func isBlock(_ node: LexicalNode) throws -> Bool {
    try (node is ElementNode || node is DecoratorNode) && !node.isInline()
  }

  /// Renders `node` at `location` into `text` and `runs`, recording it and
  /// its descendants; returns its length. A node rendered before (a move)
  /// forgets its old descendants first.
  @discardableResult
  mutating func build(
    _ node: LexicalNode, parent: NodeKey?, index: Int, at location: Int,
    into text: inout String, runs: inout [TextKitRun]
  ) throws -> Int {
    let key = node.getKey()
    if let old = entries[key]?.kids {
      for kid in old { removeSubtree(kid.key, parent: key) }
    }
    let entry: Entry
    if let node = node as? TextNode {
      let content = try node.getTextContent()
      let length = content.utf16.count
      text += content
      entry = Entry(
        kind: .text, parent: parent, index: index, length: length, format: try node.getFormat(),
        style: try node.getStyle())
      runs.append(
        TextKitRun(
          key: key, location: location, length: length, format: entry.format, style: entry.style))
    } else if node is LineBreakNode {
      text += "\u{2028}"
      entry = Entry(kind: .lineBreak, parent: parent, index: index, length: 1)
    } else if let node = node as? ElementNode {
      var kids: [Kid] = []
      var offset = 0
      var previousIsBlock = false
      for (i, child) in try node.getChildren().enumerated() {
        let isBlock = try Self.isBlock(child)
        if i > 0, previousIsBlock || isBlock {
          text += "\n"
          offset += 1
        }
        kids.append(Kid(key: child.getKey(), start: offset, isBlock: isBlock))
        offset += try build(
          child, parent: key, index: i, at: location + offset, into: &text, runs: &runs)
        previousIsBlock = isBlock
      }
      entry = Entry(kind: .element, parent: parent, index: index, length: offset, kids: kids)
    } else {
      text += "\u{FFFC}"
      entry = Entry(kind: .decorator, parent: parent, index: index, length: 1)
    }
    entries[key] = entry
    return entry.length
  }

  /// Forgets the node at `key` and its descendants, unless it has been
  /// rendered under another parent since (it moved).
  mutating func removeSubtree(_ key: NodeKey, parent: NodeKey) {
    guard let entry = entries[key], entry.parent == parent else { return }
    entries[key] = nil
    for kid in entry.kids {
      removeSubtree(kid.key, parent: key)
    }
  }

  /// The location of a model point, or `nil` when its node isn't shown.
  public func location(_ key: NodeKey, _ offset: Int, _ type: PointType) -> Int? {
    guard let entry = entries[key], let start = start(of: key) else { return nil }
    if type == .text {
      return start + min(offset, entry.length)
    }
    if offset < entry.kids.count {
      return start + entry.kids[offset].start
    }
    if let last = entry.kids.last, let lastEntry = entries[last.key] {
      return start + last.start + lastEntry.length
    }
    return start
  }

  public func location(for point: Point) -> Int? {
    location(point.key, point.offset, point.type)
  }

  /// The model point at `location`: in a text node where one contains it
  /// (the earlier one at a boundary), otherwise an element point.
  public func point(at location: Int) -> (NodeKey, Int, PointType) {
    let location = max(0, min(location, length))
    var element: NodeKey = "root"
    var base = 0
    while let entry = entries[element] {
      let kids = entry.kids
      func end(_ i: Int) -> Int { base + kids[i].start + (entries[kids[i].key]?.length ?? 0) }
      // The children that contain `location` are a run starting at the
      // first one that ends at or after it.
      var low = 0
      var high = kids.count
      while low < high {
        let mid = (low + high) / 2
        if end(mid) < location { low = mid + 1 } else { high = mid }
      }
      var child: (key: NodeKey, start: Int)?
      var i = low
      while i < kids.count, base + kids[i].start <= location {
        let start = base + kids[i].start
        switch entries[kids[i].key]?.kind {
        case .text: return (kids[i].key, location - start, .text)
        case .element where child == nil: child = (kids[i].key, start)
        default: break
        }
        i += 1
      }
      if let child {
        element = child.key
        base = child.start
        continue
      }
      low = 0
      high = kids.count
      while low < high {
        let mid = (low + high) / 2
        if base + kids[mid].start < location { low = mid + 1 } else { high = mid }
      }
      return (element, low, .element)
    }
    return (element, 0, .element)
  }

  /// The location one `granularity` unit before or after `location`.
  ///
  /// Walks the string from `location` only, so a move costs the length of
  /// what it crosses, not of the document.
  public func move(
    _ location: Int, isBackward: Bool, granularity: SelectionGranularity
  ) -> Int {
    let utf16 = string.utf16
    let start = utf16.startIndex
    let end = utf16.endIndex
    guard let at = utf16.index(start, offsetBy: location, limitedBy: end) else { return location }
    let isBreak: (UInt16) -> Bool = { $0 == 0x0A || $0 == 0x2028 }
    let offset = { utf16.distance(from: start, to: $0) }
    switch granularity {
    case .character:
      // Grapheme boundaries in UTF-16 offsets, within the line around
      // `location` (a line break is a grapheme of its own).
      var lineStart = at
      while lineStart > start, !isBreak(utf16[utf16.index(before: lineStart)]) {
        lineStart = utf16.index(before: lineStart)
      }
      var lineEnd = at
      while lineEnd < end, !isBreak(utf16[lineEnd]) { lineEnd = utf16.index(after: lineEnd) }
      let base = offset(lineStart)
      var boundaries: [Int] = []
      var position = base
      for character in Substring(utf16[lineStart..<lineEnd]) ?? "" {
        boundaries.append(position)
        position += character.utf16.count
      }
      boundaries.append(position)
      if isBackward {
        if let previous = boundaries.last(where: { $0 < location }) { return previous }
        return lineStart > start ? base - 1 : location
      }
      if let next = boundaries.first(where: { $0 > location }) { return next }
      return lineEnd < end ? offset(lineEnd) + 1 : location
    case .word:
      let isSpace: (UInt16) -> Bool = { $0 == 0x20 || $0 == 0x09 || isBreak($0) }
      var i = at
      if isBackward {
        while i > start, isSpace(utf16[utf16.index(before: i)]) { i = utf16.index(before: i) }
        while i > start, !isSpace(utf16[utf16.index(before: i)]) { i = utf16.index(before: i) }
      } else {
        while i < end, isSpace(utf16[i]) { i = utf16.index(after: i) }
        while i < end, !isSpace(utf16[i]) { i = utf16.index(after: i) }
      }
      return offset(i)
    case .line:
      // The same column on the previous or next line, clamped to its end.
      var lineStart = at
      while lineStart > start, !isBreak(utf16[utf16.index(before: lineStart)]) {
        lineStart = utf16.index(before: lineStart)
      }
      let column = utf16.distance(from: lineStart, to: at)
      if isBackward {
        guard lineStart > start else { return location }
        let prevEnd = utf16.index(before: lineStart)
        var prevStart = prevEnd
        while prevStart > start, !isBreak(utf16[utf16.index(before: prevStart)]) {
          prevStart = utf16.index(before: prevStart)
        }
        return offset(prevStart) + min(column, utf16.distance(from: prevStart, to: prevEnd))
      }
      var lineEnd = at
      while lineEnd < end, !isBreak(utf16[lineEnd]) { lineEnd = utf16.index(after: lineEnd) }
      guard lineEnd < end else { return location }
      let nextStart = utf16.index(after: lineEnd)
      var nextEnd = nextStart
      while nextEnd < end, !isBreak(utf16[nextEnd]) { nextEnd = utf16.index(after: nextEnd) }
      return offset(nextStart) + min(column, utf16.distance(from: nextStart, to: nextEnd))
    case .lineboundary:
      var i = at
      if isBackward {
        while i > start, !isBreak(utf16[utf16.index(before: i)]) { i = utf16.index(before: i) }
      } else {
        while i < end, !isBreak(utf16[i]) { i = utf16.index(after: i) }
      }
      return offset(i)
    }
  }
}

/// `Editor.selectionMovement` over the rendered text: characters are
/// graphemes, words are separated by spaces and line breaks, and a line
/// boundary is a paragraph or line break edge. The views replace the line
/// step with visual lines when they lay text out.
public final class TextKitMovement: SelectionMovement {
  /// A rendering of the editor's committed state, kept up to date by the
  /// controller's reconciler; `nil` when there is none.
  var committedDocument: (() -> TextKitDocument?)?

  public init() {}

  public func landedPoint(
    from point: Point, isBackward: Bool, granularity: SelectionGranularity
  ) throws -> (NodeKey, Int, PointType)? {
    // Until the update changes a node, the pending state renders as the
    // committed one, so a keystroke's move needs no render of its own.
    let editor = try getActiveEditor()
    let isClean =
      editor._dirtyType == NO_DIRTY_NODES && editor._dirtyLeaves.size == 0
      && editor._dirtyElements.size == 0
    let document =
      try (isClean ? committedDocument?() : nil) ?? TextKitDocument(root: getRoot())
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
