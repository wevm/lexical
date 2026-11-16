import Lexical
import LexicalCodeCore

// The tokenizers of a script's `codeHighlight` (ops.ts's `scriptTokenizer`),
// and the `codeBlocks` entry logged after each of its steps
// (`codeBlocksEntry`).

/// ops.ts's tokenizer for `spec`: CONFORMANCE_TOKENIZE (`conformance`), or
/// the tokens it recorded from Prism (`recorded`). Either has only the
/// spec's `languages`.
func scriptCodeTokenizer(_ spec: JSONValue) throws -> any CodeTokenizer {
  let languages = Set((spec.objectValue?["languages"]?.arrayValue ?? []).compactMap(\.stringValue))
  let defaultLanguage: String? =
    switch spec.objectValue?["defaultLanguage"] {
    case nil: DEFAULT_CODE_LANGUAGE
    case .null?: nil
    case let value?: try value.stringValue.required()
    }
  var recorded: [(String, String, [CodeToken])]?
  switch spec.objectValue?["tokenizer"]?.stringValue {
  case "conformance": recorded = nil
  case "recorded":
    recorded = try (spec.objectValue?["tokens"]?.arrayValue).required().map { entry in
      let entry = try entry.arrayValue.required()
      guard entry.count == 3 else { throw ScriptError("a token entry") }
      return (
        try entry[0].stringValue.required(), try entry[1].stringValue.required(),
        try codeTokens(entry[2])
      )
    }
  case let other: throw ScriptError("unknown tokenizer \(other ?? "nil")")
  }
  return ScriptCodeTokenizer(
    languages: languages, defaultLanguage: defaultLanguage, recorded: recorded)
}

extension Optional {
  fileprivate func required() throws -> Wrapped {
    guard let value = self else { throw ScriptError("a value is missing") }
    return value
  }
}

/// `TokenJson[]` as tokens. A string content is one text token.
private func codeTokens(_ json: JSONValue) throws -> [CodeToken] {
  try json.arrayValue.required().map { token in
    if let text = token.stringValue { return .text(text) }
    let content = try (token.objectValue?["content"]).required()
    return .token(
      type: try token.objectValue?["type"]?.stringValue.required() ?? "",
      content: try content.stringValue.map { [.text($0)] } ?? codeTokens(content),
      alias: token.objectValue?["alias"]?.stringValue)
  }
}

private struct ScriptCodeTokenizer: CodeTokenizer {
  let languages: Set<String>
  let defaultLanguage: String?
  /// `[language, code, tokens]` of `recorded`.
  let recorded: [(String, String, [CodeToken])]?

  func languageState(_ language: String) -> CodeLanguageState {
    languages.contains(language) ? .loaded : .unsupported
  }

  func tokenize(_ code: String, _ language: String) -> [CodeToken] {
    precondition(languages.contains(language), "the tokenizer has no \(language)")
    guard let recorded else { return conformanceTokenize(code, language) }
    guard let entry = recorded.first(where: { $0.0 == language && $0.1 == code }) else {
      preconditionFailure("no tokens recorded for \(language): \(code)")
    }
    return entry.2
  }
}

private let conformanceKeywords: Set<String> = ["fn", "if", "let", "return"]

private func isConformanceSpace(_ c: Character) -> Bool {
  c == " " || c == "\t" || c == "\n" || c == "\r"
}

/// An ASCII letter, digit or `_`, or any code point past ASCII.
private func isConformanceWord(_ c: Unicode.Scalar) -> Bool {
  (c.isASCII && (c == "_" || c.properties.isAlphabetic || ("0"..."9").contains(c)))
    || !c.isASCII
}

/// `conformanceWords(code)`, over code points.
private func conformanceWords(_ code: String) -> [CodeToken] {
  let chars = Array(code.unicodeScalars)
  var out: [CodeToken] = []
  var i = 0
  func runOf(_ test: (Unicode.Scalar) -> Bool) -> String {
    let start = i
    while i < chars.count, test(chars[i]) { i += 1 }
    return String(String.UnicodeScalarView(chars[start..<i]))
  }
  while i < chars.count {
    let c = chars[i]
    if isConformanceSpace(Character(c)) {
      out.append(.text(runOf { isConformanceSpace(Character($0)) }))
    } else if isConformanceWord(c) {
      let run = runOf(isConformanceWord)
      if conformanceKeywords.contains(run) {
        out.append(.token("keyword", run))
      } else if run.unicodeScalars.allSatisfy({ ("0"..."9").contains($0) }) {
        out.append(.token("number", run))
      } else {
        out.append(.token(type: "word", content: [.text(run)]))
      }
    } else if c == "\"" {
      var content: [CodeToken] = [.text("\"")]
      i += 1
      var run = ""
      while i < chars.count, chars[i] != "\"" {
        if chars[i] == "\\", i + 1 < chars.count {
          if !run.isEmpty {
            content.append(.text(run))
            run = ""
          }
          content.append(.token("escape", "\\" + String(chars[i + 1])))
          i += 2
        } else {
          run.unicodeScalars.append(chars[i])
          i += 1
        }
      }
      if !run.isEmpty { content.append(.text(run)) }
      if i < chars.count {
        content.append(.text("\""))
        i += 1
      }
      out.append(.token(type: "string", content: content))
    } else if c == "#" {
      out.append(.token("comment", runOf { $0 != "\n" }))
    } else {
      out.append(.token("punctuation", String(c)))
      i += 1
    }
  }
  return out
}

/// `CONFORMANCE_TOKENIZE(code, language)`.
private func conformanceTokenize(_ code: String, _ language: String) -> [CodeToken] {
  if language != "diff" { return conformanceWords(code) }
  let types: [Unicode.Scalar: String] = [
    " ": "unchanged", "!": "changed", "+": "inserted", "-": "deleted",
  ]
  var out: [CodeToken] = []
  for (i, line) in code.split(separator: "\n", omittingEmptySubsequences: false).enumerated() {
    if i > 0 { out.append(.text("\n")) }
    guard let first = line.unicodeScalars.first, let type = types[first] else {
      out.append(contentsOf: conformanceWords(String(line)))
      continue
    }
    let prefix: CodeToken = .token(
      type: "prefix", content: [.text(String(first))], alias: type == "changed" ? nil : type)
    let rest = String(String.UnicodeScalarView(line.unicodeScalars.dropFirst()))
    out.append(.token(type: type, content: [prefix] + conformanceWords(rest)))
  }
  return out
}

/// `codeBlocksEntry(editor)`: `['codeBlocks', [[key, language,
/// isSyntaxHighlightSupported, childKeys]...]]` of the root's code blocks.
func codeBlocksEntry(_ editor: Editor) throws -> JSONValue {
  try editor.read {
    .array([
      .string("codeBlocks"),
      .array(
        try getRoot().getChildren().compactMap { $0 as? CodeNode }.map { block in
          .array([
            .string(block.getKey()), try block.getLanguage().map { .string($0) } ?? .null,
            .bool(try block.getIsSyntaxHighlightSupported()),
            .array(try block.getChildrenKeys().map { .string($0) }),
          ])
        }),
    ])
  }
}
