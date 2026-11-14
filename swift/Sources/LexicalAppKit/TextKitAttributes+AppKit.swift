#if os(macOS)
  import AppKit
  import LexicalTextKit

  // The theme's platform-neutral descriptors as AppKit attributes.

  extension TextKitColor {
    /// The color, or with a dark variant, a dynamic color that resolves to
    /// it in a dark appearance.
    var nsColor: NSColor {
      let (r, g, b, a) = (red, green, blue, alpha)
      guard let dark else { return NSColor(srgbRed: r, green: g, blue: b, alpha: a) }
      return NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
          ? NSColor(srgbRed: dark.red, green: dark.green, blue: dark.blue, alpha: dark.alpha)
          : NSColor(srgbRed: r, green: g, blue: b, alpha: a)
      }
    }
  }

  /// The playground theme's check box colors (`#3d87f5` filled when
  /// checked, a `#999` border when not).
  enum ListMarkerColors {
    static let checked = NSColor(srgbRed: 0x3d / 255, green: 0x87 / 255, blue: 0xf5 / 255, alpha: 1)
    static let unchecked = NSColor(
      srgbRed: 0x99 / 255, green: 0x99 / 255, blue: 0x99 / 255, alpha: 1)
  }

  /// CSS weights as `NSFont.Weight`.
  private func nsFontWeight(_ weight: Int) -> NSFont.Weight {
    switch weight {
    case ..<150: .ultraLight
    case ..<250: .thin
    case ..<350: .light
    case ..<450: .regular
    case ..<550: .medium
    case ..<650: .semibold
    case ..<750: .bold
    case ..<850: .heavy
    default: .black
    }
  }

  /// Each family's faces, by lowercased name, read once: reading a face's
  /// traits means creating it.
  @MainActor
  private var familyFaces: [String: [TextKitFontFace]] = [:]

  /// The face of `family` that CSS font matching picks for the weight and
  /// slant.
  @MainActor
  private func namedFont(family: String, weight: Int, italic: Bool, size: CGFloat) -> NSFont? {
    let key = family.lowercased()
    let faces =
      familyFaces[key]
      ?? (NSFontManager.shared.availableMembers(ofFontFamily: family) ?? []).compactMap {
        member -> TextKitFontFace? in
        guard let name = member.first as? String, let font = NSFont(name: name, size: 12) else {
          return nil
        }
        let descriptor = font.fontDescriptor
        let traits = descriptor.object(forKey: .traits) as? [NSFontDescriptor.TraitKey: Any]
        return TextKitFontFace(
          name: name, width: traits?[.width] as? Double ?? 0,
          weight: cssFontWeight(traits?[.weight] as? CGFloat ?? 0),
          italic: descriptor.symbolicTraits.contains(.italic))
      }
    familyFaces[key] = faces
    return faces.match(weight: weight, italic: italic).flatMap { NSFont(name: $0.name, size: size) }
  }

  /// An `NSFont.Weight` as the nearest CSS weight.
  private func cssFontWeight(_ weight: CGFloat) -> Int {
    let weights: [(NSFont.Weight, Int)] = [
      (.ultraLight, 100), (.thin, 200), (.light, 300), (.regular, 400), (.medium, 500),
      (.semibold, 600), (.bold, 700), (.heavy, 800), (.black, 900),
    ]
    return weights.min { abs($0.0.rawValue - weight) < abs($1.0.rawValue - weight) }?.1 ?? 400
  }

  @MainActor
  private var fontCache: [TextKitFontKey: NSFont] = [:]

  private struct TextKitFontKey: Hashable {
    var family: [String]
    var size: Double
    var weight: Int
    var italic: Bool
  }

  extension TextKitTextAttributes {
    /// The first family of the list the system has, at the weight and
    /// slant; the system font when none is.
    @MainActor
    var nsFont: NSFont {
      let key = TextKitFontKey(
        family: fontFamily, size: fontSize, weight: fontWeight, italic: italic)
      if let font = fontCache[key] { return font }
      let size = CGFloat(fontSize)
      let weight = nsFontWeight(fontWeight)
      var font: NSFont?
      for family in fontFamily {
        switch family.lowercased() {
        case "monospace": font = .monospacedSystemFont(ofSize: size, weight: weight)
        case "sans-serif", "system-ui": font = .systemFont(ofSize: size, weight: weight)
        case "serif":
          let system = NSFont.systemFont(ofSize: size, weight: weight)
          font = system.fontDescriptor.withDesign(.serif).flatMap {
            NSFont(descriptor: $0, size: size)
          }
        default:
          font = namedFont(family: family, weight: fontWeight, italic: italic, size: size)
        }
        if font != nil { break }
      }
      var result = font ?? .systemFont(ofSize: size, weight: weight)
      if bold, !result.fontDescriptor.symbolicTraits.contains(.bold) {
        result = NSFontManager.shared.convert(result, toHaveTrait: .boldFontMask)
      }
      if italic {
        let descriptor = result.fontDescriptor.withSymbolicTraits(
          result.fontDescriptor.symbolicTraits.union(.italic))
        result = NSFont(descriptor: descriptor, size: size) ?? result
        if !result.fontDescriptor.symbolicTraits.contains(.italic) {
          result = NSFontManager.shared.convert(result, toHaveTrait: .italicFontMask)
        }
      }
      fontCache[key] = result
      return result
    }

    @MainActor
    var nsAttributes: [NSAttributedString.Key: Any] {
      var attributes: [NSAttributedString.Key: Any] = [
        .font: nsFont, .foregroundColor: foregroundColor.nsColor,
      ]
      if let backgroundColor { attributes[.backgroundColor] = backgroundColor.nsColor }
      if underline { attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue }
      if strikethrough { attributes[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
      if baselineOffset != 0 { attributes[.baselineOffset] = baselineOffset }
      if cursor == .pointer { attributes[.cursor] = NSCursor.pointingHand }
      if hoverUnderline { attributes[.lexicalHoverUnderline] = true }
      return attributes
    }
  }

  extension TextKitParagraphAttributes {
    /// The paragraph style of a block whose text is in `font`. A CSS
    /// `line-height` number is that many times the font size: the line is
    /// at least that tall, and its text is centered in it by
    /// `CenteredLineLayoutFragment`. A `tab-size` is that many of the
    /// font's spaces.
    func nsParagraphStyle(font: NSFont) -> NSParagraphStyle {
      let fontSize = font.pointSize
      let style = NSMutableParagraphStyle()
      let isRTL = writingDirection == .rightToLeft
      style.alignment =
        switch alignment {
        case .natural: .natural
        case .left: .left
        case .center: .center
        case .right: .right
        case .justified: .justified
        // `end` is the side opposite the writing direction's start.
        case .end: isRTL ? .left : .right
        }
      style.baseWritingDirection =
        switch writingDirection {
        case .natural: .natural
        case .leftToRight: .leftToRight
        case .rightToLeft: .rightToLeft
        }
      style.firstLineHeadIndent = headIndent
      style.headIndent = headIndent
      style.paragraphSpacingBefore = spacingBefore
      style.paragraphSpacing = spacingAfter
      if let lineHeightMultiple { style.minimumLineHeight = lineHeightMultiple * fontSize }
      if let tabSize {
        let space = (" " as NSString).size(withAttributes: [.font: font]).width
        style.tabStops = []
        style.defaultTabInterval = tabSize * space
      }
      return style
    }
  }

  extension TextKitStyledEdit {
    /// The edit's text with its runs' attributes, and `base` elsewhere (the
    /// separators).
    @MainActor
    func attributedString(base: [NSAttributedString.Key: Any]) -> NSAttributedString {
      let result = NSMutableAttributedString(string: edit.string, attributes: base)
      for run in runs where run.range.length > 0 {
        result.addAttributes(
          run.attributes.nsAttributes,
          range: NSRange(location: run.range.location - edit.location, length: run.range.length))
      }
      return result
    }
  }

  extension NSAttributedString.Key {
    /// Text underlined while the pointer is over its inline element
    /// (`TextKitStyle.hoverUnderline`).
    static let lexicalHoverUnderline = NSAttributedString.Key("LexicalHoverUnderline")
  }
#endif
