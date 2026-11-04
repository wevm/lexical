import Lexical
import LexicalHTML

// Port of packages/lexical-devtools-core/src/generateContent.ts: the text of
// the playground's tree view. Link, mark and table nodes aren't ported yet,
// so their branches are left out.

/// A command the tree view lists (`LexicalCommandLog`'s entries).
public struct LexicalCommandLogEntry: Sendable {
  public let index: Int
  public let type: String
  /// How the payload prints.
  public let payload: String

  public init(index: Int, type: String, payload: String) {
    self.index = index
    self.type = type
    self.payload = payload
  }
}

/// `CustomPrintNodeFn`: what to print for a node instead of the default, or
/// `nil` (or an empty string) for the default.
public typealias CustomPrintNode = (LexicalNode, Bool) throws -> String?

private enum Symbols {
  static let ancestorHasNextSibling = "|"
  static let ancestorIsLastChild = " "
  static let hasNextSibling = "├"
  static let isLastChild = "└"
  static let selectedChar = "^"
  static let selectedLine = ">"
}

private let formatNames: [TextFormatType] = [
  "bold", "code", "italic", "strikethrough", "subscript", "superscript", "underline", "highlight",
]

/// `generateContent(editor, commandsLog, exportDOM, customPrintNode,
/// obfuscateText)`: the tree of the editor's state with its selection,
/// the commands and the editor's settings, or, with `exportDOM`, the
/// state's HTML, indented.
public func generateContent(
  _ editor: Editor, _ commandsLog: [LexicalCommandLogEntry], _ exportDOM: Bool,
  _ customPrintNode: CustomPrintNode? = nil, _ obfuscateText: Bool = false
) throws -> String {
  let editorState = editor.getEditorState()
  if exportDOM {
    return try editorState.read(editor: editor) {
      try printPrettyHTML(generateHtmlFromNodes(editor))
    }
  }

  var res = " root\n"
  let selectionString = try editorState.read(editor: editor) { () throws -> String in
    let selection = try getSelection()
    try visitTree(getRoot()) { node, indent, slotName in
      let nodeKeyDisplay = "(\(node.getKey()))"
      let typeDisplay = node.getType()
      let isSelected = try node.isSelected()
      let slotPrefix = slotName.map { "[slot: \($0)] " } ?? ""
      let slotMeta = slotValueMetaSuffix(node, slotName)
      res +=
        "\(isSelected ? Symbols.selectedLine : " ") \(indent.joined(separator: " ")) \(nodeKeyDisplay) \(slotPrefix)\(typeDisplay)\(slotMeta) \(try printNode(node, customPrintNode, obfuscateText))\n"
      res += try printSelectedCharsLine(
        indent: indent, isSelected: isSelected, node: node, nodeKeyDisplay: nodeKeyDisplay,
        selection: selection, typeDisplay: typeDisplay)
    }
    switch selection {
    case nil: return ": null"
    case let range as RangeSelection: return try printRangeSelection(range)
    case let nodes as NodeSelection:
      return ": node\n  └ [\(Array(nodes._nodes).joined(separator: ", "))]"
    default: return ""
    }
  }

  res += "\n selection" + selectionString
  res += "\n\n commands:"
  if commandsLog.isEmpty {
    res += "\n  └ None dispatched."
  } else {
    for entry in commandsLog {
      res += "\n  └ \(entry.index). { type: \(entry.type), payload: \(entry.payload) }"
    }
  }
  res += "\n\n editor (v\(Editor.version)):"
  res += "\n  └ namespace \(editor._config.namespace)"
  if let compositionKey = editor._compositionKey {
    res += "\n  └ compositionKey \(compositionKey)"
  }
  res += "\n  └ editable \(editor._editable)"
  return res
}

private func printRangeSelection(_ selection: RangeSelection) throws -> String {
  let formatText = printFormatProperties { selection.hasFormat($0) }
  let anchorSlot = try resolveSlotContext(selection.anchor.getNode())
  let slotSuffix =
    anchorSlot.map {
      " (in slot \"\($0.name)\" of \($0.host.getType()) { key: \($0.host.getKey()) })"
    }
    ?? ""
  var res =
    ": range\(slotSuffix) \(formatText.isEmpty ? "" : "{ \(formatText) }") \(selection.style.isEmpty ? "" : "{ style: \(selection.style) } ")"
  let anchor = selection.anchor
  let focus = selection.focus
  res +=
    "\n  ├ anchor { key: \(anchor.key), offset: \(anchor.offset), type: \(anchor.type.rawValue) }"
  res += "\n  └ focus { key: \(focus.key), offset: \(focus.offset), type: \(focus.type.rawValue) }"
  return res
}

/// `$resolveSlotContext`: the slot `node` is in, through parents and slot
/// hosts.
private func resolveSlotContext(_ node: LexicalNode) throws -> (name: String, host: LexicalNode)? {
  var cursor: LexicalNode? = node
  while let current = cursor {
    if let name = try getSlotNameWithinHost(current), let host = try getSlotHost(current) {
      return (name, host)
    }
    cursor = try current.getParent() ?? getSlotHost(current)
  }
  return nil
}

private func slotValueMetaSuffix(_ node: LexicalNode, _ slotName: String?) -> String {
  guard slotName != nil else { return "" }
  if let element = node as? ElementNode, element.isShadowRoot() {
    return " [shadow-root]"
  }
  if node is DecoratorNode, (try? node.isInline()) == false {
    return " [atomic decorator]"
  }
  return ""
}

/// `$visitTree`: slots first, then children, each with its indent.
private func visitTree(
  _ currentNode: LexicalNode, _ indent: [String] = [],
  _ visitor: (LexicalNode, [String], String?) throws -> Void
) throws {
  var slotEntries: [(String, LexicalNode)] = []
  for slotName in try getSlotNames(currentNode) {
    if let slotNode = try getSlot(currentNode, slotName) {
      slotEntries.append((slotName, slotNode))
    }
  }
  let childNodes = try (currentNode as? ElementNode)?.getChildren() ?? []
  let entries: [(String?, LexicalNode)] =
    slotEntries.map { ($0.0, $0.1) } + childNodes.map { (nil, $0) }
  for (i, (slotName, node)) in entries.enumerated() {
    let isLast = i == entries.count - 1
    try visitor(node, indent + [isLast ? Symbols.isLastChild : Symbols.hasNextSibling], slotName)
    if node is ElementNode || node is DecoratorNode {
      try visitTree(
        node, indent + [isLast ? Symbols.ancestorIsLastChild : Symbols.ancestorHasNextSibling],
        visitor)
    }
  }
}

/// `normalize`: tabs and newlines as escapes, and with `obfuscateText`,
/// every other non-space character as `*`.
private func normalize(_ text: String, _ obfuscateText: Bool) -> String {
  let printed = text.replacing("\t", with: "\\t").replacing("\n", with: "\\n")
  guard obfuscateText else { return printed }
  return String(printed.map { $0.isWhitespace ? $0 : "*" })
}

private func printNode(
  _ node: LexicalNode, _ customPrintNode: CustomPrintNode?, _ obfuscateText: Bool
) throws -> String {
  if let custom = try customPrintNode?(node, obfuscateText), !custom.isEmpty {
    return custom
  }
  if let text = node as? TextNode {
    let content = try text.getTextContent()
    let title = content.isEmpty ? "(empty)" : "\"\(normalize(content, obfuscateText))\""
    let properties = try printAllTextNodeProperties(text)
    return jsTrim(properties.isEmpty ? title : "\(title) { \(properties) }")
  }
  if let paragraph = node as? ParagraphNode {
    let formatText = try printFormatProperties { try paragraph.hasTextFormat($0) }
    var data = formatText.isEmpty ? "" : "{ \(formatText) }"
    if !paragraph.__style.isEmpty { data += "(\(paragraph.__style))" }
    return data
  }
  return ""
}

private func printAllTextNodeProperties(_ node: TextNode) throws -> String {
  let detail = try [
    node.isDirectionless() ? "directionless" : nil, node.isUnmergeable() ? "unmergeable" : nil,
  ].compactMap { $0 }
  let mode = try [node.isToken() ? "token" : nil, node.isSegmented() ? "segmented" : nil]
    .compactMap { $0 }
  return [
    try printFormatProperties { try node.hasFormat($0) },
    detail.isEmpty ? "" : "detail: " + detail.joined(separator: ", "),
    mode.isEmpty ? "" : "mode: " + mode.joined(separator: ", "),
    printStateProperties(node),
  ].filter { !$0.isEmpty }.joined(separator: ", ")
}

private func printFormatProperties(
  _ hasFormat: (TextFormatType) throws -> Bool
) rethrows
  -> String
{
  let names = try formatNames.filter { try hasFormat($0) }
  return names.isEmpty ? "" : "format: " + names.joined(separator: ", ")
}

private func printStateProperties(_ node: LexicalNode) -> String {
  guard let state = node.__state else { return "" }
  let states = state.INTERNAL_nonDefaultKnownStates().map {
    "[\($0.key): \($0.value?.stringify() ?? "undefined")]"
  }
  return states.isEmpty ? "" : "state: " + states.joined(separator: ",")
}

/// `$printSelectedCharsLine`: a line of `^` under a text node's selected
/// characters.
private func printSelectedCharsLine(
  indent: [String], isSelected: Bool, node: LexicalNode, nodeKeyDisplay: String,
  selection: BaseSelection?, typeDisplay: String
) throws -> String {
  guard let node = node as? TextNode, let selection = selection as? RangeSelection, isSelected
  else { return "" }
  let anchor = selection.anchor
  let focus = selection.focus
  if try node.getTextContent().isEmpty
    || (anchor.getNode() === focus.getNode() && anchor.offset == focus.offset)
  {
    return ""
  }
  let (start, end) = try getSelectionStartEnd(node, selection)
  if start == end { return "" }
  let selectionLastIndent =
    indent.last == Symbols.hasNextSibling
    ? Symbols.ancestorHasNextSibling : Symbols.ancestorIsLastChild
  let indentionChars = indent.dropLast() + [selectionLastIndent]
  // 1 for the space after the type, 1 for the double quote.
  let padding = String(repeating: " ", count: nodeKeyDisplay.count + typeDisplay.count + 2)
  let chars =
    padding + String(repeating: " ", count: max(0, start + 1))
    + String(repeating: Symbols.selectedChar, count: max(0, end - start))
  return [Symbols.selectedLine, indentionChars.joined(separator: " "), chars]
    .joined(separator: " ") + "\n"
}

/// `$getSelectionStartEnd`: the selected UTF-16 range of `node`'s text, in
/// printed characters (a tab or newline prints as two).
private func getSelectionStartEnd(
  _ node: TextNode, _ selection: RangeSelection
) throws -> (
  Int, Int
) {
  let anchor = selection.anchor
  let focus = selection.focus
  let units = Array(try node.getTextContent().utf16)
  var (start, end) = (-1, -1)
  if anchor.type == .text, focus.type == .text {
    let anchorNode = try anchor.getNode()
    let focusNode = try focus.getNode()
    if anchorNode === focusNode, node === anchorNode, anchor.offset != focus.offset {
      (start, end) = (min(anchor.offset, focus.offset), max(anchor.offset, focus.offset))
    } else if node === anchorNode {
      (start, end) =
        try anchorNode.isBefore(focusNode) ? (anchor.offset, units.count) : (0, anchor.offset)
    } else if node === focusNode {
      (start, end) =
        try focusNode.isBefore(anchorNode) ? (focus.offset, units.count) : (0, focus.offset)
    } else {
      (start, end) = (0, units.count)
    }
  }
  // `String.prototype.slice` clamps, and a negative start counts from the
  // end.
  func slice(_ from: Int, _ to: Int) -> ArraySlice<UInt16> {
    let n = units.count
    let a = from < 0 ? max(0, n + from) : min(from, n)
    let b = to < 0 ? max(0, n + to) : min(to, n)
    return a < b ? units[a..<b] : []
  }
  let isWide: (UInt16) -> Bool = { $0 == 0x09 || $0 == 0x0A }
  let before = slice(0, start).filter(isWide).count
  let inside = slice(start, end).filter(isWide).count
  return (start + before, end + before + inside)
}

/// `printPrettyHTML`: the HTML with each element on its own line, indented
/// two spaces a level.
private func printPrettyHTML(_ html: String) -> String {
  let document = DOMParser().parseFromString(
    "<div>\(jsTrim(html))</div>")
  guard let div = document.body?.children.first else { return "" }
  prettify(div, 0, document)
  return div.innerHTML
}

private func prettify(_ node: DOMElement, _ level: Int, _ document: DOMDocument) {
  let indentBefore = String(repeating: "  ", count: level)
  let indentAfter = String(repeating: "  ", count: max(0, level - 1))
  let children = node.children
  for (i, child) in children.enumerated() {
    _ = node.insertBefore(document.createTextNode("\n" + indentBefore), child)
    prettify(child, level + 1, document)
    if i == children.count - 1 {
      _ = node.appendChild(document.createTextNode("\n" + indentAfter))
    }
  }
}
