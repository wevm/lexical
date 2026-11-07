import LexicalTextKit
import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

// The web toolbar's inline style controls (ToolbarPlugin/index.tsx,
// fontSize.tsx and ui/DropdownColorPicker.tsx): the font family dropdown,
// the font size stepper and the text and background color pickers. Each
// patches the selection's inline style with `$patchStyleText` and shows the
// selection's value.

/// `FONT_FAMILY_OPTIONS`.
let fontFamilyOptions = [
  "Arial", "Courier New", "Georgia", "Times New Roman", "Trebuchet MS", "Verdana",
]

/// `FontDropDown` for `font-family`.
struct FontFamilyMenu: View {
  let model: PlaygroundModel

  var body: some View {
    Menu {
      ForEach(fontFamilyOptions, id: \.self) { family in
        Button {
          model.applyStyleText("font-family", family)
        } label: {
          if family == model.fontFamily {
            Label(family, systemImage: "checkmark")
          } else {
            Text(family)
          }
        }
      }
    } label: {
      Label(model.fontFamily, systemImage: "textformat").labelStyle(.titleAndIcon)
    }
    .fixedSize()
    .accessibilityShowsLargeContentViewer()
    .accessibilityLabel("Formatting options for font family")
    .accessibilityValue(model.fontFamily)
  }
}

/// `FontSize`: decrease, the size field and increase, within
/// `MIN_ALLOWED_FONT_SIZE`…`MAX_ALLOWED_FONT_SIZE`.
struct FontSizeStepper: View {
  let model: PlaygroundModel
  @State private var inputValue = ""
  @FocusState private var isFocused: Bool
  /// Two digits' room, at the text's size.
  @ScaledMetric private var fieldWidth: CGFloat = 34

  private var selectionFontSize: String { toolbarFontSize(model.fontSize) }

  /// The web disables a button at the bound only when the selection has a
  /// single size.
  private func isAtBound(_ bound: Int, _ compare: (Double, Double) -> Bool) -> Bool {
    !selectionFontSize.isEmpty && compare(Double(inputValue) ?? 0, Double(bound))
  }

  var body: some View {
    HStack(spacing: 2) {
      Button("Decrease font size", systemImage: "minus") {
        model.stepFontSize(.decrement, from: inputValue)
      }
      .disabled(isAtBound(minAllowedFontSize, <=))
      .help("Decrease font size")
      TextField("", text: $inputValue)
        .multilineTextAlignment(.center)
        .frame(width: fieldWidth)
        .textFieldStyle(.roundedBorder)
        .focused($isFocused)
        #if os(iOS)
          .keyboardType(.numberPad)
        #endif
        .onSubmit(apply)
        .onChange(of: isFocused) { if !isFocused { apply() } }
        .accessibilityLabel("Font size")
        .accessibilityIdentifier("font-size-input")
      Button("Increase font size", systemImage: "plus") {
        model.stepFontSize(.increment, from: inputValue)
      }
      .disabled(isAtBound(maxAllowedFontSize, >=))
      .help("Increase font size")
    }
    .onAppear { inputValue = selectionFontSize }
    .onChange(of: selectionFontSize) { inputValue = selectionFontSize }
  }

  /// `updateFontSizeByInputValue`, for a number typed that differs from
  /// the selection's size.
  private func apply() {
    guard let size = Double(inputValue), inputValue != selectionFontSize else { return }
    model.setFontSize(size)
    inputValue = toolbarFontSize(
      "\(formatFontSize(min(max(size, Double(minAllowedFontSize)), Double(maxAllowedFontSize))))px")
  }
}

/// `DropdownColorPicker`: a toolbar button showing the selection's color
/// that opens the color picker.
struct ColorPickerButton: View {
  let title: String
  let symbol: String
  let color: String
  /// The text size of the picker, which the bar's own size limit leaves
  /// out.
  var panelTypeSize: DynamicTypeSize = .large
  let onChange: (String) -> Void
  @State private var isOpen = false

  var body: some View {
    Button {
      isOpen = true
    } label: {
      VStack(spacing: 1) {
        Image(systemName: symbol)
        RoundedRectangle(cornerRadius: 1)
          .fill(Color(css: color) ?? .clear)
          .overlay(
            RoundedRectangle(cornerRadius: 1).stroke(.secondary.opacity(0.5), lineWidth: 0.5)
          )
          .frame(width: 16, height: 3)
      }
    }
    .accessibilityLabel(title)
    .accessibilityValue(color)
    .help(title)
    .popover(isPresented: $isOpen) {
      ColorPickerPanel(color: color, onChange: onChange).dynamicTypeSize(panelTypeSize)
    }
  }
}

/// `ColorPicker` (ui/ColorPicker.tsx): the hex field and the basic colors;
/// the system color picker stands in for its saturation and hue areas.
struct ColorPickerPanel: View {
  let color: String
  let onChange: (String) -> Void
  @State private var hex = ""

  /// `basicColors`.
  static let basicColors = [
    "#d0021b", "#f5a623", "#f8e71c", "#8b572a", "#7ed321", "#417505", "#bd10e0", "#9013fe",
    "#4a90e2", "#50e3c2", "#b8e986", "#000000", "#4a4a4a", "#9b9b9b", "#ffffff",
  ]

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack {
        Text("Hex")
        TextField("", text: $hex)
          .textFieldStyle(.roundedBorder)
          .autocorrectionDisabled()
          .accessibilityIdentifier("color-hex")
          .onSubmit {
            if let parsed = Color(css: hex) { emit(hex, parsed) }
          }
      }
      LazyVGrid(columns: Array(repeating: GridItem(.fixed(16), spacing: 10), count: 8), spacing: 10)
      {
        ForEach(Self.basicColors, id: \.self) { basic in
          Button {
            emit(basic, Color(css: basic))
          } label: {
            RoundedRectangle(cornerRadius: 4)
              .fill(Color(css: basic) ?? .clear)
              .overlay(RoundedRectangle(cornerRadius: 4).stroke(Color.webLine(0.8)))
              .shadow(color: .black.opacity(basic == hex.lowercased() ? 0.3 : 0), radius: 2)
              .frame(width: 16, height: 16)
          }
          .buttonStyle(.plain)
          .accessibilityLabel(basic)
        }
      }
      ColorPicker(
        "Custom",
        selection: Binding(
          get: { Color(css: hex) ?? .black },
          set: { emit(hexString($0), $0) }))
    }
    .padding(12)
    .frame(width: 214 + 24)
    .onAppear { hex = color }
  }

  private func emit(_ value: String, _ parsed: Color?) {
    guard parsed != nil else { return }
    hex = value
    onChange(value)
  }
}

extension Color {
  /// A CSS color (`#rgb`, `#rrggbb`, `rgb()`, a name), parsed as the text
  /// view parses an inline `color`.
  init?(css: String) {
    guard let parsed = TextKitColor(css: css) else { return nil }
    self.init(.sRGB, red: parsed.red, green: parsed.green, blue: parsed.blue, opacity: parsed.alpha)
  }
}

/// `color` as `#rrggbb`, as the web picker emits it.
private func hexString(_ color: Color) -> String {
  #if os(macOS)
    let platform = NSColor(color).usingColorSpace(.sRGB) ?? .black
    let (r, g, b) = (platform.redComponent, platform.greenComponent, platform.blueComponent)
  #else
    var (r, g, b, a): (CGFloat, CGFloat, CGFloat, CGFloat) = (0, 0, 0, 0)
    UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
  #endif
  func byte(_ v: CGFloat) -> Int { Int((min(max(v, 0), 1) * 255).rounded()) }
  return String(format: "#%02x%02x%02x", byte(r), byte(g), byte(b))
}
