import Lexical

// The NodeStates and small helpers of
// packages/lexical-markdown/src/MarkdownTransformers.ts that export and
// import share, and `TextFormatTransformersIndex` from MarkdownImport.ts.

nonisolated(unsafe) private let LIST_MARKER = JSRegExp(#"^[-*+]$"#)
nonisolated(unsafe) private let CODE_FENCE = JSRegExp(#"^`{3,}$"#)
nonisolated(unsafe) private let HARD_LINE_BREAK = JSRegExp(#"^(\\| {2,})$"#)

/// `listMarkerState`: the bullet (`-`, `*` or `+`) a list was written with.
nonisolated(unsafe) package let listMarkerState = createState(
  "mdListMarker",
  parse: { (v: JSONValue?) -> String in
    if let s = v?.stringValue, LIST_MARKER.test(s) { return s }
    return "-"
  }, resetOnCopyNode: true)

/// `codeFenceState`: the fence a code block was written with.
nonisolated(unsafe) package let codeFenceState = createState(
  "mdCodeFence",
  parse: { (v: JSONValue?) -> String in
    if let s = v?.stringValue, CODE_FENCE.test(s) { return s }
    return "```"
  }, resetOnCopyNode: true)

/// `codeMetaState`: the info-string tail after a fenced code block's
/// language (`title="x"` in ```` ```js title="x" ````). `CodeNode` models
/// only the language, so the rest is kept here to survive the round trip.
nonisolated(unsafe) package let codeMetaState = createState(
  "mdCodeMeta", parse: { (v: JSONValue?) -> String in v?.stringValue ?? "" },
  resetOnCopyNode: true)

/// `MarkdownHardLineBreak`: `\` or two or more spaces, or `""` for a soft
/// line break.
package typealias MarkdownHardLineBreak = String

/// `hardLineBreakState`: the marker a hard line break was written with.
nonisolated(unsafe) package let hardLineBreakState = createState(
  "mdHardLineBreak",
  parse: { (v: JSONValue?) -> MarkdownHardLineBreak in
    if let s = v?.stringValue, HARD_LINE_BREAK.test(s) { return s }
    return ""
  }, resetOnCopyNode: true)

nonisolated(unsafe) private let TRAILING_SPACES = JSRegExp(#"^(.*?\S)( {2,})$"#)

/// `parseMarkdownHardLineBreak(line)`: the line without its hard line
/// break marker, and the marker, or `nil` when it does not end in one.
package func parseMarkdownHardLineBreak(_ line: String) -> (String, MarkdownHardLineBreak)? {
  if line.jsEndsWith("\\") {
    // A trailing backslash is a hard line break only when it is not itself
    // escaped: only an odd-length run counts.
    // https://spec.commonmark.org/0.31.2/#backslash-escapes
    var backslashes = 0
    var i = line.jsLength - 1
    while i >= 0, line.jsCharAt(i) == "\\" {
      backslashes += 1
      i -= 1
    }
    return backslashes % 2 == 1 ? (line.jsSlice(0, -1), "\\") : nil
  }
  guard let spaces = line.jsMatch(TRAILING_SPACES) else { return nil }
  return (spaces[1] ?? "", spaces[2] ?? "")
}

nonisolated(unsafe) private let BACKTICK_RUNS = JSRegExp("`+", "g")
nonisolated(unsafe) private let LEADING_SPACE = JSRegExp(#"^\s"#)
nonisolated(unsafe) private let TRAILING_SPACE = JSRegExp(#"\s$"#)
nonisolated(unsafe) private let NOT_SPACE = JSRegExp("[^ ]")

/// `getCodeSpanDelimiter(content)`: a CommonMark fence and the padded
/// content for an inline code span.
/// https://spec.commonmark.org/#code-spans
package func getCodeSpanDelimiter(_ content: String) -> (fence: String, padded: String) {
  let longestRun = content.jsMatchAllTexts(BACKTICK_RUNS)?.map(\.utf16.count).max() ?? 0
  let fence = String(repeating: "`", count: longestRun + 1)
  // A reader strips one space from each end only when the content is not
  // all spaces, so content made only of spaces goes out as it is.
  let needsPadding =
    content.isEmpty || content.utf16.contains(UInt16(UInt8(ascii: "`")))
    || (LEADING_SPACE.test(content) && TRAILING_SPACE.test(content) && NOT_SPACE.test(content))
  return (fence, needsPadding ? " \(content) " : content)
}

/// `TextFormatTransformersIndex`: the text format transformers by tag, with
/// the regexes import matches them by.
package struct TextFormatTransformersIndex {
  public var fullMatchRegExpByTag: [String: JSRegExp]
  public var openTagsRegExp: JSRegExp
  public var transformersByTag: [String: TextFormatTransformer]
  /// The keys of `transformersByTag` in the order JS's object keeps them:
  /// the order each tag was first added.
  public var tags: [String]

  public init(
    fullMatchRegExpByTag: [String: JSRegExp], openTagsRegExp: JSRegExp,
    transformersByTag: [String: TextFormatTransformer], tags: [String]
  ) {
    self.fullMatchRegExpByTag = fullMatchRegExpByTag
    self.openTagsRegExp = openTagsRegExp
    self.transformersByTag = transformersByTag
    self.tags = tags
  }
}
