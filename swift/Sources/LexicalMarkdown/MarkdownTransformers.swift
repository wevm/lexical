import Lexical
import LexicalCodeCore
import LexicalLink
import LexicalList
import LexicalRichText

// Port of packages/lexical-markdown/src/MarkdownTransformers.ts. The
// transformer types are in TransformerTypes.swift, and the NodeStates,
// `parseMarkdownHardLineBreak` and `getCodeSpanDelimiter` in
// MarkdownStates.swift.

nonisolated(unsafe) private let ORDERED_LIST_REGEX = JSRegExp(#"^(\s*)(\d{1,})\.\s"#)
nonisolated(unsafe) private let UNORDERED_LIST_REGEX = JSRegExp(#"^(\s*)[-*+]\s"#)
nonisolated(unsafe) private let CHECK_LIST_REGEX = JSRegExp(
  #"^(\s*)(?:[-*+]\s)?\s?(\[(\s|x)?\])\s"#, "i")
nonisolated(unsafe) private let HEADING_REGEX = JSRegExp(#"^(#{1,6})\s"#)
nonisolated(unsafe) private let QUOTE_REGEX = JSRegExp(#"^>\s"#)
nonisolated(unsafe) private let CODE_START_REGEX = JSRegExp(#"^([ \t]*`{3,})([\w-]+)?[ \t]?"#)
nonisolated(unsafe) private let CODE_END_REGEX = JSRegExp(#"^[ \t]*`{3,}$"#)
nonisolated(unsafe) private let CODE_SINGLE_LINE_REGEX = JSRegExp(
  #"^[ \t]*```[^`]+(?:(?:`{1,2}|`{4,})[^`]+)*```(?:[^`]|$)"#)
nonisolated(unsafe) private let TABLE_ROW_REG_EXP = JSRegExp(#"^(?:\|)(.+)(?:\|)\s?$"#)

private let PIPE = UInt16(UInt8(ascii: "|"))
private let SPACE = UInt16(UInt8(ascii: " "))
private let COLON = UInt16(UInt8(ascii: ":"))
private let DASH = UInt16(UInt8(ascii: "-"))

/// `isTableRowDivider(line)`: whether `line` is a Markdown table delimiter
/// row such as `| --- | :--: |`.
///
/// This is the linear-time equivalent of `/^(\| ?:?-*:? ?)+\|\s?$/`. That
/// pattern nests `-*` inside a `(...)+` group, a shape that backtracking
/// regexp engines may run in super-linear time. A manual scan is guaranteed
/// O(n).
public func isTableRowDivider(_ line: String) -> Bool {
  let units = Array(line.utf16)
  func at(_ j: Int) -> UInt16? { j < units.count ? units[j] : nil }
  // Must start with a leading pipe.
  if at(0) != PIPE {
    return false
  }
  let length = units.count
  var i = 1
  var cells = 0
  // Each iteration consumes one ` ?:?-*:? ?\|` cell-and-pipe unit. Cell
  // characters (space, colon, dash) are disjoint from the `|` delimiter, so a
  // greedy scan never needs to backtrack.
  while i < length {
    var j = i
    if at(j) == SPACE {
      j += 1
    }
    if at(j) == COLON {
      j += 1
    }
    while at(j) == DASH {
      j += 1
    }
    if at(j) == COLON {
      j += 1
    }
    if at(j) == SPACE {
      j += 1
    }
    if at(j) != PIPE {
      break
    }
    cells += 1
    i = j + 1
  }
  // Require at least one cell, then an optional single trailing whitespace
  // character (`\s?`) before the end of the line (`$`).
  return cells > 0 && (i == length || (i == length - 1 && isJSWhitespace(units[i])))
}

nonisolated(unsafe) private let TAG_START_REGEX = JSRegExp(#"^<[a-z_][\w-]*(?:\s[^<>]*)?\/?>"#, "i")
nonisolated(unsafe) private let TAG_END_REGEX = JSRegExp(#"^<\/[a-z_][\w-]*\s*>"#, "i")

/// `ENDS_WITH(regex)`.
private func ENDS_WITH(_ regex: JSRegExp) -> JSRegExp {
  JSRegExp("(?:\(regex.source))$", regex.flags)
}

// JS builds these on each test; they are the same every time.
nonisolated(unsafe) private let TAG_END_AT_END_REGEX = ENDS_WITH(TAG_END_REGEX)
nonisolated(unsafe) private let TAG_START_AT_END_REGEX = ENDS_WITH(TAG_START_REGEX)

/// `string.trimStart()`.
private func trimStart(_ string: String) -> String {
  let units = Array(string.utf16)
  var start = 0
  while start < units.count, isJSWhitespace(units[start]) { start += 1 }
  return String(decoding: units[start...], as: UTF16.self)
}

/// `string.trimEnd()`.
private func trimEnd(_ string: String) -> String {
  let units = Array(string.utf16)
  var end = units.count
  while end > 0, isJSWhitespace(units[end - 1]) { end -= 1 }
  return String(decoding: units[..<end], as: UTF16.self)
}

/// JS truthiness of a string that may be `undefined`.
private func isTruthy(_ string: String?) -> Bool { !(string ?? "").isEmpty }

nonisolated(unsafe) private let NON_WHITESPACE = JSRegExp(#"\S"#)

private func hasNonWhitespaceContentOnLine(
  _ children: [LexicalNode], _ endIndex: Int
) throws
  -> Bool
{
  var i = endIndex - 1
  while i >= 0 {
    if isLineBreakNode(children[i]) {
      return false
    }
    if NON_WHITESPACE.test(try children[i].getTextContent()) {
      return true
    }
    i -= 1
  }
  return false
}

nonisolated(unsafe) private let ONLY_SPACES = JSRegExp(#"^ {2,}$"#)

/// `$extractMarkdownHardLineBreakMarker(previousNode)`.
private func extractMarkdownHardLineBreakMarker(
  _ previousNode: ElementNode
) throws
  -> MarkdownHardLineBreak?
{
  let children = try previousNode.getChildren()
  let lastChildIndex = children.count - 1

  guard let lastChild = children.last as? TextNode else {
    return nil
  }

  let lastText = try lastChild.getTextContent()
  let hardLineBreak = parseMarkdownHardLineBreak(lastText)

  if let (text, marker) = hardLineBreak {
    try lastChild.setTextContent(text)
    return marker
  }

  if ONLY_SPACES.test(lastText), try hasNonWhitespaceContentOnLine(children, lastChildIndex) {
    try lastChild.setTextContent("")
    return lastText
  }

  return nil
}

/// `$createMarkdownLineBreakNode(previousNode)`: a line break that keeps
/// the hard line break marker ending `previousNode`, which it takes off
/// the text.
package func createMarkdownLineBreakNode(_ previousNode: ElementNode) throws -> LineBreakNode {
  let lineBreakNode = try createLineBreakNode()
  let hardLineBreak = try extractMarkdownHardLineBreakMarker(previousNode)

  if let hardLineBreak {
    try setState(lineBreakNode, hardLineBreakState, hardLineBreak)
  }

  return lineBreakNode
}

/// `$isUnreplaceableBlock(parentNode)`: block-level shortcuts convert by
/// replacing the enclosing block, which discards it. A QuoteNode holds
/// inline content, so there is nowhere to nest the new block and the quote
/// would simply be lost. Import already refuses:
/// `$convertFromMarkdownString('> # x')` keeps the quote and leaves `# x`
/// as literal text, so the shortcut declines too rather than dropping the
/// quote out from under the caret. See #7407.
private func isUnreplaceableBlock(_ parentNode: ElementNode) -> Bool {
  isQuoteNode(parentNode)
}

private let TAB = UInt16(UInt8(ascii: "\t"))

/// `stripFenceIndent(line, indent)`. CommonMark: "If the leading code
/// fence is indented N spaces, then up to N spaces of indentation are
/// removed from each line of the content (if present)."
/// https://spec.commonmark.org/0.31.2/#fenced-code-blocks
private func stripFenceIndent(_ line: String, _ indent: Int) -> String {
  var index = 0
  while index < indent, let unit = line.utf16Unit(at: index), unit == SPACE || unit == TAB {
    index += 1
  }
  return line.jsSlice(index)
}

/// `$setCodeMeta(parentNode, meta)`: attaches the opening fence's
/// info-string tail to the `CodeNode` that `CODE.replace` just appended.
/// `replace` takes the match rather than the source line, so the tail is
/// applied here, where the line is in hand.
private func setCodeMeta(_ parentNode: ElementNode, _ meta: String) throws {
  let codeNode = try parentNode.getLastChild()
  if !meta.isEmpty, let codeNode = codeNode as? CodeNode {
    try setState(codeNode, codeMetaState, meta)
  }
}

private typealias ElementReplace = (ElementNode, [LexicalNode], JSRegExpMatch, Bool) throws -> Bool?

/// `createBlockNode(createNode)`.
private func createBlockNode(
  _ createNode: @escaping (JSRegExpMatch) throws -> ElementNode
)
  -> ElementReplace
{
  { parentNode, children, match, isImport in
    let node = try createNode(match)
    try node.append(contentsOf: children)
    try parentNode.replace(node)
    if !isImport {
      try node.select(0, 0)
    }
    return nil
  }
}

// Amount of spaces that define indentation level
// TODO: should be an option
private let LIST_INDENT_SIZE = 4

nonisolated(unsafe) private let TABS = JSRegExp(#"\t"#, "g")
nonisolated(unsafe) private let SPACES = JSRegExp(#" "#, "g")

private func getIndent(_ whitespaces: String) -> Int {
  let tabs = whitespaces.jsMatchAllTexts(TABS)
  let spaces = whitespaces.jsMatchAllTexts(SPACES)

  var indent = 0

  if let tabs {
    indent += tabs.count
  }

  if let spaces {
    indent += spaces.count / LIST_INDENT_SIZE
  }

  return indent
}

/// `getColumn(whitespaces)`: the column the text after `whitespaces`
/// starts at, expanding a tab to the next multiple of `LIST_INDENT_SIZE`
/// the way CommonMark does.
private func getColumn(_ whitespaces: String) -> Int {
  var column = 0
  for char in whitespaces.unicodeScalars {
    column += char == "\t" ? LIST_INDENT_SIZE - (column % LIST_INDENT_SIZE) : 1
  }
  return column
}

/// `importListColumns`: the content columns of the list levels the lines
/// placed so far have left open, outermost first, or `nil` outside a
/// markdown import.
///
/// A sublist is measured against the column where its parent item's
/// content begins rather than against a fixed number of spaces: that is
/// what lets `1. a` take a three-space sublist while `- a` takes a
/// two-space one, and what keeps two items written at the same column
/// siblings even when that column is deep enough to have opened a level.
///
/// Only the line that opened a level knows the column it was written at,
/// and nothing in the tree records it afterwards, so the columns are
/// carried from line to line for the length of one import. A shortcut
/// typed into an editor is a line on its own with no such run behind it and
/// keeps reading its indent as a fixed `LIST_INDENT_SIZE` per level, which
/// is the step `$listExport` writes and the one the toolbar and Tab indent
/// by.
///
/// JS keeps it in module scope; here it is per thread, in `ThreadContext`.
private var importListColumns: [Int]? {
  get { ThreadContext.current.markdownImportListColumns }
  set { ThreadContext.current.markdownImportListColumns = newValue }
}
private var importJoinsLooseLists: Bool {
  get { ThreadContext.current.markdownImportJoinsLooseLists }
  set { ThreadContext.current.markdownImportJoinsLooseLists = newValue }
}

/// `withListIndentColumns(joinLooseLists, fn)`: runs `fn` with the columns
/// of a single markdown import tracked across the lines it places.
/// Restores whatever was being tracked around it, so an import that runs
/// inside another one does not disturb it.
///
/// `joinLooseLists` reads a blank line between list lines as making the
/// list loose rather than ending it. An import that preserves new lines
/// keeps blank paragraphs as content, so there a blank line closes the list
/// like any other block.
package func withListIndentColumns<T>(_ joinLooseLists: Bool, _ fn: () throws -> T) rethrows -> T {
  let previousColumns = importListColumns
  let previousJoins = importJoinsLooseLists
  importListColumns = []
  importJoinsLooseLists = joinLooseLists
  defer {
    importListColumns = previousColumns
    importJoinsLooseLists = previousJoins
  }
  return try fn()
}

/// `getColumnIndent(columns, whitespaces)`: the indent `whitespaces` names
/// given the levels open above it: the innermost level whose content column
/// it reaches, or 0 when it reaches none of them.
private func getColumnIndent(_ columns: [Int], _ whitespaces: String) -> Int {
  let column = getColumn(whitespaces)
  var i = columns.count - 1
  while i >= 0 {
    if column >= columns[i] {
      return i + 1
    }
    i -= 1
  }
  return 0
}

nonisolated(unsafe) private let BULLET = JSRegExp(#"^[-*+]\s"#)

/// `getContentColumn(match, listType)`: the column this line opens its
/// content at: where the matched marker ends, with tabs expanded, so `-\ta`
/// opens at the tab stop and not two columns in. A check list item is
/// marked by its bullet — the `[ ]` is content — so `- [ ] a` opens where
/// `- a` does. Capped one `LIST_INDENT_SIZE` past the marker so that the
/// indent `$listExport` writes always nests: a sublist of `100. ` is
/// exported with four spaces, short of that marker's real content column.
private func getContentColumn(_ match: JSRegExpMatch, _ listType: ListType) -> Int {
  var prefix = match.text
  let whitespaces = match[1] ?? ""
  if listType == "check" {
    if let bullet = prefix.jsSlice(whitespaces.jsLength).jsMatch(BULLET) {
      prefix = whitespaces + bullet.text
    }
  }
  return min(getColumn(prefix), getColumn(whitespaces) + LIST_INDENT_SIZE)
}

/// `setOpenColumn(columns, indent, match, listType)`: records the content
/// column that this line leaves open for the lines below it, closing the
/// levels it stepped back out of.
private func setOpenColumn(
  _ columns: inout [Int], _ indent: Int, _ match: JSRegExpMatch, _ listType: ListType
) {
  // An indent read from `getIndent` rather than from the columns can name a
  // level no line of this import opened, so any gap below it is filled with
  // the fixed step that reading assumed.
  while columns.count < indent {
    columns.append((columns.count + 1) * LIST_INDENT_SIZE)
  }
  columns.removeLast(columns.count - indent)
  columns.append(getContentColumn(match, listType))
}

/// JS `Number(digits)` for the digits of an ordered list marker. A number
/// past `Int.max`, which JS keeps as a float, is clamped.
private func listNumber(_ digits: String?) -> Int {
  guard let digits else { return 0 }
  return Int(digits) ?? (digits.isEmpty ? 0 : Int.max)
}

nonisolated(unsafe) private let CHECKED = JSRegExp(#"^x$"#, "i")

/// `listReplace(listType)`.
private func listReplace(_ listType: ListType) -> ElementReplace {
  { parentNode, children, match, isImport in
    if isHeadingNode(parentNode) || (!isImport && isUnreplaceableBlock(parentNode)) {
      return false
    }

    let previousNode = try parentNode.getPreviousSibling()
    let nextNode = try parentNode.getNextSibling()
    let listItem = try createListItemNode(
      // CHECK_LIST_REGEX matches case-insensitively, so `[X]` is checked too.
      listType == "check" ? CHECKED.test(match[3] ?? "") : nil)
    let firstMatchChar = jsTrim(match.text).jsCharAt(0)
    let listMarker: String? =
      (listType == "bullet" || listType == "check") && firstMatchChar != nil
        && firstMatchChar == listMarkerState.parse(.string(firstMatchChar!))
      ? firstMatchChar : nil
    // A block of another kind closes every level above this line. Blank lines
    // do not: they only make the list loose, and a blank line must not decide
    // how the line after it is read, or the same sublist would be read one way
    // written tightly and another written with a blank line before it. So the
    // line is measured against, and placed into, the list it follows across
    // any blank lines, and the levels close only when the nearest block that
    // is not one is no longer the list itself. Outside such an import — a
    // typed shortcut, or an import that keeps its blank lines as content — a
    // blank line above the line is a block like any other.
    let tracksColumns = importListColumns != nil
    var precedingBlock = previousNode
    if tracksColumns {
      if importJoinsLooseLists {
        while let block = precedingBlock, try isEmptyParagraph(block) {
          precedingBlock = try block.getPreviousSibling()
        }
      }
      if !isListNode(precedingBlock) {
        importListColumns!.removeAll()
      }
    }
    let whitespaces = match[1] ?? ""
    let indent =
      importListColumns == nil || importListColumns!.isEmpty
      ? getIndent(whitespaces)
      : getColumnIndent(importListColumns!, whitespaces)
    if let nextNode = nextNode as? ListNode, try nextNode.getListType() == listType {
      if let firstChild = try nextNode.getFirstChild() {
        try firstChild.insertBefore(listItem)
      } else {
        // should never happen, but let's handle gracefully, just in case.
        try nextNode.append(listItem)
      }
      // The new list item lands at index 0, so the typed number becomes the
      // list's starting value. #8677. An indented item only passes through —
      // `setIndent` below moves it into a sublist — so its number belongs to
      // that sublist, not to the list it leaves.
      if listType == "number" && indent == 0 {
        try nextNode.setStart(listNumber(match[2]))
      }
      try parentNode.remove()
    } else if let precedingBlock = precedingBlock as? ListNode,
      try precedingBlock.getListType() == listType || indent > 0
    {
      // An item of the same type continues the list, and inherits the
      // existing sequence — the typed number is intentionally ignored. An
      // indented item of another type belongs inside it too: it is appended
      // at the top level here, `setIndent` below moves it to its level, and a
      // list of the right type is spliced in once the level it lands in is
      // known.
      try precedingBlock.append(listItem)
      try parentNode.remove()
    } else {
      let list =
        listType == "number"
        ? try createListNode(listType, start: listNumber(match[2]))
        : try createListNode(listType)
      try list.append(listItem)
      try parentNode.replace(list)
    }
    try listItem.append(contentsOf: children)
    if !isImport {
      try listItem.select(0, 0)
    }
    if indent != 0 {
      try listItem.setIndent(indent)
      try retypeNestedList(listItem, listType, match)
    }
    // The marker belongs to the list the item actually ends up in, which is
    // only known once it has been indented and retyped.
    let listNode = try listItem.getParent()
    if let listMarker, let listNode = listNode as? ListNode {
      try setState(listNode, listMarkerState, listMarker)
    }
    if tracksColumns {
      setOpenColumn(&importListColumns!, indent, match, listType)
    }
    return nil
  }
}

/// `$retypeNestedList(listItem, listType, match)`: `setIndent` nests an
/// item by copying the list it is in, so an item whose type differs from
/// the list above it lands in a nested list of the wrong type. Split it out
/// into a list of its own type, in place, so that a sublist can change type
/// the way CommonMark lets it — `1. a` may be followed by an indented
/// `- b`, and that by an indented `- [ ] c`.
private func retypeNestedList(
  _ listItem: ListItemNode, _ listType: ListType, _ match: JSRegExpMatch
) throws {
  guard let nestedList = try listItem.getParent() as? ListNode,
    try nestedList.getListType() != listType
  else {
    return
  }
  guard let wrapper = try nestedList.getParent() as? ListItemNode else {
    return
  }
  let retypedList =
    listType == "number"
    ? try createListNode(listType, start: listNumber(match[2]))
    : try createListNode(listType)
  // `setIndent` either appends the item to the nested list or puts it in
  // front of it, so the list of its own type belongs on whichever side of the
  // one it was placed in it already sits on.
  let isFirst = try listItem.getPreviousSibling() == nil
  // `append` moves the item without disturbing the selection, which
  // `remove` would relocate to a sibling and leave there — the caret has to
  // stay in the item the shortcut just created.
  try retypedList.append(listItem)
  let retypedWrapper = try createListItemNode()
  try retypedWrapper.append(retypedList)
  if isFirst {
    try wrapper.insertBefore(retypedWrapper)
  } else {
    try wrapper.insertAfter(retypedWrapper)
  }
  if try nestedList.getChildrenSize() == 0 {
    try wrapper.remove()
  }
}

nonisolated(unsafe) private let NUMBERED_TEXT = JSRegExp(#"^(\s{0,3}\d+)(\.\s)"#)

/// `$listExport(listNode, exportChildren, depth, selection)`.
private func listExport(
  _ listNode: ListNode,
  _ exportChildren: ExportChildren,
  _ depth: Int,
  _ selection: BaseSelection?
) throws -> String {
  var output: [String] = []
  let children = try listNode.getChildren()
  var index = 0
  for listItemNode in children {
    guard let listItemNode = listItemNode as? ListItemNode else { continue }
    if try listItemNode.getChildrenSize() == 1 {
      if let firstChild = try listItemNode.getFirstChild() as? ListNode {
        let nestedResult = try listExport(firstChild, exportChildren, depth + 1, selection)
        if !nestedResult.isEmpty {
          output.append(nestedResult)
        }
        continue
      }
    }
    // Skip unselected list items when selection is provided
    if let selection,
      try !listItemNode.getChildren().contains(where: { try $0.isSelected(selection) })
    {
      continue
    }
    let indent = String(repeating: " ", count: depth * LIST_INDENT_SIZE)
    let listType = try listNode.getListType()
    let listMarker = try getState(listNode, listMarkerState)
    let prefix =
      listType == "number"
      ? "\(try listNode.getStart() + index). "
      : listType == "check"
        ? "\(listMarker) [\(try listItemNode.getChecked() == true ? "x" : " ")] "
        : listMarker + " "
    var childrenText = try exportChildren(listItemNode)
    if listType != "number" {
      childrenText = childrenText.jsReplace(NUMBERED_TEXT, template: #"$1\$2"#)
    }
    output.append(indent + prefix + childrenText)
    index += 1
  }

  return output.joined(separator: "\n")
}

nonisolated(unsafe) private let replaceWithHeading = createBlockNode { match in
  let tag: HeadingTagType = "h\((match[1] ?? "").jsLength)"
  return try createHeadingNode(tag)
}

/// `HEADING`.
nonisolated(unsafe) public let HEADING = ElementTransformer(
  dependencies: [HeadingNode.self],
  export: { node, exportChildren, _ in
    guard let node = node as? HeadingNode else {
      return nil
    }
    let level = Int(try node.getTag().jsSlice(1)) ?? 0
    return String(repeating: "#", count: level) + " " + (try exportChildren(node))
  },
  regExp: HEADING_REGEX,
  replace: { parentNode, children, match, isImport in
    if !isImport && isUnreplaceableBlock(parentNode) {
      return false
    }
    return try replaceWithHeading(parentNode, children, match, isImport)
  },
  triggerOnEnter: true)

/// `QUOTE`.
nonisolated(unsafe) public let QUOTE = ElementTransformer(
  dependencies: [QuoteNode.self],
  export: { node, exportChildren, _ in
    guard let node = node as? QuoteNode else {
      return nil
    }

    let lines = try exportChildren(node).jsSplit("\n")
    var output: [String] = []
    for line in lines {
      output.append("> " + line)
    }
    return output.joined(separator: "\n")
  },
  regExp: QUOTE_REGEX,
  replace: { parentNode, children, _, isImport in
    if isImport {
      let previousNode = try parentNode.getPreviousSibling()
      if let previousNode = previousNode as? QuoteNode {
        try previousNode.splice(
          previousNode.getChildrenSize(), 0,
          [try createMarkdownLineBreakNode(previousNode)] + children)
        try parentNode.remove()
        return nil
      }
    }

    let node = try createQuoteNode()
    try node.append(contentsOf: children)
    try parentNode.replace(node)
    if !isImport {
      try node.select(0, 0)
    }
    return nil
  },
  triggerOnEnter: true)

nonisolated(unsafe) private let LONG_BACKTICK_RUNS = JSRegExp(#"`{3,}"#, "g")

/// `CODE.export`.
private func codeExport(
  _ node: LexicalNode, _: ExportChildren, _: BaseSelection?
) throws
  -> String?
{
  guard let node = node as? CodeNode else {
    return nil
  }
  let textContent = try node.getTextContent()
  var fence = try getState(node, codeFenceState)

  if textContent.jsIndexOf(fence) > -1 {
    if let backticks = textContent.jsMatchAllTexts(LONG_BACKTICK_RUNS) {
      let maxLength = backticks.map(\.jsLength).max() ?? 0
      fence = String(repeating: "`", count: maxLength + 1)
    }
  }
  let language = try node.getLanguage() ?? ""
  let meta = language.isEmpty ? "" : try getState(node, codeMetaState)

  return fence + language + (meta.isEmpty ? "" : " " + meta)
    + (textContent.isEmpty ? "" : "\n" + textContent) + "\n" + fence
}

/// `CODE.handleImportAfterStartMatch`.
private func codeHandleImportAfterStartMatch(
  _ args: MultilineImportArgs
) throws
  -> MultilineImportResult
{
  let lines = args.lines
  let rootNode = args.rootNode
  let startLineIndex = args.startLineIndex
  let startMatch = args.startMatch
  let fence = startMatch[1] ?? ""
  let fenceLength = jsTrim(fence).jsLength
  let currentLine = lines[startLineIndex]
  let afterFenceIndex = startMatch.index + fence.jsLength
  let afterFence = currentLine.jsSlice(afterFenceIndex)

  let singleLineEndRegex = JSRegExp("`{\(fenceLength),}$")

  if singleLineEndRegex.test(afterFence) {
    let endMatch = afterFence.jsMatch(singleLineEndRegex)
    let content = afterFence.jsSlice(0, afterFence.jsLastIndexOf(endMatch?.text ?? ""))

    var fakeGroups = startMatch.groups
    while fakeGroups.count < 3 { fakeGroups.append(nil) }
    fakeGroups[2] = ""
    let fakeStartMatch = JSRegExpMatch(
      groups: fakeGroups, index: startMatch.index, input: startMatch.input)

    _ = try CODE.replace(rootNode, nil, fakeStartMatch, endMatch, [content], true)
    return .handled(true, startLineIndex)
  }

  let multilineEndRegex = JSRegExp("^[ \\t]*`{\(fenceLength),}$")

  var i = startLineIndex + 1
  while i < lines.count {
    let line = lines[i]
    if multilineEndRegex.test(line) {
      let endMatch = line.jsMatch(multilineEndRegex)
      var linesInBetween = Array(lines[(startLineIndex + 1)..<i])

      // Everything after the opening fence is the info string, and only its
      // first word is the language. When a language was captured, whatever
      // follows it on that line is metadata (```js title="x", ```ts {1,3})
      // and must not be prepended to the block's content.
      // https://spec.commonmark.org/0.31.2/#fenced-code-blocks
      //
      // With no language captured the fence carries no info string, so the
      // remainder is kept as content (``` code) as before.
      //
      // Either way the slot itself is always occupied: `replace` follows the
      // default $importMultiline contract, where linesInBetween[0] is the
      // remainder of the opening fence line and is discarded when blank. So
      // an empty placeholder goes in when the remainder is metadata, and a
      // blank remainder is unshifted rather than skipped — otherwise a code
      // block that genuinely starts with a blank line would have that line
      // mistaken for the (empty) remainder and dropped.
      let meta = isTruthy(startMatch[2]) ? jsTrim(currentLine.jsSlice(startMatch.length)) : ""
      linesInBetween.insert(meta.isEmpty ? currentLine.jsSlice(startMatch.length) : "", at: 0)

      _ = try CODE.replace(rootNode, nil, startMatch, endMatch, linesInBetween, true)
      try setCodeMeta(rootNode, meta)
      return .handled(true, i)
    }
    i += 1
  }

  var linesInBetween = Array(lines[min(startLineIndex + 1, lines.count)...])
  linesInBetween.insert(currentLine.jsSlice(startMatch.length), at: 0)

  _ = try CODE.replace(rootNode, nil, startMatch, nil, linesInBetween, true)
  return .handled(true, lines.count - 1)
}

/// `CODE.replace`.
private func codeReplace(
  _ rootNode: ElementNode,
  _ children: [LexicalNode]?,
  _ startMatch: JSRegExpMatch,
  _ endMatch: JSRegExpMatch?,
  _ linesInBetween: [String]?,
  _ isImport: Bool
) throws -> Bool? {
  let codeBlockNode: CodeNode
  let code: String

  let fenceSource = startMatch[1] ?? ""
  let fence = !fenceSource.isEmpty ? jsTrim(fenceSource) : "```"
  let language = isTruthy(startMatch[2]) ? startMatch[2] : nil
  // `startMatch[1]` keeps the whitespace that precedes the opening fence.
  let fenceIndent =
    !fenceSource.isEmpty ? fenceSource.jsLength - trimStart(fenceSource).jsLength : 0

  if children == nil, var linesInBetween {
    if linesInBetween.count == 1 {
      if endMatch != nil {
        codeBlockNode = try createCodeNode(language)
        code = stripFenceIndent(linesInBetween[0], fenceIndent)
      } else {
        codeBlockNode = try createCodeNode(language)
        code =
          linesInBetween[0].jsStartsWith(" ")
          ? linesInBetween[0].jsSlice(1)
          : linesInBetween[0]
      }
    } else {
      codeBlockNode = try createCodeNode(language)

      if linesInBetween.count > 0 {
        if jsTrim(linesInBetween[0]).isEmpty {
          linesInBetween.removeFirst()
        } else if linesInBetween[0].jsStartsWith(" ") {
          linesInBetween[0] = linesInBetween[0].jsSlice(1)
        }
      }

      while let last = linesInBetween.last, last.isEmpty {
        linesInBetween.removeLast()
      }

      code =
        linesInBetween
        .map { stripFenceIndent($0, fenceIndent) }
        .joined(separator: "\n")
    }

    try setState(codeBlockNode, codeFenceState, fence)

    let textNode = try createTextNode(code)
    try codeBlockNode.append(textNode)
    try rootNode.append(codeBlockNode)
  } else if let children {
    if !isImport && isUnreplaceableBlock(rootNode) {
      return false
    }
    _ = try createBlockNode { match in
      try createCodeNode(match[2])
    }(rootNode, children, startMatch, isImport)
  }
  return nil
}

/// `CODE`.
nonisolated(unsafe) public let CODE = MultilineElementTransformer(
  handleImportAfterStartMatch: codeHandleImportAfterStartMatch,
  dependencies: [CodeNode.self],
  export: codeExport,
  regExpStart: CODE_START_REGEX,
  regExpEnd: MultilineRegExpEnd(optional: true, regExp: CODE_END_REGEX),
  replace: codeReplace)

/// The `export` of the list transformers.
private func listTransformerExport(
  _ node: LexicalNode, _ exportChildren: ExportChildren, _ selection: BaseSelection?
) throws -> String? {
  guard let node = node as? ListNode else {
    return nil
  }
  return try listExport(node, exportChildren, 0, selection)
}

/// `UNORDERED_LIST`.
nonisolated(unsafe) public let UNORDERED_LIST = ElementTransformer(
  dependencies: [ListNode.self, ListItemNode.self],
  export: listTransformerExport,
  regExp: UNORDERED_LIST_REGEX,
  replace: listReplace("bullet"),
  triggerOnEnter: true)

/// `CHECK_LIST`.
nonisolated(unsafe) public let CHECK_LIST = ElementTransformer(
  dependencies: [ListNode.self, ListItemNode.self],
  export: listTransformerExport,
  regExp: CHECK_LIST_REGEX,
  replace: listReplace("check"),
  triggerOnEnter: true)

/// `ORDERED_LIST`.
nonisolated(unsafe) public let ORDERED_LIST = ElementTransformer(
  dependencies: [ListNode.self, ListItemNode.self],
  export: listTransformerExport,
  regExp: ORDERED_LIST_REGEX,
  replace: listReplace("number"),
  triggerOnEnter: true)

/// `INLINE_CODE`.
nonisolated(unsafe) public let INLINE_CODE = TextFormatTransformer(format: ["code"], tag: "`")

/// `HIGHLIGHT`.
nonisolated(unsafe) public let HIGHLIGHT = TextFormatTransformer(format: ["highlight"], tag: "==")

/// `BOLD_ITALIC_STAR`.
nonisolated(unsafe) public let BOLD_ITALIC_STAR = TextFormatTransformer(
  format: ["bold", "italic"], tag: "***")

/// `BOLD_ITALIC_UNDERSCORE`.
nonisolated(unsafe) public let BOLD_ITALIC_UNDERSCORE = TextFormatTransformer(
  format: ["bold", "italic"], tag: "___", intraword: false)

/// `BOLD_STAR`.
nonisolated(unsafe) public let BOLD_STAR = TextFormatTransformer(format: ["bold"], tag: "**")

/// `BOLD_UNDERSCORE`.
nonisolated(unsafe) public let BOLD_UNDERSCORE = TextFormatTransformer(
  format: ["bold"], tag: "__", intraword: false)

/// `STRIKETHROUGH`.
nonisolated(unsafe) public let STRIKETHROUGH = TextFormatTransformer(
  format: ["strikethrough"], tag: "~~")

/// `ITALIC_STAR`.
nonisolated(unsafe) public let ITALIC_STAR = TextFormatTransformer(format: ["italic"], tag: "*")

/// `ITALIC_UNDERSCORE`.
nonisolated(unsafe) public let ITALIC_UNDERSCORE = TextFormatTransformer(
  format: ["italic"], tag: "_", intraword: false)

// A raw link destination cannot hold an ASCII control character or a space in
// CommonMark, nor any Unicode whitespace in the raw branch of `LINK`'s import
// patterns, so export writes a URL with any of them in the pointy form. Import
// still reads control characters in a raw destination, as older exports wrote
// them there. A NUL reads back as U+FFFD in a CommonMark reader whichever form
// it is in, since no spelling of it avoids that.
nonisolated(unsafe) private let NOT_RAW_DESTINATION = JSRegExp(#"[\s\0-\x1f\x7f]"#)

nonisolated(unsafe) private let CHARACTER_REFERENCE_AMPERSAND = JSRegExp(
  #"&(?=#\d+;|#[Xx][\dA-Fa-f]+;|[A-Za-z][\dA-Za-z]*;)"#, "g")
nonisolated(unsafe) private let BACKTICKS = JSRegExp(#"`"#, "g")
nonisolated(unsafe) private let LINE_ENDINGS = JSRegExp(#"[\r\n]"#, "g")
nonisolated(unsafe) private let POINTY_DESTINATION_SYNTAX = JSRegExp(#"[\\<>]"#, "g")
nonisolated(unsafe) private let RAW_DESTINATION_SYNTAX = JSRegExp(#"[\\()]|^<"#, "g")
nonisolated(unsafe) private let TITLE_SYNTAX = JSRegExp(#"[\\"]"#, "g")

// A link destination or title is read back through `unescapeText`, which
// decodes a numeric character reference, and a CommonMark reader decodes the
// named ones too. So an `&` that begins one goes out as `&#38;`, or the part
// would come back as the character the reference names. An `&` that begins
// nothing is an ordinary character and stays as it is, so a query string keeps
// the separators it was written with.
//
// Then every match of `syntax`, a global regular expression for the characters
// the part's shape treats as syntax, gets a backslash. So does a backtick: this
// package's importer reads code spans before links, so one anywhere in the
// link could pair with one in the text after it and swallow the link, although
// a CommonMark reader would not let it.
//
// Neither part may hold a line ending either. CommonMark allows none in a
// destination and no blank line in a title, where two CRs make one as surely
// as two LFs, and the importer reads one LF-separated line at a time, so even
// a single LF would split the link. Each goes out as the character reference
// that `unescapeText` and a CommonMark reader both turn back into the line
// ending, written after the `&` escape so that one leaves it alone.
private func escapeLinkPart(_ value: String, _ syntax: JSRegExp) -> String {
  value
    .jsReplace(CHARACTER_REFERENCE_AMPERSAND, template: "&#38;")
    .jsReplace(syntax, template: #"\$&"#)
    .jsReplace(BACKTICKS, template: #"\`"#)
    .jsReplace(LINE_ENDINGS) { toCharacterReference($0.text) }
}

nonisolated(unsafe) private let ESCAPE_OR_BRACKET = JSRegExp(#"\\.|[[\]]"#, "g")

private let OPEN_BRACKET = UInt16(UInt8(ascii: "["))
private let CLOSE_BRACKET = UInt16(UInt8(ascii: "]"))

/// `getLinkLabelOpener(text)`: masks escaped and code brackets without
/// changing source offsets, then finds an inner opener after a literal
/// prefix. Returns -1 for balanced labels, or `nil` (JS `null`) when
/// unmatched closers leave no recoverable opener.
package func getLinkLabelOpener(_ text: String) -> Int? {
  let codeSpans = scanCodeSpans(text)
  var codeIndex = 0
  let brackets = text.jsReplace(ESCAPE_OR_BRACKET) { part in
    let index = part.index
    while codeIndex < codeSpans.count && index >= codeSpans[codeIndex].endIndex {
      codeIndex += 1
    }
    return (codeIndex < codeSpans.count && index >= codeSpans[codeIndex].startIndex)
      || part.text.jsCharAt(0) == "\\"
      ? String(repeating: " ", count: part.length)
      : part.text
  }
  var openers: [Int] = []
  var unmatchedCloser = false
  for (index, character) in brackets.utf16.enumerated() {
    if character == OPEN_BRACKET {
      openers.append(index)
    } else if character == CLOSE_BRACKET {
      if openers.popLast() == nil { unmatchedCloser = true }
    }
  }
  return openers.popLast() ?? (unmatchedCloser ? nil : -1)
}

/// `unescapeLinkPrefix(text)`.
private func unescapeLinkPrefix(_ text: String) -> String {
  var output = ""
  var previous = 0
  // Keep existing code-prefix source intact; only plain text has escapes.
  for span in scanCodeSpans(text) {
    output +=
      unescapeText(text.jsSlice(previous, span.startIndex))
      + text.jsSlice(span.startIndex, span.endIndex)
    previous = span.endIndex
  }
  return output + unescapeText(text.jsSlice(previous))
}

/// `LINK.export`.
private func linkExport(
  _ node: LexicalNode, _ exportChildren: ExportChildren,
  _: (TextNode, String) throws -> String
) throws -> String? {
  guard let node = node as? LinkNode, !isAutoLinkNode(node) else {
    return nil
  }
  let textContent = try exportChildren(node)
  let title = try node.getTitle()

  // A URL that `NOT_RAW_DESTINATION` matches goes in the pointy form, where
  // `<`, `>` and a backslash are syntax. An empty URL goes there too, since
  // the raw form has nothing left to match. Everywhere else the destination
  // is written raw, where a parenthesis would close it early, a backslash
  // would start an escape, and only a `<` in first place turns it into the
  // pointy form. An angle bracket anywhere else is an ordinary character and
  // goes out as it is.
  let rawUrl = try node.getURL()
  let url =
    rawUrl.isEmpty || NOT_RAW_DESTINATION.test(rawUrl)
    ? "<\(escapeLinkPart(rawUrl, POINTY_DESTINATION_SYNTAX))>"
    : escapeLinkPart(rawUrl, RAW_DESTINATION_SYNTAX)

  // In the double-quoted title only `"` and a backslash are syntax.
  let linkContent =
    isTruthy(title)
    ? "[\(textContent)](\(url) \"\(escapeLinkPart(title!, TITLE_SYNTAX))\")"
    : "[\(textContent)](\(url))"

  return linkContent
}

/// `LINK.replace`. https://spec.commonmark.org/0.31.2/#inline-link
private func linkReplace(_ textNode: TextNode, _ match: JSRegExpMatch) throws -> TextNode? {
  let source = match.input
  let startIndex = match.index
  let linkText = match[1] ?? ""
  if try findMatchingParent(textNode, isLinkNode) != nil
    || isEscaped(source, startIndex)
    || isEscaped(source, startIndex + linkText.jsLength + 1)
  {
    return nil
  }
  let pointyLinkUrl = match[2]
  let rawLinkUrl = match[3]
  let quotedTitle = match[4]
  let apostrophedTitle = match[5]
  let parenthesizedTitle = match[6]

  // At most one destination shape matched, either may legitimately be empty,
  // and `[a]()` matches with no destination at all, so none of this can fall
  // back on truthiness.
  let linkUrl = unescapeText(pointyLinkUrl ?? rawLinkUrl ?? "")
  // A title has three spellings and only the one that matched is defined.
  let rawLinkTitle = quotedTitle ?? apostrophedTitle ?? parenthesizedTitle
  var attributes = LinkAttributes()
  if let rawLinkTitle {
    attributes.title = .some(unescapeText(rawLinkTitle))
  }
  let linkNode = try createLinkNode(linkUrl, attributes)
  // Import passes the label from its real opener, so this finds none, but
  // a custom caller may pass a match that starts at an earlier `[`.
  guard let openBracketIndex = getLinkLabelOpener(linkText) else { return nil }
  var parsedLinkText = linkText
  var outsideLinkText = ""
  if openBracketIndex != -1 {
    outsideLinkText = "[" + unescapeLinkPrefix(linkText.jsSlice(0, openBracketIndex))
    parsedLinkText = linkText.jsSlice(openBracketIndex + 1)
  }
  // Both new nodes stand in for the TextNode being replaced, so the text
  // left outside the link carries its inline format just like the link's own
  // text node does. Read the format before the replace below detaches it.
  let format = try textNode.getFormat()
  let linkTextNode = try createTextNode(parsedLinkText)
  try linkTextNode.setFormat(format)
  try linkNode.append(linkTextNode)
  try textNode.replace(linkNode)

  if !outsideLinkText.isEmpty {
    try linkNode.insertBefore(try createTextNode(outsideLinkText).setFormat(format))
  }
  return linkTextNode
}

// Order of text transformers matters:
//
// - code should go first as it prevents any transformations inside
// - then longer tags match (e.g. ** or __ should go before * or _)

/// `LINK`.
nonisolated(unsafe) public let LINK = TextMatchTransformer(
  dependencies: [LinkNode.self],
  export: linkExport,
  // A link destination comes in two shapes, is itself optional, and may have
  // whitespace on either side of it inside the parentheses. Between `<` and
  // `>` it holds anything but a line ending and an unescaped angle bracket,
  // whitespace included. Written raw it may not begin with `<`, and it holds a
  // backslash and whatever follows it, a balanced pair of parentheses, or any
  // other character that is not a space, a parenthesis or a backslash. An
  // angle bracket anywhere but the first character is an ordinary character
  // there. A backslash in front of whitespace escapes nothing, so that branch
  // takes the backslash on its own and lets the whitespace end the
  // destination.
  //
  // The parentheses nest three deep. A regular expression cannot count them,
  // so the depth is a limit written down rather than a general rule, and no
  // URL in the wild reaches past the one level a disambiguated Wikipedia
  // article needs.
  //
  // A title comes after the destination and whitespace, in any of the three
  // spellings CommonMark gives it. Inside every shape here no two alternatives
  // can match at the same place, so none of them has anything to backtrack
  // over.
  //
  // The trailing whitespace sits inside the optional group with the
  // destination rather than after it. Outside, it would neighbour the leading
  // `\s*` whenever the destination is absent, and two runs of the same
  // whitespace side by side can be split between them in as many ways as there
  // are characters, which costs a quadratic walk of every run that never
  // reaches the closing parenthesis.
  importRegExp: JSRegExp(
    #"(?:\[(.+?)\])(?:\(\s*(?:(?:<((?:\\[^\n\r]|[^<>\n\r\\])*)>|((?!<)(?:\\[^\s]|\\(?=\s)|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\])*\))*\))*\)|[^\s()\\])+))(?:\s+(?:"((?:\\[\s\S]|[^"\\])*)"|'((?:\\[\s\S]|[^'\\])*)'|\(((?:\\[\s\S]|[^()\\])*)\)))?\s*)?\))"#
  ),
  regExp: JSRegExp(
    #"(?:\[([^[\]]*(?:\[[^[\]]*\][^[\]]*)*)\])(?:\(\s*(?:(?:<((?:\\[^\n\r]|[^<>\n\r\\])*)>|((?!<)(?:\\[^\s]|\\(?=\s)|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\]|\((?:\\[^\s]|[^\s()\\])*\))*\))*\)|[^\s()\\])+))(?:\s+(?:"((?:\\[\s\S]|[^"\\])*)"|'((?:\\[\s\S]|[^'\\])*)'|\(((?:\\[\s\S]|[^()\\])*)\)))?\s*)?\))$"#
  ),
  replace: linkReplace,
  trigger: ")")

/// `ELEMENT_TRANSFORMERS`.
nonisolated(unsafe) public let ELEMENT_TRANSFORMERS: [ElementTransformer] = [
  HEADING,
  QUOTE,
  UNORDERED_LIST,
  ORDERED_LIST,
]

/// `MULTILINE_ELEMENT_TRANSFORMERS`.
nonisolated(unsafe) public let MULTILINE_ELEMENT_TRANSFORMERS: [MultilineElementTransformer] = [
  CODE
]

// Order of text format transformers matters:
//
// - code should go first as it prevents any transformations inside
// - then longer tags match (e.g. ** or __ should go before * or _)

/// `TEXT_FORMAT_TRANSFORMERS`.
nonisolated(unsafe) public let TEXT_FORMAT_TRANSFORMERS: [TextFormatTransformer] = [
  INLINE_CODE,
  BOLD_ITALIC_STAR,
  BOLD_ITALIC_UNDERSCORE,
  BOLD_STAR,
  BOLD_UNDERSCORE,
  HIGHLIGHT,
  ITALIC_STAR,
  ITALIC_UNDERSCORE,
  STRIKETHROUGH,
]

/// `TEXT_MATCH_TRANSFORMERS`.
nonisolated(unsafe) public let TEXT_MATCH_TRANSFORMERS: [TextMatchTransformer] = [LINK]

/// `concatTransformers(...lists)`.
private func concatTransformers(_ lists: [any Transformer]...) -> [any Transformer] {
  lists.flatMap { $0 }
}

/// `TRANSFORMERS`.
nonisolated(unsafe) public let TRANSFORMERS: [any Transformer] = concatTransformers(
  ELEMENT_TRANSFORMERS,
  MULTILINE_ELEMENT_TRANSFORMERS,
  TEXT_FORMAT_TRANSFORMERS,
  TEXT_MATCH_TRANSFORMERS)

/// `normalizeMarkdown(input, shouldMergeAdjacentLines)`.
package func normalizeMarkdown(_ input: String, _ shouldMergeAdjacentLines: Bool = false) -> String
{
  let lines = input.jsSplit("\n")
  var codeBlockFenceLength = 0
  var sanitizedLines: [String] = []

  for i in 0..<lines.count {
    let rawLine = lines[i]
    let line = trimEnd(rawLine)
    let lastLine = sanitizedLines.last
    let hardLineBreak = i < lines.count - 1 ? parseMarkdownHardLineBreak(rawLine) : nil
    let lastLineHasHardLineBreak =
      lastLine != nil && parseMarkdownHardLineBreak(lastLine!) != nil

    // Code blocks of ```single line``` don't toggle the inCodeBlock flag
    if CODE_SINGLE_LINE_REGEX.test(line) {
      sanitizedLines.append(line)
      continue
    }

    if codeBlockFenceLength == 0 {
      // An opening fence may carry an info string (e.g. ```ts)
      if let openMatch = line.jsMatch(CODE_START_REGEX) {
        codeBlockFenceLength = jsTrim(openMatch[1] ?? "").jsLength
        sanitizedLines.append(line)
        continue
      }
    } else {
      // A code block is closed only by a bare fence (no info string) that is at
      // least as long as the opening fence. Fence-like lines that carry an info
      // string (e.g. a nested ```ts) are part of the code block's content.
      if CODE_END_REGEX.test(line) && jsTrim(line).jsLength >= codeBlockFenceLength {
        codeBlockFenceLength = 0
        sanitizedLines.append(line)
        continue
      }
      // Inside a code block, keep the line unchanged
      sanitizedLines.append(rawLine)
      continue
    }

    // In markdown the concept of "empty paragraphs" does not exist.
    // Blocks must be separated by an empty line. Non-empty adjacent lines must be merged.
    if line.isEmpty || lastLine == nil || lastLine!.isEmpty
      || HEADING_REGEX.test(lastLine!)
      || HEADING_REGEX.test(line)
      || QUOTE_REGEX.test(line)
      || ORDERED_LIST_REGEX.test(line)
      || UNORDERED_LIST_REGEX.test(line)
      || CHECK_LIST_REGEX.test(line)
      || TABLE_ROW_REG_EXP.test(line)
      || isTableRowDivider(line)
      || lastLineHasHardLineBreak
      || !shouldMergeAdjacentLines
      || TAG_START_REGEX.test(line)
      || TAG_END_REGEX.test(line)
      || TAG_END_AT_END_REGEX.test(lastLine!)
      || TAG_START_AT_END_REGEX.test(lastLine!)
      || CODE_END_REGEX.test(lastLine!)
    {
      // When not merging, preserve trailing whitespace (e.g. hard line-break
      // markers "  " or non-breaking spaces). Whitespace-only lines still
      // collapse to '' because trimEnd() already reduced them, so they
      // continue to act as paragraph separators.
      sanitizedLines.append(
        (!shouldMergeAdjacentLines && !line.isEmpty) || hardLineBreak != nil ? rawLine : line)
    } else {
      sanitizedLines[sanitizedLines.count - 1] =
        lastLine! + " " + trimStart(hardLineBreak == nil ? line : rawLine)
    }
  }

  return sanitizedLines.joined(separator: "\n")
}
