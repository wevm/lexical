import SwiftUI

// Dynamic Type for the playground's own UI. Its popups and panels take the
// web playground's pixel sizes, which are their sizes at the default text
// size; `scaledFont` grows them with the reader's setting, as a text
// style's size grows. The bars hold icons, which stop growing at the
// largest standard size and show the large content viewer on a long press
// instead, as UIKit's bars do.

extension View {
  /// The system font, `size` points at the default text size and scaled
  /// as `style` is (`@ScaledMetric`).
  func scaledFont(
    size: CGFloat, weight: Font.Weight = .regular, design: Font.Design = .default,
    relativeTo style: Font.TextStyle = .body
  ) -> some View {
    modifier(ScaledFont(size: size, weight: weight, design: design, style: style))
  }

  /// A bar of icon buttons: they grow up to the largest standard size,
  /// past which a long press shows the large content viewer.
  func barDynamicTypeSize() -> some View {
    dynamicTypeSize(...DynamicTypeSize.xxxLarge)
  }
}

/// A bar's borderless button, which shows the large content viewer.
struct BarButtonStyle: PrimitiveButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    Button(configuration).buttonStyle(.borderless).accessibilityShowsLargeContentViewer()
  }
}

private struct ScaledFont: ViewModifier {
  @ScaledMetric private var size: CGFloat
  let weight: Font.Weight
  let design: Font.Design

  init(size: CGFloat, weight: Font.Weight, design: Font.Design, style: Font.TextStyle) {
    _size = ScaledMetric(wrappedValue: size, relativeTo: style)
    self.weight = weight
    self.design = design
  }

  func body(content: Content) -> some View {
    content.font(.system(size: size, weight: weight, design: design))
  }
}
