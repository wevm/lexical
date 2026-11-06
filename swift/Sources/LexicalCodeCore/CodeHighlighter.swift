import Lexical
import LexicalExtension

// Port of the highlighting loop of
// packages/lexical-code-prism/src/CodeHighlighterPrism.ts, with the lazy
// languages of packages/lexical-code-shiki/src/CodeHighlighterShiki.ts,
// over any `CodeTokenizer` rather than Prism: a code block's text is
// re-tokenized when it changes, only the children that differ from the new
// tokens are replaced, and a caret inside the block keeps its text offset.

/// Gives `to` the NodeState of `from`, flat states included, keeping the
/// state `to` has for keys `from` does not set (`$copyNodeState`).
private func copyNodeState(_ from: LexicalNode, _ to: LexicalNode) throws {
  guard let json = try from.__state?.toJSON(), !json.keys.isEmpty else { return }
  let state = try getWritableNodeState(to)
  for (key, value) in json {
    if key == NODE_STATE_KEY, let nested = value.objectValue {
      for (nestedKey, nestedValue) in nested { state.updateFromUnknown(nestedKey, nestedValue) }
    } else {
      state.updateFromUnknown(key, value)
    }
  }
}

/// `TransformState`. `nodesCurrentlyHighlighting` keeps the CodeNode and
/// text transforms, which both run when a token is inserted, from
/// highlighting a block twice in one update.
private final class TransformState {
  var didTransform = false
  var nodesCurrentlyHighlighting = Set<NodeKey>()
  /// The blocks waiting for each language that is loading, so a language
  /// is loaded once however many transforms ask for it.
  var waitingForLanguage: [String: Set<NodeKey>] = [:]
}

private func textNodeTransform(
  _ editor: Editor, _ tokenizer: any CodeTokenizer, _ transformState: TransformState,
  _ node: TextNode
) throws {
  // A CodeNode's children are flat, so only the parent needs checking.
  if let parentNode = try node.getParent() as? CodeNode {
    try codeNodeTransform(editor, tokenizer, transformState, parentNode)
  } else if node is CodeHighlightNode {
    // A code block turned into another block: its tokens are text again,
    // keeping their NodeState (such as comment ids).
    let text = try createTextNode(node.__text)
    try copyNodeState(node, text)
    try node.replace(text)
  }
}

private func codeNodeTransform(
  _ editor: Editor, _ tokenizer: any CodeTokenizer, _ transformState: TransformState,
  _ node: CodeNode
) throws {
  let nodeKey = node.getKey()
  // A new block may have no language. A tokenizer with no default leaves
  // it unset, so Markdown round-trips a fence without an info string.
  if try node.getLanguage() == nil, let defaultLanguage = tokenizer.defaultLanguage {
    try node.setLanguage(defaultLanguage)
  }
  let language = try nonEmpty(node.getLanguage()) ?? tokenizer.defaultLanguage
  if let language {
    switch tokenizer.languageState(language) {
    case .loaded:
      if try !node.getIsSyntaxHighlightSupported() { try node.setIsSyntaxHighlightSupported(true) }
    case .loading:
      // The grammar is on its way (Shiki's `loadCodeLanguage`): the block
      // is highlighted once it has loaded.
      if transformState.waitingForLanguage[language] != nil {
        transformState.waitingForLanguage[language]!.insert(nodeKey)
        return
      }
      transformState.waitingForLanguage[language] = [nodeKey]
      tokenizer.loadLanguage(language) { [weak editor] in
        let keys = transformState.waitingForLanguage.removeValue(forKey: language) ?? []
        try? editor?.update(tag: HISTORY_MERGE_TAG) {
          for key in keys {
            guard let codeNode = try getNodeByKey(key) as? CodeNode,
              try nonEmpty(codeNode.getLanguage()) ?? tokenizer.defaultLanguage == language
            else { continue }
            try codeNode.setIsSyntaxHighlightSupported(true)
            try codeNode.markDirty()
          }
        }
      }
      return
    case .unsupported:
      // An unknown language ("```After ") falls through to plain tokens,
      // so the indent and line-move handlers can work on it.
      if try node.getIsSyntaxHighlightSupported() { try node.setIsSyntaxHighlightSupported(false) }
    }
  } else if try node.getIsSyntaxHighlightSupported() {
    try node.setIsSyntaxHighlightSupported(false)
  }

  if transformState.nodesCurrentlyHighlighting.contains(nodeKey) { return }
  transformState.nodesCurrentlyHighlighting.insert(nodeKey)
  if !transformState.didTransform {
    transformState.didTransform = true
    try onUpdate {
      transformState.didTransform = false
      transformState.nodesCurrentlyHighlighting.removeAll()
    }
  }

  try updateAndRetainSelection(nodeKey) {
    guard let currentNode = try getNodeByKey(nodeKey) as? CodeNode, try currentNode.isAttached()
    else { return false }
    let currentLanguage = try nonEmpty(currentNode.getLanguage()) ?? tokenizer.defaultLanguage
    let isLoaded = currentLanguage.map { tokenizer.languageState($0) == .loaded } ?? false
    let children = try currentNode.getChildren()
    // A tokenizer that builds the nodes itself (`$tokenize`).
    if isLoaded, let highlightNodes = try tokenizer.tokenize(codeNode: currentNode, currentLanguage)
    {
      let (from, to, nodesForReplacement) = getDiffRange(children, highlightNodes)
      if from != to || !nodesForReplacement.isEmpty {
        try node.splice(from, to - from, nodesForReplacement)
        return true
      }
      return false
    }
    // Otherwise the tokens are compared with the children first, and only
    // those that differ become nodes.
    let code = try currentNode.getTextContent()
    let runs = try codeRuns(
      isLoaded && currentLanguage != nil
        ? tokenizer.tokenize(code, currentLanguage!) : [.text(code)])
    let (from, to, runsForReplacement) = getDiffRange(children, runs)
    if from != to || !runsForReplacement.isEmpty {
      try node.splice(from, to - from, runsForReplacement.map { try $0.createNode() })
      return true
    }
    return false
  }
}

/// `$updateAndRetainSelection`: runs `updateFn`, and puts a caret that was
/// in the code block back at the same text offset.
private func updateAndRetainSelection(
  _ nodeKey: NodeKey, _ updateFn: () throws -> Bool
) throws {
  guard let node = try getNodeByKey(nodeKey) as? CodeNode, try node.isAttached() else { return }
  // Only a range selection inside this block is retained: restoring one
  // elsewhere would drag the caret into the block.
  guard let selection = try getSelection() as? RangeSelection else {
    _ = try updateFn()
    return
  }
  let anchor = selection.anchor
  let anchorNode = try anchor.getNode()
  if anchorNode !== node, try !node.isParentOf(anchorNode) {
    _ = try updateFn()
    return
  }
  let anchorOffset = anchor.offset
  let isNewLineAnchor =
    try anchor.type == .element && node.getChildAtIndex(anchorOffset - 1) is LineBreakNode
  var textOffset = 0
  // The text before the anchor in the block, and its own offset.
  if !isNewLineAnchor {
    textOffset = anchorOffset
    for sibling in try anchorNode.getPreviousSiblings() {
      textOffset += try sibling.getTextContentSize()
    }
  }

  if try !updateFn() { return }

  // Only a line break puts the anchor on the element.
  if isNewLineAnchor {
    _ = try (anchor.getNode() as? ElementNode)?.select(anchorOffset, anchorOffset)
    return
  }
  // Walk to the text offset. A LineBreakNode takes one unit but holds no
  // text point, so an offset that lands on one is an element point.
  let children = try node.getChildren()
  for (index, child) in children.enumerated() {
    if let child = child as? TextNode {
      let textContentSize = try child.getTextContentSize()
      if textContentSize >= textOffset {
        _ = try child.select(textOffset, textOffset)
        return
      }
      textOffset -= textContentSize
    } else if child is LineBreakNode {
      if textOffset == 0 {
        _ = try node.select(index, index)
        return
      }
      textOffset -= 1
    }
  }
  _ = try node.select(children.count, children.count)
}

/// `getDiffRange`: the range `from..<to` of `prevNodes` to replace with
/// the part of `nextNodes` that differs.
private func getDiffRange<Next>(
  _ prevNodes: [LexicalNode], _ nextNodes: [Next], _ isEqual: (LexicalNode, Next) -> Bool
) -> (from: Int, to: Int, nodesForReplacement: [Next]) {
  var leadingMatch = 0
  while leadingMatch < prevNodes.count, leadingMatch < nextNodes.count,
    isEqual(prevNodes[leadingMatch], nextNodes[leadingMatch])
  {
    leadingMatch += 1
  }
  let maxTrailingMatch = min(prevNodes.count, nextNodes.count) - leadingMatch
  var trailingMatch = 0
  while trailingMatch < maxTrailingMatch,
    isEqual(
      prevNodes[prevNodes.count - trailingMatch - 1],
      nextNodes[nextNodes.count - trailingMatch - 1])
  {
    trailingMatch += 1
  }
  return (
    leadingMatch, prevNodes.count - trailingMatch,
    Array(nextNodes[leadingMatch..<(nextNodes.count - trailingMatch)])
  )
}

private func getDiffRange(
  _ prevNodes: [LexicalNode], _ nextNodes: [LexicalNode]
) -> (from: Int, to: Int, nodesForReplacement: [LexicalNode]) {
  getDiffRange(prevNodes, nextNodes, isEqual)
}

private func getDiffRange(
  _ prevNodes: [LexicalNode], _ runs: [CodeRun]
) -> (from: Int, to: Int, nodesForReplacement: [CodeRun]) {
  getDiffRange(prevNodes, runs) { $1.matches($0) }
}

/// `isEqual`: CodeHighlightNodes of the same text and type, tabs and line
/// breaks. Any other text node differs, so it becomes a token.
private func isEqual(_ nodeA: LexicalNode, _ nodeB: LexicalNode) -> Bool {
  if let a = nodeA as? CodeHighlightNode, let b = nodeB as? CodeHighlightNode {
    return a.__text == b.__text && a.__highlightType == b.__highlightType
  }
  return (nodeA is TabNode && nodeB is TabNode)
    || (nodeA is LineBreakNode && nodeB is LineBreakNode)
}

/// `registerHighlightingOnly`: the highlighting transforms, without the
/// indent handlers of `registerCodeIndentation`.
public func registerHighlightingOnly(
  _ editor: Editor, _ tokenizer: any CodeTokenizer
) throws -> () -> Void {
  let transformState = TransformState()
  return mergeRegister(
    try editor.registerNodeTransform(CodeNode.self) { [unowned editor] in
      try codeNodeTransform(editor, tokenizer, transformState, $0)
    },
    try editor.registerNodeTransform(TextNode.self) { [unowned editor] in
      try textNodeTransform(editor, tokenizer, transformState, $0)
    },
    try editor.registerNodeTransform(CodeHighlightNode.self) { [unowned editor] in
      try textNodeTransform(editor, tokenizer, transformState, $0)
    })
}

/// `registerCodeHighlighting`: highlighting with `tokenizer` and the
/// indent and arrow-key handlers, for code that does not use
/// `CodeHighlighterExtension`.
public func registerCodeHighlighting(
  _ editor: Editor,
  _ tokenizer: any CodeTokenizer = PlainCodeTokenizer(
    defaultLanguage: DEFAULT_CODE_LANGUAGE)
) throws -> () -> Void {
  guard editor.hasNodes([CodeNode.self, CodeHighlightNode.self]) else {
    throw LexicalError(
      "CodeHighlightPlugin: CodeNode or CodeHighlightNode not registered on editor")
  }
  return mergeRegister(
    try registerHighlightingOnly(editor, tokenizer), registerCodeIndentation(editor))
}

/// `CodePrismConfig` / `CodeShikiConfig`.
public struct CodeHighlighterConfig {
  /// When true, the highlighter is not registered. Flipping the signal at
  /// runtime switches it on or off without rebuilding the editor.
  public var disabled: Bool
  /// The engine that tokenizes code.
  public var tokenizer: any CodeTokenizer

  public init(
    disabled: Bool = false,
    tokenizer: any CodeTokenizer = PlainCodeTokenizer(defaultLanguage: DEFAULT_CODE_LANGUAGE)
  ) {
    self.disabled = disabled
    self.tokenizer = tokenizer
  }
}

/// `CodePrismExtension`, over the configured `tokenizer`: highlights code
/// blocks. `CodeExtension` registers the nodes, and `CodeIndentExtension`
/// brings Tab, Shift+Tab and the other keyboard handlers.
public let CodeHighlighterExtension = defineExtension(
  name: "@lexical/code-highlighter", config: safeCast(CodeHighlighterConfig()),
  dependencies: [CodeExtension, CodeIndentExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let stores = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if stores.disabled.value { return nil }
      return try registerHighlightingOnly(editor, stores.tokenizer.value)
    }
  })
