import Lexical
import LexicalCodeCore
import LexicalHTML
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText
import Testing

/// Replays `vectors/markdown/import-export.json`, upstream
/// LexicalMarkdown.test.ts's `IMPORT_AND_EXPORT` table: each `md` imports
/// to the editor state and HTML JS makes from it, without selecting, and
/// each `html` exports to the markdown JS writes.
@Suite(.enabled(if: Vectors.available))
struct MarkdownTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let md: String
    let html: String
    let shouldPreserveNewLines: Bool?
    let shouldMergeAdjacentLines: Bool?
    let customTransformers: [String]?
    let importState: String?
    let importHtml: String?
    let exportState: String?
    let exportMd: String?
    var testDescription: String { String(name.prefix(60)) }
  }

  static let cases: [Case] = (try? Vectors.load("markdown/import-export")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("markdown/import-export", as: Case.self).count == Self.cases.count)
    #expect(Self.cases.count == 149)
  }

  func createTestEditor() throws -> Editor {
    try createEditor(
      CreateEditorArgs(
        nodes: [
          .klass(HeadingNode.self), .klass(ListNode.self), .klass(ListItemNode.self),
          .klass(QuoteNode.self), .klass(CodeNode.self), .klass(LinkNode.self),
        ], onError: { throw $0 }, headless: true))
  }

  func custom(_ vector: Case) -> [any Transformer] {
    (vector.customTransformers ?? []).map { name in
      switch name {
      case "CHECK_LIST": CHECK_LIST
      case "CODE_TAG_COUNTER_EXAMPLE": CODE_TAG_COUNTER_EXAMPLE
      case "HIGHLIGHT_TEXT_MATCH_IMPORT": HIGHLIGHT_TEXT_MATCH_IMPORT
      case "MDX_HTML_TRANSFORMER": MDX_HTML_TRANSFORMER
      case "SIMPLE_INLINE_JSX_MATCHER": SIMPLE_INLINE_JSX_MATCHER
      default: preconditionFailure("Unknown custom transformer \(name)")
      }
    }
  }

  static let importCases = cases.filter { $0.importState != nil }
  static let exportCases = cases.filter { $0.exportState != nil }

  @Test(arguments: importCases)
  func importMarkdown(_ vector: Case) throws {
    let editor = try createTestEditor()
    try editor.update(
      {
        try convertFromMarkdownString(
          vector.md, custom(vector) + TRANSFORMERS + [HIGHLIGHT_TEXT_MATCH_IMPORT], nil,
          vector.shouldPreserveNewLines ?? false, vector.shouldMergeAdjacentLines ?? false)
      }, EditorUpdateOptions(discrete: true))
    #expect(try editor.getEditorState().toJSON().stringify() == vector.importState)
    #expect(try editor.read(.latest) { try generateHtmlFromNodes(editor) } == vector.importHtml)
    // "should not select when importing"
    #expect(try editor.read(.latest) { try getSelection() } == nil)
  }

  @Test(arguments: exportCases)
  func exportMarkdown(_ vector: Case) throws {
    let editor = try createTestEditor()
    try editor.update(
      {
        let dom = DOMParser().parseFromString(vector.html, "text/html")
        let nodes = try generateNodesFromDOM(editor, dom)
        try getRoot().select()
        try insertNodes(nodes)
      }, EditorUpdateOptions(discrete: true))
    #expect(try editor.getEditorState().toJSON().stringify() == vector.exportState)
    #expect(
      try editor.read(.latest) {
        try convertToMarkdownString(
          custom(vector) + TRANSFORMERS, nil, vector.shouldPreserveNewLines ?? false)
      } == vector.exportMd)
  }
}

// The custom transformers of upstream LexicalMarkdown.test.ts, which the
// vectors name.

nonisolated(unsafe) private let HIGHLIGHT_TEXT_MATCH_IMPORT: TextMatchTransformer = {
  let t = LINK.copy()
  t.importRegExp = JSRegExp(#"\$([^$]+?)\$"#)
  t.replace = { textNode, _ in
    try textNode.toggleFormat("highlight")
    return nil
  }
  return t
}()

nonisolated(unsafe) private let SIMPLE_INLINE_JSX_MATCHER = TextMatchTransformer(
  dependencies: [LinkNode.self],
  importRegExp: JSRegExp(#"<(MyTag)\s*>"#),
  regExp: JSRegExp("__ignore__"),
  replace: { textNode, match in
    let linkNode = try createLinkNode("simple-jsx")
    let textStart = match.length + match.index
    let textEnd = try match.index + textNode.getTextContent().jsLength - "</MyTag>".jsLength
    let text = match.input.jsSubstring(textStart, textEnd)
    let linkTextNode = try createTextNode(text)
    try linkTextNode.setFormat(textNode.getFormat())
    try linkNode.append(linkTextNode)
    try textNode.replace(linkNode)
    return nil
  },
  getEndIndex: { node, match in
    // Find the closing tag, counting the opening and closing tags.
    var openedSubStartMatches = 0
    let start = match.index + match.length
    var endIndex = start
    let line = try node.getTextContent()
    var i = start
    while i < line.jsLength {
      if line.jsCharAt(i) == "<" {
        if line.jsCharAt(i + 1) == "/" {
          if openedSubStartMatches == 0 {
            endIndex = i + "</MyTag>".jsLength
            break
          }
          openedSubStartMatches -= 1
        } else {
          openedSubStartMatches += 1
        }
      }
      i += 1
    }
    return endIndex
  })

/// Matches HTML within an MDX file.
nonisolated(unsafe) private let MDX_HTML_TRANSFORMER = MultilineElementTransformer(
  dependencies: [CodeNode.self],
  export: { node, _, _ in
    let text = try node.getTextContent()
    if text.jsStartsWith("From HTML:") {
      return "<MyComponent>\(text.jsReplace(JSRegExp("From HTML: "), template: ""))</MyComponent>"
    }
    return nil
  },
  regExpStart: JSRegExp(#"<(\w+)[^>]*>"#),
  regExpEnd: MultilineRegExpEnd(regExp: JSRegExp(#"<\/(\w+)\s*>"#)),
  replace: { rootNode, _, startMatch, _, linesInBetween, _ in
    // Markdown shortcuts are not supported.
    guard let linesInBetween else { return false }
    if startMatch[1] == "MyComponent" {
      let codeBlockNode = try createCodeNode(startMatch[1])
      let textNode = try createTextNode("From HTML: " + linesInBetween.joined(separator: "\n"))
      try codeBlockNode.append(textNode)
      try rootNode.append(codeBlockNode)
      return nil
    }
    return false
  })

nonisolated(unsafe) private let CODE_TAG_COUNTER_EXAMPLE = MultilineElementTransformer(
  handleImportAfterStartMatch: { args in
    let lines = args.lines
    let startLineIndex = args.startLineIndex
    let startMatch = args.startMatch
    let regexpEndRegex = JSRegExp(#"[ \t]*```$"#)
    let isEndOptional = false
    var endLineIndex = startLineIndex
    let linesLength = lines.count
    var openedSubStartMatches = 0
    while endLineIndex < linesLength {
      let potentialSubStartMatch = lines[endLineIndex].jsMatch(JSRegExp(#"^[ \t]*```(\w+)?"#))
      let endMatch = lines[endLineIndex].jsMatch(regexpEndRegex)
      if let potentialSubStartMatch {
        if let endMatch {
          if potentialSubStartMatch.index < endMatch.index { openedSubStartMatches += 1 }
        } else {
          openedSubStartMatches += 1
        }
      }
      if endMatch != nil { openedSubStartMatches -= 1 }
      if endMatch == nil || openedSubStartMatches > 0 {
        if !isEndOptional || endLineIndex < linesLength - 1 {
          endLineIndex += 1
          continue
        }
      }
      if let endMatch, startLineIndex == endLineIndex, endMatch.index == startMatch.index {
        endLineIndex += 1
        continue
      }
      var linesInBetween: [String] = []
      if let endMatch, startLineIndex == endLineIndex {
        linesInBetween.append(
          lines[startLineIndex].jsSubstring(startMatch.length, -endMatch.length))
      } else {
        for i in startLineIndex...endLineIndex {
          if i == startLineIndex {
            linesInBetween.append(lines[i].jsSubstring(startMatch.length))
          } else if i == endLineIndex, let endMatch {
            linesInBetween.append(lines[i].jsSubstring(0, -endMatch.length))
          } else {
            linesInBetween.append(lines[i])
          }
        }
      }
      if try CODE.replace(args.rootNode, nil, startMatch, endMatch, linesInBetween, true) != false {
        return .handled(true, endLineIndex)
      }
      break
    }
    return .handled(false, startLineIndex)
  },
  dependencies: CODE.dependencies,
  export: CODE.export,
  regExpStart: CODE.regExpStart,
  replace: CODE.replace)
