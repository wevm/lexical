import Lexical

// Port of packages/lexical-text/src/registerLexicalTextEntity.ts.
//
// JS keeps its per-update bookkeeping in `WeakMap`s keyed by the pending
// editor state. Only one state is pending per editor at a time, so each map
// here (`PerPendingState`) holds the entry of the last state it was asked
// about, and starts afresh for a new one. `INTERNAL_$recordTextMove` and
// `INTERNAL_$recordTextRetype` record text edits for collaboration
// (`@lexical/yjs`), which the Swift port does not have: the move runs as
// is, and the retype records nothing.

/// `EntityMatch`: the UTF-16 offsets `[start, end)` of a match.
public struct EntityMatch: Equatable, Sendable {
  public var start: Int
  public var end: Int

  public init(start: Int, end: Int) {
    self.start = start
    self.end = end
  }
}

// How many matches in a text node are passed over to check the node after
// it, where its transform returns before the loop that checks that node.
private let MAX_MATCHES_PASSED = 8

// How many times one update turns the same token or segmented entity back
// into text before it leaves it as an entity.
private let MAX_TURNED_INTO_TEXT = 2

/// A value per pending editor state: `WeakMap<EditorState, V>` in JS.
private final class PerPendingState<V> {
  private weak var state: EditorState?
  private var value: V?
  private let make: () -> V

  init(_ make: @escaping () -> V) {
    self.make = make
  }

  func get(_ editor: Editor) -> V {
    let editorState = internalGetActiveEditorState() ?? editor.getEditorState()
    if state !== editorState || value == nil {
      state = editorState
      value = make()
    }
    return value!
  }
}

/// A set of keys that the maps above hold by reference, as JS holds a `Set`.
private final class KeySet {
  var keys = Set<NodeKey>()
}

private final class CountMap {
  var counts: [String: Int] = [:]
}

/// Give `to`, the text node that takes the place of `from` with the same
/// characters (an entity made plain text, or plain text made an entity),
/// the NodeState of `from`: the ad-hoc data an app keeps on text, such as
/// comment ids or block ids, flat states included, as `splitText` gives it
/// to its pieces. The properties inherent to the entity type (its class
/// fields and mode) are not NodeState and stay with the type. A key that
/// `to` already holds keeps its value (when the text is made an entity:
/// state that `createNode` gave it).
private func carryNodeState(_ from: TextNode, _ to: TextNode) throws {
  guard let fromState = try from.getLatest().__state, fromState.size != 0 else {
    return
  }
  var held = Set<String>()
  if let toState = try to.getLatest().__state {
    for key in toState.unknownState?.keys ?? [] {
      held.insert(key)
    }
    for (_, entry) in toState.knownState {
      held.insert(entry.config.key)
    }
  }
  var state: NodeState?
  func writableState() throws -> NodeState {
    if let state { return state }
    let writable = try getWritableNodeState(to)
    state = writable
    return writable
  }
  for (_, entry) in fromState.knownState {
    let stateConfig = entry.config
    let key = stateConfig.key
    if held.contains(key) {
      continue
    }
    held.insert(key)
    if stateConfig.isEqualAny(entry.value, stateConfig.defaultAny) {
      continue
    }
    let writable = try writableState()
    let own = writable.sharedNodeState.sharedConfigMap.get(key)
    if own == nil || own === stateConfig {
      // The same value (comment ids compare by identity)
      writable.updateFromKnown(stateConfig, entry.value)
    } else {
      // The type of `to` has another config for this key: parse with it
      writable.updateFromUnknown(key, stateConfig.unparseAny(entry.value))
    }
  }
  if let unknown = fromState.unknownState {
    for key in unknown.keys where !held.contains(key) {
      try writableState().updateFromUnknown(key, unknown[key])
    }
  }
}

/// The value `state` holds for `stateConfig` (parsed from JSON if it was not
/// read yet), without caching it, or its default.
private func readStateValue(_ state: NodeState?, _ stateConfig: AnyStateConfig) -> Any {
  guard let state else {
    return stateConfig.defaultAny
  }
  if let known = state.knownState.get(ObjectIdentifier(stateConfig)) {
    return known.value
  }
  if let unknown = state.unknownState, unknown.has(stateConfig.key) {
    return stateConfig.parseAny(unknown[stateConfig.key])
  }
  return stateConfig.defaultAny
}

/// Whether `a` and `b` hold equivalent NodeState, by the rule that decides
/// whether two text nodes merge: known values compared with their
/// `StateConfig.isEqual`, values not read yet compared as they are (JS
/// compares those by identity, which for JSON primitives is by value;
/// Swift compares JSON objects and arrays by value too).
private func haveEquivalentNodeState(_ a: TextNode, _ b: TextNode) throws -> Bool {
  let stateA = try a.getLatest().__state
  let stateB = try b.getLatest().__state
  if stateA === stateB {
    return true
  }
  var keys = Set<String>()
  func hasUnequalKnown(_ source: NodeState?, _ other: NodeState?) -> Bool {
    guard let source else { return false }
    for (_, entry) in source.knownState {
      let stateConfig = entry.config
      if keys.contains(stateConfig.key) {
        continue
      }
      keys.insert(stateConfig.key)
      let otherValue = readStateValue(other, stateConfig)
      if !stateConfig.isEqualAny(otherValue, entry.value) {
        return true
      }
    }
    return false
  }
  func hasUnequalUnknown(_ source: NodeState?, _ other: NodeState?) -> Bool {
    guard let unknown = source?.unknownState else { return false }
    let otherUnknown = other?.unknownState
    for key in unknown.keys {
      if keys.contains(key) {
        continue
      }
      keys.insert(key)
      let otherValue: JSONValue?? = otherUnknown.flatMap { $0.has(key) ? .some($0[key]) : nil }
      if otherValue == nil || otherValue! != unknown[key] {
        return true
      }
    }
    return false
  }
  return
    !(hasUnequalKnown(stateA, stateB) || hasUnequalKnown(stateB, stateA)
    || hasUnequalUnknown(stateA, stateB) || hasUnequalUnknown(stateB, stateA))
}

/// Whether `a` and `b` would merge as text: the same format, style, detail
/// and NodeState.
private func wouldMerge(_ a: TextNode, _ b: TextNode) throws -> Bool {
  try a.getFormat() == b.getFormat() && a.getStyle() == b.getStyle()
    && a.getDetail() == b.getDetail() && haveEquivalentNodeState(a, b)
}

/// Before `entity` takes the first `count` characters of `node`, the text
/// node after it: move a selection point in those characters (or right
/// after them) into the entity, and one in the rest of `node` back by
/// `count`.
private func moveSelectionIntoEntity(_ node: TextNode, _ entity: TextNode, _ count: Int) throws {
  guard let selection = try getSelection() as? RangeSelection else {
    return
  }
  let key = node.getKey()
  let entitySize = try entity.getTextContentSize()
  for point in [selection.anchor, selection.focus] where point.key == key && point.type == .text {
    if point.offset <= count {
      try point.set(entity.getKey(), entitySize + point.offset, .text)
    } else {
      try point.set(key, point.offset - count, .text)
    }
  }
}

// The keys of the text nodes that `reverseNodeTransform` split off the end
// of an entity (and of their pieces), by pending editor state (one update,
// or the updates batched into one), shared by every registration on the
// editor. `splitText` gives such a piece a copy of the entity's NodeState,
// but its text was inserted after the entity (typed or pasted), so that
// NodeState is not its own, and no entity made from it (of this
// registration or another) takes it.
private func splitOff(_ editor: Editor) -> KeySet {
  shared(editor, "@lexical/text/splitOffInUpdate").get(editor)
}

// The keys of the parents of the entities turned into text, by pending
// editor state, shared by every registration on the editor (see
// `isInsertedText`).
private func demotedParents(_ editor: Editor) -> KeySet {
  shared(editor, "@lexical/text/demotedInUpdate").get(editor)
}

private func shared(_ editor: Editor, _ name: String) -> PerPendingState<KeySet> {
  if let existing = editor._extensionStorage[name] as? PerPendingState<KeySet> {
    return existing
  }
  let created = PerPendingState { KeySet() }
  editor._extensionStorage[name] = created
  return created
}

/// Whether the characters `[start, end)` of `node`, whose text is `text`,
/// were inserted into it in this update, at its start or its end, with no
/// entity turned into text in its parent in this update (whose text may
/// have merged into it): it existed in the committed state, and they are
/// outside the text it shares with its committed version at its start and
/// its end. Such characters have a copy of the node's NodeState (as text
/// inserted into a node does), not NodeState of their own, so an entity
/// made of them does not take it (`MarkStateExtension` does not add text
/// inserted at a comment's edge to it, for example).
private func isInsertedText(
  _ editor: Editor, _ node: TextNode, _ text: String, _ start: Int, _ end: Int
) throws -> Bool {
  guard let prev = editor.getEditorState()._nodeMap.get(node.getKey()) as? TextNode else {
    return false
  }
  if let parent = try node.getParent(), demotedParents(editor).keys.contains(parent.getKey()) {
    return false
  }
  let prevText = Array(prev.__text.utf16)
  let units = Array(text.utf16)
  let max = Swift.min(prevText.count, units.count)
  var prefix = 0
  while prefix < max && prevText[prefix] == units[prefix] {
    prefix += 1
  }
  var suffix = 0
  while suffix < max - prefix
    && prevText[prevText.count - 1 - suffix] == units[units.count - 1 - suffix]
  {
    suffix += 1
  }
  return start >= prefix && end <= units.count - suffix && end > start
}

/// `registerLexicalTextEntity(editor, getMatch, targetNode, createNode)`:
/// the node transforms that turn the text `getMatch` finds into entity
/// nodes of `targetNode` (made by `createNode`, such as a `HashtagNode`)
/// and entities that no longer match back into text. Returns the two
/// unregister functions, for `mergeRegister`.
public func registerLexicalTextEntity<T: TextNode>(
  _ editor: Editor, _ getMatch: @escaping (String) -> EntityMatch?, _ targetNode: T.Type,
  _ createNode: @escaping (TextNode) throws -> T
) throws -> [() -> Void] {
  func isTargetNode(_ node: LexicalNode?) -> Bool {
    node is T
  }

  func replaceWithSimpleText(_ node: TextNode) throws {
    if let parent = try node.getParent() {
      demotedParents(editor).keys.insert(parent.getKey())
    }
    let textNode = try createTextNode(node.getTextContent())
      .setFormat(node.getFormat())
      .setStyle(node.getStyle())
      .setDetail(node.getDetail())
    try carryNodeState(node, textNode)
    try node.replace(textNode)
  }

  // The keys of the text nodes the entity transform marked dirty, by
  // pending editor state (one update, or the updates batched into one
  // commit).
  let markedInUpdate = PerPendingState { KeySet() }

  // How many times each token or segmented entity was turned back into
  // text, by pending editor state (one update, or the updates batched into
  // one commit) and its parent, place and text.
  let turnedIntoTextInUpdate = PerPendingState { CountMap() }

  // Whether a token or segmented entity with the same parent, place and
  // text as `entity` was turned back into text as many times as it may be
  // in one update; counts this time if not.
  func wasTurnedIntoText(_ entity: TextNode, _ why: String = "") throws -> Bool {
    // Told by the first node before it that is no text entity, which the
    // transforms leave in place while they make the entity and turn it back
    // (text that merges with it keeps its key), and the size of the text
    // entities between (which they may make again too).
    var before = try entity.getPreviousSibling()
    var entitiesBefore = 0
    while let text = before as? TextNode, text.isTextEntity() {
      entitiesBefore += try text.getTextContentSize()
      before = try text.getPreviousSibling()
    }
    let at = "\(before?.getKey() ?? "")+\(entitiesBefore)"
    let parent = try entity.getParent()
    let id = "\(why)\(parent?.getKey() ?? ""):\(at):\(try entity.getTextContent())"
    let turned = turnedIntoTextInUpdate.get(editor)
    let count = turned.counts[id] ?? 0
    if count >= MAX_TURNED_INTO_TEXT {
      return true
    }
    turned.counts[id] = count + 1
    return false
  }

  func getMode(_ node: TextNode) throws -> Int {
    try node.getLatest().__mode
  }

  // Whether the first match of `textAfter` after `text` starts where
  // `textAfter` does, passing over the matches of `text` as the loop below
  // does (it leaves those as text when the transform returns before it),
  // and telling a match that ends where `textAfter` starts (which touches
  // it) from one that starts there. `nil` when `text` has more matches than
  // are worth passing over.
  func getFirstMatchAfter(_ text: String, _ textAfter: String) -> Bool? {
    let combinedText = text + textAfter
    let textLength = text.utf16.count
    var offset = 0
    for _ in 0..<MAX_MATCHES_PASSED {
      guard let match = getMatch(combinedText.jsSlice(offset)) else {
        return false
      }
      let start = offset + match.start
      let end = offset + match.end
      if end <= start {
        return nil
      }
      if end >= textLength {
        return start == textLength
      }
      offset = end
    }
    return nil
  }

  // Whether the node after `node` is a text entity (an entity right after
  // another is handled by the entity's own transform).
  func isNextTextEntity(_ node: TextNode) throws -> Bool {
    (try node.getNextSibling() as? TextNode)?.isTextEntity() ?? false
  }

  // Whether a text node after `entity` is not text it was split from: one
  // in another format, style or detail, or not simple text.
  func isTextAfterOutsideSource(_ entity: TextNode) throws -> Bool {
    guard let after = try entity.getNextSibling() as? TextNode else {
      return false
    }
    return try
      !(after.isSimpleText() && !isTargetNode(after)
      && after.getFormat() == entity.getFormat() && after.getStyle() == entity.getStyle()
      && after.getDetail() == entity.getDetail())
  }

  // Whether `entity`, right after a simple text node with `textBefore`, is
  // an entity the transforms keep: the first match from `textBefore` on
  // starts where it does, with the text after it in its source node (the
  // simple text node after it in the same format, style and detail, which
  // it was split from), as the check that made it an entity found it; and
  // that match still holds with the text of any other text node after it,
  // as the check that keeps a match at the end of a node from becoming an
  // entity found it. When `textBefore` has too many matches to tell,
  // `ifUnsure` is returned.
  func isEntityAfter(_ textBefore: String, _ entity: TextNode, _ ifUnsure: Bool) throws -> Bool {
    func holds(_ result: Bool?) -> Bool { result ?? ifUnsure }
    let entityText = try entity.getTextContent()
    guard let after = try entity.getNextSibling() as? TextNode else {
      return holds(getFirstMatchAfter(textBefore, entityText))
    }
    let isSuffix = try !isTextAfterOutsideSource(entity)
    let withAfter = getFirstMatchAfter(textBefore, entityText + (try after.getTextContent()))
    if isSuffix {
      return holds(withAfter)
    }
    return holds(withAfter) && holds(getFirstMatchAfter(textBefore, entityText))
  }

  // Checks the entity after a simple text node whose transform returned
  // before its loop, which checks that node: it is turned back into text
  // unless the first match from this node's text on, with the text after
  // the entity, starts where the entity does, as it must for a match at the
  // start of a node to be made an entity.
  func checkNextSibling(_ node: TextNode, _ text: String) throws {
    guard let nextSibling = try node.getNextSibling() as? TextNode, isTargetNode(nextSibling)
    else {
      return
    }
    let entityText = try nextSibling.getTextContent()
    // Leave an entity that its own transform is still splitting or turning
    // back into text to that transform.
    guard let entityMatch = getMatch(entityText), entityMatch.start == 0,
      entityMatch.end == entityText.utf16.count
    else {
      return
    }
    if try !isEntityAfter(text, nextSibling, true) {
      try replaceWithSimpleText(nextSibling)
    }
  }

  func textNodeTransform(_ node: TextNode) throws {
    if try !node.isSimpleText() {
      return
    }
    // Text split off the end of an entity in this update: its pieces do
    // not give their NodeState to the entities made from them
    let splitOffKeys: KeySet? =
      splitOff(editor).keys.contains(node.getKey()) ? splitOff(editor) : nil

    var prevSibling = try node.getPreviousSibling()
    var text = try node.getTextContent()
    let nodeText = text
    var currentNode: TextNode? = node
    // The first match of the previous text node's text and this node's.
    var firstMatch: EntityMatch?

    if let prev = prevSibling as? TextNode {
      let previousText = try prev.getTextContent()
      let previousLength = previousText.utf16.count
      let combinedText = previousText + text
      let prevMatch = getMatch(combinedText)
      firstMatch = prevMatch

      if isTargetNode(prev) {
        let turnBack: Bool
        if let prevMatch {
          // A token or segmented entity followed by text is turned back
          // into text, unless the same entity was already turned back into
          // text twice for this pending editor state: the transforms would
          // only make it again and turn it back forever. It is kept then
          // (below).
          turnBack =
            try getMode(prev) != 0
            && !(prevMatch.start == 0 && prevMatch.end >= previousLength
              && wasTurnedIntoText(prev))
        } else {
          turnBack = true
        }
        if turnBack {
          try replaceWithSimpleText(prev)
          return
        }
        let diff = prevMatch!.end - previousLength
        // Whether the entity takes the first `diff` characters of this node
        var grows = diff > 0
        if grows, let textBefore = try prev.getPreviousSibling() as? TextNode {
          if textBefore.isTextEntity() {
            // Two entities next to each other: the entities' own transforms
            // turn them back into text (or keep them as they are); this one
            // does not take this node's text first.
            grows = false
          } else if try textBefore.isSimpleText() {
            let before = try textBefore.getTextContent()
            if try !isEntityAfter(before, prev, true) {
              // An entity that the transforms do not keep after the text
              // before it is turned back into text rather than grown over
              // this node first (which would give this node's characters
              // its format and NodeState before it turns into text anyway).
              try replaceWithSimpleText(prev)
              return
            }
            if try !isEntityAfter(before, prev, false) {
              // Not sure (too many matches in the text before it to tell):
              // it stays as it is, without taking this node's text.
              grows = false
            }
          }
        }
        // A token or segmented entity kept above takes the text its match
        // extends into, as an entity in normal mode does, only when that
        // text would merge with it (the same format, style, detail and
        // NodeState); text with other NodeState or format stays apart,
        // rather than take the entity's (or make it drop its own).
        if try grows && (getMode(prev) == 0 || wouldMerge(prev, node)) {
          let concatText = text.jsSlice(0, diff)
          let newTextContent = previousText + concatText
          if try getMode(prev) == 0 {
            try prev.select()
          } else {
            // A token or segmented entity kept above: the selection stays
            // where it was in the text
            try moveSelectionIntoEntity(node, prev, diff)
          }
          // The entity takes the first `diff` characters of this node.
          try prev.setTextContent(newTextContent)
          if diff == text.utf16.count {
            try node.remove()
          } else {
            let remainingText = text.jsSlice(diff)
            try node.setTextContent(remainingText)
          }
          return
        }
      } else if prevMatch == nil || prevMatch!.start < previousLength {
        try checkNextSibling(node, text)
        return
      }
    }

    var prevMatchLengthToSkip = 0

    while true {
      let remainingText = text
      let match = getMatch(remainingText)
      let nextText = match.map { remainingText.jsSlice($0.end) } ?? ""
      text = nextText

      if nextText == "" {
        if let nextSibling = try currentNode!.getNextSibling() as? TextNode {
          // The match ends where this node ends, so the next sibling's text
          // may extend, shorten or invalidate it. Re-run getMatch over the
          // text that is still to be processed plus the sibling's text and
          // bail out unless the same match is found at the same offset.
          // Matches that were already replaced (or skipped) are excluded
          // from `remainingText`, so the comparison is against
          // `match.start` rather than 0.
          let combinedText = remainingText + (try nextSibling.getTextContent())
          let nextMatch = getMatch(combinedText)
          if nextMatch == nil {
            if isTargetNode(nextSibling) {
              // Unless the transforms keep it there, which they do for a
              // match that needs the text after it in its source node.
              if try !isEntityAfter(remainingText, nextSibling, false) {
                try replaceWithSimpleText(nextSibling)
              }
            } else {
              try nextSibling.markDirty()
            }
            return
          } else if match == nil || nextMatch!.start != match!.start {
            return
          }
        }
      }

      guard let match else {
        return
      }

      if match.start == 0, let prev = prevSibling as? TextNode {
        var skip = prev.isTextEntity()
        if !skip, try prev.isSimpleText(), match.end > 0 {
          // The transform of a simple text node checks the entity after it
          // with its own text in front, and turns it back into text if that
          // is no match. Leave a match there as text (and any match right
          // after it) rather than make an entity that check undoes, which
          // would make the two transforms undo each other forever (#9637).
          if prevMatchLengthToSkip > 0 {
            skip = true
          } else {
            let prevSize = try prev.getTextContentSize()
            skip =
              try getMatch(prev.getTextContent() + remainingText.jsSlice(0, match.end)) == nil
              // Unless it is this match with the text after it too.
              && !(firstMatch != nil && firstMatch!.start == prevSize
                && firstMatch!.end == prevSize + match.end)
          }
        }
        if skip {
          prevMatchLengthToSkip += match.end
          continue
        }
      }

      // The offsets of the match in the text the node had (`currentNode` is
      // what is left of its end)
      let base = try nodeText.utf16.count - currentNode!.getTextContentSize()
      let (matchStart, matchEnd) =
        match.start == 0
        ? (base, base + match.end)
        : (base + match.start + prevMatchLengthToSkip, base + match.end + prevMatchLengthToSkip)
      var nodeToReplace: TextNode?
      if match.start == 0 {
        let parts = try currentNode!.splitText(match.end)
        nodeToReplace = parts.first
        currentNode = parts.count > 1 ? parts[1] : nil
      } else {
        let parts = try currentNode!.splitText(
          match.start + prevMatchLengthToSkip, match.end + prevMatchLengthToSkip)
        nodeToReplace = parts.count > 1 ? parts[1] : nil
        currentNode = parts.count > 2 ? parts[2] : nil
      }
      if let splitOffKeys, let currentNode {
        splitOffKeys.keys.insert(currentNode.getKey())
      }

      guard let nodeToReplace else {
        throw LexicalError(
          "nodeToReplace should not be undefined. You may want to check splitOffsets passed to the splitText."
        )
      }

      let replacementNode = try createNode(nodeToReplace)
      try replacementNode
        .setFormat(nodeToReplace.getFormat())
        .setStyle(nodeToReplace.getStyle())
        .setDetail(nodeToReplace.getDetail())
      if try splitOffKeys == nil && !isInsertedText(editor, node, nodeText, matchStart, matchEnd) {
        // The NodeState of the text becomes the entity's
        try carryNodeState(nodeToReplace, replacementNode)
      }
      try nodeToReplace.replace(replacementNode)

      if currentNode == nil {
        return
      }
      prevMatchLengthToSkip = 0
      prevSibling = replacementNode
    }
  }

  func reverseNodeTransform(_ node: T) throws {
    let text = try node.getTextContent()
    guard let match = getMatch(text), match.start == 0 else {
      try replaceWithSimpleText(node)
      return
    }

    if text.utf16.count > match.end {
      // This will split out the rest of the text as simple text
      let parts = try node.splitText(match.end)
      if parts.count > 1 {
        splitOff(editor).keys.insert(parts[1].getKey())
      }
      return
    }

    let prevSibling = try node.getPreviousSibling() as? TextNode

    if let prevSibling, try prevSibling.isSimpleText(),
      // The text node before it checks it on its own while it is a match
      // after its text.
      try !isNextTextEntity(node),
      try !isEntityAfter(prevSibling.getTextContent(), node, true)
    {
      // The text after this entity may have changed so that the transforms
      // no longer keep it after the text before it: the transform of the
      // text node before it turns it back into text then. Once per pending
      // editor state, as the transforms of other entities may leave it
      // dirty again.
      let marked = markedInUpdate.get(editor)
      if !marked.keys.contains(prevSibling.getKey()) {
        marked.keys.insert(prevSibling.getKey())
        try prevSibling.markDirty()
      }
    }

    // Two entities next to each other are turned back into text, unless
    // this entity was already turned back into text for that twice for this
    // pending editor state: entities of two registrations that touch would
    // be made again and turned back forever. They are kept then.
    if let prevSibling, prevSibling.isTextEntity(), try !wasTurnedIntoText(node, "adjacent:") {
      try replaceWithSimpleText(prevSibling)
      try replaceWithSimpleText(node)
    }

    if let nextSibling = try node.getNextSibling() as? TextNode, nextSibling.isTextEntity(),
      // This may have already been converted in the previous block
      try !isTargetNode(node) || !wasTurnedIntoText(node, "adjacent:")
    {
      try replaceWithSimpleText(nextSibling)
      if isTargetNode(node) {
        try replaceWithSimpleText(node)
      }
    }
  }

  let removePlainTextTransform = try editor.registerNodeTransform(
    TextNode.self, textNodeTransform)
  let removeReverseNodeTransform = try editor.registerNodeTransform(
    targetNode, reverseNodeTransform)
  return [removePlainTextTransform, removeReverseNodeTransform]
}
