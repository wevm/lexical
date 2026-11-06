import CTreeSitter
import Lexical
import LexicalCodeCore

#if canImport(Glibc)
  import Glibc
#elseif canImport(Musl)
  import Musl
#elseif canImport(Darwin)
  import Darwin
#endif

// A `CodeTokenizer` on tree-sitter: the code is parsed with a grammar, and
// the captures of the grammar's highlights query become Prism token names,
// so a theme written for Prism (the playground's) colors them.
//
// The C API is called directly. Code is passed as UTF-16 (little-endian),
// so a byte offset halved is a UTF-16 offset, Lexical's unit. Each grammar
// keeps the last tree it parsed and the code it parsed, and parses an edit
// to that code incrementally, and re-runs the highlights query only over
// the lines the edit can change: the other tokens keep their types, and
// the last call's tokens there are returned again rather than rebuilt.

/// A grammar and its highlights query, for the code languages it names.
/// Use it on the thread that owns the editor.
public final class TreeSitterGrammar: @unchecked Sendable {
  /// The `CodeNode` languages it highlights, such as `js` and `javascript`.
  public let languages: [String]
  private let language: OpaquePointer
  private let highlightsSource: String
  private lazy var query = HighlightsQuery(language, highlightsSource)
  private var parser: OpaquePointer?
  /// The last parse, which the next one edits, with its token types.
  private var last: (code: [UInt16], tree: OpaquePointer, types: [UInt8])?
  /// The tokens of `last`, with the offset each starts at and, last, the
  /// code's length.
  private var lastTokens: (tokens: [CodeToken], starts: [Int])?

  /// `language` is the grammar's `const TSLanguage *`, and `highlights` its
  /// highlights query.
  public init(languages: [String], language: UnsafeRawPointer?, highlights: UnsafePointer<CChar>?) {
    self.languages = languages
    self.language = OpaquePointer(language!)
    highlightsSource = highlights.map { String(cString: $0) } ?? ""
  }

  deinit {
    if let last { ts_tree_delete(last.tree) }
    if let parser { ts_parser_delete(parser) }
  }

  /// Whether the highlights query compiles against the grammar.
  public var isValid: Bool { query != nil }

  /// `code` as Prism-style tokens.
  public func tokenize(_ code: String) -> [CodeToken] {
    let units = Array(code.utf16)
    if let last, let lastTokens, last.code == units { return lastTokens.tokens }
    let previous = lastTokens
    lastTokens = nil
    guard let query, !units.isEmpty, let (types, retyped) = highlight(units, query) else {
      return [.text(code)]
    }
    // Outside the retyped lines the types are the last parse's, so are its
    // tokens there, as far as a type change on each side: only the lines
    // between become new strings.
    var head = 0
    var from = 0
    var tail = previous?.tokens.count ?? 0
    var to = units.count
    var shift = 0
    if let previous, let retyped {
      shift = retyped.shift
      let starts = previous.starts
      head = firstIndex(in: starts) { $0 > retyped.range.lowerBound } - 1
      while head > 0, types[starts[head] - 1] == types[starts[head]] { head -= 1 }
      from = starts[head]
      tail = firstIndex(in: starts) { $0 + shift >= retyped.range.upperBound }
      while tail < previous.tokens.count, starts[tail] + shift > 0,
        types[starts[tail] + shift - 1] == types[starts[tail] + shift]
      {
        tail += 1
      }
      to = starts[tail] + shift
    }
    var tokens: [CodeToken] = []
    var starts: [Int] = []
    tokens.reserveCapacity(head + (previous?.tokens.count ?? 0) - tail + 16)
    starts.reserveCapacity(tokens.capacity + 1)
    if let previous {
      tokens.append(contentsOf: previous.tokens[0..<head])
      starts.append(contentsOf: previous.starts[0..<head])
    }
    if from < to {
      var start = from
      for i in (from + 1)...to where i == to || types[i] != types[start] {
        let text = String(decoding: units[start..<i], as: UTF16.self)
        tokens.append(
          types[start] == 0 ? .text(text) : .token(query.typeNames[Int(types[start])], text))
        starts.append(start)
        start = i
      }
    }
    if let previous {
      tokens.append(contentsOf: previous.tokens[tail...])
      starts.append(contentsOf: previous.starts[tail...].lazy.map { $0 + shift })
    } else {
      starts.append(units.count)
    }
    lastTokens = (tokens, starts)
    return tokens
  }

  /// The token type id of each unit of `units`, from the last parse's
  /// where the edit leaves them be; with, after an edit, the units the
  /// query typed again and how far the edit moved the units after them.
  private func highlight(
    _ units: [UInt16], _ query: HighlightsQuery
  ) -> ([UInt8], retyped: (range: Range<Int>, shift: Int)?)? {
    if parser == nil {
      parser = ts_parser_new()
      guard ts_parser_set_language(parser, language) else { return nil }
    }
    guard let last else {
      guard let tree = parse(units, nil) else { return nil }
      var types = [UInt8](repeating: 0, count: units.count)
      query.highlight(ts_tree_root_node(tree), units, 0..<units.count, &types)
      self.last = (units, tree, types)
      return (types, nil)
    }
    if last.code == units { return (last.types, (0..<0, 0)) }
    var edit = Self.edit(from: last.code, to: units)
    ts_tree_edit(last.tree, &edit)
    let tree = parse(units, last.tree)
    defer { ts_tree_delete(last.tree) }
    guard let tree else {
      self.last = nil
      return nil
    }
    let start = Int(edit.start_byte) / 2
    let oldEnd = Int(edit.old_end_byte) / 2
    let newEnd = Int(edit.new_end_byte) / 2
    // What the edit can retype: its span, the ranges whose structure
    // changed, and the node around the edit, whose text a predicate may
    // read; on whole lines.
    var low = start
    var high = newEnd
    var count: UInt32 = 0
    if let ranges = ts_tree_get_changed_ranges(last.tree, tree, &count) {
      for range in UnsafeBufferPointer(start: ranges, count: Int(count)) {
        low = min(low, Int(range.start_byte) / 2)
        high = max(high, Int(range.end_byte) / 2)
      }
      free(ranges)
    }
    let around = ts_node_descendant_for_byte_range(
      ts_tree_root_node(tree), UInt32(low * 2), UInt32(high * 2))
    low = min(low, Int(ts_node_start_byte(around)) / 2)
    high = min(units.count, max(high, Int(ts_node_end_byte(around)) / 2))
    while low > 0, units[low - 1] != 0x0A { low -= 1 }
    while high < units.count, units[high] != 0x0A { high += 1 }
    var types = [UInt8](repeating: 0, count: units.count)
    types.withUnsafeMutableBufferPointer { types in
      last.types.withUnsafeBufferPointer { old in
        for i in 0..<start { types[i] = old[i] }
        for i in newEnd..<units.count { types[i] = old[i - newEnd + oldEnd] }
      }
    }
    query.highlight(ts_tree_root_node(tree), units, low..<high, &types)
    self.last = (units, tree, types)
    return (types, (low..<high, newEnd - oldEnd))
  }

  private func parse(_ units: [UInt16], _ oldTree: OpaquePointer?) -> OpaquePointer? {
    units.withUnsafeBufferPointer { buffer in
      buffer.withMemoryRebound(to: CChar.self) {
        ts_parser_parse_string_encoding(
          parser, oldTree, $0.baseAddress, UInt32(buffer.count * 2), TSInputEncodingUTF16LE)
      }
    }
  }

  /// The edit from `old` to `new`: the span between their common prefix
  /// and suffix, in bytes and points (columns in bytes).
  private static func edit(from old: [UInt16], to new: [UInt16]) -> TSInputEdit {
    var prefix = 0
    while prefix < old.count, prefix < new.count, old[prefix] == new[prefix] { prefix += 1 }
    var suffix = 0
    while suffix < old.count - prefix, suffix < new.count - prefix,
      old[old.count - 1 - suffix] == new[new.count - 1 - suffix]
    {
      suffix += 1
    }
    func point(_ units: [UInt16], _ offset: Int) -> TSPoint {
      var row: UInt32 = 0
      var lineStart = 0
      for i in 0..<offset where units[i] == 0x0A {
        row += 1
        lineStart = i + 1
      }
      return TSPoint(row: row, column: UInt32((offset - lineStart) * 2))
    }
    return TSInputEdit(
      start_byte: UInt32(prefix * 2), old_end_byte: UInt32((old.count - suffix) * 2),
      new_end_byte: UInt32((new.count - suffix) * 2), start_point: point(old, prefix),
      old_end_point: point(old, old.count - suffix), new_end_point: point(new, new.count - suffix))
  }
}

/// The first index of the ascending `values` whose value satisfies
/// `predicate`, which holds from some index on, or `values.count`.
private func firstIndex(in values: [Int], where predicate: (Int) -> Bool) -> Int {
  var low = 0
  var high = values.count
  while low < high {
    let middle = (low + high) / 2
    if predicate(values[middle]) { high = middle } else { low = middle + 1 }
  }
  return low
}

/// A compiled highlights query: its captures as Prism token names, and the
/// text predicates of its patterns, which tree-sitter leaves to the caller.
private final class HighlightsQuery {
  private let query: OpaquePointer
  private let cursor: OpaquePointer
  /// The Prism names, by id; id 0 is no type.
  private(set) var typeNames = [""]
  /// The type id of each capture, 0 for one that colors nothing.
  private var tokenTypes: [UInt8] = []
  private let predicates: [[Predicate]]

  init?(_ language: OpaquePointer, _ source: String) {
    var errorOffset: UInt32 = 0
    var errorType = TSQueryErrorNone
    let utf8 = Array(source.utf8)
    guard
      let query = utf8.withUnsafeBufferPointer({
        $0.withMemoryRebound(to: CChar.self) {
          ts_query_new(language, $0.baseAddress, UInt32($0.count), &errorOffset, &errorType)
        }
      })
    else { return nil }
    self.query = query
    cursor = ts_query_cursor_new()
    func string(
      _ id: UInt32,
      _ get: (OpaquePointer, UInt32, UnsafeMutablePointer<UInt32>) -> UnsafePointer<CChar>?
    ) -> String {
      var length: UInt32 = 0
      guard let pointer = get(query, id, &length) else { return "" }
      return pointer.withMemoryRebound(to: UInt8.self, capacity: Int(length)) {
        String(decoding: UnsafeBufferPointer(start: $0, count: Int(length)), as: UTF8.self)
      }
    }
    var ids: [String: UInt8] = [:]
    var names = [""]
    let tokenTypes = (0..<ts_query_capture_count(query)).map { capture -> UInt8 in
      guard let type = prismTokenType(string(capture, ts_query_capture_name_for_id)) else {
        return 0
      }
      if let id = ids[type] { return id }
      names.append(type)
      ids[type] = UInt8(names.count - 1)
      return UInt8(names.count - 1)
    }
    predicates = (0..<ts_query_pattern_count(query)).map { pattern in
      var count: UInt32 = 0
      guard let steps = ts_query_predicates_for_pattern(query, pattern, &count) else { return [] }
      var result: [Predicate] = []
      var arguments: [Predicate.Argument] = []
      for step in UnsafeBufferPointer(start: steps, count: Int(count)) {
        switch step.type {
        case TSQueryPredicateStepTypeCapture: arguments.append(.capture(step.value_id))
        case TSQueryPredicateStepTypeString:
          arguments.append(.string(string(step.value_id, ts_query_string_value_for_id)))
        default:
          if let predicate = Predicate(arguments) { result.append(predicate) }
          arguments = []
        }
      }
      return result
    }
    self.tokenTypes = tokenTypes
    typeNames = names
  }

  deinit {
    ts_query_cursor_delete(cursor)
    ts_query_delete(query)
  }

  /// Sets the type id of each UTF-16 unit of `units` in `range`: the
  /// innermost capture over it and, of captures of the same node, the
  /// earliest pattern's, as tree-sitter's highlighter ranks them.
  func highlight(_ root: TSNode, _ units: [UInt16], _ range: Range<Int>, _ types: inout [UInt8]) {
    var spans: [(size: Int, pattern: Int, start: Int, end: Int, type: UInt8)] = []
    ts_query_cursor_set_byte_range(
      cursor, UInt32(range.lowerBound * 2), UInt32(range.upperBound * 2))
    ts_query_cursor_exec(cursor, query, root)
    var match = TSQueryMatch()
    while ts_query_cursor_next_match(cursor, &match) {
      let captures = UnsafeBufferPointer(start: match.captures, count: Int(match.capture_count))
      let pattern = Int(match.pattern_index)
      if !predicates[pattern].isEmpty {
        func text(_ id: UInt32) -> [String] {
          captures.filter { $0.index == id }.map {
            let start = Int(ts_node_start_byte($0.node)) / 2
            let end = Int(ts_node_end_byte($0.node)) / 2
            return String(decoding: units[start..<end], as: UTF16.self)
          }
        }
        guard predicates[pattern].allSatisfy({ $0.holds(text) }) else { continue }
      }
      for capture in captures {
        let type = tokenTypes[Int(capture.index)]
        let nodeStart = Int(ts_node_start_byte(capture.node)) / 2
        let nodeEnd = Int(ts_node_end_byte(capture.node)) / 2
        let start = max(range.lowerBound, nodeStart)
        let end = min(range.upperBound, nodeEnd)
        guard type != 0, start < end else { continue }
        spans.append((nodeEnd - nodeStart, pattern, start, end, type))
      }
    }
    // Larger nodes first, so an inner one overwrites them; of one size, the
    // latest pattern first, so the earliest wins. The spans are clipped to
    // `range`; the order is by the whole node.
    spans.sort { ($0.size, $0.pattern) > ($1.size, $1.pattern) }
    types.withUnsafeMutableBufferPointer { types in
      for i in range { types[i] = 0 }
      for span in spans {
        for i in span.start..<span.end { types[i] = span.type }
      }
    }
  }
}

/// A text predicate of a query pattern: `#eq?`, `#match?` and `#any-of?`,
/// and their `not-` forms. Others (`#set!`, `#is?`) do not filter.
private struct Predicate {
  enum Argument {
    case capture(UInt32)
    case string(String)
  }

  private let name: String
  private let capture: UInt32
  private let other: Argument?
  private let values: [String]
  private let regex: Regex<AnyRegexOutput>?

  init?(_ arguments: [Argument]) {
    guard case .string(let name)? = arguments.first, arguments.count >= 3,
      case .capture(let capture) = arguments[1]
    else { return nil }
    var bare = Substring(name)
    for prefix in ["not-", "any-"] where bare.hasPrefix(prefix) {
      bare = bare.dropFirst(prefix.count)
    }
    guard ["eq?", "match?", "lua-match?", "of?"].contains(bare) else { return nil }
    self.name = name
    self.capture = capture
    other = arguments[2]
    values = arguments.dropFirst(2).compactMap {
      if case .string(let value) = $0 { value } else { nil }
    }
    regex = bare.hasSuffix("match?") ? values.first.flatMap { try? Regex($0) } : nil
  }

  func holds(_ text: (UInt32) -> [String]) -> Bool {
    let negated = name.hasPrefix("not-")
    let texts = text(capture)
    let test: (String) -> Bool
    if name.hasSuffix("any-of?") {
      test = { values.contains($0) }
    } else if name.hasSuffix("match?") {
      guard let regex else { return true }
      test = { (try? regex.firstMatch(in: $0)) != nil }
    } else if case .capture(let id)? = other {
      let others = text(id)
      test = { others.contains($0) }
    } else {
      test = { $0 == values.first }
    }
    return texts.allSatisfy { test($0) != negated }
  }
}

/// The Prism token name of a tree-sitter capture, such as `keyword` for
/// `@keyword.return`, or `nil` for one Prism leaves untyped (variables,
/// parameters). The names are those the playground theme's `codeHighlight`
/// map knows.
public func prismTokenType(_ capture: String) -> String? {
  if capture.hasPrefix("_") { return nil }
  let parts = capture.split(separator: ".").map(String.init)
  switch (parts.first ?? "", parts.dropFirst().first) {
  case ("comment", _): return "comment"
  case ("string", "regex"), ("string", "regexp"): return "regex"
  case ("string", "escape"), ("escape", _), ("char", _), ("character", _): return "char"
  case ("string", "special"): return parts.count > 2 && parts[2] == "url" ? "url" : "string"
  case ("string", _): return "string"
  case ("number", _), ("float", _): return "number"
  case ("boolean", _): return "boolean"
  case ("constant", _): return "constant"
  case ("keyword", _), ("conditional", _), ("repeat", _), ("include", _), ("exception", _):
    return "keyword"
  case ("operator", _): return "operator"
  case ("punctuation", _): return "punctuation"
  case ("function", "builtin"): return "builtin"
  case ("function", _), ("method", _): return "function"
  case ("type", "builtin"): return "builtin"
  case ("constructor", _), ("type", _): return "class-name"
  case ("property", _), ("field", _): return "property"
  case ("variable", "member"): return "property"
  case ("variable", "builtin"): return "keyword"
  case ("tag", "attribute"), ("attribute", _): return "attr"
  case ("tag", _): return "tag"
  case ("module", _), ("namespace", _): return "namespace"
  case ("label", _): return "symbol"
  case ("embedded", _): return nil
  default: return nil
  }
}

/// A `CodeTokenizer` that highlights the languages of `grammars`.
public struct TreeSitterTokenizer: CodeTokenizer {
  public let grammars: [TreeSitterGrammar]
  public let defaultLanguage: String?
  private let byLanguage: [String: TreeSitterGrammar]

  public init(_ grammars: [TreeSitterGrammar], defaultLanguage: String? = DEFAULT_CODE_LANGUAGE) {
    self.grammars = grammars
    self.defaultLanguage = defaultLanguage
    var byLanguage: [String: TreeSitterGrammar] = [:]
    for grammar in grammars {
      for language in grammar.languages where byLanguage[language] == nil {
        byLanguage[language] = grammar
      }
    }
    self.byLanguage = byLanguage
  }

  public func languageState(_ language: String) -> CodeLanguageState {
    byLanguage[language.lowercased()] == nil ? .unsupported : .loaded
  }

  public func tokenize(_ code: String, _ language: String) -> [CodeToken] {
    byLanguage[language.lowercased()]?.tokenize(code) ?? [.text(code)]
  }
}
