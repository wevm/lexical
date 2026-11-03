#if canImport(UIKit)
  import UIKit
  import LexicalTextKit

  // The theme's platform-neutral descriptors as UIKit attributes.

  extension TextKitColor {
    var uiColor: UIColor {
      UIColor(red: red, green: green, blue: blue, alpha: alpha)
    }
  }

  /// CSS weights as `UIFont.Weight`.
  private func uiFontWeight(_ weight: Int) -> UIFont.Weight {
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

  @MainActor
  private var fontCache: [TextKitFontKey: UIFont] = [:]

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
    var uiFont: UIFont {
      let key = TextKitFontKey(
        family: fontFamily, size: fontSize, weight: fontWeight, italic: italic)
      if let font = fontCache[key] { return font }
      let size = CGFloat(fontSize)
      let weight = uiFontWeight(fontWeight)
      var font: UIFont?
      for family in fontFamily {
        switch family.lowercased() {
        case "monospace": font = .monospacedSystemFont(ofSize: size, weight: weight)
        case "sans-serif", "system-ui": font = .systemFont(ofSize: size, weight: weight)
        case "serif":
          let system = UIFont.systemFont(ofSize: size, weight: weight)
          font = system.fontDescriptor.withDesign(.serif).map { UIFont(descriptor: $0, size: size) }
        default:
          if !UIFont.fontNames(forFamilyName: family).isEmpty {
            let descriptor = UIFontDescriptor(fontAttributes: [
              .family: family, .traits: [UIFontDescriptor.TraitKey.weight: weight],
            ])
            font = UIFont(descriptor: descriptor, size: size)
          }
        }
        if font != nil { break }
      }
      var result = font ?? .systemFont(ofSize: size, weight: weight)
      var traits = result.fontDescriptor.symbolicTraits
      if bold { traits.insert(.traitBold) }
      if italic { traits.insert(.traitItalic) }
      if traits != result.fontDescriptor.symbolicTraits,
        let descriptor = result.fontDescriptor.withSymbolicTraits(traits)
      {
        result = UIFont(descriptor: descriptor, size: size)
      }
      fontCache[key] = result
      return result
    }

    @MainActor
    var uiAttributes: [NSAttributedString.Key: Any] {
      var attributes: [NSAttributedString.Key: Any] = [
        .font: uiFont, .foregroundColor: foregroundColor.uiColor,
      ]
      if let backgroundColor { attributes[.backgroundColor] = backgroundColor.uiColor }
      if underline { attributes[.underlineStyle] = NSUnderlineStyle.single.rawValue }
      if strikethrough { attributes[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
      if baselineOffset != 0 { attributes[.baselineOffset] = baselineOffset }
      return attributes
    }
  }

  extension TextKitParagraphAttributes {
    var uiParagraphStyle: NSParagraphStyle {
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
      if let lineHeightMultiple { style.lineHeightMultiple = lineHeightMultiple }
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
          run.attributes.uiAttributes,
          range: NSRange(location: run.range.location - edit.location, length: run.range.length))
      }
      return result
    }
  }
#endif
