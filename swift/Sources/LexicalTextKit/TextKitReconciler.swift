import Lexical

// The incremental TextKit reconciler (Q11, Q12). After each commit it
// brings a `TextKitDocument` from the previous editor state to the next one
// and returns the replacements that do the same to the view's text storage,
// walking the trees the way `$reconcileRoot` in LexicalReconciler.ts does:
// a node that is neither dirty nor cloned is skipped with its subtree, a
// dirty text node is re-rendered, and a dirty element splices its changed
// children between the unchanged prefix and suffix. The walk goes from the
// end of the document to its start, so each edit's location, taken from the
// previous rendering, is still right in the storage when the edit applies.

/// One replacement in the rendered text.
public struct TextKitEdit: Equatable, Sendable {
  /// The replaced range, in UTF-16 code units of the text as it is when
  /// this edit applies (after the edits before it).
  public let location: Int
  public let length: Int
  /// The new text.
  public let string: String
  /// The text nodes in `string`, at their locations once this edit applies.
  public let runs: [TextKitRun]

  public init(location: Int, length: Int, string: String, runs: [TextKitRun]) {
    self.location = location
    self.length = length
    self.string = string
    self.runs = runs
  }
}

/// The text storage a TextKit view renders into.
public protocol TextKitStorage: AnyObject {
  func beginEditing()
  /// Replaces `length` UTF-16 code units at `location` with `string`,
  /// whose text nodes are `runs`.
  func replaceCharacters(location: Int, length: Int, with string: String, runs: [TextKitRun])
  func endEditing()
}

extension TextKitStorage {
  /// Applies `edits`, in order, between `beginEditing()` and `endEditing()`.
  public func apply(_ edits: [TextKitEdit]) {
    guard !edits.isEmpty else { return }
    beginEditing()
    for edit in edits {
      replaceCharacters(
        location: edit.location, length: edit.length, with: edit.string, runs: edit.runs)
    }
    endEditing()
  }
}

/// Keeps a `TextKitDocument` in step with an editor's commits.
public final class TextKitReconciler {
  /// The rendering of `editorState`.
  public private(set) var document = TextKitDocument()
  /// The state `document` shows, `nil` before the first render.
  public private(set) var editorState: EditorState?
  /// How many incremental passes failed and rendered in full instead.
  package private(set) var fallbacks = 0

  public init() {}

  /// Renders `editorState` in full: one edit replacing the whole text.
  @discardableResult
  public func render(_ editorState: EditorState) throws -> [TextKitEdit] {
    let oldLength = document.length
    document = try editorState.read { try TextKitDocument(root: getRoot()) }
    self.editorState = editorState
    return [
      TextKitEdit(location: 0, length: oldLength, string: document.string, runs: document.runs)
    ]
  }

  /// `$reconcileRoot`: the edits from the previous commit's rendering to
  /// `payload.editorState`. A full reconcile (`setEditorState`, an error
  /// recovery), or a commit from a state other than the one rendered,
  /// renders in full.
  public func reconcile(_ payload: UpdateListenerPayload) throws -> [TextKitEdit] {
    guard let editorState, editorState === payload.prevEditorState,
      payload.dirtyType != FULL_RECONCILE
    else { return try render(payload.editorState) }
    let oldLength = document.length
    var context = ReconcileContext(payload)
    do {
      try payload.editorState.read {
        _ = try self.document.reconcileNode("root", at: 0, &context)
      }
      document.applyPendingEdits(&context)
    } catch {
      // The rendering no longer matches the previous state; start over.
      fallbacks += 1
      document = TextKitDocument()
      var edits = try render(payload.editorState)
      edits[0] = TextKitEdit(
        location: 0, length: oldLength, string: edits[0].string, runs: edits[0].runs)
      return edits
    }
    self.editorState = payload.editorState
    return context.edits
  }
}

/// A rendering that does not match the previous editor state.
struct TextKitMismatch: Error {}

/// What one reconcile pass reads, and the edits it makes.
struct ReconcileContext {
  let prevNodeMap: NodeMap
  let nextNodeMap: NodeMap
  let dirtyElements: JSMap<NodeKey, Bool>
  let dirtyLeaves: JSSet<NodeKey>
  /// The dirty nodes of each parent in the next state.
  var dirtyChildren: [NodeKey: [NodeKey]] = [:]
  var edits: [TextKitEdit] = []
  /// How many of `edits` the document's `string` has applied: the rest are
  /// pending (see `TextKitDocument.replace`).
  var appliedEdits = 0

  init(_ payload: UpdateListenerPayload) {
    prevNodeMap = payload.prevEditorState._nodeMap
    nextNodeMap = payload.editorState._nodeMap
    dirtyElements = payload.dirtyElements
    dirtyLeaves = payload.dirtyLeaves
    var keys: [NodeKey] = []
    for (key, _) in dirtyElements { keys.append(key) }
    for key in dirtyLeaves { keys.append(key) }
    for key in keys {
      if let parent = nextNodeMap.get(key)?.__parent {
        dirtyChildren[parent, default: []].append(key)
      }
    }
  }

  func isDirty(_ key: NodeKey) -> Bool {
    dirtyLeaves.has(key) || dirtyElements.has(key)
  }

  /// Whether the node at `key` is the same object in both states and not
  /// dirty, so neither it nor its subtree changed.
  func isUnchanged(_ key: NodeKey, _ node: LexicalNode) -> Bool {
    !isDirty(key) && prevNodeMap.get(key) === node
  }
}

extension TextKitDocument {
  /// Records the edit replacing a range of `string`, which applies it
  /// later. Splicing each edit into `string` as it came copied the text
  /// after it every time, quadratic in a restyle of the whole document,
  /// which edits every run. The walk goes from the end to the start, so
  /// the pending edits are in descending order and apply in one pass
  /// (`applyPendingEdits`); an edit that ends after the last pending one
  /// starts applies them first.
  mutating func replace(
    _ location: Int, _ length: Int, with text: String, runs: [TextKitRun],
    _ context: inout ReconcileContext
  ) {
    applyPendingEdits(before: location + length, &context)
    context.edits.append(TextKitEdit(location: location, length: length, string: text, runs: runs))
  }

  /// Applies the pending edits when one of them starts before `end`, so
  /// `string` is current up to `end`.
  mutating func applyPendingEdits(before end: Int, _ context: inout ReconcileContext) {
    if context.appliedEdits < context.edits.count, end > context.edits.last!.location {
      applyPendingEdits(&context)
    }
  }

  /// Applies the pending edits to `string`, in one pass from its start.
  mutating func applyPendingEdits(_ context: inout ReconcileContext) {
    let pending = context.edits[context.appliedEdits...]
    context.appliedEdits = context.edits.count
    guard let first = pending.first else { return }
    let utf16 = string.utf16
    if pending.count == 1 {
      let lower = utf16.index(utf16.startIndex, offsetBy: first.location)
      let upper = utf16.index(lower, offsetBy: first.length)
      string.replaceSubrange(lower..<upper, with: first.string)
      return
    }
    var result = ""
    result.reserveCapacity(string.utf8.count)
    var cursor = utf16.startIndex
    var cursorOffset = 0
    for edit in pending.reversed() {
      let lower = utf16.index(cursor, offsetBy: edit.location - cursorOffset)
      result += string[cursor..<lower]
      result += edit.string
      cursor = utf16.index(lower, offsetBy: edit.length)
      cursorOffset = edit.location + edit.length
    }
    result += string[cursor...]
    string = result
  }

  /// `$reconcileNode`: brings the node at `key`, rendered at `start`, up to
  /// date and returns its new length.
  mutating func reconcileNode(
    _ key: NodeKey, at start: Int, _ context: inout ReconcileContext
  ) throws -> Int {
    guard let kind = entries[key]?.kind, let oldLength = entries[key]?.length,
      let next = context.nextNodeMap.get(key)
    else { throw TextKitMismatch() }
    if context.isUnchanged(key, next) {
      return oldLength
    }
    switch kind {
    case .text:
      guard let next = next as? TextNode else { throw TextKitMismatch() }
      let content = try next.getTextContent()
      let format = try next.getFormat()
      let style = try next.getStyle()
      applyPendingEdits(before: start + oldLength, &context)
      let utf16 = string.utf16
      let lower = utf16.index(utf16.startIndex, offsetBy: start)
      let upper = utf16.index(lower, offsetBy: oldLength)
      if format == entries[key]!.format, style == entries[key]!.style,
        utf16[lower..<upper].elementsEqual(content.utf16)
      {
        return oldLength
      }
      let length = content.utf16.count
      replace(
        start, oldLength, with: content,
        runs: [TextKitRun(key: key, location: start, length: length, format: format, style: style)],
        &context)
      entries[key]!.length = length
      entries[key]!.format = format
      entries[key]!.style = style
      return length
    case .lineBreak, .decorator:
      // Always one character; a change of `isInline()` is the parent's
      // separators.
      return oldLength
    case .element:
      guard let next = next as? ElementNode else { throw TextKitMismatch() }
      if context.prevNodeMap.get(key) === next,
        let length = try reconcileDirtyChildren(key, at: start, &context)
      {
        return length
      }
      return try reconcileChildren(key, next, at: start, &context)
    }
  }

  /// The children of an element that was not cloned: they are the same
  /// nodes in the same order (inserting or removing a child writes its
  /// parent), so only the dirty ones are reconciled. Returns `nil` when a
  /// dirty child changed whether it is a block, which moves separators.
  mutating func reconcileDirtyChildren(
    _ key: NodeKey, at start: Int, _ context: inout ReconcileContext
  ) throws -> Int? {
    var changed: [(index: Int, key: NodeKey)] = []
    for child in context.dirtyChildren[key] ?? [] {
      guard let entry = entries[child], entry.parent == key,
        entry.index < entries[key]!.kids.count, entries[key]!.kids[entry.index].key == child,
        let node = context.nextNodeMap.get(child)
      else { return nil }
      if try Self.isBlock(node) != entries[key]!.kids[entry.index].isBlock {
        return nil
      }
      changed.append((entry.index, child))
    }
    changed.sort { $0.index > $1.index }
    var deltas: [(index: Int, delta: Int)] = []
    for (index, child) in changed {
      let oldLength = entries[child]!.length
      let length = try reconcileNode(child, at: start + entries[key]!.kids[index].start, &context)
      if length != oldLength {
        deltas.append((index, length - oldLength))
      }
    }
    var length = entries[key]!.length
    guard !deltas.isEmpty else { return length }
    var kids = entries[key]!.kids
    entries[key]!.kids = []
    var shift = 0
    var next = deltas.count - 1
    for i in (deltas[next].index + 1)..<max(kids.count, deltas[next].index + 1) {
      while next >= 0, deltas[next].index < i {
        shift += deltas[next].delta
        next -= 1
      }
      kids[i].start += shift
    }
    length += deltas.reduce(0) { $0 + $1.delta }
    entries[key]!.kids = kids
    entries[key]!.length = length
    return length
  }

  /// `$reconcileChildren`: the children before the first change and after
  /// the last are reconciled in place; the ones between, with the
  /// separators around them, are replaced by a fresh rendering.
  ///
  /// The clean children around the change are skipped, neither walked nor
  /// reconciled: a child that isn't dirty kept its siblings, so the
  /// element's dirty children bound the window that can have changed
  /// (`changedWindow`). Only the window is compared from both ends, and the
  /// children after it are shifted.
  mutating func reconcileChildren(
    _ key: NodeKey, _ element: ElementNode, at start: Int, _ context: inout ReconcileContext
  ) throws -> Int {
    guard let oldKids = entries[key]?.kids, let oldLength = entries[key]?.length else {
      throw TextKitMismatch()
    }
    let prevCount = oldKids.count
    let window =
      try changedWindow(key, element, oldKids, context)
      ?? (0, 0, try createChildrenArray(element, context.nextNodeMap))
    let (cleanPrefix, cleanSuffix, nextWindow) = window
    let windowEnd = prevCount - cleanSuffix
    let oldWindow = oldKids[cleanPrefix..<windowEnd]
    let nextCount = cleanPrefix + nextWindow.count + cleanSuffix

    func isKept(_ kid: Kid, _ nextKey: NodeKey) throws -> Bool {
      guard kid.key == nextKey, let node = context.nextNodeMap.get(nextKey) else { return false }
      return try context.isUnchanged(nextKey, node) || Self.isBlock(node) == kid.isBlock
    }
    let count = min(oldWindow.count, nextWindow.count)
    var prefix = 0
    while prefix < count,
      try isKept(oldKids[cleanPrefix + prefix], nextWindow[prefix])
    {
      prefix += 1
    }
    var suffix = 0
    while suffix < count - prefix,
      try isKept(oldKids[windowEnd - 1 - suffix], nextWindow[nextWindow.count - 1 - suffix])
    {
      suffix += 1
    }
    // Global indices: kept children are [0, keptPrefix) and the last
    // `keptSuffix`; the new ones between are [keptPrefix, middleEnd).
    let keptPrefix = cleanPrefix + prefix
    let keptSuffix = cleanSuffix + suffix
    let middleEnd = nextCount - keptSuffix
    var blocks = [Bool](repeating: false, count: nextCount)
    for i in 0..<keptPrefix { blocks[i] = oldKids[i].isBlock }
    for s in 0..<keptSuffix { blocks[nextCount - 1 - s] = oldKids[prevCount - 1 - s].isBlock }

    for s in cleanSuffix..<keptSuffix {
      let kid = oldKids[prevCount - 1 - s]
      _ = try reconcileNode(kid.key, at: start + kid.start, &context)
    }
    if keptPrefix < prevCount - keptSuffix || keptPrefix < middleEnd {
      let regionStart =
        keptPrefix > 0
        ? oldKids[keptPrefix - 1].start + entries[oldKids[keptPrefix - 1].key]!.length : 0
      let regionEnd = keptSuffix > 0 ? oldKids[prevCount - keptSuffix].start : oldLength
      for kid in oldKids[keptPrefix..<(prevCount - keptSuffix)] {
        removeSubtree(kid.key, parent: key)
      }
      var text = ""
      var runs: [TextKitRun] = []
      var offset = 0
      var previousIsBlock = keptPrefix > 0 && blocks[keptPrefix - 1]
      for i in keptPrefix..<middleEnd {
        let nextKey = nextWindow[i - cleanPrefix]
        guard let node = context.nextNodeMap.get(nextKey) else { throw TextKitMismatch() }
        let isBlock = try Self.isBlock(node)
        if i > 0, previousIsBlock || isBlock {
          text += "\n"
          offset += 1
        }
        offset += try build(
          node, parent: key, index: i, at: start + regionStart + offset, into: &text, runs: &runs)
        blocks[i] = isBlock
        previousIsBlock = isBlock
      }
      if keptSuffix > 0, middleEnd > 0, previousIsBlock || blocks[middleEnd] {
        text += "\n"
      }
      replace(start + regionStart, regionEnd - regionStart, with: text, runs: runs, &context)
    }
    for i in (cleanPrefix..<keptPrefix).reversed() {
      _ = try reconcileNode(oldKids[i].key, at: start + oldKids[i].start, &context)
    }

    // Lay out the window again; the clean children before it keep their
    // starts and the ones after it move by how much it grew.
    var windowKids: [Kid] = []
    windowKids.reserveCapacity(nextWindow.count)
    var offset =
      cleanPrefix > 0
      ? oldKids[cleanPrefix - 1].start + entries[oldKids[cleanPrefix - 1].key]!.length : 0
    for (j, nextKey) in nextWindow.enumerated() {
      let i = cleanPrefix + j
      if i > 0, blocks[i - 1] || blocks[i] {
        offset += 1
      }
      windowKids.append(Kid(key: nextKey, start: offset, isBlock: blocks[i]))
      offset += entries[nextKey]!.length
    }
    let length: Int
    if cleanSuffix > 0 {
      let i = cleanPrefix + nextWindow.count
      if i > 0, blocks[i - 1] || blocks[i] {
        offset += 1
      }
      let delta = offset - oldKids[windowEnd].start
      length = oldLength + delta
      entries[key]!.kids = []
      var kids = oldKids
      kids.replaceSubrange(cleanPrefix..<windowEnd, with: windowKids)
      if delta != 0 {
        for i in (cleanPrefix + nextWindow.count)..<nextCount { kids[i].start += delta }
      }
      entries[key]!.kids = kids
    } else {
      length = offset
      entries[key]!.kids = []
      var kids = oldKids
      kids.replaceSubrange(cleanPrefix..<windowEnd, with: windowKids)
      entries[key]!.kids = kids
    }
    entries[key]!.length = length
    if prevCount != nextCount {
      for i in middleEnd..<nextCount {
        entries[entries[key]!.kids[i].key]!.index = i
      }
    }
    return length
  }

  /// The window of an element's children that can have changed, bounded by
  /// its dirty children: a child that isn't dirty has the same siblings as
  /// before, so the children before the first dirty one and after the last
  /// are as they were, except removed children next to the window
  /// (removing a node dirties its siblings, and the removed node leaves the
  /// map). Returns how many old children are kept before and after it and
  /// the new children in it, or `nil` when the dirty children don't bound
  /// it, as for an element that had no children.
  func changedWindow(
    _ key: NodeKey, _ element: ElementNode, _ oldKids: [Kid], _ context: ReconcileContext
  ) throws -> (Int, Int, [NodeKey])? {
    guard !oldKids.isEmpty, let dirty = context.dirtyChildren[key] else { return nil }
    var low = oldKids.count
    var high = -1
    for child in dirty {
      guard let entry = entries[child], entry.parent == key, entry.index < oldKids.count,
        oldKids[entry.index].key == child
      else { continue }
      low = min(low, entry.index)
      high = max(high, entry.index)
    }
    guard high >= 0 else { return nil }
    func isStill(_ kid: Kid) -> Bool {
      context.nextNodeMap.get(kid.key)?.__parent == key && !context.isDirty(kid.key)
    }
    while low > 0, !isStill(oldKids[low - 1]) { low -= 1 }
    while high < oldKids.count - 1, !isStill(oldKids[high + 1]) { high += 1 }
    var window: [NodeKey] = []
    let stop = high + 1 < oldKids.count ? oldKids[high + 1].key : nil
    var nextKey =
      low > 0 ? context.nextNodeMap.get(oldKids[low - 1].key)?.__next : element.__first
    while let current = nextKey, current != stop {
      guard let node = context.nextNodeMap.get(current), node.__parent == key else { return nil }
      window.append(current)
      nextKey = node.__next
    }
    guard nextKey == stop else { return nil }
    return (low, oldKids.count - 1 - high, window)
  }
}
