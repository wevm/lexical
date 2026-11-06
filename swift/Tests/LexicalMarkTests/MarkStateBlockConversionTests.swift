import Lexical
import LexicalClipboard
import LexicalCodeCore
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalMark
import LexicalRichText
import LexicalSelection
import LexicalTesting
import Testing

// Port of packages/lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts.
//
// Converting or normalizing blocks keeps the ids of every character: no
// text is inserted or deleted, so the document read as a sequence of
// (character, ids) pairs is the same before and after, whichever block
// types are involved, wherever the comments are and wherever the selection
// is.
//
// The `highlighted-code` source and target highlight the code with
// `CodeHighlighterExtension` in place of `CodePrismExtension`. Prism's
// JavaScript grammar gives the converted text one plain token (and a typed
// `X` a `constant` token), which `MarkTestCodeTokenizer` reproduces; each
// highlighted case also runs with `splitWords`, which splits the text into
// a token per word, so that ids are also checked across several
// CodeHighlightNodes.

private enum Source: String, CaseIterable, Sendable {
  case paragraph, heading, quote, code
  case highlightedCode = "highlighted-code"
  case bullet, number, check
  case nestedBullet = "nested-bullet"
  case nestedNumber = "nested-number"
}

private enum Target: String, CaseIterable, Sendable {
  case paragraph, heading, quote, code
  case highlightedCode = "highlighted-code"
  case bullet, number, check
  case noList = "no-list"
}

/// A (source, target) pair: one upstream test of a `from`/`pasted` describe.
private struct Pair: Sendable, CustomTestStringConvertible {
  let source: Source
  let target: Target
  var testDescription: String { "\(source.rawValue) -> \(target.rawValue)" }
  static let all: [Pair] = Source.allCases.flatMap { s in
    Target.allCases.map { Pair(source: s, target: $0) }
  }
}

/// The converted text; `beta` is a link in the `link` content.
private let TEXT = "alpha beta gamma "
private let TEXT_LENGTH = TEXT.utf16.count

private enum Content: CaseIterable { case plain, link }

/// Comments as [id, start, end) offsets in TEXT, in upstream key order.
private let MARKS: [(String, [(String, Int, Int)])] = [
  ("all", [("c1", 0, TEXT_LENGTH)]),
  ("overlapping", [("c1", 3, 8), ("c2", 7, 14)]),
  ("part", [("c1", 6, 10)]),
  ("partial", [("c1", 7, 9)]),
]

/// - `alone`: nothing else in the document.
/// - `linedUp`: a list before it, of the target's type, whose only item is
///   `gamma ` (the end of the converted text): a list conversion merges
///   with it.
/// - `markedBefore`: that list's item is `old gamma ` with `c4`, and the
///   last character of the converted text has `c4` too.
/// - `markedAfter`: a list of the target's type after it, with `c5` on its
///   text, and `c5` on the first character of the converted text.
private enum Context: String, CaseIterable {
  case alone
  case linedUp = "lined-up"
  case markedBefore = "marked-before"
  case markedAfter = "marked-after"
}

/// Collapsed selections at offsets in TEXT, a range over it, and a range
/// from it into the neighbour.
private enum SelectionAt: String, CaseIterable {
  case at0 = "0"
  case at8 = "8"
  case at16 = "16"
  case at17 = "17"
  case range, across
}

/// The tokenizers a highlighted case runs with: none without highlighting.
private func tokenizers(_ highlighting: Bool) -> [MarkTestCodeTokenizer?] {
  highlighting ? [MarkTestCodeTokenizer(), MarkTestCodeTokenizer(splitWords: true)] : [nil]
}

private func buildEditor(_ tokenizer: MarkTestCodeTokenizer? = nil) throws -> Editor {
  let code: any AnyLexicalExtensionArgument =
    if let tokenizer {
      configExtension(CodeHighlighterExtension) { $0.tokenizer = tokenizer }
    } else {
      CodeExtension
    }
  return try buildEditorFromExtensions(
    defineExtension(
      name: "block-conversion",
      dependencies: [RichTextExtension, ListExtension, LinkExtension, code, MarkStateExtension],
      editorConfig: InitialEditorConfig(
        onError: { error, _ in throw error }, initialEditorState: InitialEditorStateType.none)))
}

/// The document as `character:ids` for every character, in order.
private func sequence(_ node: LexicalNode? = nil) throws -> [String] {
  let node: LexicalNode = try node ?? getRoot()
  if let text = node as? TextNode {
    let ids = try getNodeMarkIDs(text).joined(separator: ",")
    return try text.getTextContent().unicodeScalars.map { "\($0):\(ids)" }
  }
  if let element = node as? ElementNode {
    return try element.getChildren().flatMap { try sequence($0) }
  }
  return []
}

private func isCodeSource(_ source: Source) -> Bool {
  source == .code || source == .highlightedCode
}

private func listTypeOf(_ target: Target) -> ListType {
  target == .number || target == .check ? target.rawValue : "bullet"
}

private func content(_ content: Content) throws -> [LexicalNode] {
  content == .link
    ? [
      try createTextNode("alpha "),
      try createLinkNode("https://example.org").append(createTextNode("beta")),
      try createTextNode(" gamma "),
    ]
    : [try createTextNode(TEXT)]
}

/// Append the source block to the root, holding `children`, and return the
/// block that holds them.
private func appendSource(_ source: Source, _ children: [LexicalNode]) throws -> ElementNode {
  let root = try getRoot()
  switch source {
  case .paragraph, .heading, .quote, .code, .highlightedCode:
    let block: ElementNode =
      switch source {
      case .paragraph: try createParagraphNode()
      case .heading: try createHeadingNode("h1")
      case .quote: try createQuoteNode()
      default: try createCodeNode()
      }
    _ = try root.append(block.append(contentsOf: children))
    return block
  case .bullet, .number, .check:
    let item = try createListItemNode().append(contentsOf: children)
    _ = try root.append(createListNode(source.rawValue).append(item))
    return item
  case .nestedBullet, .nestedNumber:
    let type = source == .nestedBullet ? "bullet" : "number"
    let item = try createListItemNode().append(contentsOf: children)
    _ = try root.append(
      createListNode(type).append(
        createListItemNode().append(createTextNode("outer")),
        createListItemNode().append(createListNode(type).append(item))))
    return item
  }
}

/// The TextNode and offset at `offset` in the text of `block`.
private func locateIn(_ block: ElementNode, _ offset: Int) throws -> (TextNode, Int) {
  var remaining = offset
  let texts = try block.getAllTextNodes()
  for text in texts {
    let size = try text.getTextContentSize()
    if remaining < size {
      return (text, remaining)
    }
    remaining -= size
  }
  let last = texts[texts.count - 1]
  return (last, try last.getTextContentSize())
}

@discardableResult
private func selectIn(
  _ anchor: (ElementNode, Int), _ focus: (ElementNode, Int)
) throws
  -> RangeSelection
{
  let selection = createRangeSelection()
  let (anchorNode, anchorAt) = try locateIn(anchor.0, anchor.1)
  let (focusNode, focusAt) = try locateIn(focus.0, focus.1)
  try selection.anchor.set(anchorNode.getKey(), anchorAt, .text)
  try selection.focus.set(focusNode.getKey(), focusAt, .text)
  try setSelection(selection)
  return selection
}

private func markIn(_ block: ElementNode, _ id: String, _ start: Int, _ end: Int) throws {
  _ = try addMarkID(id, selectIn((block, start), (block, end)))
}

private func convert(_ target: Target) throws {
  switch target {
  case .paragraph: try setBlocksType(getSelection()) { try createParagraphNode() }
  case .heading: try setBlocksType(getSelection()) { try createHeadingNode("h2") }
  case .quote: try setBlocksType(getSelection()) { try createQuoteNode() }
  case .code, .highlightedCode: try setBlocksType(getSelection()) { try createCodeNode() }
  case .bullet, .number, .check: try insertList(target.rawValue)
  case .noList: try removeList()
  }
}

/// The nearest block of `node`, or `node` itself.
private func getBlockOf(_ node: LexicalNode) throws -> ElementNode {
  var current = node
  while true {
    if let element = current as? ElementNode, !(try element.isInline()) {
      return element
    }
    current = try current.getParentOrThrow()
  }
}

/// The type of a block, with the list type for a list item.
private func label(_ block: LexicalNode) throws -> String {
  if block is ListItemNode, let parent = try block.getParent() as? ListNode {
    return "listitem:\(try parent.getListType())"
  }
  return block.getType()
}

/// The label of the block that holds the converted text after `target`,
/// for text in a block labeled `from`.
private func expectedLabel(_ from: String, _ target: Target) -> String {
  switch target {
  case .bullet, .number, .check: "listitem:\(target.rawValue)"
  case .noList: from.hasPrefix("listitem:") ? "paragraph" : from
  case .highlightedCode: "code"
  default: target.rawValue
  }
}

/// The block whose own text is `text`, or includes it with `partial`.
private func findTextBlock(_ text: String, _ partial: Bool = false) throws -> ElementNode? {
  var order: [NodeKey] = []
  var texts: [NodeKey: (ElementNode, String)] = [:]
  for node in try getRoot().getAllTextNodes() {
    let block = try getBlockOf(node)
    let key = block.getKey()
    if let entry = texts[key] {
      texts[key] = (block, entry.1 + (try node.getTextContent()))
    } else {
      order.append(key)
      texts[key] = (block, try node.getTextContent())
    }
  }
  for key in order {
    let (block, content) = texts[key]!
    if partial ? content.contains(text) : content == text {
      return block
    }
  }
  return nil
}

private func typeX() throws {
  let selection = try #require(try getSelection() as? RangeSelection)
  #expect(selection.isCollapsed())
  try selection.insertText("X")
}

private struct Case {
  let content: Content
  let context: Context
  let marks: String
  let selection: SelectionAt
  /// Type `X` at the caret in the update that converts.
  let typed: Bool
}

private func cases() -> [Case] {
  var out: [Case] = []
  for content in Content.allCases {
    for (marks, _) in MARKS {
      for context in Context.allCases {
        for selection in SelectionAt.allCases {
          if selection == .across && context == .alone {
            continue
          }
          out.append(
            Case(
              content: content, context: context, marks: marks, selection: selection, typed: false))
          if selection != .range && selection != .across {
            out.append(
              Case(
                content: content, context: context, marks: marks, selection: selection, typed: true)
            )
          }
        }
      }
    }
  }
  return out
}

/// Build the document for `c` in `editor`, with the converted block made by
/// `appendBlock`, and return the function that sets the selection that the
/// case converts. The comments are added in an update of their own.
private func build(
  _ editor: Editor, _ target: Target, _ c: Case, _ appendBlock: @escaping () throws -> ElementNode
) throws -> () throws -> Void {
  var blockKey: NodeKey = ""
  var neighbourKey: NodeKey = ""
  try update(editor) {
    let root = try getRoot()
    _ = try root.clear()
    let neighbour = try createListItemNode()
    if c.context == .linedUp || c.context == .markedBefore {
      _ = try neighbour.append(createTextNode(c.context == .linedUp ? "gamma " : "old gamma "))
      _ = try root.append(createListNode(listTypeOf(target)).append(neighbour))
    }
    let block = try appendBlock()
    if c.context == .markedAfter {
      _ = try neighbour.append(createTextNode("delta"))
      _ = try root.append(createListNode(listTypeOf(target)).append(neighbour))
    }
    blockKey = block.getKey()
    neighbourKey = neighbour.getKey()
  }
  try update(editor) {
    let block = try getNodeByKeyOrThrow(blockKey) as! ElementNode
    for (id, start, end) in MARKS.first(where: { $0.0 == c.marks })!.1 {
      try markIn(block, id, start, end)
    }
    if c.context == .markedBefore || c.context == .markedAfter {
      let neighbour = try getNodeByKeyOrThrow(neighbourKey) as! ElementNode
      if c.context == .markedBefore {
        try markIn(neighbour, "c4", 0, "old gamma ".utf16.count)
        try markIn(block, "c4", TEXT_LENGTH - 1, TEXT_LENGTH)
      } else {
        try markIn(neighbour, "c5", 0, "delta".utf16.count)
        try markIn(block, "c5", 0, 1)
      }
    }
  }
  return {
    let at = try getNodeByKeyOrThrow(blockKey) as! ElementNode
    switch c.selection {
    case .range:
      try selectIn((at, 0), (at, TEXT_LENGTH))
    case .across:
      let other = try getNodeByKeyOrThrow(neighbourKey) as! ElementNode
      if c.context == .markedAfter {
        try selectIn((at, 3), (other, 2))
      } else {
        try selectIn((other, 2), (at, 3))
      }
    default:
      let offset = Int(c.selection.rawValue)!
      try selectIn((at, offset), (at, offset))
    }
  }
}

private func describeCase(_ source: String, _ target: Target, _ c: Case) -> String {
  "\(source) -> \(target.rawValue), \(c.content), marks \(c.marks), \(c.context.rawValue), selection \(c.selection.rawValue)\(c.typed ? ", typed" : "")"
}

/// Whether the characters of `after` are those of `before` with the same
/// ids. A code highlighting token is not split where its ids change, and
/// gets every id its text had, so in highlighted code each character only
/// has to keep every id it had.
private func keepsIDs(_ before: [String], _ after: [String], _ inHighlightedCode: Bool) -> Bool {
  if !inHighlightedCode {
    return after == before
  }
  func split(_ unit: String) -> (String, String) {
    let i = unit.firstIndex(of: ":")!
    return (String(unit[..<i]), String(unit[unit.index(after: i)...]))
  }
  return after.count == before.count
    && zip(after, before).allSatisfy { unit, prev in
      let (c, ids) = split(unit)
      let (prevC, prevIDs) = split(prev)
      let have = ids.split(separator: ",").map(String.init)
      return c == prevC
        && (prevIDs.isEmpty
          || prevIDs.split(separator: ",").allSatisfy { have.contains(String($0)) })
    }
}

@Suite struct MarkStateBlockConversionTests {
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from paragraph > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from heading > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from quote > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from code > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from highlighted-code > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from bullet > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from number > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from check > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-bullet > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > from nested-number > to no-list"
    ),
    arguments: Pair.all)
  fileprivate func from(_ pair: Pair) throws {
    let (source, target) = (pair.source, pair.target)
    let highlighting = source == .highlightedCode || target == .highlightedCode
    var failures: [String] = []
    for tokenizer in tokenizers(highlighting) {
      let editor = try buildEditor(tokenizer)
      for c in cases() {
        if isCodeSource(source) && c.content == .link {
          continue
        }
        if highlighting && (c.context == .markedBefore || c.context == .markedAfter)
          && (isCodeSource(source) || c.selection == .across)
        {
          // Comment ids on part of a code highlighting token, and text that a
          // range conversion merges into a code block, are re-tokenized
          // without them (#9491)
          continue
        }
        if c.typed && highlighting && c.content == .link {
          // Highlighting that unwraps the link puts the caret back at a wrong
          // offset (counted from the start of the code), so the typed text
          // is not right before the caret, as the inserted-text rule needs
          continue
        }
        let select = try build(editor, target, c) {
          try appendSource(source, content(c.content))
        }
        let before = try editor.read { try sequence() }
        let from = try editor.read { try label(findTextBlock(TEXT) ?? getRoot()) }
        try update(editor) {
          try select()
          try convert(target)
          if c.typed {
            try typeX()
          }
        }
        let (after, converted) = try editor.read {
          (try sequence(), try findTextBlock("alpha", true).map(label) ?? "missing")
        }
        var problems: [String] = []
        if converted != expectedLabel(from, target) {
          problems.append("  converted \(from) to \(converted)")
        }
        // The typed text gets the ids of the inserted-text rule where it can
        // compare the changed blocks with the previous state, and otherwise
        // those of the TextNode it lands in, so only the other characters
        // are checked
        let kept = c.typed ? after.filter { !$0.hasPrefix("X:") } : after
        if c.typed && kept.count != after.count - 1 {
          problems.append("  X was not typed once")
        }
        if !keepsIDs(before, kept, highlighting && expectedLabel(from, target) == "code") {
          problems.append("  before \(before.joined(separator: " "))")
        }
        if !problems.isEmpty {
          failures.append(
            ([describeCase(source.rawValue, target, c) + tokenizerNote(tokenizer) + ":"] + problems
              + [
                "  after  \(after.joined(separator: " "))"
              ]).joined(separator: "\n"))
        }
      }
    }
    #expect(failures == [])
  }

  // The source block is copied with its comments, pasted into an empty
  // document, and the pasted blocks are converted: the pasted text keeps
  // its ids through both
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted paragraph > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted heading > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted quote > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted code > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted highlighted-code > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted bullet > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted number > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted check > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-bullet > to no-list"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to paragraph"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to heading"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to quote"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to highlighted-code"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to bullet"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to number"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to check"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > pasted nested-number > to no-list"
    ),
    arguments: Pair.all)
  fileprivate func pasted(_ pair: Pair) throws {
    let (source, target) = (pair.source, pair.target)
    let highlighting = source == .highlightedCode || target == .highlightedCode
    var failures: [String] = []
    for tokenizer in tokenizers(highlighting) {
      let from = try buildEditor(tokenizer)
      for c in cases() {
        if c.context != .alone || c.typed || (isCodeSource(source) && c.content == .link) {
          continue
        }
        // A fresh editor: pasting into the document of the previous case in
        // the update that clears it would compare the pasted text with that
        // document's equal text
        let editor = try buildEditor(tokenizer)
        _ = try build(from, target, c) { try appendSource(source, content(c.content)) }
        var json: SerializedSelectedNodes? = nil
        try update(from) {
          let root = try getRoot()
          json = try generateJSONFromSelectedNodes(from, root.select(0, root.getChildrenSize()))
        }
        let copied = try from.read { try sequence() }
        try update(editor) {
          let paragraph = try createParagraphNode()
          _ = try getRoot().append(paragraph)
          try insertGeneratedNodes(
            editor, generateNodesFromSerializedNodes(json!.nodes), paragraph.select(0, 0))
        }
        let pasted = try editor.read { try sequence() }
        let pastedLabel = try editor.read { try findTextBlock(TEXT).map(label) ?? "missing" }
        try update(editor) {
          let block = try #require(try findTextBlock(TEXT), "the pasted text is in one block")
          if c.selection == .range {
            try selectIn((block, 0), (block, TEXT_LENGTH))
          } else {
            let offset = Int(c.selection.rawValue)!
            try selectIn((block, offset), (block, offset))
          }
          try convert(target)
        }
        let after = try editor.read { try sequence() }
        let converted = try editor.read { try findTextBlock(TEXT).map(label) ?? "missing" }
        if pasted != copied
          || !keepsIDs(
            copied, after, highlighting && expectedLabel(pastedLabel, target) == "code")
          || converted != expectedLabel(pastedLabel, target)
        {
          failures.append(
            "\(describeCase(source.rawValue, target, c))\(tokenizerNote(tokenizer)):\n  copied \(copied.joined(separator: " "))\n  pasted \(pasted.joined(separator: " "))\n  after  \(after.joined(separator: " ")), converted to \(converted)"
          )
        }
      }
    }
    #expect(failures == [])
  }
}

private func tokenizerNote(_ tokenizer: MarkTestCodeTokenizer?) -> String {
  guard let tokenizer else { return "" }
  return tokenizer.splitWords ? ", word tokens" : ", Prism-like tokens"
}

private func textMarks() throws -> [String] {
  try getRoot().getAllTextNodes().map {
    "\(try $0.getTextContent()):\(try getNodeMarkIDs($0).joined(separator: ","))"
  }
}

@Suite struct MarkStateBlockConversionReportedTests {
  // #9490: the converted paragraph merges into the list above it, whose
  // item's text lines up with the end of the paragraph's text
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a paragraph turned into a list item keeps the comment on a link"
    ))
  func paragraphToListKeepsLinkComment() throws {
    let editor = try buildEditor()
    try update(editor) {
      let commented = try createTextNode("abcd")
      _ = try getRoot().append(
        createListNode("number").append(createListItemNode().append(createTextNode("echo "))),
        createParagraphNode().append(
          createLinkNode("https://example.org").append(commented), createTextNode(" Z echo ")))
      _ = try addMarkID("c0", commented.select(0, 4))
    }
    let before = try editor.read { try sequence() }
    try update(editor) {
      _ = try (getRoot().getLastChild() as! ElementNode).getAllTextNodes().last!.select(4, 4)
      try insertList("number")
    }
    #expect(try editor.read { try getRoot().getChildrenSize() } == 1)
    #expect(try editor.read { try sequence() } == before)
  }

  // #9490: the text of the paragraph lines up with the marked item above
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a paragraph turned into a list item does not take the comment of the item above"
    ))
  func paragraphToListDoesNotTakeComment() throws {
    let editor = try buildEditor()
    try update(editor) {
      let item = try createTextNode("old echo ")
      let space = try createTextNode(" ")
      _ = try getRoot().append(
        createListNode("bullet").append(createListItemNode().append(item)),
        createParagraphNode().append(createTextNode("new echo"), space))
      _ = try addMarkID("c1", item.select(0, item.getTextContentSize()))
      _ = try addMarkID("c1", space.select(0, 1))
    }
    let before = try editor.read { try sequence() }
    try update(editor) {
      _ = try (getRoot().getLastChild() as! ElementNode).getAllTextNodes()[0].selectEnd()
      try insertList("bullet")
    }
    #expect(try editor.read { try sequence() } == before)
  }

  // Code highlighting turns its tokens back into TextNodes when the code
  // block becomes a paragraph; with the code selected, the inserted-text
  // rule does not run, so the new TextNodes keep the ids on their own
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a comment on code text survives turning the code block into a paragraph"
    ),
    arguments: tokenizers(true).compactMap { $0 })
  fileprivate func commentOnCodeSurvives(_ tokenizer: MarkTestCodeTokenizer) throws {
    let editor = try buildEditor(tokenizer)
    try update(editor) {
      _ = try getRoot().append(createCodeNode().append(createTextNode(TEXT)))
    }
    try update(editor) {
      let code = try getRoot().getFirstChild() as! ElementNode
      try markIn(code, "c1", 0, TEXT_LENGTH)
      try selectIn((code, 0), (code, TEXT_LENGTH))
    }
    let before = try editor.read { try sequence() }
    // Not upstream: the stand-in tokenizer did highlight the code, in one
    // token as Prism does, or one per word and space run
    let tokens = try editor.read {
      try (getRoot().getFirstChild() as! ElementNode).getChildren().map { $0 is CodeHighlightNode }
    }
    #expect(tokens == Array(repeating: true, count: tokenizer.splitWords ? 6 : 1))
    try update(editor) { try convert(.paragraph) }
    #expect(
      try editor.read { [try getRoot().getFirstChildOrThrow().getType()] + (try sequence()) }
        == ["paragraph"] + before)
    #expect(before.first == "a:c1")
  }

  // A lone list item turned into another block with the caret at its end
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a list item turned into a paragraph keeps its comment"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a list item turned into a heading keeps its comment"
    ),
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > reported cases > a list item turned into a quote keeps its comment"
    ),
    arguments: [Target.paragraph, .heading, .quote])
  fileprivate func listItemKeepsComment(_ target: Target) throws {
    let editor = try buildEditor()
    try update(editor) {
      let text = try createTextNode("alpha beta")
      _ = try getRoot().append(createListNode("bullet").append(createListItemNode().append(text)))
      _ = try addMarkID("c1", text.select(6, 10))
    }
    try update(editor) {
      _ = try getRoot().getAllTextNodes().last!.selectEnd()
      try convert(target)
    }
    #expect(
      try editor.read { [try getRoot().getFirstChildOrThrow().getType()] + (try textMarks()) }
        == [target.rawValue, "alpha :", "beta:c1"])
  }
}

@Suite struct MarkStateBlockConversionMovedTextTests {
  // A node that code moves is not inserted text, unlike a new node
  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > moved text > a node moved into a marked run keeps its ids, a new node joins the run"
    ))
  func movedNodeKeepsIDs() throws {
    let editor = try buildEditor()
    try update(editor) {
      let a = try createTextNode("aaa")
      let b = try createTextNode("bbb").toggleFormat("italic")
      _ = try getRoot().append(
        createParagraphNode().append(a, b),
        createParagraphNode().append(createTextNode("moved").toggleFormat("bold")))
      _ = try addMarkID("c1", a.select(0, 3))
      _ = try addMarkID("c1", b.select(0, 3))
    }
    try update(editor) {
      let created = try createTextNode("new").toggleFormat("underline")
      _ = try getRoot().getAllTextNodes()[0].insertAfter(created)
      _ = try created.selectEnd()
    }
    #expect(try editor.read(textMarks) == ["aaa:c1", "new:c1", "bbb:c1", "moved:"])
    try update(editor) {
      let texts = try getRoot().getAllTextNodes()
      let (created, moved) = (texts[1], texts[3])
      _ = try created.insertAfter(moved)
      _ = try moved.selectEnd()
    }
    #expect(try editor.read(textMarks) == ["aaa:c1", "new:c1", "moved:", "bbb:c1"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > moved text > a node moved into the text that replaces a selection: the new text keeps the replaced ids"
    ))
  func movedIntoReplacingText() throws {
    let editor = try buildEditor()
    try update(editor) {
      let replaced = try createTextNode("zzz")
      _ = try getRoot().append(
        createParagraphNode().append(createTextNode("keep").toggleFormat("bold")),
        createParagraphNode().append(replaced))
      _ = try addMarkID("c1", replaced.select(0, 3))
      _ = try replaced.select(0, 3)
    }
    try update(editor) {
      _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "Q")
      let texts = try getRoot().getAllTextNodes()
      let (moved, inserted) = (texts[0], texts[1])
      _ = try inserted.insertBefore(moved)
      _ = try inserted.selectEnd()
    }
    #expect(try editor.read(textMarks) == ["keep:", "Q:c1"])
  }

  @Test(
    .upstream(
      "lexical-mark/src/__tests__/unit/MarkStateBlockConversion.test.ts::MarkStateExtension: converting blocks keeps the ids of every character > moved text > a selection replaced in the update that turns its list item into a paragraph"
    ))
  func selectionReplacedWhileConverting() throws {
    let editor = try buildEditor()
    try update(editor) {
      let a = try createTextNode("abc")
      let b = try createTextNode("def").toggleFormat("bold")
      _ = try getRoot().append(createListNode("bullet").append(createListItemNode().append(a, b)))
      _ = try addMarkID("c1", a.select(0, 3))
      _ = try addMarkID("c1", b.select(0, 3))
      _ = try a.selectEnd()
    }
    try update(editor) {
      try convert(.paragraph)
      let texts = try getRoot().getAllTextNodes()
      let selection = createRangeSelection()
      try selection.anchor.set(texts[0].getKey(), 1, .text)
      try selection.focus.set(texts[1].getKey(), 2, .text)
      try setSelection(selection)
      _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "Q")
    }
    #expect(try editor.read { try sequence() } == ["a:c1", "Q:c1", "f:c1"])
  }
}
