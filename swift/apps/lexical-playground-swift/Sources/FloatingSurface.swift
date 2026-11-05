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
      self.init(
        uiColor: UIColor { traits in
          traits.userInterfaceStyle == .dark ? UIColor(dark) : UIColor(light)
        })
    #endif
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
