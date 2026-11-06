import Lexical
import Synchronization

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
  /// The theme type the run takes its look from in place of its node's
  /// (a `TextKitSuffix`'s), or `nil` for the node's own.
  public var type: String? = nil
}

/// Text shown after a text node that is not in the editor state, as JS
/// appends an unmanaged `<span>` to a text node's DOM (the playground's
/// autocomplete ghost). Its characters are in the rendered string, so the
/// text after it moves aside and it wraps, but every location in it maps
/// to the end of the node, and a caret skips it.
public struct TextKitSuffix: Equatable, Sendable {
  /// The text node it follows.
  public var key: NodeKey
  public var text: String
  /// The theme type it is styled as, over the node's look.
  public var type: String

  public init(key: NodeKey, text: String, type: String) {
    self.key = key
    self.text = text
    self.type = type
  }
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
    /// How many of a text node's last `length` code units are its
    /// suffix's.
    var suffixLength = 0
    /// The length of a text node's text, without its suffix.
    var contentLength: Int { length - suffixLength }
  }

  /// The rendered text.
  public internal(set) var string = ""
  /// The suffix shown after a text node, if any.
  public internal(set) var suffix: TextKitSuffix?
  /// Every rendered node, by key.
  var entries: [FastHashKey<NodeKey>: Entry] = [:]

  public init() {}

  /// The document of `root` (call it in a read or update).
  public init(root: RootNode) throws {
    try self.init(root: root, suffix: nil)
  }

  /// The document of `root` (call it in a read or update), with `suffix`
  /// after its text node.
  public init(root: RootNode, suffix: TextKitSuffix?) throws {
    self.suffix = suffix
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
      if entry.kind == .text { runs += textRuns(key, entry, at: location) }
    }
    return runs
  }

  /// The text nodes under the node at `key`, in document order, or none
  /// when it isn't shown. Walks only that subtree.
  func runs(under key: NodeKey) -> [TextKitRun] {
    guard let location = start(of: key) else { return [] }
    var runs: [TextKitRun] = []
    walk(from: key, at: location) { key, entry, location in
      if entry.kind == .text { runs += textRuns(key, entry, at: location) }
    }
    return runs
  }

  /// A text node's run, and its suffix's after it.
  private func textRuns(_ key: NodeKey, _ entry: Entry, at location: Int) -> [TextKitRun] {
    var runs = [
      TextKitRun(
        key: key, location: location, length: entry.contentLength, format: entry.format,
        style: entry.style)
    ]
    if entry.suffixLength > 0, let suffix, suffix.key == key {
      runs.append(
        TextKitRun(
          key: key, location: location + entry.contentLength, length: entry.suffixLength,
          format: entry.format, style: entry.style, type: suffix.type))
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
    if let element = node as? ElementNode { return try !isInline(element) }
    return try node is DecoratorNode && !node.isInline()
  }

  /// Whether the views lay out `element` inline: it is inline in the model,
  /// or its `createDOM` element is inline-level in HTML, which a browser
  /// lays out inline whatever the model says (OverflowNode's `<span>`).
  static func isInline(_ element: ElementNode) throws -> Bool {
    try element.isInline() || rendersInline(element)
  }

  /// Whether `element`'s class makes an inline-level element in `createDOM`,
  /// worked out once per class.
  private static func rendersInline(_ element: ElementNode) -> Bool {
    if element is RootNode { return false }
    let id = ObjectIdentifier(type(of: element))
    if let cached = inlineRenderingCache.withLock({ $0[id] }) { return cached }
    let tag = (try? element.createDOM(EditorConfig(namespace: "")))?.localName
    let inline = tag.map(inlineLevelTags.contains) ?? false
    inlineRenderingCache.withLock { $0[id] = inline }
    return inline
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
      let content = try node.getTextContent() + (suffix?.key == key ? suffix!.text : "")
      text += content
      var textEntry = Entry(
        kind: .text, parent: parent, index: index, length: content.utf16.count,
        format: try node.getFormat(), style: try node.getStyle())
      textEntry.suffixLength = suffix?.key == key ? suffix!.text.utf16.count : 0
      entry = textEntry
      runs += textRuns(key, entry, at: location)
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
      return start + min(offset, entry.contentLength)
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
        let entry = entries[kids[i].key]
        switch entry?.kind {
        case .text:
          // In a suffix, the end of the node's text; after it, the next
          // node, if one starts there.
          let offset = location - start
          if let entry, offset > entry.contentLength {
            if offset < entry.length
              || !(i + 1 < kids.count && base + kids[i + 1].start == location)
            {
              return (kids[i].key, entry.contentLength, .text)
            }
            break
          }
          return (kids[i].key, offset, .text)
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

  /// Replaces the suffix shown after the text node at `key` with `text`,
  /// and returns the edit to `string`, or none when the node isn't shown
  /// or the suffix is unchanged. The node's ancestors grow with it.
  mutating func replaceSuffix(of key: NodeKey, with text: String, type: String) -> [TextKitEdit] {
    guard let entry = entries[key], entry.kind == .text, let start = start(of: key) else {
      return []
    }
    let location = start + entry.contentLength
    let utf16 = string.utf16
    let lower = utf16.index(utf16.startIndex, offsetBy: location)
    let upper = utf16.index(lower, offsetBy: entry.suffixLength)
    guard !utf16[lower..<upper].elementsEqual(text.utf16) else { return [] }
    string.replaceSubrange(lower..<upper, with: text)
    let length = text.utf16.count
    let delta = length - entry.suffixLength
    entries[key]!.length += delta
    entries[key]!.suffixLength = length
    var child = key
    while let parent = entries[child]?.parent, let index = entries[child]?.index {
      for i in (index + 1)..<max(index + 1, entries[parent]!.kids.count) {
        entries[parent]!.kids[i].start += delta
      }
      entries[parent]!.length += delta
      child = parent
    }
    return [
      TextKitEdit(
        location: location, length: entry.suffixLength, string: text,
        runs: length == 0
          ? []
          : [
            TextKitRun(
              key: key, location: location, length: length, format: entry.format,
              style: entry.style, type: type)
          ])
    ]
  }

  /// Where the suffix is shown, if it is.
  private var suffixRange: TextKitRange? {
    guard let suffix, let entry = entries[suffix.key], entry.suffixLength > 0,
      let start = start(of: suffix.key)
    else { return nil }
    return TextKitRange(location: start + entry.contentLength, length: entry.suffixLength)
  }

  /// The location one `granularity` unit before or after `location`. A
  /// caret crosses a suffix in one step, as a browser crosses a
  /// `contenteditable=false` span, and a move that ends in it ends at its
  /// node's text end.
  public func move(
    _ location: Int, isBackward: Bool, granularity: SelectionGranularity
  ) -> Int {
    guard let suffix = suffixRange else {
      return moveInString(location, isBackward: isBackward, granularity: granularity)
    }
    let (start, end) = (suffix.location, suffix.end)
    if isBackward, location > start, location <= end { return start }
    if !isBackward, location == start {
      // To the node after the suffix, or past it when none starts there.
      if point(at: end) != point(at: start) { return end }
      return moveInString(end, isBackward: false, granularity: granularity)
    }
    let target = moveInString(location, isBackward: isBackward, granularity: granularity)
    if target > start, target < end || (target == end && point(at: end) == point(at: start)) {
      return start
    }
    return target
  }

  /// `move` over the rendered string alone.
  ///
  /// Walks the string from `location` only, so a move costs the length of
  /// what it crosses, not of the document.
  private func moveInString(
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

private let inlineRenderingCache = Mutex<[ObjectIdentifier: Bool]>([:])

/// HTML's inline-level (phrasing) elements that an element node's
/// `createDOM` may make. `code` is left out: CodeNode makes one, and the
/// playground's theme shows it as a block (`display: block`).
private let inlineLevelTags: Set<String> = [
  "a", "abbr", "b", "bdi", "bdo", "cite", "data", "dfn", "em", "i", "kbd", "mark", "q", "s",
  "samp", "small", "span", "strong", "sub", "sup", "time", "u", "var",
]
