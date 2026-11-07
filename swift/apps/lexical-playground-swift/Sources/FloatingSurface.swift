import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

// The look of the playground's floating UI (the format toolbar, the
// typeahead menus, the link editor) in both appearances. The web playground
// is light only: its popups are white, with `box-shadow: 0 5px 10px
// rgba(0, 0, 0, 0.3)`. A light appearance keeps that; a dark one uses the
// system's material with a hairline edge, as the views give the light-only
// theme dark variants (`TextKitColor.withDark`).

extension Color {
  /// `light` in a light appearance and `dark` in a dark one.
  init(light: Color, dark: Color) {
    #if os(macOS)
      self.init(
        nsColor: NSColor(name: nil) { appearance in
          appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(dark) : NSColor(light)
        })
    #else
      // Resolved for the traits, so a semantic color (`.label`) on either
      // side takes the appearance too.
      self.init(
        uiColor: UIColor { traits in
          (traits.userInterfaceStyle == .dark ? UIColor(dark) : UIColor(light))
            .resolvedColor(with: traits)
        })
    #endif
  }

  // The playground's panels and controls (the comments, find and replace,
  // the poll, the page break): the web's light-only grays in a light
  // appearance, and the system's semantic colors in a dark one.

  /// A panel's or card's background: the web's white, or the system's
  /// background for content over the window.
  static let panelBackground = Color(light: .white, dark: system(\.panel))

  /// The editor's own background, for what cuts out the lines over it.
  static let editorBackground = Color(light: .white, dark: system(\.editor))

  /// Text in the web's gray `white` (#444 is 0.27): the system's label
  /// colors in a dark appearance, by how strong the gray is.
  static func webText(_ white: Double) -> Color {
    Color(
      light: Color(white: white),
      dark: system(white <= 0.35 ? \.label : white <= 0.55 ? \.secondaryLabel : \.tertiaryLabel))
  }

  /// A border or divider in the web's light gray: the system separator.
  static func webLine(_ white: Double) -> Color {
    Color(light: Color(white: white), dark: system(\.separator))
  }

  /// A fill in the web's light gray (a button's or a field's): the
  /// system's fills, darker grays the stronger ones.
  static func webFill(_ white: Double) -> Color {
    Color(
      light: Color(white: white),
      dark: system(white < 0.85 ? \.fill2 : white < 0.95 ? \.fill3 : \.fill4))
  }

  private struct SystemColors {
    #if os(macOS)
      var panel = Color(nsColor: .windowBackgroundColor)
      var editor = Color(nsColor: .textBackgroundColor)
      var label = Color(nsColor: .labelColor)
      var secondaryLabel = Color(nsColor: .secondaryLabelColor)
      var tertiaryLabel = Color(nsColor: .tertiaryLabelColor)
      var separator = Color(nsColor: .separatorColor)
      var fill2 = Color(nsColor: .secondarySystemFill)
      var fill3 = Color(nsColor: .tertiarySystemFill)
      var fill4 = Color(nsColor: .quaternarySystemFill)
    #else
      var panel = Color(uiColor: .secondarySystemBackground)
      var editor = Color(uiColor: .systemBackground)
      var label = Color(uiColor: .label)
      var secondaryLabel = Color(uiColor: .secondaryLabel)
      var tertiaryLabel = Color(uiColor: .tertiaryLabel)
      var separator = Color(uiColor: .separator)
      var fill2 = Color(uiColor: .secondarySystemFill)
      var fill3 = Color(uiColor: .tertiarySystemFill)
      var fill4 = Color(uiColor: .quaternarySystemFill)
    #endif
  }

  private static func system(_ key: KeyPath<SystemColors, Color>) -> Color {
    SystemColors()[keyPath: key]
  }
}

extension View {
  /// A floating popup's background: white in a light appearance, the
  /// regular material in a dark one, with the web's shadow.
  func floatingSurface(cornerRadius: CGFloat = 8) -> some View {
    modifier(FloatingSurface(cornerRadius: cornerRadius))
  }
}

private struct FloatingSurface: ViewModifier {
  let cornerRadius: CGFloat
  @Environment(\.colorScheme) private var colorScheme

  func body(content: Content) -> some View {
    let shape = RoundedRectangle(cornerRadius: cornerRadius)
    content.background {
      if colorScheme == .dark {
        shape.fill(.regularMaterial)
          .overlay(shape.strokeBorder(.separator, lineWidth: 1))
          .shadow(color: .black.opacity(0.5), radius: 5, y: 5)
      } else {
        shape.fill(.white)
          .shadow(color: .black.opacity(0.3), radius: 5, y: 5)
      }
    }
  }
}
