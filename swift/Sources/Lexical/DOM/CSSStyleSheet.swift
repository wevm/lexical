/// A `<style>` element's `CSSStyleSheet`: its top-level rules, as
/// `sheet.cssRules` gives them. Only style rules (`CSSStyleRule`) carry
/// anything; at-rules (`@media`, `@page`...) are parsed past.
///
/// The rules follow CSS Syntax's "consume a list of rules" as jsdom's CSSOM
/// parses them: comments and the `<!--`/`-->` tokens between rules are
/// dropped, strings and nested blocks are skipped over when finding a
/// block's end, and an unterminated block runs to the end of the sheet. A
/// style rule's declarations parse as an inline style's do, but with
/// property names matched in any case, as a rule's are.
public enum CSSRule {
  /// `CSSStyleRule`: `selectorText` and `style`.
  case style(selectorText: String, style: CSSStyleDeclaration)
  /// Any other rule (an at-rule).
  case other
}

/// The top-level rules of the style sheet `css` (`sheet.cssRules`).
public func parseStyleSheet(_ css: String) -> [CSSRule] {
  let chars = Array(preprocessCSS(css).unicodeScalars)
  var rules: [CSSRule] = []
  var i = 0
  while i < chars.count {
    let c = chars[i]
    if c.properties.isWhitespace {
      i += 1
    } else if starts(chars, i, "/*") {
      i = skipComment(chars, i)
    } else if starts(chars, i, "<!--") {
      i += 4
    } else if starts(chars, i, "-->") {
      i += 3
    } else if c == "@" {
      // An at-rule ends at a `;` or after its block.
      var j = i + 1
      while j < chars.count, chars[j] != ";", chars[j] != "{" { j = skipToken(chars, j) }
      i = j < chars.count && chars[j] == "{" ? blockEnd(chars, j).next : j + 1
      rules.append(.other)
    } else {
      // A qualified rule: its prelude, then its block. One without a block
      // is a parse error and dropped.
      let start = i
      var j = i
      while j < chars.count, chars[j] != "{" { j = skipToken(chars, j) }
      if j >= chars.count { break }
      let prelude = stripComments(chars[start..<j])
      let (bodyEnd, next) = blockEnd(chars, j)
      let body = stripComments(chars[(j + 1)..<bodyEnd])
      let style = CSSStyleDeclaration()
      style.parse(body, keepCase: false)
      rules.append(.style(selectorText: jsTrim(prelude), style: style))
      i = next
    }
  }
  return rules
}

private func starts(_ chars: [Unicode.Scalar], _ i: Int, _ s: String) -> Bool {
  var j = i
  for c in s.unicodeScalars {
    guard j < chars.count, chars[j] == c else { return false }
    j += 1
  }
  return true
}

private func skipComment(_ chars: [Unicode.Scalar], _ i: Int) -> Int {
  var j = i + 2
  while j < chars.count, !starts(chars, j, "*/") { j += 1 }
  return min(j + 2, chars.count)
}

/// The index after the string starting at `i`; a newline ends an
/// unterminated one.
private func skipString(_ chars: [Unicode.Scalar], _ i: Int) -> Int {
  let quote = chars[i]
  var j = i + 1
  while j < chars.count {
    switch chars[j] {
    case "\\": j += 2
    case "\n": return j
    case quote: return j + 1
    default: j += 1
    }
  }
  return chars.count
}

/// The index after the token at `i`: a comment, a string, an escape, or one
/// character.
private func skipToken(_ chars: [Unicode.Scalar], _ i: Int) -> Int {
  switch chars[i] {
  case "\"", "'": return skipString(chars, i)
  case "\\": return min(i + 2, chars.count)
  case "/" where starts(chars, i, "/*"): return skipComment(chars, i)
  default: return i + 1
  }
}

/// For the block opening at `open`: where its contents end and the index
/// after it (the end of the sheet when it is unterminated).
private func blockEnd(_ chars: [Unicode.Scalar], _ open: Int) -> (end: Int, next: Int) {
  var depth = 0
  var j = open
  while j < chars.count {
    switch chars[j] {
    case "{", "(", "[": depth += 1
    case "}", ")", "]":
      depth = max(depth - 1, 0)
      if depth == 0 && chars[j] == "}" { return (j, j + 1) }
    default: break
    }
    j = skipToken(chars, j)
  }
  return (chars.count, chars.count)
}

/// The text without its comments (strings kept as written).
private func stripComments(_ chars: ArraySlice<Unicode.Scalar>) -> String {
  let chars = Array(chars)
  var out = String.UnicodeScalarView()
  var i = 0
  while i < chars.count {
    let next = min(skipToken(chars, i), chars.count)
    if starts(chars, i, "/*") {
      out.append(" ")
    } else {
      out.append(contentsOf: chars[i..<next])
    }
    i = next
  }
  return String(out)
}
