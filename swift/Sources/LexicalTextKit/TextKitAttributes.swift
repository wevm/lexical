import Lexical

// The attribute layer between the model and the text views: what each JS
// `EditorThemeClasses` key looks like natively, and the attributes a text
// node and its block resolve to. Resolution follows the CSS cascade the DOM
// editor gets: the root, then each ancestor from the outside in (its theme
// key, `ltr`/`rtl`, `indent`), then the text node's `text.*` keys, then its
// inline `style`, which wins as an inline style does. It holds no UI types;
// the views map the descriptors to platform fonts and attributes.

/// An sRGB color, components in 0...1, with an optional variant for a dark
/// appearance. The views turn a color with a dark variant into a dynamic
/// system color; a color without one (such as an inline CSS `color`) looks
/// the same in both appearances, as it does on the web.
public struct TextKitColor: Hashable, Sendable {
  public var red: Double
  public var green: Double
  public var blue: Double
  public var alpha: Double
  /// The components in a dark appearance, when they differ.
  public var dark: Components?

  /// A color's sRGB components, in 0...1.
  public struct Components: Hashable, Sendable {
    public var red: Double
    public var green: Double
    public var blue: Double
    public var alpha: Double
  }

  public init(red: Double, green: Double, blue: Double, alpha: Double = 1) {
    self.red = red
    self.green = green
    self.blue = blue
    self.alpha = alpha
  }

  /// This color, shown as `dark` in a dark appearance.
  public func withDark(_ dark: TextKitColor) -> TextKitColor {
    var color = self
    color.dark = Components(red: dark.red, green: dark.green, blue: dark.blue, alpha: dark.alpha)
    return color
  }

  /// The color in a dark appearance (the color itself without a variant).
  public var darkAppearance: TextKitColor {
    guard let dark else { return self }
    return TextKitColor(red: dark.red, green: dark.green, blue: dark.blue, alpha: dark.alpha)
  }

  /// A color from 0...255 channels, as CSS `rgb(r, g, b)` writes it.
  public init(_ red: Int, _ green: Int, _ blue: Int, alpha: Double = 1) {
    self.init(
      red: Double(red) / 255, green: Double(green) / 255, blue: Double(blue) / 255, alpha: alpha)
  }

  /// Parses a CSS color: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`,
  /// `rgba()` (comma or space separated, numbers or percentages) and the
  /// basic named colors. Anything else is `nil`.
  public init?(css: String) {
    let value = cssTrim(css).lowercased()
    if value.hasPrefix("#") {
      let digits = value.dropFirst().map { $0.hexDigitValue }
      guard digits.allSatisfy({ $0 != nil }) else { return nil }
      let d = digits.map { Double($0!) }
      switch d.count {
      case 3, 4:
        self.init(
          red: d[0] * 17 / 255, green: d[1] * 17 / 255, blue: d[2] * 17 / 255,
          alpha: d.count == 4 ? d[3] * 17 / 255 : 1)
      case 6, 8:
        self.init(
          red: (d[0] * 16 + d[1]) / 255, green: (d[2] * 16 + d[3]) / 255,
          blue: (d[4] * 16 + d[5]) / 255, alpha: d.count == 8 ? (d[6] * 16 + d[7]) / 255 : 1)
      default:
        return nil
      }
      return
    }
    if let open = value.firstIndex(of: "("), value.hasSuffix(")") {
      let name = cssTrim(String(value[..<open]))
      guard name == "rgb" || name == "rgba" else { return nil }
      let inner = String(value[value.index(after: open)..<value.index(before: value.endIndex)])
      var parts: [String]
      if inner.contains(",") {
        parts = inner.split(separator: ",", omittingEmptySubsequences: false).map {
          cssTrim(String($0))
        }
      } else {
        let halves = inner.split(separator: "/", omittingEmptySubsequences: false)
        guard halves.count <= 2 else { return nil }
        parts = halves[0].split(whereSeparator: { $0 == " " || $0 == "\t" }).map(String.init)
        if halves.count == 2 {
          guard parts.count == 3 else { return nil }
          parts.append(cssTrim(String(halves[1])))
        }
      }
      guard parts.count == 3 || parts.count == 4 else { return nil }
      var channels: [Double] = []
      for part in parts.prefix(3) {
        guard let channel = cssNumber(part, percentOf: 255) else { return nil }
        channels.append(min(max(channel, 0), 255) / 255)
      }
      var alpha = 1.0
      if parts.count == 4 {
        guard let a = cssNumber(parts[3], percentOf: 1) else { return nil }
        alpha = min(max(a, 0), 1)
      }
      self.init(red: channels[0], green: channels[1], blue: channels[2], alpha: alpha)
      return
    }
    guard let named = namedColors[value] else { return nil }
    self = named
  }
}

/// The default theme's secondary text (quotes, `h2`) and code background.
private let secondaryText = TextKitColor(101, 103, 107).withDark(TextKitColor(160, 163, 170))
private let codeBackground = TextKitColor(240, 242, 245).withDark(TextKitColor(44, 46, 51))

/// The CSS basic color keywords, plus `orange`, `grey` and `transparent`.
private let namedColors: [String: TextKitColor] = [
  "black": TextKitColor(0, 0, 0), "silver": TextKitColor(192, 192, 192),
  "gray": TextKitColor(128, 128, 128), "grey": TextKitColor(128, 128, 128),
  "white": TextKitColor(255, 255, 255), "maroon": TextKitColor(128, 0, 0),
  "red": TextKitColor(255, 0, 0), "purple": TextKitColor(128, 0, 128),
  "fuchsia": TextKitColor(255, 0, 255), "magenta": TextKitColor(255, 0, 255),
  "green": TextKitColor(0, 128, 0), "lime": TextKitColor(0, 255, 0),
  "olive": TextKitColor(128, 128, 0), "yellow": TextKitColor(255, 255, 0),
  "navy": TextKitColor(0, 0, 128), "blue": TextKitColor(0, 0, 255),
  "teal": TextKitColor(0, 128, 128), "aqua": TextKitColor(0, 255, 255),
  "cyan": TextKitColor(0, 255, 255), "orange": TextKitColor(255, 165, 0),
  "transparent": TextKitColor(0, 0, 0, alpha: 0),
]

private func cssTrim(_ s: String) -> String {
  var units = Substring(s)
  while let first = units.first, first.isWhitespace { units.removeFirst() }
  while let last = units.last, last.isWhitespace { units.removeLast() }
  return String(units)
}

/// A finite CSS number, or a percentage of `percentOf`.
private func cssNumber(_ s: String, percentOf: Double) -> Double? {
  var text = cssTrim(s)
  var scale = 1.0
  if text.hasSuffix("%") {
    text.removeLast()
    scale = percentOf / 100
  }
  guard text.first.map({ $0.isNumber || $0 == "." || $0 == "-" || $0 == "+" }) == true,
    let number = Double(text), number.isFinite
  else { return nil }
  return number * scale
}

/// A font size: in points (CSS `px`; `pt` is converted at 4/3 points
/// each), or a multiple of the surrounding size (`em`, `%`).
public enum TextKitFontSize: Hashable, Sendable {
  case points(Double)
  case scale(Double)

  /// Parses a CSS `font-size` of `Npx`, `Npt`, `Nem` or `N%`; keywords and
  /// other units are `nil`.
  public init?(css: String) {
    let value = cssTrim(css).lowercased()
    for (unit, make) in [
      ("px", { TextKitFontSize.points($0) }), ("pt", { .points($0 * 4 / 3) }),
      ("em", { .scale($0) }), ("%", { .scale($0 / 100) }),
    ] as [(String, (Double) -> TextKitFontSize)] where value.hasSuffix(unit) {
      let number = String(value.dropLast(unit.count))
      guard !number.contains(where: \.isWhitespace), let n = cssNumber(number, percentOf: 1),
        n > 0
      else {
        return nil
      }
      self = make(n)
      return
    }
    return nil
  }

  func resolve(in size: Double) -> Double {
    switch self {
    case .points(let points): points
    case .scale(let scale): scale * size
    }
  }
}

/// CSS `text-transform`. The views apply it when drawing, so the string and
/// its UTF-16 offsets stay the model's.
public enum TextKitTextTransform: String, Hashable, Sendable {
  case none, uppercase, lowercase, capitalize
}

/// CSS `cursor`: the pointer shown over the text, where the platform has
/// one.
public enum TextKitCursor: String, Hashable, Sendable {
  /// The text (I-beam) cursor.
  case text
  /// The pointing hand, over a link.
  case pointer
}

/// A quote's bar: drawn `offset` points after the start of its block's box,
/// `width` wide.
public struct TextKitBlockBar: Hashable, Sendable {
  public var offset: Double
  public var width: Double
  public var color: TextKitColor

  public init(offset: Double, width: Double, color: TextKitColor) {
    self.offset = offset
    self.width = width
    self.color = color
  }
}

/// A code block's line-number gutter, the playground theme's `__code`
/// `::before`: `width` points (padding included) at the start of the
/// block's box, a `borderWidth` border on its end side, and each line's
/// number right-aligned `padding` points before that, in the block's font.
public struct TextKitGutter: Hashable, Sendable {
  public var width: Double
  public var padding: Double
  public var backgroundColor: TextKitColor
  public var borderWidth: Double
  public var borderColor: TextKitColor
  public var foregroundColor: TextKitColor

  public init(
    width: Double, padding: Double, backgroundColor: TextKitColor, borderWidth: Double,
    borderColor: TextKitColor, foregroundColor: TextKitColor
  ) {
    self.width = width
    self.padding = padding
    self.backgroundColor = backgroundColor
    self.borderWidth = borderWidth
    self.borderColor = borderColor
    self.foregroundColor = foregroundColor
  }
}

/// A block's box, which a block's `backgroundColor` fills: from `offset`
/// points after the inline start to the line's end, and from `paddingTop`
/// points above its first line to `paddingBottom` below its last, as CSS
/// paints a block's background over its padding box.
public struct TextKitBlockBox: Hashable, Sendable {
  public var offset: Double
  public var paddingTop: Double
  public var paddingBottom: Double
  public var color: TextKitColor
  /// Line numbers at the box's start, for a code block.
  public var gutter: TextKitGutter?

  public init(
    offset: Double, paddingTop: Double, paddingBottom: Double, color: TextKitColor,
    gutter: TextKitGutter? = nil
  ) {
    self.offset = offset
    self.paddingTop = paddingTop
    self.paddingBottom = paddingBottom
    self.color = color
    self.gutter = gutter
  }
}

/// A block's box and the look of its text, which its gutter's numbers take.
public struct TextKitBlockBoxLayout: Hashable, Sendable {
  public var box: TextKitBlockBox
  public var text: TextKitTextAttributes
  /// The inline start is the right edge.
  public var isRightToLeft: Bool

  public init(box: TextKitBlockBox, text: TextKitTextAttributes, isRightToLeft: Bool) {
    self.box = box
    self.text = text
    self.isRightToLeft = isRightToLeft
  }
}

/// What one theme key looks like. Every property is optional: `nil` leaves
/// what the key's element inherits. The character properties apply to the
/// text inside; the paragraph ones only to blocks.
public struct TextKitStyle: Hashable, Sendable {
  /// Font families in fallback order; `monospace`, `serif` and `sans-serif`
  /// are the CSS generic families.
  public var fontFamily: [String]?
  public var fontSize: TextKitFontSize?
  /// A CSS weight: 400 is normal, 700 bold.
  public var fontWeight: Int?
  public var italic: Bool?
  public var foregroundColor: TextKitColor?
  /// Behind the text for text and inline elements, behind the paragraph for
  /// blocks.
  public var backgroundColor: TextKitColor?
  public var underline: Bool?
  public var strikethrough: Bool?
  /// Raises (positive) or lowers the baseline, in ems of the surrounding
  /// font, as `vertical-align: super` and `sub` do.
  public var baselineShift: Double?
  public var textTransform: TextKitTextTransform?
  /// Points the block's content starts after its own start (margin, border
  /// and padding at the inline start).
  public var headIndent: Double?
  public var spacingBefore: Double?
  public var spacingAfter: Double?
  /// A CSS `line-height` number, a multiple of the block's font size.
  public var lineHeightMultiple: Double?
  public var blockBar: TextKitBlockBar?
  /// A block's padding above its first line and below its last: inside
  /// the box its `backgroundColor` fills, where `spacingBefore` and
  /// `spacingAfter` are outside it.
  public var paddingTop: Double?
  public var paddingBottom: Double?
  /// A block's line-number gutter, at the start of its box.
  public var gutter: TextKitGutter?
  /// CSS `tab-size`: a tab advances to the next multiple of this many
  /// spaces of the block's font.
  public var tabSize: Double?
  /// CSS `cursor` over the text.
  public var cursor: TextKitCursor?

  public init(
    fontFamily: [String]? = nil, fontSize: TextKitFontSize? = nil, fontWeight: Int? = nil,
    italic: Bool? = nil, foregroundColor: TextKitColor? = nil,
    backgroundColor: TextKitColor? = nil, underline: Bool? = nil, strikethrough: Bool? = nil,
    baselineShift: Double? = nil, textTransform: TextKitTextTransform? = nil,
    headIndent: Double? = nil, spacingBefore: Double? = nil, spacingAfter: Double? = nil,
    lineHeightMultiple: Double? = nil, blockBar: TextKitBlockBar? = nil
  ) {
    self.fontFamily = fontFamily
    self.fontSize = fontSize
    self.fontWeight = fontWeight
    self.italic = italic
    self.foregroundColor = foregroundColor
    self.backgroundColor = backgroundColor
    self.underline = underline
    self.strikethrough = strikethrough
    self.baselineShift = baselineShift
    self.textTransform = textTransform
    self.headIndent = headIndent
    self.spacingBefore = spacingBefore
    self.spacingAfter = spacingAfter
    self.lineHeightMultiple = lineHeightMultiple
    self.blockBar = blockBar
  }

  /// The character properties of an inline `style` string, parsed with
  /// `getStyleObjectFromCSS`: `font-size`, `font-family`, `color`,
  /// `background-color` (or a color `background`), `font-weight`,
  /// `font-style`, `text-decoration` and `text-transform`. Unknown
  /// properties and values are ignored.
  public init(css: String) {
    self.init()
    for (property, value) in getStyleObjectFromCSS(css) {
      let lowered = cssTrim(value).lowercased()
      switch property.lowercased() {
      case "font-size": fontSize = TextKitFontSize(css: value) ?? fontSize
      case "font-family": fontFamily = parseFontFamily(value) ?? fontFamily
      case "color": foregroundColor = TextKitColor(css: value) ?? foregroundColor
      case "background-color", "background":
        backgroundColor = TextKitColor(css: value) ?? backgroundColor
      case "font-weight":
        switch lowered {
        case "normal": fontWeight = 400
        case "bold", "bolder": fontWeight = 700
        default:
          if let weight = Int(lowered), (1...1000).contains(weight) { fontWeight = weight }
        }
      case "font-style":
        switch lowered {
        case "italic", "oblique": italic = true
        case "normal": italic = false
        default: break
        }
      case "text-decoration", "text-decoration-line":
        let words = Set(lowered.split(separator: " ").map(String.init))
        if words == ["none"] {
          underline = false
          strikethrough = false
        } else if !words.isEmpty, words.isSubset(of: ["underline", "line-through"]) {
          underline = words.contains("underline")
          strikethrough = words.contains("line-through")
        }
      case "text-transform":
        textTransform = TextKitTextTransform(rawValue: lowered) ?? textTransform
      default: break
      }
    }
  }

  /// `other` on top of this: its non-`nil` properties win.
  func merging(_ other: TextKitStyle) -> TextKitStyle {
    TextKitStyle(
      fontFamily: other.fontFamily ?? fontFamily, fontSize: other.fontSize ?? fontSize,
      fontWeight: other.fontWeight ?? fontWeight, italic: other.italic ?? italic,
      foregroundColor: other.foregroundColor ?? foregroundColor,
      backgroundColor: other.backgroundColor ?? backgroundColor,
      underline: other.underline ?? underline, strikethrough: other.strikethrough ?? strikethrough,
      baselineShift: other.baselineShift ?? baselineShift,
      textTransform: other.textTransform ?? textTransform,
      headIndent: other.headIndent ?? headIndent,
      spacingBefore: other.spacingBefore ?? spacingBefore,
      spacingAfter: other.spacingAfter ?? spacingAfter,
      lineHeightMultiple: other.lineHeightMultiple ?? lineHeightMultiple,
      blockBar: other.blockBar ?? blockBar
    )
    .with {
      $0.paddingTop = other.paddingTop ?? paddingTop
      $0.paddingBottom = other.paddingBottom ?? paddingBottom
      $0.gutter = other.gutter ?? gutter
      $0.tabSize = other.tabSize ?? tabSize
      $0.cursor = other.cursor ?? cursor
    }
  }

  /// This style changed by `body`.
  public func with(_ body: (inout TextKitStyle) -> Void) -> TextKitStyle {
    var style = self
    body(&style)
    return style
  }
}

/// A CSS `font-family` list, quotes removed; `nil` when empty.
private func parseFontFamily(_ value: String) -> [String]? {
  var families: [String] = []
  var current = ""
  var quote: Character?
  for c in value {
    if let q = quote {
      if c == q { quote = nil } else { current.append(c) }
    } else if c == "\"" || c == "'" {
      quote = c
    } else if c == "," {
      families.append(cssTrim(current))
      current = ""
    } else {
      current.append(c)
    }
  }
  families.append(cssTrim(current))
  families = families.filter { !$0.isEmpty }
  return families.isEmpty ? nil : families
}

/// An element as the attribute layer sees it.
public struct TextKitElement: Hashable, Sendable {
  /// The node type, which is also its theme key (`paragraph`, `quote`,
  /// `code`, `link`, ...).
  public var type: String
  /// A heading's tag (`h1`...`h6`), which makes its key `heading.h1`.
  public var tag: String?
  public var isInline: Bool
  /// `getFormatType()`: `""`, `left`, `center`, `right`, `justify`,
  /// `start` or `end`.
  public var format: ElementFormatType
  public var indent: Int
  /// `getDirection()`: `ltr`, `rtl` or `nil`.
  public var direction: ElementDirection?
  /// A list's `listType`: `bullet`, `number` or `check`.
  public var listType: String?
  /// A list item's `value`, its number in its list.
  public var value: Int?
  /// A check list item's `checked`; `nil` outside a check list.
  public var checked: Bool?

  public init(
    type: String, tag: String? = nil, isInline: Bool = false, format: ElementFormatType = "",
    indent: Int = 0, direction: ElementDirection? = nil, listType: String? = nil,
    value: Int? = nil, checked: Bool? = nil
  ) {
    self.type = type
    self.tag = tag
    self.isInline = isInline
    self.format = format
    self.indent = indent
    self.direction = direction
    self.listType = listType
    self.value = value
    self.checked = checked
  }

  /// Reads `element` (call it in a read or update). The heading tag and the
  /// list properties are read from its JSON, so headings and lists need no
  /// dependency on LexicalRichText or LexicalList. The indent is the
  /// element's own `__indent`, which the JS reconciler pads by: a list item
  /// derives `getIndent()` from its nesting, which the list's own margins
  /// already show.
  public init(_ element: ElementNode) throws {
    let type = element.getType()
    let json =
      type == "heading" || type == "list" || type == "listitem"
      ? try element.exportJSON() : nil
    self.init(
      type: type,
      tag: type == "heading" ? json?["tag"]?.stringValue : nil,
      isInline: try element.isInline(), format: try element.getFormatType(),
      indent: try element.getLatest().__indent, direction: try element.getDirection(),
      listType: type == "list" ? json?["listType"]?.stringValue : nil,
      value: type == "listitem" ? json?["value"]?.numberValue.map { Int($0) } : nil,
      checked: type == "listitem" ? json?["checked"]?.boolValue : nil)
  }

  /// The `EditorThemeClasses` key the JS node reads in `createDOM`.
  public var themeKey: String {
    if type == "heading", let tag {
      "heading.\(tag)"
    } else if type == "autolink" {
      // AutoLinkNode is a LinkNode, whose createDOM takes `theme.link`.
      "link"
    } else {
      type
    }
  }
}

/// Where and how a view draws a list item's marker.
public struct TextKitListMarkerLayout: Hashable, Sendable {
  public var marker: TextKitListMarker
  /// Points from the inline start to where the item's text starts.
  public var headIndent: Double
  /// The look of the item's text, which the marker takes.
  public var text: TextKitTextAttributes
  /// The inline start is the right edge.
  public var isRightToLeft: Bool

  /// The check box's frame in the line, as points from the inline start and
  /// a side length, for a font of `fontSize`: the playground theme's
  /// `::before` box, `0.9em` square at the start of the item's `1.5em`
  /// padding.
  public func checkboxOffset(fontSize: Double) -> (start: Double, side: Double) {
    (headIndent - 1.5 * fontSize, 0.9 * fontSize)
  }
}

/// What a list item's paragraph draws before its text: a bullet or number
/// (`list-style-position: outside`), or a check box.
public enum TextKitListMarker: Hashable, Sendable {
  /// `•`, `1.`, `B.`, `iv.`…, drawn to end `gap` points before the text.
  case text(String)
  case checkbox(checked: Bool)

  /// The space between a text marker and the text.
  public static let gap: Double = 8

  /// The marker of an item with `value` in a list of `listType` that is
  /// `depth` lists deep (0 at the top), as the playground theme draws it:
  /// bullets disc, circle, then square, and numbers by `olDepth` (decimal,
  /// upper-alpha, lower-alpha, upper-roman, lower-roman, repeating).
  public static func marker(
    listType: String, depth: Int, value: Int, checked: Bool?
  )
    -> TextKitListMarker
  {
    switch listType {
    case "check": return .checkbox(checked: checked ?? false)
    case "number":
      let n = max(value, 1)
      switch depth % 5 {
      case 1: return .text(alphabetic(n, upper: true) + ".")
      case 2: return .text(alphabetic(n, upper: false) + ".")
      case 3: return .text(roman(n).uppercased() + ".")
      case 4: return .text(roman(n) + ".")
      default: return .text("\(value).")
      }
    default: return .text(depth == 0 ? "•" : depth == 1 ? "◦" : "▪")
    }
  }

  /// CSS `lower-alpha`/`upper-alpha`: a, b… z, aa, ab…
  private static func alphabetic(_ n: Int, upper: Bool) -> String {
    var n = n
    var out = ""
    while n > 0 {
      n -= 1
      out = String(UnicodeScalar(UInt8((upper ? 65 : 97) + n % 26))) + out
      n /= 26
    }
    return out
  }

  /// CSS `lower-roman`, for 1…3999 (decimal past that, as browsers do).
  private static func roman(_ n: Int) -> String {
    guard n < 4000 else { return "\(n)" }
    let numerals: [(Int, String)] = [
      (1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"),
      (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i"),
    ]
    var n = n
    var out = ""
    for (value, numeral) in numerals {
      while n >= value {
        out += numeral
        n -= value
      }
    }
    return out
  }
}

/// `text-align`. `start` is `natural`, which follows the writing direction;
/// `end` is the opposite side, which the view resolves.
public enum TextKitTextAlignment: Hashable, Sendable {
  case natural, left, center, right, justified, end
}

public enum TextKitWritingDirection: Hashable, Sendable {
  /// `dir="auto"` or inherited: detected from the content.
  case natural
  case leftToRight
  case rightToLeft
}

/// The resolved look of a text node's run.
public struct TextKitTextAttributes: Hashable, Sendable {
  /// Families in fallback order; empty is the system font.
  public var fontFamily: [String] = []
  public var fontSize: Double = 16
  public var fontWeight: Int = 400
  public var italic = false
  public var foregroundColor = TextKitColor(0, 0, 0)
  public var backgroundColor: TextKitColor?
  public var underline = false
  public var strikethrough = false
  /// Points the baseline is raised (negative: lowered).
  public var baselineOffset: Double = 0
  /// The text has the `highlight` format.
  public var highlight = false
  public var textTransform = TextKitTextTransform.none
  /// `nil` is the platform's text cursor.
  public var cursor: TextKitCursor?

  public init() {}

  public var bold: Bool { fontWeight >= 600 }
  public var monospace: Bool { fontFamily.contains("monospace") }

  mutating func apply(_ style: TextKitStyle, background: Bool) {
    let size = fontSize
    if let v = style.fontFamily { fontFamily = v }
    if let v = style.fontSize { fontSize = v.resolve(in: size) }
    if let v = style.fontWeight { fontWeight = v }
    if let v = style.italic { italic = v }
    if let v = style.foregroundColor { foregroundColor = v }
    if background, let v = style.backgroundColor { backgroundColor = v }
    if let v = style.underline { underline = v }
    if let v = style.strikethrough { strikethrough = v }
    if let v = style.baselineShift { baselineOffset += v * size }
    if let v = style.textTransform { textTransform = v }
    if let v = style.cursor { cursor = v }
  }
}

/// The resolved look of a block's paragraph.
public struct TextKitParagraphAttributes: Hashable, Sendable {
  public var alignment = TextKitTextAlignment.natural
  public var writingDirection = TextKitWritingDirection.natural
  /// Points from the inline start (left in LTR, right in RTL).
  public var headIndent: Double = 0
  public var spacingBefore: Double = 0
  public var spacingAfter: Double = 0
  /// A CSS `line-height` number: each line is at least this many times the
  /// block's font size, with its text centered in it. `nil` is the font's
  /// own line height.
  public var lineHeightMultiple: Double?
  public var backgroundColor: TextKitColor?
  /// The box the innermost block with a `backgroundColor` fills, its
  /// offset from the inline start.
  public var blockBox: TextKitBlockBox?
  /// CSS `tab-size`, in spaces of the block's font; `nil` is TextKit's
  /// default tab stops.
  public var tabSize: Double?
  /// Quote bars, outermost first, offsets from the inline start.
  public var blockBars: [TextKitBlockBar] = []
  /// A list item's marker, drawn before `headIndent`.
  public var listMarker: TextKitListMarker?

  public init() {}
}

/// What each `EditorThemeClasses` key looks like natively. Keys are the JS
/// key paths, nested keys joined with dots: `root`, `paragraph`, `quote`,
/// `code`, `link`, `mark`, `heading.h1`...`heading.h6`, `indent`, `ltr`,
/// `rtl`, and `text.bold`...`text.underlineStrikethrough`.
/// How a view draws a horizontal rule (`HorizontalRuleNode`, whose
/// `createDOM` is an `<hr>` with the theme's `hr` class): a line in a box
/// with padding and margins, outlined while the rule is selected.
public struct TextKitRule: Hashable, Sendable {
  /// The line's height (`hr:after { height }`).
  public var thickness: Double
  public var color: TextKitColor
  /// Around the line, inside the outline (`hr { padding }`).
  public var padding: Double
  /// Above and below the box (`hr { margin }`).
  public var margin: Double
  /// The outline of a selected rule (`hrSelected { outline }`), drawn
  /// outside the box.
  public var selectedOutlineWidth: Double
  public var selectedOutlineColor: TextKitColor

  public init(
    thickness: Double = 2, color: TextKitColor, padding: Double = 2, margin: Double = 16,
    selectedOutlineWidth: Double = 2, selectedOutlineColor: TextKitColor
  ) {
    self.thickness = thickness
    self.color = color
    self.padding = padding
    self.margin = margin
    self.selectedOutlineWidth = selectedOutlineWidth
    self.selectedOutlineColor = selectedOutlineColor
  }

  /// The box's height with its margins: what the rule's line takes.
  public var height: Double { 2 * margin + 2 * padding + thickness }
}

public struct TextKitTheme: Hashable, Sendable {
  public var styles: [String: TextKitStyle]
  /// Points per indent level, `--lexical-indent-base-value` in JS.
  public var indentWidth: Double
  /// How horizontal rules are drawn; `nil` draws none.
  public var horizontalRule: TextKitRule?

  public init(styles: [String: TextKitStyle] = [:], indentWidth: Double = 40) {
    self.styles = styles
    self.indentWidth = indentWidth
  }

  public subscript(key: String) -> TextKitStyle? {
    get { styles[key] }
    set { styles[key] = newValue }
  }

  /// The upstream playground's look (`PlaygroundEditorTheme.css`), with the
  /// browser's defaults for `h4`...`h6`. The playground is light only; the
  /// colors here have dark variants, so the text stays readable in a dark
  /// appearance.
  public static let `default` = TextKitTheme.playground

  private static let playground: TextKitTheme = {
    var theme = playgroundStyles
    // `.PlaygroundEditorTheme__hr`: `padding: 2px; margin: 1em 0`, a 2px
    // #ccc line, and `outline: 2px solid rgb(60, 132, 244)` when selected.
    theme.horizontalRule = TextKitRule(
      color: TextKitColor(204, 204, 204).withDark(TextKitColor(85, 85, 90)),
      selectedOutlineColor: TextKitColor(60, 132, 244))
    return theme
  }()

  private static let playgroundStyles = TextKitTheme(styles: [
    "root": TextKitStyle(
      fontSize: .points(16), fontWeight: 400,
      foregroundColor: TextKitColor(0, 0, 0).withDark(TextKitColor(235, 235, 240)),
      lineHeightMultiple: 1.7),
    "paragraph": TextKitStyle(spacingBefore: 0, spacingAfter: 0),
    "quote": TextKitStyle(
      foregroundColor: secondaryText, headIndent: 40, spacingAfter: 10,
      blockBar: TextKitBlockBar(
        offset: 20, width: 4, color: TextKitColor(206, 208, 212).withDark(TextKitColor(84, 86, 92)))
    ),
    // `__code`: an 8-point margin above and below, 8 points of padding
    // inside the background, and the `::before` gutter (25 points wide,
    // 8 of padding on each side and a 1-point border), 10 points before
    // the text.
    "code": TextKitStyle(
      fontFamily: ["Menlo", "Consolas", "Monaco", "monospace"], fontSize: .points(13),
      backgroundColor: codeBackground, headIndent: 52, spacingBefore: 8,
      spacingAfter: 8, lineHeightMultiple: 1.53
    ).with {
      $0.paddingTop = 8
      $0.paddingBottom = 8
      $0.tabSize = 2
      $0.gutter = TextKitGutter(
        width: 41, padding: 8,
        backgroundColor: TextKitColor(238, 238, 238).withDark(TextKitColor(52, 54, 60)),
        borderWidth: 1, borderColor: TextKitColor(204, 204, 204).withDark(TextKitColor(72, 74, 80)),
        foregroundColor: TextKitColor(119, 119, 119).withDark(TextKitColor(150, 152, 158)))
    },
    // `__listItem { margin: 0 32px }`; a check list item's `margin-left:
    // 0.5em; padding-left: 1.5em` comes to the same 32 points at 16.
    "listitem": TextKitStyle(headIndent: 32),
    "listitemChecked": TextKitStyle(strikethrough: true),
    // `__link:hover { cursor: pointer }`.
    "link": TextKitStyle(
      foregroundColor: TextKitColor(33, 111, 219).withDark(TextKitColor(88, 166, 255)),
      underline: false
    ).with { $0.cursor = .pointer },
    "mark": TextKitStyle(backgroundColor: TextKitColor(255, 212, 0, alpha: 0.14)),
    "heading.h1": TextKitStyle(
      fontSize: .points(24), fontWeight: 400,
      foregroundColor: TextKitColor(5, 5, 5).withDark(TextKitColor(245, 245, 250))),
    "heading.h2": TextKitStyle(
      fontSize: .points(16), fontWeight: 700, foregroundColor: secondaryText,
      textTransform: .uppercase),
    "heading.h3": TextKitStyle(fontSize: .points(12), textTransform: .uppercase),
    "heading.h4": TextKitStyle(fontSize: .scale(1), fontWeight: 700),
    "heading.h5": TextKitStyle(fontSize: .scale(0.83), fontWeight: 700),
    "heading.h6": TextKitStyle(fontSize: .scale(0.67), fontWeight: 700),
    "text.bold": TextKitStyle(fontWeight: 700),
    "text.italic": TextKitStyle(italic: true),
    "text.underline": TextKitStyle(underline: true),
    "text.strikethrough": TextKitStyle(strikethrough: true),
    "text.underlineStrikethrough": TextKitStyle(underline: true, strikethrough: true),
    "text.code": TextKitStyle(
      fontFamily: ["Menlo", "Consolas", "Monaco", "monospace"], fontSize: .scale(0.94),
      backgroundColor: codeBackground),
    "text.highlight": TextKitStyle(backgroundColor: TextKitColor(255, 212, 0, alpha: 0.14)),
    "text.subscript": TextKitStyle(fontSize: .scale(0.8), baselineShift: -0.2),
    "text.superscript": TextKitStyle(fontSize: .scale(0.8), baselineShift: 0.33),
    "text.lowercase": TextKitStyle(textTransform: .lowercase),
    "text.uppercase": TextKitStyle(textTransform: .uppercase),
    "text.capitalize": TextKitStyle(textTransform: .capitalize),
  ])

  /// The keys an element applies, in cascade order.
  private func styles(of element: TextKitElement) -> [TextKitStyle] {
    var keys = [element.themeKey]
    if let checked = element.checked {
      keys.append(checked ? "listitemChecked" : "listitemUnchecked")
    }
    if element.direction == "ltr" || element.direction == "rtl" { keys.append(element.direction!) }
    if element.indent > 0 { keys.append("indent") }
    return keys.compactMap { styles[$0] }
  }

  /// The `text.*` keys of `format`, as one layer: like the classes JS puts
  /// on one element, later keys (in `TEXT_TYPE_TO_FORMAT` order) win, and
  /// `underlineStrikethrough`, when the theme has it, replaces `underline`
  /// and `strikethrough`.
  private func textStyle(format: Int) -> TextKitStyle {
    var layer = TextKitStyle()
    let both = format & IS_UNDERLINE != 0 && format & IS_STRIKETHROUGH != 0
    let combined = both ? styles["text.underlineStrikethrough"] : nil
    if let combined { layer = layer.merging(combined) }
    for key in TEXT_TYPE_TO_FORMAT.keys.sorted() where format & TEXT_TYPE_TO_FORMAT[key]! != 0 {
      if combined != nil, key == "underline" || key == "strikethrough" { continue }
      if let style = styles["text.\(key)"] { layer = layer.merging(style) }
    }
    return layer
  }

  /// The attributes of a text node with `format` and inline `style` inside
  /// `ancestors`, outermost first (the root may be omitted).
  public func textAttributes(
    format: Int, style: String, ancestors: [TextKitElement]
  )
    -> TextKitTextAttributes
  {
    var attributes = TextKitTextAttributes()
    if let root = styles["root"] { attributes.apply(root, background: false) }
    for element in ancestors where element.type != "root" {
      for style in styles(of: element) { attributes.apply(style, background: element.isInline) }
    }
    attributes.apply(textStyle(format: format).merging(TextKitStyle(css: style)), background: true)
    attributes.highlight = format & IS_HIGHLIGHT != 0
    return attributes
  }

  /// The attributes of the paragraph of `blocks`: the block itself last,
  /// preceded by its ancestors, outermost first. Inline elements are
  /// skipped. Indents and quote bars add up, spacing is the largest, and
  /// alignment, direction, line height and background come from the
  /// innermost block that sets them.
  public func paragraphAttributes(blocks: [TextKitElement]) -> TextKitParagraphAttributes {
    var attributes = TextKitParagraphAttributes()
    var chain: [[TextKitStyle]] = [styles["root"].map { [$0] } ?? []]
    var elements: [TextKitElement?] = [nil]
    for element in blocks where element.type != "root" && !element.isInline {
      chain.append(styles(of: element))
      elements.append(element)
    }
    // A list item whose text this paragraph is (not one that only wraps a
    // nested list, which is no paragraph) shows its list's marker.
    let lists = elements.compactMap { $0 }.filter { $0.type == "list" }
    if let item = elements.last ?? nil, item.type == "listitem", let list = lists.last {
      attributes.listMarker = .marker(
        listType: list.listType ?? "bullet", depth: lists.count - 1, value: item.value ?? 1,
        checked: item.checked)
    }
    for (element, layer) in zip(elements, chain) {
      for style in layer {
        if let bar = style.blockBar {
          attributes.blockBars.append(
            TextKitBlockBar(
              offset: attributes.headIndent + bar.offset, width: bar.width, color: bar.color))
        }
      }
      // A block with a background has a box, from its own start (before
      // its indent) and over its padding.
      if element != nil, let color = layer.compactMap(\.backgroundColor).last {
        attributes.blockBox = TextKitBlockBox(
          offset: attributes.headIndent,
          paddingTop: layer.compactMap(\.paddingTop).last ?? 0,
          paddingBottom: layer.compactMap(\.paddingBottom).last ?? 0, color: color,
          gutter: layer.compactMap(\.gutter).last)
      }
      for style in layer {
        attributes.headIndent += style.headIndent ?? 0
        attributes.spacingBefore = max(attributes.spacingBefore, style.spacingBefore ?? 0)
        attributes.spacingAfter = max(attributes.spacingAfter, style.spacingAfter ?? 0)
        if let v = style.lineHeightMultiple { attributes.lineHeightMultiple = v }
        if let v = style.backgroundColor { attributes.backgroundColor = v }
        if let v = style.tabSize { attributes.tabSize = v }
      }
      guard let element else { continue }
      attributes.headIndent += Double(element.indent) * indentWidth
      switch element.direction {
      case "ltr": attributes.writingDirection = .leftToRight
      case "rtl": attributes.writingDirection = .rightToLeft
      default: break
      }
      switch element.format {
      case "left": attributes.alignment = .left
      case "center": attributes.alignment = .center
      case "right": attributes.alignment = .right
      case "justify": attributes.alignment = .justified
      case "start": attributes.alignment = .natural
      case "end": attributes.alignment = .end
      default: break
      }
    }
    // The padding is inside the box, after the spacing outside it.
    if let box = attributes.blockBox {
      attributes.spacingBefore += box.paddingTop
      attributes.spacingAfter += box.paddingBottom
    }
    return attributes
  }

  /// The attributes of `node`'s run (call it in a read or update).
  public func textAttributes(for node: TextNode) throws -> TextKitTextAttributes {
    try textAttributes(
      format: node.getFormat(), style: node.getStyle(),
      ancestors: node.getParents().reversed().map(TextKitElement.init))
  }

  /// The attributes of `block`'s paragraph (call it in a read or update).
  public func paragraphAttributes(for block: ElementNode) throws -> TextKitParagraphAttributes {
    try paragraphAttributes(
      blocks: ([block] + block.getParents()).reversed().map(TextKitElement.init))
  }
}
