import Lexical

// Port of packages/lexical-markdown/src/utils.ts. A "character" is a JS
// string (one UTF-16 code unit, or a surrogate pair from `characterAt`),
// and `nil` is JS `undefined`, the edge of the line.

/// The transformers of each type, in their order.
package struct TransformersByType {
  public var element: [ElementTransformer] = []
  public var multilineElement: [MultilineElementTransformer] = []
  public var textFormat: [TextFormatTransformer] = []
  public var textMatch: [TextMatchTransformer] = []
}

/// `transformersByType(transformers)`.
package func transformersByType(_ transformers: [any Transformer]) -> TransformersByType {
  var byType = TransformersByType()
  for transformer in transformers {
    switch transformer {
    case let t as ElementTransformer: byType.element.append(t)
    case let t as MultilineElementTransformer: byType.multilineElement.append(t)
    case let t as TextFormatTransformer: byType.textFormat.append(t)
    case let t as TextMatchTransformer: byType.textMatch.append(t)
    default: break
    }
  }
  return byType
}

// CommonMark counts every character in the Unicode P (punctuation) and S
// (symbol) general categories as punctuation. Its Unicode whitespace is Zs
// plus tab, line feed, form feed and carriage return.
// https://spec.commonmark.org/0.31.2/#unicode-punctuation-character
nonisolated(unsafe) private let PUNCTUATION = JSRegExp(#"[\p{P}\p{S}]"#, "u")
nonisolated(unsafe) private let WHITESPACE = JSRegExp(#"[\p{Zs}\t\n\f\r]"#, "u")

private let WHITESPACE_CLASS = 0
private let PUNCTUATION_CLASS = 1
private let OTHER_CLASS = 2

/// How flanking sees `character`; the edge of the line is whitespace.
private func classify(_ character: String?) -> Int {
  guard let character, !WHITESPACE.test(character) else { return WHITESPACE_CLASS }
  return PUNCTUATION.test(character) ? PUNCTUATION_CLASS : OTHER_CLASS
}

/// `isFlankedBy(primary, secondary)`: whether a delimiter run with
/// `primary` on the side it faces and `secondary` on the other is flanking.
/// https://spec.commonmark.org/0.31.2/#left-flanking-delimiter-run
package func isFlankedBy(_ primary: String?, _ secondary: String?) -> Bool {
  isFlankedByClass(classify(primary), classify(secondary))
}

private func isFlankedByClass(_ primary: Int, _ secondary: Int) -> Bool {
  primary == OTHER_CLASS || (primary == PUNCTUATION_CLASS && secondary != OTHER_CLASS)
}

/// Whether a delimiter run can open and close emphasis.
package struct EmphasisSides: Equatable {
  public var canOpen: Bool
  public var canClose: Bool
}

/// `getEmphasisSides(marker, before, after)`: `_` opens or closes inside a
/// word only beside punctuation.
/// https://spec.commonmark.org/0.31.2/#can-open-emphasis
package func getEmphasisSides(
  _ marker: String, _ before: String?, _ after: String?
)
  -> EmphasisSides
{
  let beforeClass = classify(before)
  let afterClass = classify(after)
  let isLeft = isFlankedByClass(afterClass, beforeClass)
  let isRight = isFlankedByClass(beforeClass, afterClass)
  return marker == "_"
    ? EmphasisSides(
      canOpen: isLeft && (!isRight || beforeClass == PUNCTUATION_CLASS),
      canClose: isRight && (!isLeft || afterClass == PUNCTUATION_CLASS))
    : EmphasisSides(canOpen: isLeft, canClose: isRight)
}

/// `isPunctuationOrSpace(character)`.
package func isPunctuationOrSpace(_ character: String?) -> Bool {
  classify(character) != OTHER_CLASS
}

// Readers built on micromark read a character outside the BMP one UTF-16
// unit at a time, so as a letter, and count JavaScript's whitespace, U+FEFF
// among it, as whitespace. Export has to open and close for them as well.
nonisolated(unsafe) private let MICROMARK_ONLY_WHITESPACE = JSRegExp(
  #"[\v\u2028\u2029\ufeff]"#)

private func asMicromarkReads(_ character: String?) -> String? {
  guard let character else { return nil }
  if character.utf16.count > 1 { return "a" }
  return MICROMARK_ONLY_WHITESPACE.test(character) ? " " : character
}

/// `isPortablyFlankedBy`: `isFlankedBy`, both for CommonMark and micromark.
package func isPortablyFlankedBy(_ primary: String?, _ secondary: String?) -> Bool {
  isFlankedBy(primary, secondary)
    && isFlankedBy(asMicromarkReads(primary), asMicromarkReads(secondary))
}

/// `getPortableEmphasisSides`: `getEmphasisSides`, both for CommonMark and
/// micromark.
package func getPortableEmphasisSides(
  _ marker: String, _ before: String?, _ after: String?
)
  -> EmphasisSides
{
  let sides = getEmphasisSides(marker, before, after)
  let micromark = getEmphasisSides(marker, asMicromarkReads(before), asMicromarkReads(after))
  return EmphasisSides(
    canOpen: sides.canOpen && micromark.canOpen, canClose: sides.canClose && micromark.canClose)
}

/// `isLetterForSomeReader(character)`: whether CommonMark or micromark
/// reads `character` as a letter.
package func isLetterForSomeReader(_ character: String) -> Bool {
  !isPunctuationOrSpace(character) || !isPunctuationOrSpace(asMicromarkReads(character))
}

/// `characterAt(text, index)`: the character (code point) that starts at
/// `index`, if any.
package func characterAt(_ text: String, _ index: Int) -> String? {
  guard index >= 0, let unit = text.utf16Unit(at: index) else { return nil }
  if UTF16.isLeadSurrogate(unit), let next = text.utf16Unit(at: index + 1),
    UTF16.isTrailSurrogate(next)
  {
    return String(decoding: [unit, next], as: UTF16.self)
  }
  return String(decoding: [unit], as: UTF16.self)
}

/// `characterBefore(text, index)`: the character (code point) that ends
/// just before `index`, if any.
package func characterBefore(_ text: String, _ index: Int) -> String? {
  if let pair = characterAt(text, index - 2), pair.utf16.count == 2 { return pair }
  return text.jsCharAt(index - 1)
}

/// `toCharacterReference(character)`: `&#<code point>;`.
package func toCharacterReference(_ character: String) -> String {
  "&#\(character.jsCodePointAt(0) ?? 0);"
}

nonisolated(unsafe) private let MARKDOWN_EMPTY_LINE_REG_EXP = JSRegExp(#"^\s{0,3}$"#)

/// `isEmptyParagraph(node)`.
package func isEmptyParagraph(_ node: LexicalNode) throws -> Bool {
  guard let node = node as? ParagraphNode else { return false }
  guard let firstChild = try node.getFirstChild() else { return true }
  return try node.getChildrenSize() == 1 && firstChild is TextNode
    && MARKDOWN_EMPTY_LINE_REG_EXP.test(firstChild.getTextContent())
}

/// A numeric reference that names U+0000, a surrogate or a number past
/// U+10FFFF reads as U+FFFD.
/// https://spec.commonmark.org/0.31.2/#decimal-numeric-character
private func decodeCharacterReference(_ digits: String?, _ hex: String?) -> String {
  let codePoint = digits.flatMap { UInt32($0) } ?? hex.flatMap { UInt32($0, radix: 16) } ?? 0
  guard codePoint != 0, let scalar = Unicode.Scalar(codePoint) else { return "\u{FFFD}" }
  return String(scalar)
}

nonisolated(unsafe) private let UNESCAPE = JSRegExp(
  #"\\([!-/:-@[-`{-~])|&#(?:(\d{1,7})|[xX]([\da-fA-F]{1,6}));"#, "g")

/// `unescapeText(value)`: decodes backslash escapes and numeric character
/// references, without decoding an escaped ampersand or revisiting a
/// decoded reference.
package func unescapeText(_ value: String) -> String {
  value.jsReplace(UNESCAPE) { match in
    match[1] ?? decodeCharacterReference(match[2], match[3])
  }
}

private let BACKSLASH = UInt16(UInt8(ascii: "\\"))

/// `isEscaped(text, index)`: whether an odd run of backslashes precedes
/// `index`.
package func isEscaped(_ text: String, _ index: Int) -> Bool {
  var backslashes = 0
  var i = index - 1
  while i >= 0, text.utf16Unit(at: i) == BACKSLASH {
    backslashes += 1
    i -= 1
  }
  return backslashes % 2 == 1
}

/// `getDelimiterRunEdge(text, index, step)`: where the delimiter run that
/// `text` is cut in at `index` ends, walking from `index` by `step` (1 or
/// -1). Walking back, an escaped delimiter is not part of the run.
package func getDelimiterRunEdge(_ text: String, _ index: Int, _ step: Int) -> Int {
  let delimiter = text.utf16Unit(at: step == 1 ? index - 1 : index)
  let length = text.utf16.count
  var edge = index
  while step == 1
    ? edge < length && text.utf16Unit(at: edge) == delimiter
    : edge > 0 && text.utf16Unit(at: edge - 1) == delimiter && !isEscaped(text, edge - 1)
  {
    edge += step
  }
  return edge
}
