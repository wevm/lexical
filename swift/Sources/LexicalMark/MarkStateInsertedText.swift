import Lexical

// Port of the inserted-text rule of packages/lexical-mark/src/
// MarkStateExtension.ts (`$applyInsertedTextMarkIDs` and its helpers).
// Texts are compared as UTF-16 code units, as JS strings are.

/// A character of a markable TextNode (a code point, so that a surrogate
/// pair is never split), or a run of characters of one TextNode that both
/// states share at the start or the end of the compared text.
let CHAR = 0
/// A markable leaf that is not a TextNode (a LineBreakNode, a decorator).
let LEAF = 1
/// The boundary between two blocks.
let BREAK = 2
/// Anything else: a nested block, a leaf that cannot carry ids.
let OPAQUE = 3

/// `PointLike`: a point to locate in the flattened units.
package struct PointLike {
  let key: NodeKey
  let offset: Int
  let type: PointType
}

/// The part of a block that is flattened: its children from `from` to
/// `to`, both included, or from its first child or to its last one when
/// `nil`.
private struct BlockWindow {
  let block: ElementNode
  var from: LexicalNode?
  var to: LexicalNode?
}

/// A markable TextNode with its text, or one other unit.
private struct Segment {
  /// How the unit compares, for a unit that is not a text.
  let code: String
  let ids: [String]
  let kind: Int
  let node: LexicalNode
  /// The text of a TextNode, otherwise empty.
  let text: [UInt16]
}

private final class Segments {
  var length = 0
  /// The position of each point that was asked for, -1 when it is not there.
  var points: [Int]
  var segs: [Segment] = []
  /// The texts, with U+FFFF for each other unit.
  var text: [UInt16] = []
  /// The code of each other unit, by position.
  var codes: [Int: String] = [:]

  init(points: Int) {
    self.points = Array(repeating: -1, count: points)
  }
}

/// How a unit compares: JS compares the strings `code`, which never
/// collide across these cases.
private enum UnitCode: Hashable {
  /// A character, by its code units.
  case text([UInt16])
  /// A unit that is not a text (`segment.code`).
  case other(String)
  /// A run of the shared start, at a position.
  case prefix(Int)
  /// A run of the shared end, at a distance from the end.
  case suffix(Int)
}

/// Units that the text inserted into them is found with.
private struct Flattened {
  var codes: [UnitCode] = []
  var kinds: [Int] = []
  var ids: [[String]] = []
  var nodes: [LexicalNode] = []
  /// The offset of the unit in its node.
  var offsets: [Int] = []
  /// The index of each point that was asked for, -1 when it is not there.
  var points: [Int] = []
}

private func toPointLike(_ point: Point) -> PointLike {
  PointLike(key: point.key, offset: point.offset, type: point.type)
}

/// `$getReplacedRange(selection)`: the start and end of a selection that is
/// not collapsed, otherwise `nil`.
func getReplacedRange(_ selection: (any BaseSelection)?) throws -> (PointLike, PointLike)? {
  guard let selection = selection as? RangeSelection, !selection.isCollapsed() else {
    return nil
  }
  let (start, end) =
    try selection.isBackward()
    ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
  return (toPointLike(start), toPointLike(end))
}

/// The text a composition puts in place of the selection it replaces until
/// the input method writes: nothing, or the core's placeholder.
private let COMPOSITION_PLACEHOLDERS: [[UInt16]] = [[], [0x200b], [0x00a0]]

/// How `childrenText` writes a child that is not text: one unit.
private let NON_TEXT_UNIT: UInt16 = 0xfffc

/// `$childrenText(block)`: the text of `block`'s children as
/// `offsetInBlock` counts it.
private func childrenText(_ block: ElementNode) throws -> [UInt16] {
  var text: [UInt16] = []
  for child in try block.getChildren() {
    if let element = child as? ElementNode {
      text += try element.isInline() ? childrenText(element) : Array(element.getTextContent().utf16)
    } else if let textNode = child as? TextNode {
      text += try textNode.getTextContent().utf16
    } else {
      text.append(NON_TEXT_UNIT)
    }
  }
  return text
}

/// `$unitSize(node)`: the size of a leaf in `childrenText`.
private func unitSize(_ node: LexicalNode) throws -> Int {
  if let text = node as? TextNode {
    return try text.getTextContentSize()
  }
  return 1
}

/// `$offsetInBlock(block, point)`: the offset of `point` in the text of
/// `block`'s children, or -1 when `block` holds a nested block before it or
/// does not hold the point.
private func offsetInBlock(_ block: ElementNode, _ point: PointLike) throws -> Int {
  var offset = 0
  func add(_ node: LexicalNode) throws -> Bool {
    if let element = node as? ElementNode {
      if try !element.isInline() {
        return false
      }
      for child in try element.getChildren() {
        if try !add(child) {
          return false
        }
      }
      return true
    }
    offset += try unitSize(node)
    return true
  }
  func walk(_ element: ElementNode) throws -> Bool {
    if point.type == .element && point.key == element.getKey() {
      let children = try element.getChildren()
      var i = 0
      while i < point.offset && i < children.count {
        if try !add(children[i]) {
          return false
        }
        i += 1
      }
      return true
    }
    var child = try element.getFirstChild()
    while let current = child {
      if current is TextNode && current.getKey() == point.key {
        offset += point.offset
        return true
      }
      if let inner = current as? ElementNode {
        if try !inner.isInline() {
          offset = -1
          return true
        }
        if try walk(inner) {
          return true
        }
      } else {
        offset += try unitSize(current)
      }
      child = try current.getNextSibling()
    }
    return false
  }
  return try walk(block) ? offset : -1
}

/// `$getCompositionReplacedRange(editor, base, started)`: the selection a
/// composition started with (in `base`), when the update that started it
/// (`started`) replaced it, otherwise `nil`.
func getCompositionReplacedRange(
  _ editor: Editor, _ base: EditorState, _ started: EditorState
) throws -> (PointLike, PointLike)? {
  struct Before {
    let block: NodeKey
    let end: Int
    let range: (PointLike, PointLike)
    let start: Int
    let text: [UInt16]
  }
  let before: Before? = try base.read(editor: editor) {
    guard let range = try getReplacedRange(getSelection()) else { return nil }
    let startNode = try getNodeByKey(range.0.key)
    let endNode = try getNodeByKey(range.1.key)
    let block = try startNode.flatMap { try getBlock($0) }
    guard let block, let endNode, try block.is(getBlock(endNode)) else { return nil }
    let start = try offsetInBlock(block, range.0)
    let end = try offsetInBlock(block, range.1)
    if start < 0 || end < start {
      return nil
    }
    return Before(
      block: block.getKey(), end: end, range: range, start: start, text: try childrenText(block))
  }
  guard let before else { return nil }
  return try started.read(editor: editor) {
    guard let selection = try getSelection() as? RangeSelection,
      let block = try getNodeByKey(before.block) as? ElementNode, try block.isAttached()
    else {
      return nil
    }
    let anchor = try offsetInBlock(block, toPointLike(selection.anchor))
    let focus = try offsetInBlock(block, toPointLike(selection.focus))
    let text = try childrenText(block)
    let head = Array(before.text[0..<before.start])
    let tail = Array(before.text[before.end...])
    let selected = Array(before.text[before.start..<before.end])
    for placeholder in COMPOSITION_PLACEHOLDERS {
      // A selection that is the placeholder itself, kept by a start that
      // wrote nothing, would look replaced: leave it to the comparison
      if !placeholder.isEmpty && placeholder == selected {
        continue
      }
      let end = before.start + placeholder.count
      if text == head + placeholder + tail && max(anchor, focus) == end
        && (min(anchor, focus) == end || min(anchor, focus) == before.start)
      {
        return before.range
      }
    }
    return nil
  }
}

private func collectSegments(
  _ flavor: MarkFlavor, _ windows: [BlockWindow], _ points: [PointLike],
  _ isUnchangedBlock: (ElementNode) -> Bool
) throws -> Segments {
  let out = Segments(points: points.count)
  // The first segment of the current window, and whether the units that
  // follow need a boundary from a nested block before them
  var windowStart = 0
  var afterNested = false
  func add(_ node: LexicalNode, _ kind: Int, _ code: String, _ text: [UInt16], _ ids: [String]) {
    if afterNested {
      afterNested = false
      if kind != BREAK {
        separate(node)
      }
    }
    out.segs.append(Segment(code: code, ids: ids, kind: kind, node: node, text: text))
    var text = text
    if kind != CHAR {
      out.codes[out.length] = code
      text = [0xffff]
    }
    out.text += text
    out.length += text.count
  }
  // A boundary between blocks, where units of the window precede it
  func separate(_ node: LexicalNode) {
    if out.segs.count > windowStart, let last = out.segs.last, last.kind != BREAK {
      add(node, BREAK, "\u{0}", [], NO_MARK_IDS)
    }
  }
  func locateElementPoints(_ key: NodeKey, _ index: Int) {
    for (i, point) in points.enumerated()
    where point.type == .element && point.key == key && point.offset == index {
      out.points[i] = out.length
    }
  }
  func visitChildren(_ element: ElementNode, _ from: LexicalNode?, _ to: LexicalNode?) throws {
    let key = element.getKey()
    // The index of the child is only needed for an element point in it
    var index =
      points.contains { $0.type == .element && $0.key == key }
      ? (try from?.getIndexWithinParent() ?? 0) : -1
    var child = try from ?? element.getFirstChild()
    while let current = child {
      if index >= 0 {
        locateElementPoints(key, index)
        index += 1
      }
      try visitNode(current)
      if let to, current.is(to) {
        break
      }
      child = try current.getNextSibling()
    }
    if index >= 0 {
      locateElementPoints(key, index)
    }
  }
  func visitNode(_ node: LexicalNode) throws {
    let key = node.getKey()
    let markable = try isMarkableNode(flavor, node)
    if let textNode = node as? TextNode, markable {
      let text = Array(try textNode.getTextContent().utf16)
      for (i, point) in points.enumerated()
      where point.type == .text && point.key == key && point.offset <= text.count {
        out.points[i] = out.length + point.offset
      }
      add(node, CHAR, "", text, try readNodeMarkIDs(flavor, node))
    } else if markable {
      add(node, LEAF, "\u{2}" + key, [], try readNodeMarkIDs(flavor, node))
    } else if let element = node as? ElementNode, try element.isInline() {
      try visitChildren(element, nil, nil)
    } else if let element = node as? ElementNode, !isUnchangedBlock(element) {
      // A nested block that the update changed (the items of a list, a list
      // in a list item): its text is compared like the text of the blocks
      // around it. An unchanged one is a unit of its own.
      separate(node)
      try visitChildren(element, nil, nil)
      afterNested = true
    } else {
      add(node, OPAQUE, "\u{1}" + key, [], NO_MARK_IDS)
    }
  }
  for (i, window) in windows.enumerated() {
    afterNested = false
    if i > 0 {
      add(window.block, BREAK, "\u{0}", [], NO_MARK_IDS)
    }
    windowStart = out.segs.count
    try visitChildren(window.block, window.from, window.to)
  }
  return out
}

private func isHighSurrogate(_ code: UInt16) -> Bool { code >= 0xd800 && code <= 0xdbff }

private func isLowSurrogate(_ code: UInt16) -> Bool { code >= 0xdc00 && code <= 0xdfff }

/// `sameEnd(a, b, limit, fromEnd)`: how many positions at the start of `a`
/// and `b` (or at their end) are the same, at most `limit`, without ending
/// inside a surrogate pair.
private func sameEnd(_ a: Segments, _ b: Segments, _ limit: Int, _ fromEnd: Bool) -> Int {
  func at(_ side: Segments, _ k: Int) -> Int { fromEnd ? side.length - 1 - k : k }
  var k = 0
  while k < limit {
    let code = a.text[at(a, k)]
    if code != b.text[at(b, k)] || (code == 0xffff && a.codes[at(a, k)] != b.codes[at(b, k)]) {
      break
    }
    k += 1
  }
  let last: UInt16 = k > 0 ? a.text[at(a, k - 1)] : 0
  return (fromEnd ? isLowSurrogate(last) : isHighSurrogate(last)) ? k - 1 : k
}

/// The positions where the segments of `side` end.
private func segmentEnds(_ side: Segments) -> [Int] {
  var position = 0
  return side.segs.map { seg in
    position += seg.kind == CHAR ? seg.text.count : 1
    return position
  }
}

/// `flattenSide(side, other, prefix, suffix)`: `side` as units: a unit per
/// character, except in the first `prefix` and the last `suffix` positions,
/// which both sides share. There a run of characters is one unit, cut where
/// a segment of either side ends.
private func flattenSide(
  _ side: Segments, _ other: Segments, _ prefix: Int, _ suffix: Int
) -> Flattened {
  var out = Flattened()
  out.points = side.points.map { _ in -1 }
  let suffixStart = side.length - suffix
  // Where a shared run is cut, in the positions of `side`
  var cutSet = Set(segmentEnds(side))
  for end in segmentEnds(other) {
    if end < prefix {
      cutSet.insert(end)
    } else if end > other.length - suffix {
      cutSet.insert(end - other.length + side.length)
    }
  }
  cutSet.insert(prefix)
  cutSet.insert(suffixStart)
  let cuts = cutSet.sorted()
  var cut = 0
  var starts: [Int] = []
  func push(
    _ code: UnitCode, _ kind: Int, _ node: LexicalNode, _ offset: Int, _ ids: [String],
    _ start: Int
  ) {
    out.codes.append(code)
    out.kinds.append(kind)
    out.nodes.append(node)
    out.offsets.append(offset)
    out.ids.append(ids)
    starts.append(start)
  }
  var position = 0
  for seg in side.segs {
    if seg.kind != CHAR {
      push(.other(seg.code), seg.kind, seg.node, 0, seg.ids, position)
      position += 1
      continue
    }
    let text = seg.text
    var offset = 0
    while offset < text.count {
      let at = position + offset
      if at < prefix || at >= suffixStart {
        // JS reads `undefined` past the end of `cuts`, which compares false
        while cut < cuts.count && cuts[cut] <= at {
          cut += 1
        }
        let end = cut < cuts.count ? min(cuts[cut], position + text.count) : position + text.count
        push(
          at < prefix ? .prefix(at) : .suffix(side.length - at), CHAR, seg.node, offset, seg.ids,
          at)
        offset = end - position
      } else {
        let size =
          offset + 1 < text.count && isHighSurrogate(text[offset])
            && isLowSurrogate(text[offset + 1]) ? 2 : 1
        push(.text(Array(text[offset..<offset + size])), CHAR, seg.node, offset, seg.ids, at)
        offset += size
      }
    }
    position += text.count
  }
  for (i, point) in side.points.enumerated() where point >= 0 {
    var index = 0
    while index < starts.count && starts[index] < point {
      index += 1
    }
    out.points[i] = index
  }
  return out
}

/// The ids in both `a` and `b`.
private func intersect(_ a: [String], _ b: [String]) -> [String] {
  a.isEmpty || b.isEmpty ? NO_MARK_IDS : a.filter(b.contains)
}

/// The ids in `a` that are not in `removed`.
private func without(_ a: [String], _ removed: Set<String>) -> [String] {
  removed.isEmpty || !a.contains(where: removed.contains) ? a : a.filter { !removed.contains($0) }
}

/// The ids in `a` or `b`, sorted.
private func union(_ a: [String], _ b: [String]) -> [String] {
  if b.isEmpty {
    return a
  }
  if a.isEmpty {
    return b
  }
  var seen = Set<String>()
  return jsSorted((a + b).filter { seen.insert($0).inserted })
}

/// The largest number of unit pairs `matchUnits` compares. A change that is
/// larger and is not a replaced selection is taken as one replacement.
private let MATCH_BUDGET = 250_000

/// `matchUnits(a, a0, a1, b, b0, b1)`: the longest common subsequence of
/// `a[a0, a1)` and `b[b0, b1)`, as the index pairs it matches (taking the
/// earliest matches), or `nil` when there are too many units to compare.
private func matchUnits(
  _ a: [UnitCode], _ a0: Int, _ a1: Int, _ b: [UnitCode], _ b0: Int, _ b1: Int
) -> [(Int, Int)]? {
  let n = a1 - a0
  let m = b1 - b0
  if n <= 0 || m <= 0 {
    return []
  }
  if n * m > MATCH_BUDGET {
    return nil
  }
  let width = m + 1
  // lengths[i * width + j]: the length of the LCS of a[a0 + i, a1) and
  // b[b0 + j, b1)
  var lengths = [UInt32](repeating: 0, count: (n + 1) * width)
  for i in stride(from: n - 1, through: 0, by: -1) {
    for j in stride(from: m - 1, through: 0, by: -1) {
      lengths[i * width + j] =
        a[a0 + i] == b[b0 + j]
        ? lengths[(i + 1) * width + j + 1] + 1
        : max(lengths[(i + 1) * width + j], lengths[i * width + j + 1])
    }
  }
  var pairs: [(Int, Int)] = []
  var i = 0
  var j = 0
  while i < n && j < m {
    if a[a0 + i] == b[b0 + j] {
      pairs.append((a0 + i, b0 + j))
      i += 1
      j += 1
    } else if lengths[(i + 1) * width + j] >= lengths[i * width + j + 1] {
      i += 1
    } else {
      j += 1
    }
  }
  return pairs
}

/// `$hasMarkIDsState(flavor, node)`: whether ids were ever set on `node`,
/// or on the node it was split or copied from.
private func hasMarkIDsState(_ flavor: MarkFlavor, _ node: LexicalNode) throws -> Bool {
  // @internal access: NodeState does not expose whether a value was set
  guard let state = try node.getLatest().__state else { return false }
  return state.knownState.has(ObjectIdentifier(flavor.state))
    || state.unknownState?.has(flavor.stateKey) == true
}

/// `$splitsIntoOwnClass(node)`: whether `splitText` cuts `node` into nodes
/// of its own class: a TextNode in normal mode that is a TextNode, or of
/// the class that replaces TextNode in this editor.
private func splitsIntoOwnClass(_ node: TextNode) throws -> Bool {
  guard try node.getMode() == "normal" else { return false }
  // An unattached node, garbage collected with the update
  return try node.isSimpleText() || type(of: createTextNode("")) == type(of: node)
}

/// `$setUnitIDs(flavor, flat, ids, start, end, splits)`: sets the ids of
/// the units `[start, end)` of `flat`, which are all the units of one node,
/// to `ids[start, end)`. A TextNode is split where they differ when its
/// pieces are of its own class; another TextNode gets all of them.
private func setUnitIDs(
  _ flavor: MarkFlavor, _ flat: Flattened, _ ids: [[String]], _ start: Int, _ end: Int,
  _ splits: SplitOrigins
) throws {
  let node = flat.nodes[start]
  guard let textNode = node as? TextNode else {
    try setNodeMarkIDs(flavor, node, ids[start])
    return
  }
  var offsets: [Int] = []
  for i in (start + 1)..<max(start + 1, end) where ids[i] != ids[i - 1] {
    offsets.append(flat.offsets[i])
  }
  if !offsets.isEmpty, try !splitsIntoOwnClass(textNode) {
    var all = NO_MARK_IDS
    for i in start..<end {
      all = union(all, ids[i])
    }
    if try readNodeMarkIDs(flavor, textNode) != all {
      try setNodeMarkIDs(flavor, textNode, all)
    }
    return
  }
  let parts = try offsets.isEmpty ? [textNode] : textNode.splitText(offsets: offsets)
  // The other flavors must not take the pieces for inserted content: they
  // carry copies of the other flavors' ids of the node they were cut from
  for part in parts.dropFirst() {
    splits.entries[part.getKey()] = (flavor, textNode.getKey())
  }
  var i = start
  for part in parts {
    if try readNodeMarkIDs(flavor, part) != ids[i] {
      try setNodeMarkIDs(flavor, part, ids[i])
    }
    // The next part starts where the ids change
    repeat {
      i += 1
    } while i < end && ids[i] == ids[i - 1]
  }
}

/// `getInsertedRange(prevText, text, caret)`: the offsets `[start, end)` of
/// `text` that are not in `prevText`, the text that the same TextNode had.
/// With `caret` (an offset in `text`, -1 when the caret is elsewhere), the
/// insertion ends at the caret when the text after the caret is the end of
/// `prevText`. Neither end splits a surrogate pair.
private func getInsertedRange(_ prevText: [UInt16], _ text: [UInt16], _ caret: Int) -> (Int, Int) {
  var end = -1
  var maxStart = min(prevText.count, text.count)
  if caret >= 0 && caret <= text.count && prevText.ends(with: text[caret...]) {
    end = caret
    maxStart = min(caret, prevText.count - (text.count - caret))
  }
  var start = 0
  while start < maxStart && prevText[start] == text[start] {
    start += 1
  }
  if start > 0 && isHighSurrogate(text[start - 1]) {
    start -= 1
  }
  if end < 0 {
    var suffix = 0
    let maxSuffix = min(prevText.count, text.count) - start
    while suffix < maxSuffix
      && prevText[prevText.count - 1 - suffix] == text[text.count - 1 - suffix]
    {
      suffix += 1
    }
    end = text.count - suffix
    if end < text.count && isLowSurrogate(text[end]) {
      end += 1
    }
  }
  return (start, end)
}

extension Array where Element == UInt16 {
  fileprivate func ends(with suffix: ArraySlice<UInt16>) -> Bool {
    suffix.count <= count && self[(count - suffix.count)...].elementsEqual(suffix)
  }
}

/// `$applyInsertedTextMarkIDs(...)`: gives the text a local edit inserted
/// the ids the documented rule says it has, whatever TextNode the core put
/// it in, and gives text that the edit re-created back its ids:
///
/// - Inserted text gets the ids shared by the units right before and right
///   after it, within its block.
/// - When it replaces a selection, also the ids all of the replaced text
///   had.
/// - Its own ids: those of the nodes it came in. Text inserted into a node
///   that existed has none, also when a transform split it off into a copy
///   of that node.
/// - Text that was not deleted keeps its ids, also when the core or a
///   transform moved it into a new TextNode that has none, and when the
///   update moved a leaf that existed.
/// - None of this gives back an id in `removed`.
///
/// The change is found by comparing the units around the caret with the
/// same units in `prevState`, within the blocks the update changed. See the
/// JS source for the full account.
func applyInsertedTextMarkIDs(
  _ flavor: MarkFlavor, _ editor: Editor, _ prevState: EditorState,
  _ replaced: (PointLike, PointLike)?, _ removed: Set<String>, _ splits: SplitOrigins,
  _ replacedForSure: Bool = false
) throws {
  guard let selection = try getSelection() as? RangeSelection, selection.isCollapsed() else {
    return
  }
  let caret = selection.anchor
  let caretNode = try caret.getNode()
  guard let block = try getBlock(caretNode) else { return }
  // @internal access: a node that this update did not change is the same
  // object in both states
  let prevNodes = prevState._nodeMap
  func isUnchangedInline(_ element: ElementNode) throws -> Bool {
    var hasUnits = false
    var child = try element.getFirstChild()
    while let current = child {
      if prevNodes.get(current.getKey()) !== current {
        return false
      }
      if let inner = current as? ElementNode, try inner.isInline() {
        if try !isUnchangedInline(inner) {
          return false
        }
        hasUnits = true
      } else if let text = current as? TextNode {
        if try text.getTextContentSize() > 0 {
          hasUnits = true
        }
      } else {
        hasUnits = true
      }
      child = try current.getNextSibling()
    }
    return hasUnits
  }
  // A child of a block that bounds the comparison: unchanged, with units
  func isContext(_ node: LexicalNode) throws -> Bool {
    if prevNodes.get(node.getKey()) !== node {
      return false
    }
    if let text = node as? TextNode {
      return try text.getTextContentSize() > 0
    }
    if let element = node as? ElementNode, try element.isInline() {
      return try isUnchangedInline(element)
    }
    return true
  }
  func expand(_ start: LexicalNode?, _ direction: LeafDirection) throws -> LexicalNode? {
    var node = start
    while let current = node, try !isContext(current) {
      node = try direction == .previous ? current.getPreviousSibling() : current.getNextSibling()
    }
    return node
  }
  // The children of the caret's block around the caret that may have
  // changed, bounded by an unchanged child on each side (or the block's
  // ends)
  var left: LexicalNode?
  var right: LexicalNode?
  if caret.type == .element && caret.key == block.getKey() {
    right = try block.getChildAtIndex(caret.offset)
    left = try right != nil ? right!.getPreviousSibling() : block.getLastChild()
  } else {
    var top: LexicalNode = caretNode
    while let parent = try top.getParent(), !parent.is(block) {
      top = parent
    }
    left = try top.getPreviousSibling()
    right = try top.getNextSibling()
  }
  let from = try expand(left, .previous)
  let to = try expand(right, .next)
  // The blocks around the caret that may have changed: from the nearest one
  // at or before it that existed before the update, up to the next one that
  // did and did not lose its first children to the blocks before it
  func isNextBoundary(_ node: LexicalNode) throws -> Bool {
    let key = node.getKey()
    guard prevNodes.has(key) else { return false }
    let firstChild = try (node as? ElementNode)?.getFirstChild()
    return prevNodes.get(key) === node || firstChild == nil
      || prevNodes.get(firstChild!.getKey()) === firstChild
  }
  let parent = try block.getParent()
  var first: LexicalNode? = block
  var next: LexicalNode? = nil
  if parent != nil {
    while let current = first, !prevNodes.has(current.getKey()) {
      first = try current.getPreviousSibling()
    }
    next = try block.getNextSibling()
    while let current = next, try !isNextBoundary(current) {
      next = try current.getNextSibling()
    }
  } else if !prevNodes.has(block.getKey()) {
    return
  }
  var windows: [BlockWindow] = []
  var current: LexicalNode? = try first ?? parent!.getFirstChild()
  while let node = current, node !== next {
    guard let element = node as? ElementNode else { return }
    let isCaretBlock = element.is(block)
    var windowFrom: LexicalNode? = nil
    if windows.isEmpty && first != nil {
      windowFrom = try isCaretBlock ? from : expand(element.getLastChild(), .previous)
    }
    windows.append(BlockWindow(block: element, from: windowFrom, to: isCaretBlock ? to : nil))
    if (isCaretBlock && to != nil) || parent == nil {
      // What follows an unchanged child after the caret is not compared
      break
    }
    current = try node.getNextSibling()
  }
  // A nested block that is the same object in both states, with nothing in
  // it changed by this update: compared as one unit, without reading it
  // @internal access: the elements this update changed or changed inside
  let dirtyElements = editor._dirtyElements
  // @internal access: the node map of the pending state
  let nextNodes = editor._pendingEditorState!._nodeMap
  let isUnchangedBlock: (ElementNode) -> Bool = { node in
    let key = node.getKey()
    guard let prevNode = prevNodes.get(key) else { return false }
    return prevNode === nextNodes.get(key) && !dirtyElements.has(key)
  }
  let afterSegments = try collectSegments(
    flavor, windows, [toPointLike(caret)], isUnchangedBlock)
  let firstKey = first?.getKey()
  let nextKey = next?.getKey()
  let parentKey = parent?.getKey()
  let blockKey = block.getKey()
  let fromKey = windows[0].from?.getKey()
  let toKey = windows[windows.count - 1].to?.getKey()
  let replacedPoints = replaced.map { [$0.0, $0.1] } ?? []
  let beforeSegments: Segments? = try prevState.read(editor: editor) {
    var prevWindows: [BlockWindow] = []
    var current: LexicalNode?
    if let firstKey {
      current = try getNodeByKey(firstKey)
      let prevParent = try current?.getParent()
      if prevParent?.getKey() != parentKey {
        return nil
      }
    } else {
      guard let parentKey, let prevParent = try getNodeByKey(parentKey) as? ElementNode else {
        return nil
      }
      current = try prevParent.getFirstChild()
    }
    var reachedTo = false
    while let node = current {
      if node.getKey() == nextKey {
        break
      }
      guard let element = node as? ElementNode else { return nil }
      var prevWindow = BlockWindow(block: element, from: nil, to: nil)
      if prevWindows.isEmpty, let fromKey {
        // Unchanged, so in the same block as in the pending state
        prevWindow.from = try getNodeByKey(fromKey)
        guard let windowFrom = prevWindow.from, try element.is(windowFrom.getParent()) else {
          return nil
        }
      }
      if let toKey, element.getKey() == blockKey {
        prevWindow.to = try getNodeByKey(toKey)
        guard let windowTo = prevWindow.to, try element.is(windowTo.getParent()) else {
          return nil
        }
        prevWindows.append(prevWindow)
        reachedTo = true
        break
      }
      prevWindows.append(prevWindow)
      if parentKey == nil {
        break
      }
      current = try node.getNextSibling()
    }
    if toKey != nil ? !reachedTo : (nextKey != nil && current == nil) {
      return nil
    }
    return try collectSegments(flavor, prevWindows, replacedPoints, isUnchangedBlock)
  }
  let caretAt = afterSegments.points[0]
  guard let beforeSegments, caretAt >= 0 else { return }
  // The text both sides share at the start and the end is compared as a few
  // units. Those stay outside the change.
  var suffixLimit = afterSegments.length - caretAt
  var prefixLimit = max(0, min(caretAt, beforeSegments.length - suffixLimit))
  if beforeSegments.points.count == 2 {
    let replacedStart = beforeSegments.points[0]
    let replacedEnd = beforeSegments.points[1]
    if replacedStart >= 0 && replacedEnd >= 0 {
      prefixLimit = min(prefixLimit, replacedStart)
      suffixLimit = min(suffixLimit, beforeSegments.length - replacedEnd)
    }
  }
  let prefix = sameEnd(afterSegments, beforeSegments, prefixLimit, false)
  let suffix = sameEnd(
    afterSegments, beforeSegments,
    min(suffixLimit, min(afterSegments.length, beforeSegments.length) - prefix), true)
  let after = flattenSide(afterSegments, beforeSegments, prefix, suffix)
  let before = flattenSide(beforeSegments, afterSegments, prefix, suffix)
  let c = after.points[0]
  let n = after.codes.count
  let m = before.codes.count
  // Anchored at the caret: what follows it is what followed the change
  let k = n - c
  if k > m {
    return
  }
  if k > 0 {
    for i in 1...k where after.codes[n - i] != before.codes[m - i] {
      return
    }
  }
  let e = m - k
  var s = 0
  while s < c && s < e && after.codes[s] == before.codes[s] {
    s += 1
  }
  // @internal access: the keys of the TextNodes merged by this update
  let normalized = editor._normalizedNodes
  var absorbedNew = false
  for key in normalized where !prevNodes.has(key) {
    let node = try getNodeByKey(key)
    if try node == nil || !node!.isAttached() {
      absorbedNew = true
      break
    }
  }
  // A node that did not exist before the update brought its ids with it,
  // and so did a node that absorbed one when normalization merged them. A
  // piece that another flavor's rule cut off a node in this update is as
  // new as that node.
  func isNewText(_ node: LexicalNode) -> Bool {
    let key = getSplitOrigin(splits, flavor, node.getKey())
    return !prevNodes.has(key) || (absorbedNew && normalized.has(key))
  }
  // The replaced selection, when the text in its place is new text
  var replacedSelection = false
  if replaced != nil {
    let start = before.points[0]
    let end = before.points[1]
    if start >= 0 && end > start && end == e && start <= s {
      replacedSelection = true
      var i = start
      while i < c && replacedSelection && !replacedForSure {
        replacedSelection = after.kinds[i] > LEAF || isNewText(after.nodes[i])
        i += 1
      }
      if replacedSelection {
        s = start
      }
    }
  }
  // The nearest previous sibling of a node that existed before the update,
  // by key, for the nodes that did not
  var origins: [NodeKey: LexicalNode?] = [:]
  func getOrigin(_ node: LexicalNode) throws -> LexicalNode? {
    var keys: [NodeKey] = []
    var origin = try node.getPreviousSibling()
    while let current = origin, !prevNodes.has(current.getKey()) {
      if let known = origins[current.getKey()] {
        origin = known
        break
      }
      keys.append(current.getKey())
      origin = try current.getPreviousSibling()
    }
    for key in keys {
      origins[key] = .some(origin)
    }
    return origin
  }
  // Whether `node` is text that a transform split off a TextNode that
  // existed, of another class than the pieces `splitText` creates (a text
  // entity), with the text inserted at its end
  var splitOff: [NodeKey: Bool] = [:]
  func isSplitOffEntity(_ node: LexicalNode) throws -> Bool {
    let key = node.getKey()
    if let result = splitOff[key] {
      return result
    }
    let origin = try node is TextNode && !prevNodes.has(key) ? getOrigin(node) : nil
    var result = false
    if let origin = origin as? TextNode, type(of: origin) != type(of: node) {
      // The same value: `splitText` cloned the node's state into the piece
      // in this update. Inserted nodes bring ids of their own, even equal
      // ones.
      result =
        try sameIDsValue(
          readNodeMarkIDsValue(flavor, origin), readNodeMarkIDsValue(flavor, node))
        // An unattached node, garbage collected with the update
        && type(of: createTextNode("")) == type(of: node)
    }
    splitOff[key] = result
    return result
  }
  var ids = after.ids
  // A kept character in a TextNode that the update created without ids (or
  // one that absorbed such a node) was re-created: it keeps its ids
  func keep(_ i: Int, _ j: Int) throws {
    if after.kinds[i] == CHAR && ids[i].isEmpty && !before.ids[j].isEmpty {
      let node = after.nodes[i]
      let key = node.getKey()
      if key != before.nodes[j].getKey() {
        if try (!prevNodes.has(key) && !hasMarkIDsState(flavor, node))
          || (absorbedNew && normalized.has(key))
        {
          ids[i] = without(before.ids[j], removed)
        }
      }
    }
  }
  for i in 0..<s {
    try keep(i, i)
  }
  for i in c..<max(c, n) {
    try keep(i, i - c + e)
  }
  // The inserted runs [i0, i1), each with the run [j0, j1) of `before` it
  // replaced
  var gaps: [(Int, Int, Int, Int)] = []
  let pairs = replacedSelection ? [] : matchUnits(after.codes, s, c, before.codes, s, e)
  var i0 = s
  var j0 = s
  for (i, j) in pairs ?? [] {
    if i > i0 || j > j0 {
      gaps.append((i0, i, j0, j))
    }
    try keep(i, j)
    i0 = i + 1
    j0 = j + 1
  }
  if c > i0 || e > j0 {
    gaps.append((i0, c, j0, e))
  }
  func unitIDs(_ i: Int) -> [String] {
    i >= 0 && i < n && after.kinds[i] <= LEAF ? ids[i] : NO_MARK_IDS
  }
  // The leaves whose text in `prevState` is part of `before`
  let compared = Set(before.nodes.map { $0.getKey() })
  // For a TextNode that existed, with other text, outside the compared
  // blocks: the offsets [start, end) of its text that are not in the text
  // it had, anchored at the caret when it holds the caret
  var insertedRanges: [NodeKey: (Int, Int)] = [:]
  func insertedRange(_ node: TextNode, _ prevText: [UInt16]) throws -> (Int, Int) {
    let key = node.getKey()
    if let range = insertedRanges[key] {
      return range
    }
    let range = getInsertedRange(
      prevText, Array(try node.getTextContent().utf16),
      caret.type == .text && caret.key == key ? caret.offset : -1)
    insertedRanges[key] = range
    return range
  }
  // Whether unit `i`, in a gap, is text that was there before the update
  // rather than inserted by it
  func isKeptUnit(_ i: Int) throws -> Bool {
    let node = after.nodes[i]
    guard let prevNode = prevNodes.get(node.getKey()) else { return false }
    guard let textNode = node as? TextNode, let prevText = prevNode as? TextNode else {
      return true
    }
    // @internal access: the text of the node in `prevState` (its methods
    // read the pending version)
    let prevTextContent = prevText.__text
    if try prevTextContent == textNode.getTextContent() {
      return true
    }
    if compared.contains(node.getKey()) {
      return false
    }
    let (start, end) = try insertedRange(textNode, Array(prevTextContent.utf16))
    return after.offsets[i] < start || after.offsets[i] >= end
  }
  for (g0, g1, r0, r1) in gaps where g1 != g0 {
    var kept: [Bool] = []
    for i in g0..<g1 {
      kept.append(try after.kinds[i] <= LEAF && isKeptUnit(i))
    }
    // The ids every replaced unit had (line breaks between blocks aside)
    var replacedIDs: [String]? = nil
    for j in r0..<max(r0, r1) where before.kinds[j] != BREAK {
      let unit = before.kinds[j] <= LEAF ? before.ids[j] : NO_MARK_IDS
      replacedIDs = replacedIDs.map { intersect($0, unit) } ?? unit
    }
    // Each run of inserted units between kept ones (the whole gap, when it
    // holds none) is an insertion
    var start = g0
    while start < g1 {
      if kept[start - g0] {
        start += 1
        continue
      }
      var end = start + 1
      while end < g1 && !kept[end - g0] {
        end += 1
      }
      var shared = intersect(unitIDs(start - 1), unitIDs(end))
      if let replacedIDs {
        shared = union(shared, without(replacedIDs, removed))
      }
      for i in start..<end where after.kinds[i] <= LEAF {
        // Text inserted into a node that existed, or split off one, has no
        // ids of its own
        let node = after.nodes[i]
        let own =
          try isNewText(node) && !after.ids[i].isEmpty && !isSplitOffEntity(node)
          ? after.ids[i] : NO_MARK_IDS
        ids[i] = union(own, shared)
      }
      start = end
    }
  }
  var i = 0
  while i < n {
    var end = i + 1
    while end < n && after.nodes[end] === after.nodes[i] {
      end += 1
    }
    for u in i..<end where after.kinds[u] <= LEAF && ids[u] != after.ids[u] {
      try setUnitIDs(flavor, after, ids, i, end, splits)
      break
    }
    i = end
  }
}
