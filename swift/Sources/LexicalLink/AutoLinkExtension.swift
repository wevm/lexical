import Lexical
import LexicalExtension

// Port of packages/lexical-link/src/LexicalAutoLinkExtension.ts. Offsets
// are in UTF-16 code units, as in JS.

/// `ChangeHandler`: called when an autolink is created, updated or removed,
/// with the new `url` and the `prevUrl` (`nil` when a link is added or
/// removed).
public typealias ChangeHandler = (_ url: String?, _ prevUrl: String?) -> Void

/// `LinkMatcherResult`. `index` and `length` are in UTF-16 code units.
public struct LinkMatcherResult {
  public var attributes: AutoLinkAttributes?
  public var index: Int
  public var length: Int
  public var text: String
  public var url: String

  public init(
    attributes: AutoLinkAttributes? = nil, index: Int, length: Int, text: String, url: String
  ) {
    self.attributes = attributes
    self.index = index
    self.length = length
    self.text = text
    self.url = url
  }
}

/// `LinkMatcher`: the first link in `text`, or `nil`.
public typealias LinkMatcher = (_ text: String) -> LinkMatcherResult?

/// `regExp.exec(text)`: the first match, with its UTF-16 index. Matching
/// is by Unicode scalar, as a JS `u` regex matches by code point.
private func exec(_ regex: Regex<AnyRegexOutput>, _ text: String) -> (index: Int, text: String)? {
  guard let m = text.firstMatch(of: regex.matchingSemantics(.unicodeScalar)) else { return nil }
  return (text.utf16.distance(from: text.startIndex, to: m.range.lowerBound), String(m.0))
}

/// `regExp.test(value)`.
private func test(_ regex: Regex<AnyRegexOutput>, _ value: String) -> Bool {
  value.firstMatch(of: regex.matchingSemantics(.unicodeScalar)) != nil
}

/// `createLinkMatcherWithRegExp(regExp, urlTransformer)`: a `LinkMatcher`
/// for the first match of `regExp`, whose URL is the matched text, as
/// `urlTransformer` rewrites it.
public func createLinkMatcherWithRegExp(
  _ regExp: Regex<AnyRegexOutput>, _ urlTransformer: @escaping (String) -> String = { $0 }
) -> LinkMatcher {
  { text in
    guard let match = exec(regExp, text) else { return nil }
    return LinkMatcherResult(
      index: match.index, length: match.text.utf16.count, text: match.text,
      url: urlTransformer(match.text))
  }
}

nonisolated(unsafe) private let URL_REGEX = try! Regex(
  #"((https?:\/\/(www\.)?)|(www\.))[-\p{L}\p{N}@:%._+~#=]{1,256}\.[\p{L}\p{N}]{1,6}(?:[-\p{L}\p{N}()@:%_+.~#?&\/\/=]*[\p{L}\p{N}()@_~#?&\/\/=])?"#
)

// JS reads `[` inside a class as a literal; Swift starts a nested class, so
// it is escaped here.
nonisolated(unsafe) private let EMAIL_REGEX = try! Regex(
  #"(([^<>()\[\]\\.,;:\s@"]{1,64}(\.[^<>()\[\]\\.,;:\s@"]{1,64}){0,63})|(".{1,255}"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]{1,63}\.){1,127}[a-zA-Z]{2,63}))"#
)

/// `autoLinkUrlMatcher`: URLs with full Unicode support, balancing
/// parentheses so that `https://en.wikipedia.org/wiki/Fish_(disambiguation)`
/// matches whole, even inside prose parentheses.
nonisolated(unsafe) public let autoLinkUrlMatcher: LinkMatcher = { text in
  guard let match = exec(URL_REGEX, text) else { return nil }
  var matched = match.text
  var depth = 0
  for ch in matched.unicodeScalars {
    if ch == "(" {
      depth += 1
    } else if ch == ")" {
      depth -= 1
    }
  }
  while depth < 0 && matched.hasSuffix(")") {
    matched.removeLast()
    depth += 1
  }
  return LinkMatcherResult(
    index: match.index, length: matched.utf16.count, text: matched,
    url: matched.hasPrefix("http") ? matched : "https://\(matched)")
}

/// `autoLinkEmailMatcher`: email addresses.
nonisolated(unsafe) public let autoLinkEmailMatcher: LinkMatcher = createLinkMatcherWithRegExp(
  EMAIL_REGEX
) {
  "mailto:\($0)"
}

private func findFirstMatch(_ text: String, _ matchers: [LinkMatcher]) -> LinkMatcherResult? {
  for matcher in matchers {
    if let match = matcher(text) {
      return match
    }
  }
  return nil
}

nonisolated(unsafe) private let PUNCTUATION_OR_SPACE = try! Regex(#"[.,;\s]"#)

/// `text[i]`: the UTF-16 code unit at `i` as a string, or JS `undefined`,
/// which `RegExp.test` reads as the string "undefined".
private func charAt(_ text: String, _ i: Int) -> String {
  guard i >= 0, i < text.utf16.count else { return "undefined" }
  let index = text.utf16.index(text.utf16.startIndex, offsetBy: i)
  return String(decoding: [text.utf16[index]], as: UTF16.self)
}

private func isSeparator(_ char: String, _ separatorRegex: Regex<AnyRegexOutput>) -> Bool {
  test(separatorRegex, char)
}

private func endsWithSeparator(
  _ textContent: String, _ separatorRegex: Regex<AnyRegexOutput>
)
  -> Bool
{
  isSeparator(charAt(textContent, textContent.utf16.count - 1), separatorRegex)
}

private func startsWithSeparator(
  _ textContent: String, _ separatorRegex: Regex<AnyRegexOutput>
)
  -> Bool
{
  isSeparator(charAt(textContent, 0), separatorRegex)
}

nonisolated(unsafe) private let EMAIL_TLD = try! Regex(#"^\.[a-zA-Z]{2,}"#)
nonisolated(unsafe) private let URL_TLD = try! Regex(#"^\.[a-zA-Z0-9]{1,}"#)

/// Whether the text can begin a top-level domain: a full stop and then
/// letters (or, for a URL, letters and digits).
private func startsWithTLD(_ textContent: String, _ isEmail: Bool) -> Bool {
  test(isEmail ? EMAIL_TLD : URL_TLD, textContent)
}

private func isPreviousNodeValid(
  _ node: LexicalNode, _ separatorRegex: Regex<AnyRegexOutput>
)
  throws -> Bool
{
  var previousNode = try node.getPreviousSibling()
  if let element = previousNode as? ElementNode {
    previousNode = try element.getLastDescendant()
  }
  guard let previousNode else { return true }
  if previousNode is LineBreakNode { return true }
  if let text = previousNode as? TextNode {
    return try endsWithSeparator(text.getTextContent(), separatorRegex)
  }
  return false
}

private func isNextNodeValid(
  _ node: LexicalNode, _ separatorRegex: Regex<AnyRegexOutput>
) throws
  -> Bool
{
  var nextNode = try node.getNextSibling()
  if let element = nextNode as? ElementNode {
    nextNode = try element.getFirstDescendant()
  }
  guard let nextNode else { return true }
  if nextNode is LineBreakNode { return true }
  if let text = nextNode as? TextNode {
    return try startsWithSeparator(text.getTextContent(), separatorRegex)
  }
  return false
}

private func isContentAroundIsValid(
  _ matchStart: Int, _ matchEnd: Int, _ separatorRegex: Regex<AnyRegexOutput>, _ text: String,
  _ nodes: [TextNode]
) throws -> Bool {
  let contentBeforeIsValid =
    try matchStart > 0
    ? isSeparator(charAt(text, matchStart - 1), separatorRegex)
    : isPreviousNodeValid(nodes[0], separatorRegex)
  if !contentBeforeIsValid {
    return false
  }
  return try matchEnd < text.utf16.count
    ? isSeparator(charAt(text, matchEnd), separatorRegex)
    : isNextNodeValid(nodes[nodes.count - 1], separatorRegex)
}

private func extractMatchingNodes(
  _ nodes: [TextNode], _ startIndex: Int, _ endIndex: Int
) throws
  -> (
    matchingOffset: Int, unmodifiedBeforeNodes: [TextNode], matchingNodes: [TextNode],
    unmodifiedAfterNodes: [TextNode]
  )
{
  var unmodifiedBeforeNodes: [TextNode] = []
  var matchingNodes: [TextNode] = []
  var unmodifiedAfterNodes: [TextNode] = []
  var matchingOffset = 0
  var currentOffset = 0
  for currentNode in nodes {
    let currentNodeLength = try currentNode.getTextContent().utf16.count
    let currentNodeStart = currentOffset
    let currentNodeEnd = currentOffset + currentNodeLength
    if currentNodeEnd <= startIndex {
      unmodifiedBeforeNodes.append(currentNode)
      matchingOffset += currentNodeLength
    } else if currentNodeStart >= endIndex {
      unmodifiedAfterNodes.append(currentNode)
    } else {
      matchingNodes.append(currentNode)
    }
    currentOffset += currentNodeLength
  }
  return (matchingOffset, unmodifiedBeforeNodes, matchingNodes, unmodifiedAfterNodes)
}

private func copyTextFormat(_ to: TextNode, from: TextNode) throws {
  try to.setFormat(from.getFormat())
  try to.setDetail(from.getDetail())
  try to.setStyle(from.getStyle())
}

/// `$createAutoLinkNode_(nodes, startIndex, endIndex, match)`: wraps the
/// matched text of `nodes` in a new autolink, and returns the text node
/// after it, if any.
private func createAutoLinkNodeFor(
  _ nodes: [TextNode], _ startIndex: Int, _ endIndex: Int, _ match: LinkMatcherResult
) throws -> TextNode? {
  let linkNode = try createAutoLinkNode(match.url, match.attributes ?? AutoLinkAttributes())
  if nodes.count == 1 {
    var remainingTextNode: TextNode? = nodes[0]
    let linkTextNode: TextNode
    if startIndex == 0 {
      let parts = try nodes[0].splitText(endIndex)
      linkTextNode = parts[0]
      remainingTextNode = parts.count > 1 ? parts[1] : nil
    } else {
      let parts = try nodes[0].splitText(startIndex, endIndex)
      linkTextNode = parts[1]
      remainingTextNode = parts.count > 2 ? parts[2] : nil
    }
    let textNode = try createTextNode(match.text)
    try copyTextFormat(textNode, from: linkTextNode)
    try linkNode.append(textNode)
    try linkTextNode.replace(linkNode)
    return remainingTextNode
  } else if nodes.count > 1 {
    let firstTextNode = nodes[0]
    var offset = try firstTextNode.getTextContent().utf16.count
    let firstLinkTextNode: TextNode
    if startIndex == 0 {
      firstLinkTextNode = firstTextNode
    } else {
      firstLinkTextNode = try firstTextNode.splitText(startIndex)[1]
    }
    var linkNodes: [LexicalNode] = []
    var remainingTextNode: TextNode?
    for currentNode in nodes.dropFirst() {
      let currentNodeLength = try currentNode.getTextContent().utf16.count
      let currentNodeStart = offset
      let currentNodeEnd = offset + currentNodeLength
      if currentNodeStart < endIndex {
        if currentNodeEnd <= endIndex {
          linkNodes.append(currentNode)
        } else {
          let parts = try currentNode.splitText(endIndex - currentNodeStart)
          linkNodes.append(parts[0])
          remainingTextNode = parts.count > 1 ? parts[1] : nil
        }
      }
      offset += currentNodeLength
    }
    let selection = try getSelection()
    let selectedTextNode = try selection?.getNodes().first { $0 is TextNode }
    let textNode = try createTextNode(firstLinkTextNode.getTextContent())
    try copyTextFormat(textNode, from: firstLinkTextNode)
    try linkNode.append(contentsOf: [textNode] + linkNodes)
    // The caret is not kept when it was in the first text node, so it is
    // restored.
    if let selectedTextNode, selectedTextNode === firstLinkTextNode {
      if let selection = selection as? RangeSelection {
        try textNode.select(selection.anchor.offset, selection.focus.offset)
      } else if selection is NodeSelection {
        try textNode.select(0, textNode.getTextContent().utf16.count)
      }
    }
    try firstLinkTextNode.replace(linkNode)
    return remainingTextNode
  }
  return nil
}

private func isLinkedAutoLink(_ node: LexicalNode?) throws -> Bool {
  guard let link = node as? AutoLinkNode else { return false }
  return try !link.getIsUnlinked()
}

private func handleLinkCreation(
  _ nodes: [TextNode], _ matchers: [LinkMatcher], _ onChange: ChangeHandler,
  _ separatorRegex: Regex<AnyRegexOutput>
) throws {
  // Idempotency: nothing to do when a node is already in an autolink.
  for node in nodes where try isLinkedAutoLink(node.getParent()) {
    return
  }
  var currentNodes = nodes
  let initialText = try currentNodes.map { try $0.getTextContent() }.joined()
  var text = initialText
  var invalidMatchEnd = 0

  while let match = findFirstMatch(text, matchers) {
    let matchStart = match.index
    let matchEnd = matchStart + match.length
    let isValid = try isContentAroundIsValid(
      invalidMatchEnd + matchStart, invalidMatchEnd + matchEnd, separatorRegex, initialText,
      currentNodes)
    if isValid {
      let (matchingOffset, _, matchingNodes, unmodifiedAfterNodes) = try extractMatchingNodes(
        currentNodes, invalidMatchEnd + matchStart, invalidMatchEnd + matchEnd)
      // Skip when the matching nodes are already in an autolink.
      var alreadyLinked = false
      for node in matchingNodes where try isLinkedAutoLink(node.getParent()) {
        alreadyLinked = true
        break
      }
      if alreadyLinked {
        invalidMatchEnd += matchEnd
        text = text.jsSlice(matchEnd)
        continue
      }
      let actualMatchStart = invalidMatchEnd + matchStart - matchingOffset
      let actualMatchEnd = invalidMatchEnd + matchEnd - matchingOffset
      let remainingTextNode = try createAutoLinkNodeFor(
        matchingNodes, actualMatchStart, actualMatchEnd, match)
      currentNodes =
        remainingTextNode.map { [$0] + unmodifiedAfterNodes } ?? unmodifiedAfterNodes
      onChange(match.url, nil)
      invalidMatchEnd = 0
    } else {
      invalidMatchEnd += matchEnd
    }
    text = text.jsSlice(matchEnd)
  }
}

private func handleLinkEdit(
  _ linkNode: AutoLinkNode, _ matchers: [LinkMatcher], _ onChange: ChangeHandler,
  _ separatorRegex: Regex<AnyRegexOutput>
) throws {
  // The children must be simple text.
  for child in try linkNode.getChildren() {
    guard let text = child as? TextNode, try text.isSimpleText() else {
      try replaceWithChildren(linkNode)
      onChange(nil, try linkNode.getURL())
      return
    }
  }
  // The text content must match whole.
  let text = try linkNode.getTextContent()
  guard let match = findFirstMatch(text, matchers), match.text == text else {
    try replaceWithChildren(linkNode)
    onChange(nil, try linkNode.getURL())
    return
  }
  // The neighbors must be valid.
  if try !isPreviousNodeValid(linkNode, separatorRegex)
    || !isNextNodeValid(linkNode, separatorRegex)
  {
    try replaceWithChildren(linkNode)
    onChange(nil, try linkNode.getURL())
    return
  }
  let url = try linkNode.getURL()
  if url != match.url {
    try linkNode.setURL(match.url)
    onChange(match.url, url)
  }
  if let attributes = match.attributes {
    // Kept in sync with the matcher without reporting a URL change that did
    // not happen. JS compares with `!==`, where an absent attribute is
    // `undefined` and never equals the node's `null`, and writes
    // `value || null`.
    func refresh(_ current: String?, _ value: String??, _ set: (String?) throws -> Void) throws {
      if value == nil || current != value! {
        let new = value ?? nil
        try set(new?.isEmpty == false ? new : nil)
      }
    }
    try refresh(linkNode.getRel(), attributes.rel) { try linkNode.setRel($0) }
    try refresh(linkNode.getTarget(), attributes.target) { try linkNode.setTarget($0) }
    try refresh(linkNode.getTitle(), attributes.title) { try linkNode.setTitle($0) }
  }
}

/// Edits in neighboring nodes that make an autolink invalid. Given the
/// creation preconditions, these can only be simple text nodes.
private func handleBadNeighbors(
  _ textNode: TextNode, _ matchers: [LinkMatcher], _ onChange: ChangeHandler,
  _ separatorRegex: Regex<AnyRegexOutput>
) throws {
  let parent = try textNode.getParent()
  let previousSibling = try textNode.getPreviousSibling()
  let nextSibling = try textNode.getNextSibling()
  let text = try textNode.getTextContent()

  // handleLinkEdit on the parent unwraps it if needed.
  if try isLinkedAutoLink(parent) {
    return
  }
  // Text added after an autolink.
  if let previousSibling = previousSibling as? AutoLinkNode, try !previousSibling.getIsUnlinked() {
    // Only while it is still a sibling, to prevent loops.
    if try previousSibling.is(textNode.getPreviousSibling()),
      try textNode.getParent()?.is(previousSibling.getParent())
        ?? (previousSibling.getParent() == nil)
    {
      if !startsWithSeparator(text, separatorRegex) {
        try replaceWithChildren(previousSibling)
        onChange(nil, try previousSibling.getURL())
        return
      }
      if try startsWithTLD(text, previousSibling.isEmailURI()) {
        let combinedText = try previousSibling.getTextContent() + text
        if let match = findFirstMatch(combinedText, matchers), match.text == combinedText {
          try previousSibling.append(textNode)
          try handleLinkEdit(previousSibling, matchers, onChange, separatorRegex)
          onChange(nil, try previousSibling.getURL())
        }
      }
    }
  }
  // Text added before an autolink.
  if let nextSibling = nextSibling as? AutoLinkNode, try !nextSibling.getIsUnlinked(),
    !endsWithSeparator(text, separatorRegex)
  {
    if try nextSibling.is(textNode.getNextSibling()),
      try textNode.getParent()?.is(nextSibling.getParent()) ?? (nextSibling.getParent() == nil)
    {
      try replaceWithChildren(nextSibling)
      onChange(nil, try nextSibling.getURL())
    }
  }
}

@discardableResult
private func replaceWithChildren(_ node: ElementNode) throws -> [LexicalNode] {
  let children = try node.getChildren()
  for child in children.reversed() {
    try node.insertAfter(child)
  }
  try node.remove()
  return try children.map { try $0.getLatest() }
}

nonisolated(unsafe) private let WHITESPACE = try! Regex(#"[\s]"#)

private func getTextNodesToMatch(_ textNode: TextNode) throws -> [TextNode] {
  // The next siblings that are simple text, up to one containing a space.
  var textNodesToMatch = [textNode]
  var nextSibling = try textNode.getNextSibling()
  while let next = nextSibling as? TextNode, try next.isSimpleText() {
    textNodesToMatch.append(next)
    if try test(WHITESPACE, next.getTextContent()) {
      break
    }
    nextSibling = try next.getNextSibling()
  }
  return textNodesToMatch
}

/// `AutoLinkConfig`.
public struct AutoLinkConfig {
  public var changeHandlers: [ChangeHandler]
  public var excludeParents: [(ElementNode) throws -> Bool]
  public var matchers: [LinkMatcher]
  /// Whether a character around a match counts as a separator when
  /// validating its boundaries. Defaults to `/[.,;\s]/`.
  public var separatorRegex: Regex<AnyRegexOutput>

  public init(
    changeHandlers: [ChangeHandler] = [], excludeParents: [(ElementNode) throws -> Bool] = [],
    matchers: [LinkMatcher] = [], separatorRegex: Regex<AnyRegexOutput>? = nil
  ) {
    self.changeHandlers = changeHandlers
    self.excludeParents = excludeParents
    self.matchers = matchers
    self.separatorRegex = separatorRegex ?? PUNCTUATION_OR_SPACE
  }
}

/// `registerAutoLink(editor, config)`: links text that `matchers` match as
/// it is typed, keeps autolinks in step with edits, and toggles an
/// autolink's `isUnlinked` on `TOGGLE_LINK_COMMAND` with `nil`.
public func registerAutoLink(
  _ editor: Editor, _ config: AutoLinkConfig = AutoLinkConfig()
) throws
  -> () -> Void
{
  let matchers = config.matchers
  let changeHandlers = config.changeHandlers
  let excludeParents = config.excludeParents
  let separatorRegex = config.separatorRegex
  let onChange: ChangeHandler = { url, prevUrl in
    for handler in changeHandlers {
      handler(url, prevUrl)
    }
  }
  return try mergeRegister(
    editor.registerNodeTransform(TextNode.self) { textNode in
      let parent = try textNode.getParentOrThrow()
      let previous = try textNode.getPreviousSibling()
      if let parent = parent as? AutoLinkNode {
        try handleLinkEdit(parent, matchers, onChange, separatorRegex)
      } else if !(parent is LinkNode), try !excludeParents.contains(where: { try $0(parent) }) {
        if try textNode.isSimpleText(),
          try startsWithSeparator(textNode.getTextContent(), separatorRegex)
            || !(previous is AutoLinkNode)
        {
          try handleLinkCreation(
            getTextNodesToMatch(textNode), matchers, onChange, separatorRegex)
        }
        try handleBadNeighbors(textNode, matchers, onChange, separatorRegex)
      }
    },
    editor.registerCommand(
      TOGGLE_LINK_COMMAND,
      { payload, _ in
        guard payload == nil, let selection = try getSelection() as? RangeSelection else {
          return false
        }
        for node in try selection.extract() {
          if let parent = try node.getParent() as? AutoLinkNode {
            // Invert the value.
            try parent.setIsUnlinked(!parent.getIsUnlinked())
            try parent.markDirty()
          }
        }
        return false
      },
      // Higher than TOGGLE_LINK_COMMAND in LinkExtension.
      COMMAND_PRIORITY_LOW))
}

/// `AutoLinkExtension`: makes `AutoLinkNode`s from text that the
/// configured `matchers` match. See `autoLinkUrlMatcher`,
/// `autoLinkEmailMatcher` and `createLinkMatcherWithRegExp`. Configured
/// `matchers`, `changeHandlers` and `excludeParents` concatenate.
public let AutoLinkExtension = defineExtension(
  name: "@lexical/link/AutoLink", config: AutoLinkConfig(),
  dependencies: [AutoLinkAnnounceExtension, LinkExtension],
  mergeConfig: { config, partial in
    var merged = partial.applied(to: config)
    if let matchers = partial.matchers { merged.matchers = config.matchers + matchers }
    if let changeHandlers = partial.changeHandlers {
      merged.changeHandlers = config.changeHandlers + changeHandlers
    }
    if let excludeParents = partial.excludeParents {
      merged.excludeParents = config.excludeParents + excludeParents
    }
    return merged
  },
  nodes: { [.klass(AutoLinkNode.self)] },
  register: { editor, config, _ in try registerAutoLink(editor, config) })
