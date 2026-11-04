import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx.
//
// Upstream renders a React RichTextPlugin editor with AutoFocusPlugin into
// jsdom, dispatches a `paste` event whose clipboard has only `text/html`,
// and compares the container's innerHTML. The port builds the editor with
// RichTextExtension, starts it as the composer and AutoFocusPlugin leave it
// (an empty paragraph with the selection in it), dispatches PASTE_COMMAND
// with a ClipboardEvent carrying the HTML, and compares the DOM each node's
// `createDOM` renders, as the reconciler mounts it (`renderedHTML`).
//
// Not ported: the cases that paste links or code blocks, which need
// @lexical/link and @lexical/code (skipped in the ledger).

private let theme: EditorThemeClasses = [
  "code": "editor-code",
  "heading": [
    "h1": "editor-heading-h1", "h2": "editor-heading-h2", "h3": "editor-heading-h3",
    "h4": "editor-heading-h4", "h5": "editor-heading-h5", "h6": "editor-heading-h6",
  ],
  "image": "editor-image",
  "list": [
    "listitem": "editor-listitem", "olDepth": ["editor-list-ol"], "ulDepth": ["editor-list-ul"],
  ],
  "paragraph": "editor-paragraph",
  "placeholder": "editor-placeholder",
  "quote": "editor-quote",
  "text": [
    "bold": "editor-text-bold", "code": "editor-text-code", "hashtag": "editor-text-hashtag",
    "italic": "editor-text-italic", "link": "editor-text-link",
    "strikethrough": "editor-text-strikethrough", "underline": "editor-text-underline",
    "underlineStrikethrough": "editor-text-underlineStrikethrough",
  ],
]

/// The ContentEditable's root element, which wraps the rendered blocks.
private let rootOpen =
  #"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true">"#

/// The editor of the upstream `TestBase`: rich text, with the selection at
/// the end of the initial paragraph, where AutoFocusPlugin puts it.
private func createEventHelpersEditor() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "test", dependencies: [RichTextExtension, ListExtension],
      editorConfig: InitialEditorConfig(
        theme: theme,
        initialEditorState: .update { _ in
          try getRoot().append(try createParagraphNode()).selectEnd()
        })))
  // React's `act` flushes the initial commit before the test reads the DOM.
  drainMicrotasks()
  return editor
}

/// `pasteHTML` through `applySelectionInputs`: a paste whose clipboard
/// returns `html` for `text/html` and nothing for any other type.
private func pasteHTML(_ editor: Editor, _ html: String) throws {
  let clipboardData = DataTransfer()
  clipboardData.setData("text/html", html)
  _ = try editor.dispatchCommand(
    PASTE_COMMAND, .clipboard(ClipboardEvent("paste", clipboardData: clipboardData)))
  drainMicrotasks()
}

/// `rootElement.innerHTML`: each node's `createDOM`, with the attributes
/// `$createNode` adds (`data-lexical-text`, `data-lexical-decorator` and
/// `contenteditable`, `dir="auto"` on a block in the root or a shadow root)
/// and the managed `<br>` of an empty block or one ending in a line break.
private func renderedHTML(_ editor: Editor) throws -> String {
  try editor.read(.latest) {
    let container = getDocument().createElement("div")
    for child in try getRoot().getChildren() {
      _ = container.appendChild(try renderDOM(child, editor))
    }
    return container.innerHTML
  }
}

private func renderDOM(_ node: LexicalNode, _ editor: Editor) throws -> DOMElement {
  let dom = try node.createDOM(editor._config, editor)
  if node is TextNode {
    dom.setAttribute("data-lexical-text", "true")
  } else if node is DecoratorNode {
    dom.setAttribute("data-lexical-decorator", "true")
    dom.setAttribute("contenteditable", "false")
  }
  guard let element = node as? ElementNode else { return dom }
  if let direction = element.__dir {
    dom.setAttribute("dir", direction)
  } else if let parent = try element.getParent(), try isRootOrShadowRoot(parent),
    parent.__dir == nil
  {
    dom.setAttribute("dir", "auto")
  }
  let children = try element.getChildren()
  for child in children {
    _ = dom.appendChild(try renderDOM(child, editor))
  }
  if try !element.isInline() {
    let last = children.last
    if try last == nil || last is LineBreakNode || (last is DecoratorNode && last!.isInline()) {
      let br = getDocument().createElement("br")
      br.setAttribute("data-lexical-managed-linebreak", "true")
      _ = dom.appendChild(br)
    }
  }
  return dom
}

struct PasteCase: CustomTestStringConvertible, Sendable {
  let name: String
  let html: String
  let expectedHTML: String
  var testDescription: String { name }
}

private let baselineCases: [PasteCase] = [
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML h1 element (#1)",
    html: "<meta charset='utf-8'><h1>Hello</h1>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><h1 class=\"editor-heading-h1\" dir=\"auto\"><span data-lexical-text=\"true\">Hello</span></h1></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML h2 element (#2)",
    html: "<meta charset='utf-8'><h2>From</h2>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><h2 class=\"editor-heading-h2\" dir=\"auto\"><span data-lexical-text=\"true\">From</span></h2></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML h3 element (#3)",
    html: "<meta charset='utf-8'><h3>The</h3>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><h3 class=\"editor-heading-h3\" dir=\"auto\"><span data-lexical-text=\"true\">The</span></h3></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from pasted DOM Text Node (#6)",
    html: "<meta charset='utf-8'>A thousand times",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">A thousand times</span></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML b element (#7)",
    html: "<meta charset='utf-8'><b>Bold</b>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><strong class=\"editor-text-bold\" data-lexical-text=\"true\">Bold</strong></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML i element (#8)",
    html: "<meta charset='utf-8'><i>Italic</i>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><em class=\"editor-text-italic\" data-lexical-text=\"true\">Italic</em></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML em element (#9)",
    html: "<meta charset='utf-8'><em>Italic</em>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><em class=\"editor-text-italic\" data-lexical-text=\"true\">Italic</em></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML u element (#10)",
    html: "<meta charset='utf-8'><u>Underline</u>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><span class=\"editor-text-underline\" data-lexical-text=\"true\">Underline</span></p></div>"
  ),
  PasteCase(
    name:
      "should produce the correct editor state from pasted heading node followed by a DOM Text Node (#11)",
    html: "<meta charset='utf-8'><h1>Lyrics to Hello by Adele</h1>A thousand times",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><h1 class=\"editor-heading-h1\" dir=\"auto\"><span data-lexical-text=\"true\">Lyrics to Hello by Adele</span></h1><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">A thousand times</span></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from a pasted HTML ul element (#4)",
    html: #"<meta charset='utf-8'><ul><li>Other side</li><li>I must have called</li></ul>"#,
    expectedHTML:
      #"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ul class="editor-list-ul" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">Other side</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">I must have called</span></li></ul></div>"#
  ),
  PasteCase(
    name: "should produce the correct editor state from pasted HTML ol element (#5)",
    html: #"<meta charset='utf-8'><ol><li>To tell you</li><li>I’m sorry</li></ol>"#,
    expectedHTML:
      #"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ol class="editor-list-ol" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">To tell you</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">I’m sorry</span></li></ol></div>"#
  ),
  PasteCase(
    name:
      "should ignore DOM node types that do not have transformers, but still process their children. (#15)",
    html:
      #"<meta charset='utf-8'><doesnotexist><ul><li>Hello</li><li>from the other</li><li>side</li></ul></doesnotexist>"#,
    expectedHTML:
      #"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ul class="editor-list-ul" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">Hello</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">from the other</span></li><li value="3" class="editor-listitem"><span data-lexical-text="true">side</span></li></ul></div>"#
  ),
  PasteCase(
    name:
      "should ignore multiple levels of DOM node types that do not have transformers, but still process their children. (#16)",
    html:
      #"<meta charset='utf-8'><doesnotexist><doesnotexist><ul><li>Hello</li><li>from the other</li><li>side</li></ul></doesnotexist></doesnotexist>"#,
    expectedHTML:
      #"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ul class="editor-list-ul" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">Hello</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">from the other</span></li><li value="3" class="editor-listitem"><span data-lexical-text="true">side</span></li></ul></div>"#
  ),
]

private let googleDocsCases: [PasteCase] = [
  PasteCase(
    name: "should produce the correct editor state from Normal text (#1)",
    html:
      "<b style=\"font-weight:normal;\" id=\"docs-internal-guid-2c706577-7fff-f54a-fe65-12f480020fac\"><span style=\"font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;\">Get schwifty!</span></b>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">Get schwifty!</span></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from bold text (#2)",
    html:
      "<b style=\"font-weight:normal;\" id=\"docs-internal-guid-9db03964-7fff-c26c-8b1e-9484fb3b54a4\"><span style=\"font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:700;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;\">Get schwifty!</span></b>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><strong class=\"editor-text-bold\" data-lexical-text=\"true\">Get schwifty!</strong></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from italic text (#3)",
    html:
      "<b style=\"font-weight:normal;\" id=\"docs-internal-guid-9db03964-7fff-c26c-8b1e-9484fb3b54a4\"><span style=\"font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:italic;font-variant:normal;vertical-align:baseline;white-space:pre;white-space:pre-wrap;\">Get schwifty!</span></b>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><em class=\"editor-text-italic\" data-lexical-text=\"true\">Get schwifty!</em></p></div>"
  ),
  PasteCase(
    name: "should produce the correct editor state from strikethrough text (#4)",
    html:
      "<b style=\"font-weight:normal;\" id=\"docs-internal-guid-9db03964-7fff-c26c-8b1e-9484fb3b54a4\"><span style=\"font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;vertical-align:baseline;white-space:pre;white-space:pre-wrap;\">Get schwifty!</span></b>",
    expectedHTML:
      "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\"><p class=\"editor-paragraph\" dir=\"auto\"><span class=\"editor-text-strikethrough\" data-lexical-text=\"true\">Get schwifty!</span></p></div>"
  ),
]

private let w3SpacingCases: [PasteCase] = [
  PasteCase(
    name: "inline hello world (#1)",
    html: "<span>hello world</span>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">hello world</span></p>"
  ),
  PasteCase(
    name: "inline hello world (2) (#2)",
    html: "<span>    hello  </span>world  ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">hello world</span></p>"
  ),
  PasteCase(
    name: "pre + inline (inline collapses with pre) (#3)",
    html: " <span style=\"white-space: pre\"> hello </span> world  ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\"> hello world</span></p>"
  ),
  PasteCase(
    name: "white-space: pre (1) (no touchy) (#4)",
    html: "<p style=\"white-space: pre\">  a b\tc  </p>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">  a b</span><span data-lexical-text=\"true\">\t</span><span data-lexical-text=\"true\">c  </span></p>"
  ),
  PasteCase(
    name: "tabs are collapsed (#5)",
    html: "<p>\ta\tb  <span>c\t</span>\t</p>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a b c</span></p>"
  ),
  PasteCase(
    name: "remove beginning + end spaces on the block (#6)",
    html:
      "\n              <div>\n                hello\n                world\n              </div>\n            ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">hello world</span></p>"
  ),
  PasteCase(
    name: "remove beginning + end spaces on the block (2) (#7)",
    html:
      "\n              <div>\n                <strong>\n                  hello\n                  world\n                </strong>\n              </div>\n          ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><strong class=\"editor-text-bold\" data-lexical-text=\"true\">hello world</strong></p>"
  ),
  PasteCase(
    name: "remove beginning + end spaces on the block + anonymous inlines collapsible rules (#8)",
    html:
      "\n              <div>\n                a\n                <strong>b</strong>\n                c\n              </div>\n          ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a </span><strong class=\"editor-text-bold\" data-lexical-text=\"true\">b</strong><span data-lexical-text=\"true\"> c</span></p>"
  ),
  PasteCase(
    name: "collapsibles and neighbors (1) (#9)",
    html: "<div><strong>a </strong>b</div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><strong class=\"editor-text-bold\" data-lexical-text=\"true\">a </strong><span data-lexical-text=\"true\">b</span></p>"
  ),
  PasteCase(
    name: "collapsibles and neighbors (2) (#10)",
    html: "<div>a<strong> b</strong></div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span><strong class=\"editor-text-bold\" data-lexical-text=\"true\"> b</strong></p>"
  ),
  PasteCase(
    name: "collapsibles and neighbors (3) (#11)",
    html: "<div><strong>a </strong><span></span>b</div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><strong class=\"editor-text-bold\" data-lexical-text=\"true\">a </strong><span data-lexical-text=\"true\">b</span></p>"
  ),
  PasteCase(
    name: "collapsibles and neighbors (4) (#12)",
    html: "<div>a<span></span><strong> b</strong></div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span><strong class=\"editor-text-bold\" data-lexical-text=\"true\"> b</strong></p>"
  ),
  PasteCase(
    name: "empty block (#13)",
    html: "\n              <p>\n              </p>\n          ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p>"),
  PasteCase(
    name: "redundant inline at start (#14)",
    html: "<span> </span><span>a</span>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span></p>"),
  PasteCase(
    name: "redundant inline at end (#15)",
    html: "<span>a</span><span> </span>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span></p>"),
  PasteCase(
    name: "collapsible spaces with nested structures (#16)",
    html:
      "\n            <div>\n              <p>\n                a\n              </p>\n              <p>\n                b\n              </p>\n            </div>\n            ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">b</span></p>"
  ),
  PasteCase(
    name: "collapsible spaces with nested structures (3) (#17)",
    html:
      "\n            <div>\n              <strong>\n                a\n              </strong>\n              <strong>\n                b\n              </strong>\n            </div>\n            ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><strong class=\"editor-text-bold\" data-lexical-text=\"true\">a b</strong></p>"
  ),
  PasteCase(
    name: "forced line break should remain (#18)",
    html:
      "\n            <p>\n            a\n            <br>\n            b\n            </p>\n            ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span><br><span data-lexical-text=\"true\">b</span></p>"
  ),
  PasteCase(
    name: "forced line break with tabs (#19)",
    html:
      "\n            <p>\n            a\n            \t<br>\t\n            b\n            </p>\n            ",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span><br><span data-lexical-text=\"true\">b</span></p>"
  ),
  PasteCase(
    name: "white-space: pre (no touchy) (2) (#22)",
    html: "<span style=\"white-space: pre\">\na\r\nb\r\n</span>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><br><span data-lexical-text=\"true\">a</span><br><span data-lexical-text=\"true\">b</span><br><br data-lexical-managed-linebreak=\"true\"></p>"
  ),
  PasteCase(
    name: "two Apple Notes paragraphs (#23)",
    html: "\n<p class=\"p1\">paragraph1</p>\n<p class=\"p1\">paragraph2</p>\n",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">paragraph1</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">paragraph2</span></p>"
  ),
  PasteCase(
    name: "two Apple Notes lines + two paragraphs separated by an empty paragraph (#24)",
    html:
      "\n<p class=\"p1\">line 1<br>\nline 2</p>\n<p class=\"p2\"><br></p>\n<p class=\"p1\">paragraph 1</p>\n<p class=\"p1\">paragraph 2</p>\n",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">line 1</span><br><span data-lexical-text=\"true\">line 2</span></p><p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">paragraph 1</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">paragraph 2</span></p>"
  ),
  PasteCase(
    name: "two lines + two paragraphs separated by an empty paragraph (2) (#25)",
    html:
      "\n<p class=\"p1\">line 1<br>\nline 2</p>\n<p class=\"p2\">\n<br>\n</p>\n<p class=\"p1\">paragraph 1</p>\n<p class=\"p1\">paragraph 2</p>\n",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">line 1</span><br><span data-lexical-text=\"true\">line 2</span></p><p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">paragraph 1</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">paragraph 2</span></p>"
  ),
  PasteCase(
    name: "two lines and br in spans (#26)",
    html: "<p class=\"p1\"><span>line 1</span><span><br></span><span>line 2</span></p>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">line 1</span><br><span data-lexical-text=\"true\">line 2</span></p>"
  ),
  PasteCase(
    name: "lone trailing block-level br collapses to an empty paragraph (#27)",
    html: "<p>a</p><br>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span></p><p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p>"
  ),
  PasteCase(
    name: "lone block-level br between paragraphs collapses to an empty paragraph (#28)",
    html: "<p>a</p><br><p>b</p>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span></p><p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">b</span></p>"
  ),
  PasteCase(
    name: "block-level br followed by inline content is preserved (#29)",
    html: "<br>a<p>b</p>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><br><span data-lexical-text=\"true\">a</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">b</span></p>"
  ),
  PasteCase(
    name: "empty block node in div behaves like a line break (#31)",
    html: "<div>1<div></div>2</div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">1</span><br><span data-lexical-text=\"true\">2</span></p>"
  ),
  PasteCase(
    name: "empty inline node does not behave like a line break (#32)",
    html: "<div>1<text></text>2</div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">12</span></p>"),
  PasteCase(
    name: "empty block node between non inline siblings does not behave like a line break (#33)",
    html: "<div><div>1</div><div></div><div>2</div></div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">1</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">2</span></p>"
  ),
  PasteCase(
    name: "nested divs (#34)",
    html: "<div>a<div>b b<div>c<div><div></div>z</div></div>d e</div>fg</div>",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">a</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">b b</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">c</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">z</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">d e</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">fg</span></p>"
  ),
  PasteCase(
    name: "last br in a block node is ignored (#36)",
    html: "1<p>2<br /></p>3",
    expectedHTML:
      "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">1</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">2</span></p><p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">3</span></p>"
  ),
  PasteCase(
    name: "empty block node in li behaves like a line break (#30)",
    html: #"<ol><li>1<div></div>2</li><li></li><li>3</li></ol>"#,
    expectedHTML:
      #"<ol class="editor-list-ol" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">1</span><br><span data-lexical-text="true">2</span></li><li value="2" class="editor-listitem"><br data-lexical-managed-linebreak="true"></li><li value="3" class="editor-listitem"><span data-lexical-text="true">3</span></li></ol>"#
  ),
  PasteCase(
    name: "only br in a li (#35)",
    html: #"<ol><li>1</li><li><br /></li><li>3</li></ol>"#,
    expectedHTML:
      #"<ol class="editor-list-ol" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">1</span></li><li value="2" class="editor-listitem"><br data-lexical-managed-linebreak="true"></li><li value="3" class="editor-listitem"><span data-lexical-text="true">3</span></li></ol>"#
  ),
]

@Suite struct LexicalEventHelpersTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > Expect initial output to be a block with no text"
    ))
  func initialOutput() throws {
    let editor = try createEventHelpersEditor()
    defer { editor.dispose() }
    #expect(
      try rootOpen + renderedHTML(editor) + "</div>"
        == #"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><br data-lexical-managed-linebreak="true"></p></div>"#
    )
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML h1 element (#1)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML h2 element (#2)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML h3 element (#3)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from pasted DOM Text Node (#6)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML b element (#7)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML i element (#8)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML em element (#9)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML u element (#10)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from pasted heading node followed by a DOM Text Node (#11)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML ul element (#4)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from pasted HTML ol element (#5)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should ignore DOM node types that do not have transformers, but still process their children. (#15)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should ignore multiple levels of DOM node types that do not have transformers, but still process their children. (#16)"
    ),
    arguments: baselineCases)
  func baseline(_ testCase: PasteCase) throws {
    let editor = try createEventHelpersEditor()
    defer { editor.dispose() }
    try pasteHTML(editor, testCase.html)
    #expect(
      try rootOpen + renderedHTML(editor) + "</div>" == testCase.expectedHTML)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from Normal text (#1)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from bold text (#2)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from italic text (#3)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from strikethrough text (#4)"
    ),
    arguments: googleDocsCases)
  func googleDocs(_ testCase: PasteCase) throws {
    let editor = try createEventHelpersEditor()
    defer { editor.dispose() }
    try pasteHTML(editor, testCase.html)
    #expect(
      try rootOpen + renderedHTML(editor) + "</div>" == testCase.expectedHTML)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > inline hello world (#1)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > inline hello world (2) (#2)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > pre + inline (inline collapses with pre) (#3)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > white-space: pre (1) (no touchy) (#4)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > tabs are collapsed (#5)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > remove beginning + end spaces on the block (#6)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > remove beginning + end spaces on the block (2) (#7)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > remove beginning + end spaces on the block + anonymous inlines collapsible rules (#8)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (1) (#9)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (2) (#10)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (3) (#11)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (4) (#12)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block (#13)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > redundant inline at start (#14)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > redundant inline at end (#15)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsible spaces with nested structures (#16)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsible spaces with nested structures (3) (#17)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > forced line break should remain (#18)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > forced line break with tabs (#19)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > white-space: pre (no touchy) (2) (#22)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two Apple Notes paragraphs (#23)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two Apple Notes lines + two paragraphs separated by an empty paragraph (#24)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two lines + two paragraphs separated by an empty paragraph (2) (#25)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two lines and br in spans (#26)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > lone trailing block-level br collapses to an empty paragraph (#27)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > lone block-level br between paragraphs collapses to an empty paragraph (#28)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > block-level br followed by inline content is preserved (#29)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block node in div behaves like a line break (#31)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty inline node does not behave like a line break (#32)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block node between non inline siblings does not behave like a line break (#33)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > nested divs (#34)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > last br in a block node is ignored (#36)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block node in li behaves like a line break (#30)"
    ),
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > only br in a li (#35)"
    ),
    arguments: w3SpacingCases)
  func w3Spacing(_ testCase: PasteCase) throws {
    let editor = try createEventHelpersEditor()
    defer { editor.dispose() }
    try pasteHTML(editor, testCase.html)
    #expect(try renderedHTML(editor) == testCase.expectedHTML)
  }
}
