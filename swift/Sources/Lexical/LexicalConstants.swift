// Port of packages/lexical/src/LexicalConstants.ts.

package let NO_DIRTY_NODES = 0
package let HAS_DIRTY_NODES = 1
package let FULL_RECONCILE = 2

// Text node modes
public let IS_NORMAL = 0
public let IS_TOKEN = 1
public let IS_SEGMENTED = 2

// Text node formatting
public let IS_BOLD = 1
public let IS_ITALIC = 1 << 1
public let IS_STRIKETHROUGH = 1 << 2
public let IS_UNDERLINE = 1 << 3
public let IS_CODE = 1 << 4
public let IS_SUBSCRIPT = 1 << 5
public let IS_SUPERSCRIPT = 1 << 6
public let IS_HIGHLIGHT = 1 << 7
public let IS_LOWERCASE = 1 << 8
public let IS_UPPERCASE = 1 << 9
public let IS_CAPITALIZE = 1 << 10

public let IS_ALL_FORMATTING =
  IS_BOLD | IS_ITALIC | IS_STRIKETHROUGH | IS_UNDERLINE | IS_CODE | IS_SUBSCRIPT
  | IS_SUPERSCRIPT | IS_HIGHLIGHT | IS_LOWERCASE | IS_UPPERCASE | IS_CAPITALIZE

// Text node details
public let IS_DIRECTIONLESS = 1
public let IS_UNMERGEABLE = 1 << 1

// Element node formatting
public let IS_ALIGN_LEFT = 1
public let IS_ALIGN_CENTER = 2
public let IS_ALIGN_RIGHT = 3
public let IS_ALIGN_JUSTIFY = 4
public let IS_ALIGN_START = 5
public let IS_ALIGN_END = 6

public let NON_BREAKING_SPACE = "\u{00A0}"
package let ZERO_WIDTH_SPACE = "\u{200b}"

/// The suffix the reconciler appends to a composing text node. The native
/// views follow WebKit, the engine of every Apple platform.
package let COMPOSITION_SUFFIX = NON_BREAKING_SPACE
public let DOUBLE_LINE_BREAK = "\n\n"
package let COMPOSITION_START_CHAR = COMPOSITION_SUFFIX

public let TEXT_TYPE_TO_FORMAT: [String: Int] = [
  "bold": IS_BOLD,
  "capitalize": IS_CAPITALIZE,
  "code": IS_CODE,
  "highlight": IS_HIGHLIGHT,
  "italic": IS_ITALIC,
  "lowercase": IS_LOWERCASE,
  "strikethrough": IS_STRIKETHROUGH,
  "subscript": IS_SUBSCRIPT,
  "superscript": IS_SUPERSCRIPT,
  "underline": IS_UNDERLINE,
  "uppercase": IS_UPPERCASE,
]

public let DETAIL_TYPE_TO_DETAIL: [String: Int] = [
  "directionless": IS_DIRECTIONLESS,
  "unmergeable": IS_UNMERGEABLE,
]

public let ELEMENT_TYPE_TO_FORMAT: [String: Int] = [
  "center": IS_ALIGN_CENTER,
  "end": IS_ALIGN_END,
  "justify": IS_ALIGN_JUSTIFY,
  "left": IS_ALIGN_LEFT,
  "right": IS_ALIGN_RIGHT,
  "start": IS_ALIGN_START,
]

public let ELEMENT_FORMAT_TO_TYPE: [Int: String] = Dictionary(
  uniqueKeysWithValues: ELEMENT_TYPE_TO_FORMAT.map { ($1, $0) })

public let TEXT_MODE_TO_TYPE: [String: Int] = [
  "normal": IS_NORMAL,
  "segmented": IS_SEGMENTED,
  "token": IS_TOKEN,
]

public let TEXT_TYPE_TO_MODE: [Int: String] = Dictionary(
  uniqueKeysWithValues: TEXT_MODE_TO_TYPE.map { ($1, $0) })

public let NODE_STATE_KEY = "$"

// The ranges RTL_REGEX and LTR_REGEX match, as UTF-16 code unit ranges.
private let RTL_RANGES: [ClosedRange<UInt16>] = [0x0591...0x07FF, 0xFB1D...0xFDFD, 0xFE70...0xFEFC]
private let LTR_RANGES: [ClosedRange<UInt16>] = [
  0x41...0x5A, 0x61...0x7A, 0x00C0...0x00D6, 0x00D8...0x00F6, 0x00F8...0x02B8,
  0x0300...0x0590, 0x0800...0x1FFF, 0x200E...0x200E, 0x2C00...0xFB1C,
  0xFE00...0xFE6F, 0xFEFD...0xFFFF,
]

/// `^[^exclude]*[include]`: the first code unit in either set decides.
private func matchesDirection(
  _ text: String, exclude: [ClosedRange<UInt16>], include: [ClosedRange<UInt16>]
) -> Bool {
  for unit in text.utf16 {
    if include.contains(where: { $0.contains(unit) }) { return true }
    if exclude.contains(where: { $0.contains(unit) }) { return false }
  }
  return false
}

/// `RTL_REGEX.test(text)`.
package func testRTL(_ text: String) -> Bool {
  matchesDirection(text, exclude: LTR_RANGES, include: RTL_RANGES)
}

/// `LTR_REGEX.test(text)`.
package func testLTR(_ text: String) -> Bool {
  matchesDirection(text, exclude: RTL_RANGES, include: LTR_RANGES)
}
