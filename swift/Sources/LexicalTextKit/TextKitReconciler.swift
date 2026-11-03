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
  /// Replaces a range of `string` and records the edit.
  mutating func replace(
    _ location: Int, _ length: Int, with text: String, runs: [TextKitRun],
    _ context: inout ReconcileContext
  ) {
    let utf16 = string.utf16
    let lower = utf16.index(utf16.startIndex, offsetBy: location)
    let upper = utf16.index(lower, offsetBy: length)
    string.replaceSubrange(lower..<upper, with: text)
    context.edits.append(TextKitEdit(location: location, length: length, string: text, runs: runs))
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
  mutating func reconcileChildren(
    _ key: NodeKey, _ element: ElementNode, at start: Int, _ context: inout ReconcileContext
  ) throws -> Int {
    let nextKeys = try createChildrenArray(element, context.nextNodeMap)
    guard let oldKids = entries[key]?.kids, let oldLength = entries[key]?.length else {
      throw TextKitMismatch()
    }
    let prevCount = oldKids.count
    let nextCount = nextKeys.count
    func isKept(_ kid: Kid, _ nextKey: NodeKey) throws -> Bool {
      guard kid.key == nextKey, let node = context.nextNodeMap.get(nextKey) else { return false }
      return try context.isUnchanged(nextKey, node) || Self.isBlock(node) == kid.isBlock
    }
    var prefix = 0
    while prefix < min(prevCount, nextCount), try isKept(oldKids[prefix], nextKeys[prefix]) {
      prefix += 1
    }
    var suffix = 0
    while suffix < min(prevCount, nextCount) - prefix,
      try isKept(oldKids[prevCount - 1 - suffix], nextKeys[nextCount - 1 - suffix])
    {
      suffix += 1
    }
    var lengths = [Int](repeating: 0, count: nextCount)
    var blocks = [Bool](repeating: false, count: nextCount)
    for i in 0..<prefix { blocks[i] = oldKids[i].isBlock }
    for s in 0..<suffix { blocks[nextCount - 1 - s] = oldKids[prevCount - 1 - s].isBlock }

    for s in 0..<suffix {
      let kid = oldKids[prevCount - 1 - s]
      lengths[nextCount - 1 - s] = try reconcileNode(kid.key, at: start + kid.start, &context)
    }
    let middleEnd = nextCount - suffix
    if prefix < prevCount - suffix || prefix < middleEnd {
      let regionStart =
        prefix > 0 ? oldKids[prefix - 1].start + entries[oldKids[prefix - 1].key]!.length : 0
      let regionEnd = suffix > 0 ? oldKids[prevCount - suffix].start : oldLength
      for kid in oldKids[prefix..<(prevCount - suffix)] {
        removeSubtree(kid.key, parent: key)
      }
      var text = ""
      var runs: [TextKitRun] = []
      var offset = 0
      var previousIsBlock = prefix > 0 && blocks[prefix - 1]
      for i in prefix..<middleEnd {
        guard let node = context.nextNodeMap.get(nextKeys[i]) else { throw TextKitMismatch() }
        let isBlock = try Self.isBlock(node)
        if i > 0, previousIsBlock || isBlock {
          text += "\n"
          offset += 1
        }
        let length = try build(
          node, parent: key, index: i, at: start + regionStart + offset, into: &text, runs: &runs)
        lengths[i] = length
        blocks[i] = isBlock
        offset += length
        previousIsBlock = isBlock
      }
      if suffix > 0, middleEnd > 0, previousIsBlock || blocks[middleEnd] {
        text += "\n"
      }
      replace(start + regionStart, regionEnd - regionStart, with: text, runs: runs, &context)
    }
    for i in (0..<prefix).reversed() {
      lengths[i] = try reconcileNode(oldKids[i].key, at: start + oldKids[i].start, &context)
    }

    var kids: [Kid] = []
    kids.reserveCapacity(nextCount)
    var offset = 0
    for i in 0..<nextCount {
      if i > 0, blocks[i - 1] || blocks[i] {
        offset += 1
      }
      kids.append(Kid(key: nextKeys[i], start: offset, isBlock: blocks[i]))
      offset += lengths[i]
    }
    entries[key]!.kids = kids
    entries[key]!.length = offset
    if prevCount != nextCount {
      for i in middleEnd..<nextCount {
        entries[nextKeys[i]]!.index = i
      }
    }
    return offset
  }
}
