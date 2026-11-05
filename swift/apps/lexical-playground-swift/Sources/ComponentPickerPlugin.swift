import Foundation
import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalList
import LexicalMenu
import LexicalRichText
import LexicalSelection

// Port of packages/lexical-playground/src/plugins/ComponentPickerPlugin: a
// "/" at the start of a word opens a menu of the blocks the Swift
// playground has, and the image, which opens the Insert Image dialog. The
// table, Excalidraw, poll, embed, equation, GIF,
// collapsible, card, pull quote, review and columns options insert nodes
// the port does not have yet, so they are left out, and so is the dynamic
// "3x4 Table" option.

/// `ComponentPickerOption`.
final class ComponentPickerOption: MenuOption {
  let symbol: String
  /// Text drawn in place of the symbol (the headings' `H1`, as the
  /// playground's `i.icon.h1` shows).
  let glyph: String?
  /// For extra searching.
  let keywords: [String]
  let onSelect: (String) throws -> Void

  init(
    _ title: String, symbol: String, glyph: String? = nil, keywords: [String] = [],
    onSelect: @escaping (String) throws -> Void
  ) {
    self.symbol = symbol
    self.glyph = glyph
    self.keywords = keywords
    self.onSelect = onSelect
    super.init(title)
    self.title = title
  }
}

/// `getBaseOptions`, for the nodes the port has, in the web's order.
@MainActor
func getBaseOptions(
  _ editor: Editor, showImageDialog: @escaping () -> Void = {}
) -> [ComponentPickerOption] {
  func setBlocks(_ create: @escaping () throws -> ElementNode) -> (String) throws -> Void {
    { _ in
      try editor.update {
        if let selection = try getSelection() as? RangeSelection {
          try setBlocksType(selection, create)
        }
      }
    }
  }
  var options = [
    ComponentPickerOption(
      "Paragraph", symbol: "text.alignleft", keywords: ["normal", "paragraph", "p", "text"],
      onSelect: setBlocks { try createParagraphNode() })
  ]
  for n in 1...3 {
    options.append(
      ComponentPickerOption(
        "Heading \(n)", symbol: "textformat", glyph: "H\(n)",
        keywords: ["heading", "header", "h\(n)"],
        onSelect: setBlocks { try createHeadingNode("h\(n)") }))
  }
  options += [
    ComponentPickerOption(
      "Numbered List", symbol: "list.number",
      keywords: ["numbered list", "ordered list", "ol"]
    ) { _ in _ = try editor.dispatchCommand(INSERT_ORDERED_LIST_COMMAND) },
    ComponentPickerOption(
      "Bulleted List", symbol: "list.bullet",
      keywords: ["bulleted list", "unordered list", "ul"]
    ) { _ in _ = try editor.dispatchCommand(INSERT_UNORDERED_LIST_COMMAND) },
    ComponentPickerOption(
      "Check List", symbol: "checklist", keywords: ["check list", "todo list"]
    ) { _ in _ = try editor.dispatchCommand(INSERT_CHECK_LIST_COMMAND) },
    ComponentPickerOption(
      "Quote", symbol: "text.quote", keywords: ["block quote"],
      onSelect: setBlocks { try createQuoteNode() }),
    ComponentPickerOption(
      "Code", symbol: "chevron.left.forwardslash.chevron.right",
      keywords: ["javascript", "python", "js", "codeblock"]
    ) { _ in
      try editor.update {
        guard let selection = try getSelection() as? RangeSelection else { return }
        if selection.isCollapsed() {
          try setBlocksType(selection) { try createCodeNode() }
        } else {
          let textContent = try selection.getTextContent()
          let codeNode = try createCodeNode()
          try selection.insertNodes([codeNode])
          try selection.insertRawText(textContent)
        }
      }
    },
    ComponentPickerOption(
      "Divider", symbol: "minus", keywords: ["horizontal rule", "divider", "hr"]
    ) { _ in _ = try editor.dispatchCommand(INSERT_HORIZONTAL_RULE_COMMAND) },
    ComponentPickerOption(
      "Image", symbol: "photo", keywords: ["image", "photo", "picture", "file"]
    ) { _ in showImageDialog() },
  ]
  for (alignment, symbol) in [
    ("left", "text.alignleft"), ("center", "text.aligncenter"), ("right", "text.alignright"),
    ("justify", "text.justify"),
  ] {
    options.append(
      ComponentPickerOption(
        "Align \(alignment)", symbol: symbol, keywords: ["align", "justify", alignment]
      ) { _ in _ = try editor.dispatchCommand(FORMAT_ELEMENT_COMMAND, alignment) })
  }
  return options
}

/// The options for `queryString`: those whose title or a keyword matches
/// it as a case-insensitive regular expression (`new RegExp(queryString,
/// 'i')`). A query that is not a valid pattern matches nothing, where JS
/// throws.
func filterComponentPickerOptions(
  _ options: [ComponentPickerOption], _ queryString: String?
) -> [ComponentPickerOption] {
  guard let queryString, !queryString.isEmpty else { return options }
  guard let regex = try? NSRegularExpression(pattern: queryString, options: .caseInsensitive)
  else { return [] }
  func test(_ text: String) -> Bool {
    regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) != nil
  }
  return options.filter { test($0.title ?? "") || $0.keywords.contains(where: test) }
}

/// `ComponentPickerMenuPlugin`.
@MainActor
func registerComponentPicker(
  _ editor: Editor, showImageDialog: @escaping () -> Void = {}
) -> (
  host: TypeaheadMenuHost<ComponentPickerOption>, unregister: () -> Void
) {
  let baseOptions = getBaseOptions(editor, showImageDialog: showImageDialog)
  var setOptions: ([ComponentPickerOption]) -> Void = { _ in }
  let (menu, unregister) = registerTypeaheadMenu(
    editor, triggerFn: basicTypeaheadTriggerMatch("/", minLength: 0, allowWhitespace: true),
    onQueryChange: { query in setOptions(filterComponentPickerOptions(baseOptions, query)) },
    onSelectOption: { (option: ComponentPickerOption, nodeToRemove, closeMenu, matchingString) in
      try nodeToRemove?.remove()
      try option.onSelect(matchingString)
      closeMenu()
    })
  setOptions = { [weak menu] in menu?.setOptions($0) }
  menu.setOptions(baseOptions)
  let host = TypeaheadMenuHost(menu: menu) {
    TypeaheadMenuRow(id: $0.key, title: $0.title ?? "", symbol: $0.symbol, glyph: $0.glyph)
  }
  return (host, unregister)
}
