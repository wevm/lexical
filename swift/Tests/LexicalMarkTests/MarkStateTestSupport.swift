import Lexical
import LexicalClipboard
import LexicalCodeCore
import LexicalExtension
import LexicalHTML
import LexicalLink
import LexicalMark
import LexicalRichText
import LexicalTesting
import Testing

// The helpers and test nodes of
// packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts.
//
// Upstream mounts some editors on a contenteditable root to read the DOM
// the reconciler made. Swift has no DOM reconciler (the views render), so
// those editors are mounted on a `TestRootElement`, which runs the model
// reconciler, and the rendered DOM of a leaf is what the editor's render
// config makes of it: `createDOM`, then `decorateDOM` (`renderedLeaves`).

let P = "Revenue grew 4% in Q3."

/// `ChipNode`: an inline decorator whose text is `@`.
final class ChipNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ChipNode.self, type: "test-chip", extends: DecoratorNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("span")
  }
  override func isInline() throws -> Bool { true }
  override func getTextContent() throws -> String { "@" }
  override func getTextContentSize() throws -> Int { 1 }
}

/// `FigureNode`: a block decorator that hosts a `caption` slot.
final class FigureNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(FigureNode.self, type: "test-figure", extends: DecoratorNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("figure")
  }
  override func isInline() throws -> Bool { false }
}

/// `TitledParagraphNode`: a paragraph with a named slot.
final class TitledParagraphNode: ParagraphNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      TitledParagraphNode.self, type: "test-titled-paragraph", extends: ParagraphNode.self,
      slots: ["title"])
  }
}

/// `EmptyInlineDecoratorNode`: an inline decorator without text.
final class EmptyInlineDecoratorNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      EmptyInlineDecoratorNode.self, type: "test-empty-inline-decorator",
      extends: DecoratorNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("span")
  }
  override func isInline() throws -> Bool { true }
}

/// `BracketedInlineNode`: an inline element that shows its text in
/// brackets.
final class BracketedInlineNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(BracketedInlineNode.self, type: "test-bracketed-inline", extends: ElementNode.self)
  }
  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    getDocument().createElement("span")
  }
  override func isInline() throws -> Bool { true }
  override func getTextContent() throws -> String { "[" + (try super.getTextContent()) + "]" }
  override func getTextContentSize() throws -> Int { try getTextContent().utf16.count }
}

/// `TestDecoratorTextNode`: a `DecoratorTextNode` whose text is `#`.
final class TestDecoratorTextNode: DecoratorTextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      TestDecoratorTextNode.self, type: "test-decorator-text", extends: DecoratorTextNode.self)
  }
  override func getTextContent() throws -> String { "#" }
  override func getTextContentSize() throws -> Int { 1 }
}

/// `MentionNode`: a token TextNode that a transform creates from text.
final class MentionNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(MentionNode.self, type: "test-mention", extends: TextNode.self)
  }
}

final class ReplacementTextNode: TextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ReplacementTextNode.self, type: "test-replacement-text", extends: TextNode.self)
  }
}

func createMentionNode(_ text: String) throws -> MentionNode {
  try create(MentionNode.self).setTextContent(text).setMode("token")
}

/// `JSON.stringify(text)`.
func quoted(_ text: String) -> String {
  JSONValue.string(text).stringify()
}

func describeNode(_ node: LexicalNode) throws -> String {
  let ids = try getNodeMarkIDs(node)
  let tag = ids.isEmpty ? "" : "[\(ids.joined(separator: ","))]"
  if let text = node as? TextNode {
    return quoted(try text.getTextContent()) + tag
  }
  if node is LineBreakNode {
    return "br" + tag
  }
  if let link = node as? LinkNode {
    return "link(\(try link.getChildren().map(describeNode).joined(separator: " ")))"
  }
  if let mark = node as? MarkNode {
    return
      "mark[\(try mark.getIDs().joined(separator: ","))](\(try mark.getChildren().map(describeNode).joined(separator: " ")))"
  }
  if let element = node as? ElementNode {
    return
      "\(element.getType())(\(try element.getChildren().map(describeNode).joined(separator: " ")))"
  }
  if node is FigureNode {
    let caption = try getSlot(node, "caption")
    return "\(node.getType())\(tag){\(try caption.map(describeNode) ?? "")}"
  }
  return node.getType() + tag
}

func treeString() throws -> String {
  try getRoot().getChildren().map(describeNode).joined(separator: " / ")
}

func paragraph(_ index: Int = 0) throws -> ElementNode {
  try getRoot().getChildAtIndex(index) as! ElementNode
}

/// `$locate(paragraph, offset, preferNext)`: the text node and offset at a
/// character offset of a paragraph.
func locate(
  _ paragraph: ElementNode, _ offset: Int, _ preferNext: Bool = false
) throws -> (TextNode, Int) {
  var acc = 0
  let texts = try paragraph.getAllTextNodes()
  for (i, node) in texts.enumerated() {
    let size = try node.getTextContentSize()
    if offset < acc + size || (offset == acc + size && (!preferNext || i == texts.count - 1)) {
      return (node, offset - acc)
    }
    acc += size
  }
  throw LexicalError("offset \(offset) out of range")
}

@discardableResult
func select(
  _ start: Int, _ end: Int? = nil, _ block: ElementNode? = nil
) throws
  -> RangeSelection
{
  let end = end ?? start
  let target = try block ?? paragraph()
  let (startNode, startOffset) = try locate(target, start, start != end)
  let (endNode, endOffset) = try locate(target, end)
  let selection = createRangeSelection()
  try selection.anchor.set(startNode.getKey(), startOffset, .text)
  try selection.focus.set(endNode.getKey(), endOffset, .text)
  try setSelection(selection)
  return selection
}

func initDocument(_ text: String = P) throws {
  try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
}

struct MarkEditorOptions {
  var disabled = false
  var exportIDsToHTML = false
  var isMarkIDShown: ((String) -> Bool)? = nil
  var markableNodes: [MarkableNodeMatch]? = nil
  var withRoot = false
}

/// `createTestEditor(options)` of the upstream test.
func createMarkEditor(_ options: MarkEditorOptions = MarkEditorOptions()) throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[root]",
      dependencies: [
        RichTextExtension, LinkExtension, MarkExtension, CoreImportExtension,
        configExtension(MarkStateExtension) {
          $0.disabled = options.disabled
          $0.exportIDsToHTML = options.exportIDsToHTML
          if let isMarkIDShown = options.isMarkIDShown {
            $0.isMarkIDShown = isMarkIDShown
          }
          $0.markableNodes = options.markableNodes ?? DEFAULT_MARKABLE_NODES
        },
      ],
      nodes: {
        [
          .klass(ChipNode.self), .klass(FigureNode.self), .klass(BracketedInlineNode.self),
          .klass(EmptyInlineDecoratorNode.self), .klass(TestDecoratorTextNode.self),
          .klass(TitledParagraphNode.self),
        ]
      },
      editorConfig: InitialEditorConfig(
        theme: ["mark": .string("mark"), "markOverlap": .string("mark-overlap")])))
  if options.withRoot {
    try editor.setRootElement(TestRootElement())
  }
  return editor
}

/// `editor.update(fn, {discrete: true})`, which leaves the microtasks
/// queued, as a synchronous JS test does.
func update(_ editor: Editor, _ fn: @escaping () throws -> Void) throws {
  try editor.update(discrete: true, fn)
}

func tree(_ editor: Editor) throws -> String {
  try editor.read(treeString)
}

func mark(_ editor: Editor, _ start: Int, _ end: Int, _ id: String) throws {
  try update(editor) {
    try addMarkID(id, select(start, end))
  }
}

func markIDNodeKeys(
  _ editor: Editor, _ flavor: MarkFlavorExtension = MarkStateExtension
) throws -> JSMap<String, [NodeKey]> {
  try getExtensionDependencyFromEditor(editor, flavor).output.markIDNodeKeys.value
}

/// JS `Array.prototype.sort()` on strings: by UTF-16 code unit.
func jsSort(_ values: [String]) -> [String] {
  values.sorted { $0.utf16.lexicographicallyPrecedes($1.utf16) }
}

/// id -> text of the nodes that carry it, from the markIDNodeKeys output.
func registryText(
  _ editor: Editor, _ flavor: MarkFlavorExtension = MarkStateExtension
) throws -> [String: [String]] {
  try editor.read {
    let map = try markIDNodeKeys(editor, flavor)
    var result: [String: [String]] = [:]
    for (id, keys) in map {
      result[id] = jsSort(
        try keys.map { key in try getNodeByKey(key).map { try $0.getTextContent() } ?? "<missing>" }
      )
    }
    return result
  }
}

func typeText(_ editor: Editor, _ text: String) throws {
  try update(editor) {
    _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, text)
  }
}

/// Clipboard or drag data with `data` by MIME type.
func dataTransfer(_ data: [(String, String)]) -> DataTransfer {
  let transfer = DataTransfer()
  for (type, value) in data {
    transfer.setData(type, value)
  }
  return transfer
}

/// A paste event with `text` as its plain text clipboard data, and
/// `lexical` as its Lexical data and `html` as its HTML when given.
func pasteEvent(
  _ text: String, _ lexical: String? = nil, _ html: String? = nil
)
  -> PasteCommandType
{
  var data = [("text/plain", text)]
  if let lexical {
    data.append(("application/x-lexical-editor", lexical))
  }
  if let html {
    data.append(("text/html", html))
  }
  return .clipboard(ClipboardEvent("paste", clipboardData: dataTransfer(data)))
}

/// The ids of the text node whose text includes `text`.
func nodeMarkIDsOfText(_ text: String) throws -> [String] {
  guard
    let node = try getRoot().getAllTextNodes().first(where: {
      try $0.getTextContent().contains(text)
    })
  else {
    throw LexicalError("no text node with \(text)")
  }
  return try getNodeMarkIDs(node)
}

/// The DOM the editor renders for `node`: its render config's `createDOM`,
/// then `decorateDOM`, as the reconciler runs them.
func renderDOM(_ editor: Editor, _ node: LexicalNode) throws -> DOMElement {
  let dom = getEditorDOMRenderConfig(editor)
  let element = try dom.createDOM(node, editor)
  try dom.decorateDOM(node, nil, element, editor)
  return element
}

/// `[text, ids attribute, class]` of each text leaf of the editor, as the
/// upstream test reads each `span` of the editor DOM.
func renderedLeaves(
  _ editor: Editor, _ attribute: String = MARK_STATE_IDS_ATTRIBUTE
) throws
  -> [[String?]]
{
  try editor.read {
    try getRoot().getAllTextNodes().map { node in
      let element = try renderDOM(editor, node)
      return [try node.getTextContent(), element.getAttribute(attribute), element.className]
    }
  }
}

/// The text leaves whose rendered DOM has `attribute`, as
/// `root.querySelectorAll('[attr]')` finds them.
func renderedWithAttribute(
  _ editor: Editor, _ attribute: String = MARK_STATE_IDS_ATTRIBUTE
)
  throws -> [[String?]]
{
  try renderedLeaves(editor, attribute).filter { $0[1] != nil }
}

/// The JSON of a serialized clipboard payload.
func clipboardJSON(_ selected: SerializedSelectedNodes) -> String {
  JSONValue.object(selected.toJSON()).stringify()
}

/// Dispatches `COMPOSITION_START_COMMAND`, inside an update, and then does
/// what JS's `$handleCompositionStart` does on jsdom and Swift's core leaves
/// out (its empty space heuristic, which DESIGN.md says the views replace):
/// where the composition cannot write into the caret's TextNode as it is
/// (an element point, a selection, a toggled format or style, a token or
/// segmented node, an edge that takes no text), it inserts
/// `COMPOSITION_START_CHAR` through `CONTROLLED_TEXT_INSERTION_COMMAND` and
/// composes in the node that holds it.
func dispatchCompositionStart(_ editor: Editor) throws {
  let wasComposing = editor.isComposing()
  _ = try editor.dispatchCommand(COMPOSITION_START_COMMAND, CompositionEvent("compositionstart"))
  guard !wasComposing, let selection = try getSelection() as? RangeSelection else { return }
  let anchor = selection.anchor
  var prepare = anchor.type == .element || !selection.isCollapsed()
  if !prepare, let text = try anchor.getNode() as? TextNode {
    prepare =
      try text.getFormat() != selection.format || text.getStyle() != selection.style
      || text.isToken() || text.isSegmented()
      || (anchor.offset == 0 && !text.canInsertTextBefore())
      || (anchor.offset == text.getTextContentSize() && !text.canInsertTextAfter())
  }
  if prepare {
    _ = try editor.dispatchCommand(CONTROLLED_TEXT_INSERTION_COMMAND, "\u{200B}")
    if let updated = try getSelection() as? RangeSelection {
      try setCompositionKey(updated.anchor.key)
    }
  }
}

/// The `$` record of each child of the first paragraph in the JSON.
func paragraphChildStates(_ editor: Editor) throws -> [JSONValue?] {
  let json = try editor.getEditorState().toJSON()
  let paragraph = json["root"]?.objectValue?["children"]?.arrayValue?.first?.objectValue
  return (paragraph?["children"]?.arrayValue ?? []).map { $0.objectValue?["$"] }
}

/// `{"markIds": ids}`.
func markIdsState(_ ids: String...) -> JSONValue {
  .object(JSONObject([(MARK_STATE_KEY, .array(ids.map { .string($0) }))]))
}

/// A stand-in for Prism's JavaScript grammar (`CodePrismExtension`'s
/// default language), adapted from `TestCodeTokenizer` of
/// LexicalCodeCoreTests: `let`, `const` and `var` are keywords, runs of
/// digits numbers, `=` an operator and an identifier in capitals (Prism's
/// `constant`, `/\b[A-Z](?:[A-Z_]|\dx?)*\b/`) a constant; other text is
/// plain. With `splitWords`, every other word is a `variable` token too,
/// so text is split across CodeHighlightNodes at each word.
struct MarkTestCodeTokenizer: CodeTokenizer {
  var splitWords = false
  var defaultLanguage: String? = DEFAULT_CODE_LANGUAGE

  func languageState(_ language: String) -> CodeLanguageState {
    language == DEFAULT_CODE_LANGUAGE ? .loaded : .unsupported
  }

  func tokenize(_ code: String, _ language: String) -> [CodeToken] {
    var tokens: [CodeToken] = []
    var plain = ""
    func flush() {
      if !plain.isEmpty { tokens.append(.text(plain)) }
      plain = ""
    }
    var rest = Substring(code)
    while let first = rest.first {
      let word = rest.prefix { $0.isLetter || $0.isNumber || $0 == "_" }
      let digits = rest.prefix { $0.isNumber }
      let type: String? =
        if !digits.isEmpty {
          "number"
        } else if ["let", "const", "var"].contains(word) {
          "keyword"
        } else if let initial = word.first, initial.isUppercase,
          word.allSatisfy({ $0.isUppercase || $0 == "_" || $0.isNumber })
        {
          "constant"
        } else if first == "=" {
          "operator"
        } else if splitWords && !word.isEmpty {
          "variable"
        } else {
          nil
        }
      let run = !digits.isEmpty ? digits : !word.isEmpty ? word : rest.prefix(1)
      if let type {
        flush()
        tokens.append(.token(type, String(run)))
      } else {
        plain += run
      }
      rest = rest.dropFirst(run.count)
    }
    flush()
    return tokens
  }
}

extension MarkTestCodeTokenizer: CustomTestStringConvertible {
  var testDescription: String { splitWords ? "word tokens" : "Prism-like tokens" }
}
