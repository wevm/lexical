import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText
import LexicalSelection
import LexicalTesting
import LexicalUtils
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockContainers.test.ts.

private final class PlainBoxNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(PlainBoxNode.self, type: "test_plain_box", extends: ElementNode.self)
  }
}

private class DeclaredBoxNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      DeclaredBoxNode.self, type: "test_declared_box", extends: ElementNode.self, children: .block)
  }
}

private final class ShadowBoxNode: ElementNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ShadowBoxNode.self, type: "test_shadow_box", extends: ElementNode.self)
  }
  override func isShadowRoot() -> Bool { true }
}

private final class ComponentNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(ComponentNode.self, type: "test_component", extends: DecoratorNode.self)
  }
  override func isInline() throws -> Bool { false }
  override func decorate(_ editor: Editor) throws -> Any? { nil }
}

private func box(_ kind: String, _ depth: Int, _ inner: [LexicalNode]) throws -> LexicalNode {
  func make() throws -> ElementNode {
    switch kind {
    case "plain": try create(PlainBoxNode.self)
    case "declared": try create(DeclaredBoxNode.self)
    default: try create(ShadowBoxNode.self)
    }
  }
  var node: LexicalNode = try make().append(contentsOf: inner)
  for _ in 1..<max(depth, 1) {
    node = try make().append(node)
  }
  return node
}

/// root > p(before) > CONTAINER(inner) > p(after)
private func build(_ ctx: String, _ inner: () throws -> [LexicalNode]) throws {
  let root = try getRoot().clear()
  try root.append(createParagraphNode().append(createTextNode("before")))
  if ctx == "root" {
    try root.append(contentsOf: inner())
  } else if ctx == "slot-shadow" {
    let c = try create(ComponentNode.self)
    _ = try setSlot(c, "body", create(ShadowBoxNode.self).append(contentsOf: inner()))
    try root.append(c)
  } else if ctx == "slot-bare" {
    let c = try create(ComponentNode.self)
    _ = try setSlot(c, "body", inner()[0])
    try root.append(c)
  } else {
    let kind = ctx.replacing("2", with: "")
    try root.append(box(kind, ctx.hasSuffix("2") ? 2 : 1, inner()))
  }
  try root.append(createParagraphNode().append(createTextNode("after")))
}

private let NAMES: [String: String] = [
  "code": "code", "heading": "h", "list": "list", "listitem": "li", "paragraph": "p",
  "quote": "quote", "test_component": "component", "test_declared_box": "dbox",
  "test_plain_box": "box", "test_shadow_box": "sbox",
]

private func describeNode(_ n: LexicalNode) throws -> String {
  let type = n.getType()
  let name = NAMES[type] ?? type
  if let text = n as? TextNode {
    return JSONValue.string(try text.getTextContent()).stringify()
  }
  if type == "linebreak" {
    return "br"
  }
  var extra: [String] = []
  if let element = n as? ElementNode, type != "listitem", try element.getIndent() > 0 {
    extra.append("indent\(try element.getIndent())")
  }
  if let list = n as? ListNode {
    extra.append(try list.getListType())
  }
  let slots = try getSlotNames(n).map { "\($0):\(try describeNode(getSlot(n, $0)!))" }
    .joined(separator: " ")
  let kids = try (n as? ElementNode)?.getChildren().map(describeNode).joined(separator: " ") ?? ""
  return
    "\(name)\(extra.isEmpty ? "" : "[" + extra.joined(separator: ",") + "]")\(slots.isEmpty ? "" : "{" + slots + "}")(\(kids))"
}

private func shapeOfRoot() throws -> String {
  try getRoot().getChildren().map(describeNode).joined(separator: " / ")
}

private func findText(_ text: String) throws -> TextNode {
  func visit(_ n: LexicalNode) throws -> TextNode? {
    if let t = n as? TextNode, try t.getTextContent() == text {
      return t
    }
    for s in try getSlotNames(n) {
      if let r = try visit(getSlot(n, s)!) { return r }
    }
    if let element = n as? ElementNode {
      for c in try element.getChildren() {
        if let r = try visit(c) { return r }
      }
    }
    return nil
  }
  guard let found = try visit(getRoot()) else {
    throw LexicalError("no text \(text)")
  }
  return found
}

private func selectRange(_ a: TextNode, _ ao: Int, _ b: TextNode, _ bo: Int) throws {
  let sel = createRangeSelection()
  try sel.anchor.set(a.getKey(), ao, .text)
  try sel.focus.set(b.getKey(), bo, .text)
  try setSelection(sel)
}

private func p(_ t: String) throws -> ParagraphNode {
  try createParagraphNode().append(createTextNode(t))
}

private struct Cmd {
  let inner: () throws -> [LexicalNode]
  /// Typed a character per discrete update after `run`.
  var type = ""
  /// Run in the same update as the build (after it).
  let run: (Editor) throws -> Void
}

private func twoParagraphs() throws -> [LexicalNode] { [try p("abc"), try p("def")] }

nonisolated(unsafe) private let CMDS: [String: Cmd] = [
  "backspace-after-container": Cmd(inner: twoParagraphs) { editor in
    _ = try findText("after").select(0, 0)
    try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
  },
  "backspace-at-container-start": Cmd(inner: twoParagraphs) { editor in
    _ = try findText("abc").select(0, 0)
    try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, true)
  },
  "delete-at-container-end": Cmd(inner: twoParagraphs) { editor in
    _ = try findText("def").select(3, 3)
    try editor.dispatchCommand(DELETE_CHARACTER_COMMAND, false)
  },
  "indent": Cmd(inner: twoParagraphs) { editor in
    _ = try findText("abc").select(1, 1)
    try editor.dispatchCommand(INDENT_CONTENT_COMMAND)
  },
  "insertNodeToNearestRoot-hr-end": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("def").select(3, 3)
    try insertNodeToNearestRoot(createHorizontalRuleNode())
  },
  "insertNodeToNearestRoot-hr-mid": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("abc").select(1, 1)
    try insertNodeToNearestRoot(createHorizontalRuleNode())
  },
  "insertNodes-blocks": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("abc").select(1, 1)
    try (getSelection() as! RangeSelection).insertNodes([p("X"), p("Y")])
  },
  "insertNodes-heading": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("abc").select(1, 1)
    try (getSelection() as! RangeSelection).insertNodes([
      createHeadingNode("h2").append(createTextNode("H"))
    ])
  },
  "markdown-heading": Cmd(inner: { [try createParagraphNode(), try p("def")] }, type: "# x") { _ in
    // caret in the empty first paragraph of the container
    let def = try findText("def")
    _ = try (def.getParentOrThrow().getPreviousSibling() as! ElementNode).select()
  },
  "markdown-list": Cmd(inner: { [try createParagraphNode(), try p("def")] }, type: "- x") { _ in
    let def = try findText("def")
    _ = try (def.getParentOrThrow().getPreviousSibling() as! ElementNode).select()
  },
  "enter-empty-last-listitem": Cmd(inner: {
    [
      try createListNode("bullet").append(
        createListItemNode().append(createTextNode("abc")), createListItemNode())
    ]
  }) { editor in
    let list = try findText("abc").getParentOrThrow().getParentOrThrow()
    _ = try (list.getLastChildOrThrow() as! ElementNode).select()
    try editor.dispatchCommand(INSERT_PARAGRAPH_COMMAND)
  },
  "insertList-collapsed": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("abc").select(1, 1)
    try insertList("bullet")
  },
  "insertList-quote-in-container": Cmd(inner: {
    [try createQuoteNode().append(createTextNode("abc")), try p("def")]
  }) { _ in
    _ = try findText("abc").select(1, 1)
    try insertList("bullet")
  },
  "insertList-range": Cmd(inner: twoParagraphs) { _ in
    try selectRange(findText("abc"), 1, findText("def"), 1)
    try insertList("number")
  },
  "outdent": Cmd(inner: { [try p("abc").setIndent(1), try p("def")] }) { editor in
    _ = try findText("abc").select(1, 1)
    try editor.dispatchCommand(OUTDENT_CONTENT_COMMAND)
  },
  "removeList": Cmd(inner: {
    [
      try createListNode("bullet").append(
        createListItemNode().append(createTextNode("abc")),
        createListItemNode().append(createTextNode("def")))
    ]
  }) { _ in
    _ = try findText("abc").select(1, 1)
    try removeList()
  },
  "setBlocksType-code-range": Cmd(inner: twoParagraphs) { _ in
    try selectRange(findText("abc"), 1, findText("def"), 1)
    try setBlocksType(getSelection()) { try createCodeNode() }
  },
  "setBlocksType-heading": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("abc").select(1, 1)
    try setBlocksType(getSelection()) { try createHeadingNode("h1") }
  },
  "setBlocksType-quote": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("abc").select(1, 1)
    try setBlocksType(getSelection()) { try createQuoteNode() }
  },
  "wrapNodes-quote": Cmd(inner: twoParagraphs) { _ in
    _ = try findText("abc").select(1, 1)
    try wrapNodes(getSelection()!) { try createQuoteNode() }
  },
]

/// One string when the extension makes no difference, else without and
/// with NodeStructureExtension.
private enum Expected {
  case same(String)
  case split(String, String)

  func with(_ structure: Bool) -> String {
    switch self {
    case .same(let s): s
    case .split(let without, let with): structure ? with : without
    }
  }
}

// The expected shape for each command and context, upstream's EXPECTED
// table in full (the commands that need other packages included, for the
// test of the table itself).
private let EXPECTED: [(String, [(String, Expected)])] = [
  (
    "backspace-after-container",
    [
      ("declared", .same(##"dbox(p("abc") p("defafter"))"##)),
      ("declared2", .same(##"dbox(dbox(p("abc") p("defafter")))"##)),
      ("plain", .same(##"box(p("abc") p("defafter"))"##)),
      ("plain2", .same(##"box(box(p("abc") p("defafter")))"##)),
      ("root", .same(##"p("abc") / p("defafter")"##)),
      ("shadow", .same(##"sbox(p("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("abc") p("def")))"##)),
      ("slot-bare", .same(##"p("after")"##)),
      ("slot-shadow", .same(##"p("after")"##)),
    ]
  ),
  (
    "backspace-at-container-start",
    [
      ("declared", .same(##"p("beforeabc") / dbox(p("def"))"##)),
      ("declared2", .same(##"p("beforeabc") / dbox(dbox(p("def")))"##)),
      ("plain", .same(##"p("beforeabc") / box(p("def"))"##)),
      ("plain2", .same(##"p("beforeabc") / box(box(p("def")))"##)),
      ("root", .same(##"p("beforeabc") / p("def")"##)),
      ("shadow", .same(##"sbox(p("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("abc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(p("abc") p("def"))}()"##)),
    ]
  ),
  (
    "delete-at-container-end",
    [
      ("declared", .same(##"dbox(p("abc") p("defafter"))"##)),
      ("declared2", .same(##"dbox(dbox(p("abc") p("defafter")))"##)),
      ("plain", .same(##"box(p("abc") p("defafter"))"##)),
      ("plain2", .same(##"box(box(p("abc") p("defafter")))"##)),
      ("root", .same(##"p("abc") / p("defafter")"##)),
      ("shadow", .same(##"sbox(p("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("abc") p("def")))"##)),
      ("slot-shadow", .same(##"component{body:sbox(p("abc") p("def"))}()"##)),
    ]
  ),
  (
    "enter-empty-last-listitem",
    [
      (
        "declared",
        .split(
          ##"dbox(list[bullet](li("abc") li() li()))"##, ##"dbox(list[bullet](li("abc")) p())"##)
      ),
      (
        "declared2",
        .split(
          ##"dbox(dbox(list[bullet](li("abc") li() li())))"##,
          ##"dbox(dbox(list[bullet](li("abc")) p()))"##)
      ),
      ("plain", .same(##"box(list[bullet](li("abc") li() li()))"##)),
      ("plain2", .same(##"box(box(list[bullet](li("abc") li() li())))"##)),
      ("root", .same(##"list[bullet](li("abc")) / p()"##)),
      ("shadow", .same(##"sbox(list[bullet](li("abc")) p())"##)),
      ("shadow2", .same(##"sbox(sbox(list[bullet](li("abc")) p()))"##)),
      ("slot-bare", .same(##"component{body:list[bullet](li("abc") li() li())}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(list[bullet](li("abc")) p())}()"##)),
    ]
  ),
  (
    "indent",
    [
      ("declared", .same(##"dbox(p[indent1]("abc") p("def"))"##)),
      ("declared2", .same(##"dbox(dbox(p[indent1]("abc") p("def")))"##)),
      ("plain", .same(##"box(p[indent1]("abc") p("def"))"##)),
      ("plain2", .same(##"box(box(p[indent1]("abc") p("def")))"##)),
      ("root", .same(##"p[indent1]("abc") / p("def")"##)),
      ("shadow", .same(##"sbox(p[indent1]("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p[indent1]("abc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p[indent1]("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(p[indent1]("abc") p("def"))}()"##)),
    ]
  ),
  (
    "insertList-collapsed",
    [
      (
        "declared",
        .split(
          ##"list[bullet](li(p("abc") p("def")))"##, ##"dbox(list[bullet](li("abc")) p("def"))"##)
      ),
      (
        "declared2",
        .split(
          ##"list[bullet](li(dbox(p("abc") p("def"))))"##,
          ##"dbox(dbox(list[bullet](li("abc")) p("def")))"##)
      ),
      (
        "plain",
        .split(##"list[bullet](li(p("abc") p("def")))"##, ##"list[bullet](li("abc" br "def"))"##)
      ),
      (
        "plain2",
        .split(
          ##"list[bullet](li(box(p("abc") p("def"))))"##,
          ##"box(box(list[bullet](li("abc")) p("def")))"##)
      ),
      ("root", .same(##"list[bullet](li("abc")) / p("def")"##)),
      ("shadow", .same(##"sbox(list[bullet](li("abc")) p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(list[bullet](li("abc")) p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(list[bullet](li("abc")) p("def"))}()"##)),
    ]
  ),
  (
    "insertList-quote-in-container",
    [
      (
        "declared",
        .split(
          ##"list[bullet](li(quote("abc") p("def")))"##,
          ##"dbox(list[bullet](li("abc")) p("def"))"##)
      ),
      (
        "declared2",
        .split(
          ##"list[bullet](li(dbox(quote("abc") p("def"))))"##,
          ##"dbox(dbox(list[bullet](li("abc")) p("def")))"##)
      ),
      (
        "plain",
        .split(
          ##"list[bullet](li(quote("abc") p("def")))"##, ##"box(list[bullet](li("abc")) p("def"))"##
        )
      ),
      (
        "plain2",
        .split(
          ##"list[bullet](li(box(quote("abc") p("def"))))"##,
          ##"box(box(list[bullet](li("abc")) p("def")))"##)
      ),
      ("root", .same(##"list[bullet](li("abc")) / p("def")"##)),
      ("shadow", .same(##"sbox(list[bullet](li("abc")) p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(list[bullet](li("abc")) p("def")))"##)),
      ("slot-bare", .same(##"component{body:quote("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(list[bullet](li("abc")) p("def"))}()"##)),
    ]
  ),
  (
    "insertList-range",
    [
      (
        "declared",
        .split(
          ##"list[number](li(p("abc") p("def")))"##, ##"dbox(list[number](li("abc") li("def")))"##)
      ),
      (
        "declared2",
        .split(
          ##"list[number](li(dbox(p("abc") p("def"))))"##,
          ##"dbox(dbox(list[number](li("abc") li("def"))))"##)
      ),
      (
        "plain",
        .split(##"list[number](li(p("abc") p("def")))"##, ##"list[number](li("abc" br "def"))"##)
      ),
      (
        "plain2",
        .split(
          ##"list[number](li(box(p("abc") p("def"))))"##,
          ##"box(box(list[number](li("abc") li("def"))))"##)
      ),
      ("root", .same(##"list[number](li("abc") li("def"))"##)),
      ("shadow", .same(##"sbox(list[number](li("abc") li("def")))"##)),
      ("shadow2", .same(##"sbox(sbox(list[number](li("abc") li("def"))))"##)),
      ("slot-shadow", .same(##"component{body:sbox(list[number](li("abc") li("def")))}()"##)),
    ]
  ),
  (
    "insertNodeToNearestRoot-hr-end",
    [
      (
        "declared",
        .split(
          ##"dbox(p("abc") p("def")) / horizontalrule()"##,
          ##"dbox(p("abc") p("def") horizontalrule() p())"##)
      ),
      (
        "declared2",
        .split(
          ##"dbox(dbox(p("abc") p("def"))) / horizontalrule()"##,
          ##"dbox(dbox(p("abc") p("def") horizontalrule() p()))"##)
      ),
      ("plain", .same(##"box(p("abc") p("def")) / horizontalrule()"##)),
      ("plain2", .same(##"box(box(p("abc") p("def"))) / horizontalrule()"##)),
      ("root", .same(##"p("abc") / p("def") / horizontalrule()"##)),
      ("shadow", .same(##"sbox(p("abc") p("def") horizontalrule() p())"##)),
      ("shadow2", .same(##"sbox(sbox(p("abc") p("def") horizontalrule() p()))"##)),
      ("slot-shadow", .same(##"component{body:sbox(p("abc") p("def") horizontalrule() p())}()"##)),
    ]
  ),
  (
    "insertNodeToNearestRoot-hr-mid",
    [
      (
        "declared",
        .split(
          ##"dbox(p("a")) / horizontalrule() / dbox(p("bc") p("def"))"##,
          ##"dbox(p("a") horizontalrule() p("bc") p("def"))"##)
      ),
      (
        "declared2",
        .split(
          ##"dbox(dbox(p("a"))) / horizontalrule() / dbox(dbox(p("bc") p("def")))"##,
          ##"dbox(dbox(p("a") horizontalrule() p("bc") p("def")))"##)
      ),
      ("plain", .same(##"box(p("a")) / horizontalrule() / box(p("bc") p("def"))"##)),
      ("plain2", .same(##"box(box(p("a"))) / horizontalrule() / box(box(p("bc") p("def")))"##)),
      ("root", .same(##"p("a") / horizontalrule() / p("bc") / p("def")"##)),
      ("shadow", .same(##"sbox(p("a") horizontalrule() p("bc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("a") horizontalrule() p("bc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("a" horizontalrule() "bc")}()"##)),
      (
        "slot-shadow", .same(##"component{body:sbox(p("a") horizontalrule() p("bc") p("def"))}()"##)
      ),
    ]
  ),
  (
    "insertNodes-blocks",
    [
      ("declared", .split(##"dbox(p("aXYbc") p("def"))"##, ##"dbox(p("aX") p("Ybc") p("def"))"##)),
      (
        "declared2",
        .split(##"dbox(dbox(p("aXYbc") p("def")))"##, ##"dbox(dbox(p("aX") p("Ybc") p("def")))"##)
      ),
      ("plain", .same(##"box(p("aXYbc") p("def"))"##)),
      ("plain2", .same(##"box(box(p("aXYbc") p("def")))"##)),
      ("root", .same(##"p("aX") / p("Ybc") / p("def")"##)),
      ("shadow", .same(##"sbox(p("aX") p("Ybc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("aX") p("Ybc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("aXYbc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(p("aX") p("Ybc") p("def"))}()"##)),
    ]
  ),
  (
    "insertNodes-heading",
    [
      ("declared", .same(##"dbox(p("aHbc") p("def"))"##)),
      ("declared2", .same(##"dbox(dbox(p("aHbc") p("def")))"##)),
      ("plain", .same(##"box(p("aHbc") p("def"))"##)),
      ("plain2", .same(##"box(box(p("aHbc") p("def")))"##)),
      ("root", .same(##"p("aHbc") / p("def")"##)),
      ("shadow", .same(##"sbox(p("aHbc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("aHbc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("aHbc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(p("aHbc") p("def"))}()"##)),
    ]
  ),
  (
    "markdown-heading",
    [
      ("declared", .split(##"dbox(p("# x") p("def"))"##, ##"dbox(h("x") p("def"))"##)),
      ("declared2", .split(##"dbox(dbox(p("# x") p("def")))"##, ##"dbox(dbox(h("x") p("def")))"##)),
      ("plain", .same(##"box(p("# x") p("def"))"##)),
      ("plain2", .same(##"box(box(p("# x") p("def")))"##)),
      ("root", .same(##"h("x") / p("def")"##)),
      ("shadow", .same(##"sbox(h("x") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(h("x") p("def")))"##)),
      ("slot-shadow", .same(##"component{body:sbox(h("x") p("def"))}()"##)),
    ]
  ),
  (
    "markdown-list",
    [
      (
        "declared",
        .split(##"dbox(p("- x") p("def"))"##, ##"dbox(list[bullet](li("x")) p("def"))"##)
      ),
      (
        "declared2",
        .split(
          ##"dbox(dbox(p("- x") p("def")))"##, ##"dbox(dbox(list[bullet](li("x")) p("def")))"##)
      ),
      ("plain", .same(##"box(p("- x") p("def"))"##)),
      ("plain2", .same(##"box(box(p("- x") p("def")))"##)),
      ("root", .same(##"list[bullet](li("x")) / p("def")"##)),
      ("shadow", .same(##"sbox(list[bullet](li("x")) p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(list[bullet](li("x")) p("def")))"##)),
      ("slot-shadow", .same(##"component{body:sbox(list[bullet](li("x")) p("def"))}()"##)),
    ]
  ),
  (
    "outdent",
    [
      ("declared", .same(##"dbox(p("abc") p("def"))"##)),
      ("declared2", .same(##"dbox(dbox(p("abc") p("def")))"##)),
      ("plain", .same(##"box(p("abc") p("def"))"##)),
      ("plain2", .same(##"box(box(p("abc") p("def")))"##)),
      ("root", .same(##"p("abc") / p("def")"##)),
      ("shadow", .same(##"sbox(p("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("abc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(p("abc") p("def"))}()"##)),
    ]
  ),
  (
    "removeList",
    [
      ("declared", .same(##"dbox(p("abc") p("def"))"##)),
      ("declared2", .same(##"dbox(dbox(p("abc") p("def")))"##)),
      ("plain", .same(##"box(p("abc") p("def"))"##)),
      ("plain2", .same(##"box(box(p("abc") p("def")))"##)),
      ("root", .same(##"p("abc") / p("def")"##)),
      ("shadow", .same(##"sbox(p("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(p("abc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:list[bullet](li("abc") li("def"))}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(p("abc") p("def"))}()"##)),
    ]
  ),
  (
    "setBlocksType-code-range",
    [
      ("declared", .same(##"dbox(code("abc") code("def"))"##)),
      ("declared2", .same(##"dbox(dbox(code("abc") code("def")))"##)),
      ("plain", .same(##"box(code("abc") code("def"))"##)),
      ("plain2", .same(##"box(box(code("abc") code("def")))"##)),
      ("root", .same(##"code("abc") / code("def")"##)),
      ("shadow", .same(##"sbox(code("abc") code("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(code("abc") code("def")))"##)),
      ("slot-shadow", .same(##"component{body:sbox(code("abc") code("def"))}()"##)),
    ]
  ),
  (
    "setBlocksType-heading",
    [
      ("declared", .same(##"dbox(h("abc") p("def"))"##)),
      ("declared2", .same(##"dbox(dbox(h("abc") p("def")))"##)),
      ("plain", .same(##"box(h("abc") p("def"))"##)),
      ("plain2", .same(##"box(box(h("abc") p("def")))"##)),
      ("root", .same(##"h("abc") / p("def")"##)),
      ("shadow", .same(##"sbox(h("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(h("abc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(h("abc") p("def"))}()"##)),
    ]
  ),
  (
    "setBlocksType-quote",
    [
      ("declared", .same(##"dbox(quote("abc") p("def"))"##)),
      ("declared2", .same(##"dbox(dbox(quote("abc") p("def")))"##)),
      ("plain", .same(##"box(quote("abc") p("def"))"##)),
      ("plain2", .same(##"box(box(quote("abc") p("def")))"##)),
      ("root", .same(##"quote("abc") / p("def")"##)),
      ("shadow", .same(##"sbox(quote("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(quote("abc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(quote("abc") p("def"))}()"##)),
    ]
  ),
  (
    "wrapNodes-quote",
    [
      ("declared", .split(##"quote("abc") / dbox(p("def"))"##, ##"dbox(quote("abc") p("def"))"##)),
      (
        "declared2",
        .split(##"quote("abc") / dbox(dbox(p("def")))"##, ##"dbox(dbox(quote("abc") p("def")))"##)
      ),
      ("plain", .same(##"quote("abc") / box(p("def"))"##)),
      ("plain2", .same(##"quote("abc") / box(box(p("def")))"##)),
      ("root", .same(##"quote("abc") / p("def")"##)),
      ("shadow", .same(##"sbox(quote("abc") p("def"))"##)),
      ("shadow2", .same(##"sbox(sbox(quote("abc") p("def")))"##)),
      ("slot-bare", .same(##"component{body:p("abc")}()"##)),
      ("slot-shadow", .same(##"component{body:sbox(quote("abc") p("def"))}()"##)),
    ]
  ),
]

private func expected(_ cmd: String, _ ctx: String) -> Expected? {
  EXPECTED.first { $0.0 == cmd }?.1.first { $0.0 == ctx }?.1
}

private func strip(_ shape: String) -> String {
  var s = shape
  if s.hasPrefix(#"p("before") / "#) { s.removeFirst(#"p("before") / "#.count) }
  if s.hasSuffix(#" / p("after")"#) { s.removeLast(#" / p("after")"#.count) }
  return s
}

private func buildEditor(_ structure: Bool) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(
      name: "block-containers",
      dependencies: [
        RichTextExtension, ListExtension, LinkExtension, CodeExtension, HorizontalRuleExtension,
      ]
        + (structure ? [NodeStructureExtension] : []),
      nodes: {
        [
          .klass(PlainBoxNode.self), .klass(DeclaredBoxNode.self), .klass(ShadowBoxNode.self),
          .klass(ComponentNode.self),
        ]
      }, editorConfig: InitialEditorConfig(onError: { error, _ in throw error }),
      register: { editor, _, _ in try registerMarkdownShortcuts(editor) }))
}

private func run(_ cmd: Cmd, _ ctx: String, _ structure: Bool) throws -> String {
  let editor = try buildEditor(structure)
  defer { editor.dispose() }
  try editor.setRootElement(TestRootElement())
  defer { try? editor.setRootElement(nil) }
  try editor.update(discrete: true) { try build(ctx, cmd.inner) }
  try editor.update(discrete: true) { try cmd.run(editor) }
  for ch in cmd.type.unicodeScalars {
    try editor.update(discrete: true) {
      if let selection = try getSelection() as? RangeSelection {
        try selection.insertText(String(ch))
      }
    }
  }
  return strip(try editor.read(shapeOfRoot))
}

struct StructuralCase: Sendable, CustomTestStringConvertible {
  let cmd: String
  let ctx: String
  let structure: Bool
  init(_ cmd: String, _ ctx: String, _ structure: Bool) {
    self.cmd = cmd
    self.ctx = ctx
    self.structure = structure
  }
  var testDescription: String { "\(cmd) in \(ctx) (NodeStructureExtension: \(structure))" }
}

@Suite struct BlockContainersStructuralCommandTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-after-container in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > backspace-at-container-start in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > delete-at-container-end in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > indent in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-end in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodeToNearestRoot-hr-mid in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-blocks in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertNodes-heading in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > outdent in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-heading in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-quote in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > wrapNodes-quote in slot-bare (NodeStructureExtension: true)"
    ),
    arguments: [
      StructuralCase("backspace-after-container", "root", false),
      StructuralCase("backspace-after-container", "root", true),
      StructuralCase("backspace-after-container", "plain", false),
      StructuralCase("backspace-after-container", "plain", true),
      StructuralCase("backspace-after-container", "plain2", false),
      StructuralCase("backspace-after-container", "plain2", true),
      StructuralCase("backspace-after-container", "declared", false),
      StructuralCase("backspace-after-container", "declared", true),
      StructuralCase("backspace-after-container", "declared2", false),
      StructuralCase("backspace-after-container", "declared2", true),
      StructuralCase("backspace-after-container", "shadow", false),
      StructuralCase("backspace-after-container", "shadow", true),
      StructuralCase("backspace-after-container", "shadow2", false),
      StructuralCase("backspace-after-container", "shadow2", true),
      StructuralCase("backspace-after-container", "slot-shadow", false),
      StructuralCase("backspace-after-container", "slot-shadow", true),
      StructuralCase("backspace-after-container", "slot-bare", false),
      StructuralCase("backspace-after-container", "slot-bare", true),
      StructuralCase("backspace-at-container-start", "root", false),
      StructuralCase("backspace-at-container-start", "root", true),
      StructuralCase("backspace-at-container-start", "plain", false),
      StructuralCase("backspace-at-container-start", "plain", true),
      StructuralCase("backspace-at-container-start", "plain2", false),
      StructuralCase("backspace-at-container-start", "plain2", true),
      StructuralCase("backspace-at-container-start", "declared", false),
      StructuralCase("backspace-at-container-start", "declared", true),
      StructuralCase("backspace-at-container-start", "declared2", false),
      StructuralCase("backspace-at-container-start", "declared2", true),
      StructuralCase("backspace-at-container-start", "shadow", false),
      StructuralCase("backspace-at-container-start", "shadow", true),
      StructuralCase("backspace-at-container-start", "shadow2", false),
      StructuralCase("backspace-at-container-start", "shadow2", true),
      StructuralCase("backspace-at-container-start", "slot-shadow", false),
      StructuralCase("backspace-at-container-start", "slot-shadow", true),
      StructuralCase("backspace-at-container-start", "slot-bare", false),
      StructuralCase("backspace-at-container-start", "slot-bare", true),
      StructuralCase("delete-at-container-end", "root", false),
      StructuralCase("delete-at-container-end", "root", true),
      StructuralCase("delete-at-container-end", "plain", false),
      StructuralCase("delete-at-container-end", "plain", true),
      StructuralCase("delete-at-container-end", "plain2", false),
      StructuralCase("delete-at-container-end", "plain2", true),
      StructuralCase("delete-at-container-end", "declared", false),
      StructuralCase("delete-at-container-end", "declared", true),
      StructuralCase("delete-at-container-end", "declared2", false),
      StructuralCase("delete-at-container-end", "declared2", true),
      StructuralCase("delete-at-container-end", "shadow", false),
      StructuralCase("delete-at-container-end", "shadow", true),
      StructuralCase("delete-at-container-end", "shadow2", false),
      StructuralCase("delete-at-container-end", "shadow2", true),
      StructuralCase("delete-at-container-end", "slot-shadow", false),
      StructuralCase("delete-at-container-end", "slot-shadow", true),
      StructuralCase("indent", "root", false),
      StructuralCase("indent", "root", true),
      StructuralCase("indent", "plain", false),
      StructuralCase("indent", "plain", true),
      StructuralCase("indent", "plain2", false),
      StructuralCase("indent", "plain2", true),
      StructuralCase("indent", "declared", false),
      StructuralCase("indent", "declared", true),
      StructuralCase("indent", "declared2", false),
      StructuralCase("indent", "declared2", true),
      StructuralCase("indent", "shadow", false),
      StructuralCase("indent", "shadow", true),
      StructuralCase("indent", "shadow2", false),
      StructuralCase("indent", "shadow2", true),
      StructuralCase("indent", "slot-shadow", false),
      StructuralCase("indent", "slot-shadow", true),
      StructuralCase("indent", "slot-bare", false),
      StructuralCase("indent", "slot-bare", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "root", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "root", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "plain", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "plain", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "plain2", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "plain2", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "declared", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "declared", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "declared2", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "declared2", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "shadow", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "shadow", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "shadow2", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "shadow2", true),
      StructuralCase("insertNodeToNearestRoot-hr-end", "slot-shadow", false),
      StructuralCase("insertNodeToNearestRoot-hr-end", "slot-shadow", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "root", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "root", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "plain", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "plain", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "plain2", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "plain2", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "declared", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "declared", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "declared2", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "declared2", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "shadow", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "shadow", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "shadow2", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "shadow2", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "slot-shadow", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "slot-shadow", true),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "slot-bare", false),
      StructuralCase("insertNodeToNearestRoot-hr-mid", "slot-bare", true),
      StructuralCase("insertNodes-blocks", "root", false),
      StructuralCase("insertNodes-blocks", "root", true),
      StructuralCase("insertNodes-blocks", "plain", false),
      StructuralCase("insertNodes-blocks", "plain", true),
      StructuralCase("insertNodes-blocks", "plain2", false),
      StructuralCase("insertNodes-blocks", "plain2", true),
      StructuralCase("insertNodes-blocks", "declared", false),
      StructuralCase("insertNodes-blocks", "declared", true),
      StructuralCase("insertNodes-blocks", "declared2", false),
      StructuralCase("insertNodes-blocks", "declared2", true),
      StructuralCase("insertNodes-blocks", "shadow", false),
      StructuralCase("insertNodes-blocks", "shadow", true),
      StructuralCase("insertNodes-blocks", "shadow2", false),
      StructuralCase("insertNodes-blocks", "shadow2", true),
      StructuralCase("insertNodes-blocks", "slot-shadow", false),
      StructuralCase("insertNodes-blocks", "slot-shadow", true),
      StructuralCase("insertNodes-blocks", "slot-bare", false),
      StructuralCase("insertNodes-blocks", "slot-bare", true),
      StructuralCase("insertNodes-heading", "root", false),
      StructuralCase("insertNodes-heading", "root", true),
      StructuralCase("insertNodes-heading", "plain", false),
      StructuralCase("insertNodes-heading", "plain", true),
      StructuralCase("insertNodes-heading", "plain2", false),
      StructuralCase("insertNodes-heading", "plain2", true),
      StructuralCase("insertNodes-heading", "declared", false),
      StructuralCase("insertNodes-heading", "declared", true),
      StructuralCase("insertNodes-heading", "declared2", false),
      StructuralCase("insertNodes-heading", "declared2", true),
      StructuralCase("insertNodes-heading", "shadow", false),
      StructuralCase("insertNodes-heading", "shadow", true),
      StructuralCase("insertNodes-heading", "shadow2", false),
      StructuralCase("insertNodes-heading", "shadow2", true),
      StructuralCase("insertNodes-heading", "slot-shadow", false),
      StructuralCase("insertNodes-heading", "slot-shadow", true),
      StructuralCase("insertNodes-heading", "slot-bare", false),
      StructuralCase("insertNodes-heading", "slot-bare", true),
      StructuralCase("outdent", "root", false),
      StructuralCase("outdent", "root", true),
      StructuralCase("outdent", "plain", false),
      StructuralCase("outdent", "plain", true),
      StructuralCase("outdent", "plain2", false),
      StructuralCase("outdent", "plain2", true),
      StructuralCase("outdent", "declared", false),
      StructuralCase("outdent", "declared", true),
      StructuralCase("outdent", "declared2", false),
      StructuralCase("outdent", "declared2", true),
      StructuralCase("outdent", "shadow", false),
      StructuralCase("outdent", "shadow", true),
      StructuralCase("outdent", "shadow2", false),
      StructuralCase("outdent", "shadow2", true),
      StructuralCase("outdent", "slot-shadow", false),
      StructuralCase("outdent", "slot-shadow", true),
      StructuralCase("outdent", "slot-bare", false),
      StructuralCase("outdent", "slot-bare", true),
      StructuralCase("setBlocksType-heading", "root", false),
      StructuralCase("setBlocksType-heading", "root", true),
      StructuralCase("setBlocksType-heading", "plain", false),
      StructuralCase("setBlocksType-heading", "plain", true),
      StructuralCase("setBlocksType-heading", "plain2", false),
      StructuralCase("setBlocksType-heading", "plain2", true),
      StructuralCase("setBlocksType-heading", "declared", false),
      StructuralCase("setBlocksType-heading", "declared", true),
      StructuralCase("setBlocksType-heading", "declared2", false),
      StructuralCase("setBlocksType-heading", "declared2", true),
      StructuralCase("setBlocksType-heading", "shadow", false),
      StructuralCase("setBlocksType-heading", "shadow", true),
      StructuralCase("setBlocksType-heading", "shadow2", false),
      StructuralCase("setBlocksType-heading", "shadow2", true),
      StructuralCase("setBlocksType-heading", "slot-shadow", false),
      StructuralCase("setBlocksType-heading", "slot-shadow", true),
      StructuralCase("setBlocksType-heading", "slot-bare", false),
      StructuralCase("setBlocksType-heading", "slot-bare", true),
      StructuralCase("setBlocksType-quote", "root", false),
      StructuralCase("setBlocksType-quote", "root", true),
      StructuralCase("setBlocksType-quote", "plain", false),
      StructuralCase("setBlocksType-quote", "plain", true),
      StructuralCase("setBlocksType-quote", "plain2", false),
      StructuralCase("setBlocksType-quote", "plain2", true),
      StructuralCase("setBlocksType-quote", "declared", false),
      StructuralCase("setBlocksType-quote", "declared", true),
      StructuralCase("setBlocksType-quote", "declared2", false),
      StructuralCase("setBlocksType-quote", "declared2", true),
      StructuralCase("setBlocksType-quote", "shadow", false),
      StructuralCase("setBlocksType-quote", "shadow", true),
      StructuralCase("setBlocksType-quote", "shadow2", false),
      StructuralCase("setBlocksType-quote", "shadow2", true),
      StructuralCase("setBlocksType-quote", "slot-shadow", false),
      StructuralCase("setBlocksType-quote", "slot-shadow", true),
      StructuralCase("setBlocksType-quote", "slot-bare", false),
      StructuralCase("setBlocksType-quote", "slot-bare", true),
      StructuralCase("wrapNodes-quote", "root", false),
      StructuralCase("wrapNodes-quote", "root", true),
      StructuralCase("wrapNodes-quote", "plain", false),
      StructuralCase("wrapNodes-quote", "plain", true),
      StructuralCase("wrapNodes-quote", "plain2", false),
      StructuralCase("wrapNodes-quote", "plain2", true),
      StructuralCase("wrapNodes-quote", "declared", false),
      StructuralCase("wrapNodes-quote", "declared", true),
      StructuralCase("wrapNodes-quote", "declared2", false),
      StructuralCase("wrapNodes-quote", "declared2", true),
      StructuralCase("wrapNodes-quote", "shadow", false),
      StructuralCase("wrapNodes-quote", "shadow", true),
      StructuralCase("wrapNodes-quote", "shadow2", false),
      StructuralCase("wrapNodes-quote", "shadow2", true),
      StructuralCase("wrapNodes-quote", "slot-shadow", false),
      StructuralCase("wrapNodes-quote", "slot-shadow", true),
      StructuralCase("wrapNodes-quote", "slot-bare", false),
      StructuralCase("wrapNodes-quote", "slot-bare", true),
    ])
  func structuralCommand(_ c: StructuralCase) throws {
    let want = try #require(expected(c.cmd, c.ctx)).with(c.structure)
    #expect(try run(CMDS[c.cmd]!, c.ctx, c.structure) == want)
  }

  // The list commands, which need LexicalList.
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > enter-empty-last-listitem in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-collapsed in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-quote-in-container in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > insertList-range in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-bare (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-bare (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > removeList in slot-shadow (NodeStructureExtension: true)"
    ),
    arguments: [
      StructuralCase("enter-empty-last-listitem", "declared", false),
      StructuralCase("enter-empty-last-listitem", "declared", true),
      StructuralCase("enter-empty-last-listitem", "declared2", false),
      StructuralCase("enter-empty-last-listitem", "declared2", true),
      StructuralCase("enter-empty-last-listitem", "plain", false),
      StructuralCase("enter-empty-last-listitem", "plain", true),
      StructuralCase("enter-empty-last-listitem", "plain2", false),
      StructuralCase("enter-empty-last-listitem", "plain2", true),
      StructuralCase("enter-empty-last-listitem", "root", false),
      StructuralCase("enter-empty-last-listitem", "root", true),
      StructuralCase("enter-empty-last-listitem", "shadow", false),
      StructuralCase("enter-empty-last-listitem", "shadow", true),
      StructuralCase("enter-empty-last-listitem", "shadow2", false),
      StructuralCase("enter-empty-last-listitem", "shadow2", true),
      StructuralCase("enter-empty-last-listitem", "slot-bare", false),
      StructuralCase("enter-empty-last-listitem", "slot-bare", true),
      StructuralCase("enter-empty-last-listitem", "slot-shadow", false),
      StructuralCase("enter-empty-last-listitem", "slot-shadow", true),
      StructuralCase("insertList-collapsed", "declared", false),
      StructuralCase("insertList-collapsed", "declared", true),
      StructuralCase("insertList-collapsed", "declared2", false),
      StructuralCase("insertList-collapsed", "declared2", true),
      StructuralCase("insertList-collapsed", "plain", false),
      StructuralCase("insertList-collapsed", "plain", true),
      StructuralCase("insertList-collapsed", "plain2", false),
      StructuralCase("insertList-collapsed", "plain2", true),
      StructuralCase("insertList-collapsed", "root", false),
      StructuralCase("insertList-collapsed", "root", true),
      StructuralCase("insertList-collapsed", "shadow", false),
      StructuralCase("insertList-collapsed", "shadow", true),
      StructuralCase("insertList-collapsed", "shadow2", false),
      StructuralCase("insertList-collapsed", "shadow2", true),
      StructuralCase("insertList-collapsed", "slot-bare", false),
      StructuralCase("insertList-collapsed", "slot-bare", true),
      StructuralCase("insertList-collapsed", "slot-shadow", false),
      StructuralCase("insertList-collapsed", "slot-shadow", true),
      StructuralCase("insertList-quote-in-container", "declared", false),
      StructuralCase("insertList-quote-in-container", "declared", true),
      StructuralCase("insertList-quote-in-container", "declared2", false),
      StructuralCase("insertList-quote-in-container", "declared2", true),
      StructuralCase("insertList-quote-in-container", "plain", false),
      StructuralCase("insertList-quote-in-container", "plain", true),
      StructuralCase("insertList-quote-in-container", "plain2", false),
      StructuralCase("insertList-quote-in-container", "plain2", true),
      StructuralCase("insertList-quote-in-container", "root", false),
      StructuralCase("insertList-quote-in-container", "root", true),
      StructuralCase("insertList-quote-in-container", "shadow", false),
      StructuralCase("insertList-quote-in-container", "shadow", true),
      StructuralCase("insertList-quote-in-container", "shadow2", false),
      StructuralCase("insertList-quote-in-container", "shadow2", true),
      StructuralCase("insertList-quote-in-container", "slot-bare", false),
      StructuralCase("insertList-quote-in-container", "slot-bare", true),
      StructuralCase("insertList-quote-in-container", "slot-shadow", false),
      StructuralCase("insertList-quote-in-container", "slot-shadow", true),
      StructuralCase("insertList-range", "declared", false),
      StructuralCase("insertList-range", "declared", true),
      StructuralCase("insertList-range", "declared2", false),
      StructuralCase("insertList-range", "declared2", true),
      StructuralCase("insertList-range", "plain", false),
      StructuralCase("insertList-range", "plain", true),
      StructuralCase("insertList-range", "plain2", false),
      StructuralCase("insertList-range", "plain2", true),
      StructuralCase("insertList-range", "root", false),
      StructuralCase("insertList-range", "root", true),
      StructuralCase("insertList-range", "shadow", false),
      StructuralCase("insertList-range", "shadow", true),
      StructuralCase("insertList-range", "shadow2", false),
      StructuralCase("insertList-range", "shadow2", true),
      StructuralCase("insertList-range", "slot-shadow", false),
      StructuralCase("insertList-range", "slot-shadow", true),
      StructuralCase("removeList", "declared", false),
      StructuralCase("removeList", "declared", true),
      StructuralCase("removeList", "declared2", false),
      StructuralCase("removeList", "declared2", true),
      StructuralCase("removeList", "plain", false),
      StructuralCase("removeList", "plain", true),
      StructuralCase("removeList", "plain2", false),
      StructuralCase("removeList", "plain2", true),
      StructuralCase("removeList", "root", false),
      StructuralCase("removeList", "root", true),
      StructuralCase("removeList", "shadow", false),
      StructuralCase("removeList", "shadow", true),
      StructuralCase("removeList", "shadow2", false),
      StructuralCase("removeList", "shadow2", true),
      StructuralCase("removeList", "slot-bare", false),
      StructuralCase("removeList", "slot-bare", true),
      StructuralCase("removeList", "slot-shadow", false),
      StructuralCase("removeList", "slot-shadow", true),
    ])
  func listStructuralCommand(_ c: StructuralCase) throws {
    let want = try #require(expected(c.cmd, c.ctx)).with(c.structure)
    #expect(try run(CMDS[c.cmd]!, c.ctx, c.structure) == want)
  }

  // The code command, which needs LexicalCodeCore.
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > setBlocksType-code-range in slot-shadow (NodeStructureExtension: true)"
    ),
    arguments: [
      StructuralCase("setBlocksType-code-range", "declared", false),
      StructuralCase("setBlocksType-code-range", "declared", true),
      StructuralCase("setBlocksType-code-range", "declared2", false),
      StructuralCase("setBlocksType-code-range", "declared2", true),
      StructuralCase("setBlocksType-code-range", "plain", false),
      StructuralCase("setBlocksType-code-range", "plain", true),
      StructuralCase("setBlocksType-code-range", "plain2", false),
      StructuralCase("setBlocksType-code-range", "plain2", true),
      StructuralCase("setBlocksType-code-range", "root", false),
      StructuralCase("setBlocksType-code-range", "root", true),
      StructuralCase("setBlocksType-code-range", "shadow", false),
      StructuralCase("setBlocksType-code-range", "shadow", true),
      StructuralCase("setBlocksType-code-range", "shadow2", false),
      StructuralCase("setBlocksType-code-range", "shadow2", true),
      StructuralCase("setBlocksType-code-range", "slot-shadow", false),
      StructuralCase("setBlocksType-code-range", "slot-shadow", true),
    ])
  func codeStructuralCommand(_ c: StructuralCase) throws {
    let want = try #require(expected(c.cmd, c.ctx)).with(c.structure)
    #expect(try run(CMDS[c.cmd]!, c.ctx, c.structure) == want)
  }

  // The markdown shortcut commands, which need LexicalMarkdown.
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-heading in slot-shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in root (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in plain2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in declared2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow2 (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in shadow2 (NodeStructureExtension: true)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in slot-shadow (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > markdown-list in slot-shadow (NodeStructureExtension: true)"
    ),
    arguments: [
      StructuralCase("markdown-heading", "root", false),
      StructuralCase("markdown-heading", "root", true),
      StructuralCase("markdown-heading", "plain", false),
      StructuralCase("markdown-heading", "plain", true),
      StructuralCase("markdown-heading", "plain2", false),
      StructuralCase("markdown-heading", "plain2", true),
      StructuralCase("markdown-heading", "declared", false),
      StructuralCase("markdown-heading", "declared", true),
      StructuralCase("markdown-heading", "declared2", false),
      StructuralCase("markdown-heading", "declared2", true),
      StructuralCase("markdown-heading", "shadow", false),
      StructuralCase("markdown-heading", "shadow", true),
      StructuralCase("markdown-heading", "shadow2", false),
      StructuralCase("markdown-heading", "shadow2", true),
      StructuralCase("markdown-heading", "slot-shadow", false),
      StructuralCase("markdown-heading", "slot-shadow", true),
      StructuralCase("markdown-list", "root", false),
      StructuralCase("markdown-list", "root", true),
      StructuralCase("markdown-list", "plain", false),
      StructuralCase("markdown-list", "plain", true),
      StructuralCase("markdown-list", "plain2", false),
      StructuralCase("markdown-list", "plain2", true),
      StructuralCase("markdown-list", "declared", false),
      StructuralCase("markdown-list", "declared", true),
      StructuralCase("markdown-list", "declared2", false),
      StructuralCase("markdown-list", "declared2", true),
      StructuralCase("markdown-list", "shadow", false),
      StructuralCase("markdown-list", "shadow", true),
      StructuralCase("markdown-list", "shadow2", false),
      StructuralCase("markdown-list", "shadow2", true),
      StructuralCase("markdown-list", "slot-shadow", false),
      StructuralCase("markdown-list", "slot-shadow", true),
    ])
  func markdownStructuralCommand(_ c: StructuralCase) throws {
    let want = try #require(expected(c.cmd, c.ctx)).with(c.structure)
    #expect(try run(CMDS[c.cmd]!, c.ctx, c.structure) == want)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > every command has a row of cases"
    ))
  func everyCommandHasARow() {
    #expect(EXPECTED.map(\.0).sorted() == CMDS.keys.sorted())
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::structural commands in block containers > a declared container behaves like a shadow root for every command but the joins at its edges"
    ))
  func declaredLikeShadowRoot() throws {
    for (cmd, _) in EXPECTED {
      func at(_ ctx: String) throws -> String {
        try #require(expected(cmd, ctx)).with(true)
          .replacing("dbox", with: "B").replacing("sbox", with: "B")
      }
      let joins = cmd.hasPrefix("backspace") || cmd.hasPrefix("delete")
      for (declared, shadow) in [("declared", "shadow"), ("declared2", "shadow2")] {
        if joins {
          #expect(try at(declared) != at(shadow), "\(cmd)")
        } else {
          #expect(try at(declared) == at(shadow), "\(cmd)")
        }
      }
    }
  }
}

// MARK: selections that pass over or sit on a container

private func rangeAround(_ empty: () throws -> ElementNode) throws {
  let a = try createTextNode("before")
  let b = try createTextNode("after")
  try getRoot().clear().append(
    createParagraphNode().append(a), empty(), createParagraphNode().append(b))
  try selectRange(a, 1, b, 1)
}

private func inBox(_ offset: Int) throws {
  let box = try create(DeclaredBoxNode.self).append(p("abc"))
  try getRoot().clear().append(box)
  _ = try box.select(offset, offset)
}

private func act(_ structure: Bool, _ action: @escaping () throws -> Void) throws -> String {
  let editor = try buildEditor(structure)
  defer { editor.dispose() }
  try editor.update(discrete: true, action)
  return try editor.read(shapeOfRoot)
}

@Suite struct BlockContainersSelectionTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box in a range: $setBlocksType (NodeStructureExtension)"
    ))
  func emptyDeclaredBoxInRangeSetBlocksType() throws {
    let shape = try act(true) {
      try rangeAround { try create(DeclaredBoxNode.self) }
      try setBlocksType(getSelection()) { try createHeadingNode("h1") }
    }
    #expect(shape == #"h("before") / dbox() / h("after")"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an element point on a declared box: insertParagraph (NodeStructureExtension)"
    ))
  func elementPointInsertParagraph() throws {
    let shape = try act(true) {
      try inBox(1)
      _ = try (getSelection() as! RangeSelection).insertParagraph()
    }
    #expect(shape == #"dbox(p("abc") p())"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an element point on a declared box: insertNodes (NodeStructureExtension)"
    ))
  func elementPointInsertNodes() throws {
    let shape = try act(true) {
      try inBox(1)
      try (getSelection() as! RangeSelection).insertNodes([p("X")])
    }
    #expect(shape == #"dbox(p("abc") p("X"))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a horizontal rule in a declared box: $wrapNodes (NodeStructureExtension)"
    ))
  func rangeOverRuleInDeclaredBoxWrapNodes() throws {
    let shape = try act(true) {
      try getRoot().clear().append(
        create(DeclaredBoxNode.self).append(p("abc"), createHorizontalRuleNode(), p("def")))
      try selectRange(findText("abc"), 1, findText("def"), 1)
      try wrapNodes(getSelection()!) { try createQuoteNode() }
    }
    #expect(shape == #"dbox(quote("abc") horizontalrule() quote("def"))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a horizontal rule at the root: $wrapNodes (NodeStructureExtension)"
    ))
  func rangeOverRuleAtRootWrapNodes() throws {
    let shape = try act(true) {
      try getRoot().clear().append(p("abc"), createHorizontalRuleNode(), p("def"))
      try selectRange(findText("abc"), 1, findText("def"), 1)
      try wrapNodes(getSelection()!) { try createQuoteNode() }
    }
    #expect(shape == #"quote("abc") / horizontalrule() / quote("def")"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box with a slot, element point: $wrapNodes (NodeStructureExtension)"
    ))
  func emptyDeclaredBoxWithSlotWrapNodes() throws {
    let shape = try act(true) {
      let box = try create(DeclaredBoxNode.self)
      _ = try setSlot(box, "aside", p("slot"))
      try getRoot().clear().append(p("before"), box, p("after"))
      _ = try box.select(0, 0)
      try wrapNodes(getSelection()!) { try createQuoteNode() }
    }
    #expect(shape == #"p("before") / dbox{aside:p("slot")}(quote()) / p("after")"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a declared box as a named-slot value: $wrapNodes (NodeStructureExtension)"
    ))
  func declaredBoxSlotValueWrapNodes() throws {
    let shape = try act(true) {
      let host = try create(ComponentNode.self)
      let body = try create(DeclaredBoxNode.self).append(p("abc"))
      _ = try setSlot(host, "body", body)
      try getRoot().clear().append(host)
      _ = try findText("abc").select(1, 1)
      try wrapNodes(getSelection()!) { try createQuoteNode() }
    }
    #expect(shape == #"component{body:dbox(quote("abc"))}()"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > removing the only, empty, declared box of a shadow root (NodeStructureExtension)"
    ))
  func removingOnlyDeclaredBox() throws {
    let shape = try act(true) {
      let shadow = try create(ShadowBoxNode.self).append(create(DeclaredBoxNode.self))
      try getRoot().clear().append(shadow)
      _ = try shadow.select(0, 1)
      try (getSelection() as! RangeSelection).removeText()
    }
    #expect(shape == #"sbox(p())"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > the caret in an empty declared box: $setBlocksType (NodeStructureExtension)"
    ))
  func caretInEmptyDeclaredBoxSetBlocksType() throws {
    let shape = try act(true) {
      let box = try create(DeclaredBoxNode.self)
      try getRoot().clear().append(p("before"), box, p("after"))
      _ = try box.select(0, 0)
      try setBlocksType(getSelection()) { try createHeadingNode("h1") }
    }
    #expect(shape == #"p("before") / dbox(h()) / p("after")"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > with no active editor, a declared box is not a container"
    ))
  func noActiveEditor() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(create(DeclaredBoxNode.self).append(p("x")))
    }
    #expect(try !editor.getEditorState().read { try isBlockContainer(getRoot().getFirstChild()) })
    #expect(try editor.read { try isBlockContainer(getRoot().getFirstChild()) })
  }
}

// MARK: fixes every editor gets

@Suite struct BlockContainersFixesTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty root (NodeStructureExtension: true)"
    ), arguments: [false, true])
  func wrapNodesInEmptyRoot(_ structure: Bool) throws {
    let shape = try act(structure) {
      _ = try getRoot().clear().select(0, 0)
      try wrapNodes(getSelection()!) { try createQuoteNode() }
    }
    #expect(shape == "quote()")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty shadow root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $wrapNodes in an empty shadow root (NodeStructureExtension: true)"
    ), arguments: [false, true])
  func wrapNodesInEmptyShadowRoot(_ structure: Bool) throws {
    let shape = try act(structure) {
      let shadow = try create(ShadowBoxNode.self)
      try getRoot().clear().append(p("before"), shadow, p("after"))
      _ = try shadow.select(0, 0)
      try wrapNodes(getSelection()!) { try createQuoteNode() }
    }
    #expect(shape == #"p("before") / sbox(quote()) / p("after")"#)
  }
}

// MARK: a new block put in an empty declared container

private func emptyBox() throws -> DeclaredBoxNode {
  let box = try create(DeclaredBoxNode.self).setIndent(1).setFormat("center")
  try getRoot().clear().append(p("before"), box, p("after"))
  return box
}

private func pointAt(_ point: Point) throws -> String {
  "\(try point.getNode().getType()):\(point.offset)"
}

@Suite struct BlockContainersEmptyDeclaredContainerTests {
  private func newBlock(_ action: @escaping () throws -> Void) throws -> (String, String) {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      _ = try emptyBox().select(0, 0)
      try action()
    }
    return try editor.read {
      let block = try (getRoot().getChildAtIndex(1) as! ElementNode).getFirstChild() as! ElementNode
      return (try shapeOfRoot(), try block.getFormatType())
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $setBlocksType does not take the container's indent and format"
    ))
  func setBlocksTypeNoIndentFormat() throws {
    let (shape, format) = try newBlock {
      try setBlocksType(getSelection()) { try createHeadingNode("h1") }
    }
    #expect(shape == #"p("before") / dbox[indent1](h()) / p("after")"#)
    #expect(format == "")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $wrapNodes does not take the container's indent and format"
    ))
  func wrapNodesNoIndentFormat() throws {
    let (shape, format) = try newBlock {
      try wrapNodes(getSelection()!) { try createQuoteNode() }
    }
    #expect(shape == #"p("before") / dbox[indent1](quote()) / p("after")"#)
    #expect(format == "")
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $setBlocksType keeps a range that starts in the empty container"
    ))
  func rangeStartsInEmptyContainer() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let box = try emptyBox()
      let selection = createRangeSelection()
      try selection.anchor.set(box.getKey(), 0, .element)
      try selection.focus.set(findText("after").getKey(), 2, .text)
      try setSelection(selection)
      try setBlocksType(getSelection()) { try createHeadingNode("h1") }
    }
    #expect(try editor.read(shapeOfRoot) == #"p("before") / dbox[indent1](h()) / h("after")"#)
    let points = try editor.read {
      let selection = try getSelection() as! RangeSelection
      return [try pointAt(selection.anchor), try pointAt(selection.focus)]
    }
    #expect(points == ["heading:0", "text:2"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::a new block put in an empty declared container > $setBlocksType keeps a backward range that ends in the empty container"
    ))
  func backwardRangeEndsInEmptyContainer() throws {
    let editor = try buildEditor(true)
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let box = try emptyBox()
      let selection = createRangeSelection()
      try selection.anchor.set(findText("after").getKey(), 2, .text)
      try selection.focus.set(box.getKey(), 0, .element)
      try setSelection(selection)
      try setBlocksType(getSelection()) { try createHeadingNode("h1") }
    }
    #expect(try editor.read(shapeOfRoot) == #"p("before") / dbox[indent1](h()) / h("after")"#)
    let points = try editor.read {
      let selection = try getSelection() as! RangeSelection
      return [try pointAt(selection.anchor), try pointAt(selection.focus)]
    }
    #expect(points == ["text:2", "heading:0"])
  }
}

// MARK: list cases (LexicalList)

private func selectNodes(_ nodes: LexicalNode...) throws {
  let selection = createNodeSelection()
  for node in nodes { selection.add(node.getKey()) }
  try setSelection(selection)
}

private func hrInShadow() throws {
  let hr = try createHorizontalRuleNode()
  try getRoot().clear().append(create(ShadowBoxNode.self).append(p("abc"), hr))
  try selectNodes(hr)
}

private func emptyInRange(_ make: () throws -> ElementNode) throws {
  try rangeAround(make)
}

@Suite struct BlockContainersListTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList over an empty shadow root in a range (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList over an empty shadow root in a range (NodeStructureExtension: true)"
    ), arguments: [false, true])
  func insertListOverEmptyShadowRoot(_ structure: Bool) throws {
    let shape = try act(structure) {
      try emptyInRange { try create(ShadowBoxNode.self) }
      try insertList("bullet")
    }
    #expect(shape == #"list[bullet](li("before")) / sbox() / list[bullet](li("after"))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList with a horizontal rule selected in a shadow root (NodeStructureExtension: false)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::fixes every editor gets > $insertList with a horizontal rule selected in a shadow root (NodeStructureExtension: true)"
    ), arguments: [false, true])
  func insertListHorizontalRuleInShadowRoot(_ structure: Bool) throws {
    let shape = try act(structure) {
      try hrInShadow()
      try insertList("bullet")
    }
    #expect(shape == #"sbox(p("abc") horizontalrule())"#)
  }

  // MARK: selections that pass over or sit on a container (NodeStructureExtension)

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box in a range: $insertList (NodeStructureExtension)"
    ))
  func emptyDeclaredBoxInRange() throws {
    let shape = try act(true) {
      try emptyInRange { try create(DeclaredBoxNode.self) }
      try insertList("bullet")
    }
    #expect(shape == #"list[bullet](li("before")) / dbox() / list[bullet](li("after"))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty shadow root in a range: $insertList (NodeStructureExtension)"
    ))
  func emptyShadowRootInRange() throws {
    let shape = try act(true) {
      try emptyInRange { try create(ShadowBoxNode.self) }
      try insertList("bullet")
    }
    #expect(shape == #"list[bullet](li("before")) / sbox() / list[bullet](li("after"))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an element point on a declared box: $insertList (NodeStructureExtension)"
    ))
  func elementPointOnDeclaredBox() throws {
    let shape = try act(true) {
      try inBox(0)
      try insertList("bullet")
    }
    #expect(shape == #"dbox(list[bullet](li("abc")))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range from before a declared box into it: $insertList (NodeStructureExtension)"
    ))
  func rangeIntoDeclaredBox() throws {
    let shape = try act(true) {
      let a = try createTextNode("before")
      let b = try createTextNode("in")
      try getRoot().clear().append(
        createParagraphNode().append(a),
        create(DeclaredBoxNode.self).append(createParagraphNode().append(b)))
      try selectRange(a, 1, b, 1)
      try insertList("bullet")
    }
    #expect(shape == #"list[bullet](li("before")) / dbox(list[bullet](li("in")))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a horizontal rule selected in a declared box: $insertList (NodeStructureExtension)"
    ))
  func horizontalRuleInDeclaredBox() throws {
    let shape = try act(true) {
      let hr = try createHorizontalRuleNode()
      try getRoot().clear().append(create(DeclaredBoxNode.self).append(p("abc"), hr))
      try selectNodes(hr)
      try insertList("bullet")
    }
    #expect(shape == #"dbox(p("abc") horizontalrule())"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a horizontal rule selected in a shadow root: $insertList (NodeStructureExtension)"
    ))
  func horizontalRuleInShadowRoot() throws {
    let shape = try act(true) {
      try hrInShadow()
      try insertList("bullet")
    }
    #expect(shape == #"sbox(p("abc") horizontalrule())"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a horizontal rule in a declared box: $insertList (NodeStructureExtension)"
    ))
  func rangeOverRuleInDeclaredBox() throws {
    let shape = try act(true) {
      try getRoot().clear().append(
        create(DeclaredBoxNode.self).append(p("abc"), createHorizontalRuleNode(), p("def")))
      try selectRange(findText("abc"), 1, findText("def"), 1)
      try insertList("bullet")
    }
    #expect(
      shape == #"dbox(list[bullet](li("abc")) horizontalrule() list[bullet](li("def")))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a range over a quote and a paragraph in a box that declares nothing: $insertList (NodeStructureExtension)"
    ))
  func rangeOverQuoteInPlainBox() throws {
    let shape = try act(true) {
      try getRoot().clear().append(
        create(PlainBoxNode.self).append(createQuoteNode().append(createTextNode("y")), p("x")))
      try selectRange(findText("y"), 0, findText("x"), 1)
      try insertList("bullet")
    }
    #expect(shape == #"box(list[bullet](li("y") li("x")))"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty declared box with a slot, node selected: $insertList (NodeStructureExtension)"
    ))
  func emptyDeclaredBoxWithSlotNodeSelected() throws {
    let shape = try act(true) {
      let box = try create(DeclaredBoxNode.self)
      _ = try setSlot(box, "aside", p("slot"))
      try getRoot().clear().append(p("before"), box, p("after"))
      try selectNodes(box)
      try insertList("bullet")
    }
    #expect(shape == #"p("before") / dbox{aside:p("slot")}() / p("after")"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > a declared box holding only a block decorator, element point: $insertList (NodeStructureExtension)"
    ))
  func declaredBoxHoldingDecorator() throws {
    let shape = try act(true) {
      let box = try create(DeclaredBoxNode.self).append(create(ComponentNode.self))
      try getRoot().clear().append(p("before"), box, p("after"))
      _ = try box.select(0, 0)
      try insertList("bullet")
    }
    #expect(shape == #"p("before") / dbox(component()) / p("after")"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::selections that pass over or sit on a container > an empty quote and a paragraph of a box that declares nothing, node selected: $insertList (NodeStructureExtension)"
    ))
  func emptyQuoteAndParagraphNodeSelected() throws {
    let shape = try act(true) {
      let quote = try createQuoteNode()
      let text = try createTextNode("x")
      let box = try create(PlainBoxNode.self).append(quote, createParagraphNode().append(text))
      _ = try setSlot(box, "aside", p("slot"))
      try getRoot().clear().append(box)
      try selectNodes(quote, text)
      try insertList("bullet")
    }
    #expect(shape == #"box{aside:p("slot")}(list[bullet](li() li("x")))"#)
  }

  // MARK: #9570: $insertList in a box inside a box

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > box (NodeStructureExtension: false) makes a list and keeps both boxes where the structure is enforced"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > box (NodeStructureExtension: true) makes a list and keeps both boxes where the structure is enforced"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > dbox (NodeStructureExtension: false) makes a list and keeps both boxes where the structure is enforced"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::#9570: $insertList in a box inside a box > dbox (NodeStructureExtension: true) makes a list and keeps both boxes where the structure is enforced"
    ),
    arguments: [
      ("box", false, #"list[bullet](li(box(p("abc"))))"#),
      ("box", true, #"box(box(list[bullet](li("abc"))))"#),
      ("dbox", false, #"list[bullet](li(dbox(p("abc"))))"#),
      ("dbox", true, #"dbox(dbox(list[bullet](li("abc"))))"#),
    ])
  func boxInsideBox(_ kind: String, _ structure: Bool, _ expected: String) throws {
    func make() throws -> ElementNode {
      kind == "box" ? try create(PlainBoxNode.self) : try create(DeclaredBoxNode.self)
    }
    let shape = try act(structure) {
      let text = try createTextNode("abc")
      try getRoot().clear().append(make().append(make().append(createParagraphNode().append(text))))
      _ = try text.select(1, 1)
      try insertList("bullet")
    }
    #expect(shape == expected)
  }

  // MARK: $isBlockContainer

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::$isBlockContainer > reads a children: block declaration only where NodeStructureExtension enforces it (absent)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::$isBlockContainer > reads a children: block declaration only where NodeStructureExtension enforces it (disabled)"
    ),
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockContainers.test.ts::$isBlockContainer > reads a children: block declaration only where NodeStructureExtension enforces it (enforced)"
    ), arguments: [("absent", false), ("disabled", false), ("enforced", true)])
  func isBlockContainerReadsDeclaration(_ mode: String, _ structure: Bool) throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "is-block-container",
        dependencies: [RichTextExtension, ListExtension]
          + (mode == "absent"
            ? [] : [configExtension(NodeStructureExtension) { $0.disabled = mode == "disabled" }]),
        nodes: {
          [
            .klass(PlainBoxNode.self), .klass(DeclaredBoxNode.self),
            .klass(DeclaredBoxSubclassNode.self), .klass(InlineDeclaredBoxNode.self),
            .klass(ShadowBoxNode.self),
          ]
        }))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      let list = try createListNode("bullet").append(
        createListItemNode().append(createTextNode("x")))
      let nodes: [(String, LexicalNode)] = [
        ("declared", try create(DeclaredBoxNode.self)),
        ("inherited", try create(DeclaredBoxSubclassNode.self)),
        ("inline", try create(InlineDeclaredBoxNode.self)),
        ("list", list),
        ("listItem", try list.getFirstChildOrThrow()),
        ("paragraph", try createParagraphNode()),
        ("plain", try create(PlainBoxNode.self)),
        ("quote", try createQuoteNode()),
        ("root", try getRoot()),
        ("shadow", try create(ShadowBoxNode.self)),
        ("shadowQuote", try createQuoteNode(shadowRoot: true)),
        ("text", try createTextNode("x")),
      ]
      let result = try nodes.map { "\($0.0)=\(try isBlockContainer($0.1))" }
      #expect(
        result == [
          "declared=\(structure)", "inherited=\(structure)", "inline=false", "list=false",
          "listItem=false", "paragraph=false", "plain=false", "quote=false", "root=true",
          "shadow=true", "shadowQuote=true", "text=false",
        ])
      #expect(try isBlockContainer(nil) == false)
    }
  }
}

private final class DeclaredBoxSubclassNode: DeclaredBoxNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      DeclaredBoxSubclassNode.self, type: "test_declared_box_subclass",
      extends: DeclaredBoxNode.self)
  }
}

private final class InlineDeclaredBoxNode: DeclaredBoxNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      InlineDeclaredBoxNode.self, type: "test_inline_declared_box", extends: DeclaredBoxNode.self)
  }
  override func isInline() throws -> Bool { true }
}
