import Foundation
import Lexical
import SwiftUI
import WebKit

// Ports of packages/lexical-playground/src/nodes/EquationComponent.tsx,
// ui/KatexRenderer.tsx, ui/EquationEditor.tsx and
// ui/KatexEquationAlterer.tsx: the equation drawn by KaTeX, the editor a
// double-click opens on it, and the Insert Equation dialog.

/// `.editor-equation.focused`'s outline, `rgb(60, 132, 244)`.
private let equationFocusColor = Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255)

/// What KaTeX drew: its size, and how far it reaches below the baseline.
struct KatexSize: Equatable {
  var width: CGFloat
  var height: CGFloat
  var depth: CGFloat
}

/// The sizes measured so far, by equation, display mode and font size, so
/// a redrawn equation is laid out at its size at once.
@MainActor private var katexSizes: [String: KatexSize] = [:]

private func katexSizeKey(_ equation: String, _ inline: Bool, _ fontSize: CGFloat) -> String {
  "\(inline ? "i" : "b")\(fontSize):\(equation)"
}

/// `EquationComponent`: `KatexRenderer` while it is not being edited, and
/// `EquationEditor` from a double-click until Escape or the selection leaves
/// it.
struct EquationComponent: View {
  let editor: Editor
  let nodeKey: NodeKey
  let equation: String
  let inline: Bool
  @State private var selection = NodeSelectionState()
  @State private var isEditing = false
  @State private var value = ""

  var body: some View {
    Group {
      if isEditing {
        EquationEditor(equation: $value, inline: inline, onDone: hide, onDeleteEmpty: deleteEmpty)
      } else {
        KatexRenderer(equation: equation, inline: inline, onSize: { _ in relayout() })
          .contentShape(Rectangle())
          .onTapGesture(count: 2) {
            guard editor.isEditable() else { return }
            value = equation
            isEditing = true
          }
      }
    }
    .overlay {
      if selection.isSelected && !isEditing {
        Rectangle().stroke(equationFocusColor, lineWidth: 2)
      }
    }
    .frame(maxWidth: inline ? nil : .infinity)
    .onAppear { selection.observe(editor, nodeKey) }
    // The view's size is the line's: lay it out again as the editor opens,
    // grows and closes.
    .onChange(of: isEditing) { relayout() }
    .onChange(of: value) { if isEditing { relayout() } }
    // Its editor's field while it is edited.
    .accessibilityElement(children: isEditing ? .contain : .ignore)
    .accessibilityLabel("Equation: \(equation)")
    .accessibilityAddTraits(.isButton)
    .accessibilityIdentifier("equation")
  }

  /// Redraws the node, which measures its view again, outside the history.
  private func relayout() {
    try? editor.update(tag: HISTORY_MERGE_TAG) { try getNodeByKey(nodeKey)?.markDirty() }
  }

  /// `onHide`: writes the edited equation, and on Escape puts the caret
  /// after it.
  private func hide(_ restoreSelection: Bool) {
    isEditing = false
    let value = value
    try? editor.update {
      guard let node = try getNodeByKey(nodeKey) as? EquationNode else { return }
      try node.setEquation(value)
      if restoreSelection { _ = try node.selectNext(0, 0) }
    }
  }

  /// `onDeleteEmpty`: Backspace in an empty editor removes the equation.
  private func deleteEmpty() {
    isEditing = false
    try? editor.update {
      guard let node = try getNodeByKey(nodeKey) as? EquationNode else { return }
      if try node.isInline() {
        try setSelection(nil)
        try node.remove(preserveEmptyParent: true)
        return
      }
      if let previous = try node.getPreviousSibling(),
        previous is ElementNode || previous is TextNode
      {
        try node.remove()
        _ = try previous.selectEnd()
        return
      }
      let paragraph = try createParagraphNode()
      try node.replace(paragraph)
      _ = try paragraph.select()
    }
  }
}

/// `EquationEditor`: the LaTeX between grey `$` (inline) or `$$` lines
/// (block), in `#8421a2` on `#eee`.
private struct EquationEditor: View {
  @Binding var equation: String
  let inline: Bool
  let onDone: (Bool) -> Void
  let onDeleteEmpty: () -> Void
  @FocusState private var isFocused: Bool

  var body: some View {
    let dollar = Text(inline ? "$" : "$$").foregroundStyle(Color.webText(0.69))
    let field = TextField("", text: $equation, axis: inline ? .horizontal : .vertical)
      .textFieldStyle(.plain)
      .texInput()
      .foregroundStyle(
        Color(
          light: Color(red: 0x84 / 255, green: 0x21 / 255, blue: 0xa2 / 255),
          dark: Color(red: 0xd4 / 255, green: 0x8f / 255, blue: 0xec / 255))
      )
      .focused($isFocused)
      .onKeyPress(.escape) {
        onDone(true)
        return .handled
      }
      .onKeyPress(.delete) {
        guard equation.isEmpty else { return .ignored }
        onDeleteEmpty()
        return .handled
      }
      .onSubmit { if inline { onDone(true) } }
      .accessibilityIdentifier("equation-editor")
    Group {
      if inline {
        HStack(spacing: 0) {
          dollar
          field.fixedSize()
          dollar
        }
      } else {
        VStack(alignment: .leading, spacing: 0) {
          dollar
          field
          dollar
        }
      }
    }
    .scaledFont(size: 15, design: .monospaced, relativeTo: .subheadline)
    .background(Color.webFill(0.933))
    .onAppear { isFocused = true }
    .onChange(of: isFocused) { if !isFocused { onDone(false) } }
  }
}

/// `KatexRenderer`: `katex.render` with the playground's options, in a web
/// view sized to what it drew. `onSize` follows a size it had not
/// measured yet.
struct KatexRenderer: View {
  let equation: String
  let inline: Bool
  var onSize: ((KatexSize) -> Void)?
  @State private var measured: KatexSize?
  /// The page's 16 pixels, at the reader's text size.
  @ScaledMetric(relativeTo: .body) private var fontSize: CGFloat = 16

  var body: some View {
    let key = katexSizeKey(equation, inline, fontSize)
    let size = katexSizes[key] ?? measured
    KatexWebView(
      equation: equation, inline: inline, fontSize: fontSize,
      onSize: { size in
        measured = size
        guard katexSizes[key] != size else { return }
        katexSizes[key] = size
        onSize?(size)
      }
    )
    // The size KaTeX drew at: an inline equation sits on the text's
    // baseline, and a block one centers in the line, as `.katex-display`.
    .frame(width: max(size?.width ?? 1, 1), height: max(size?.height ?? 20, 1))
    .frame(maxWidth: inline ? nil : .infinity)
    .alignmentGuide(.lastTextBaseline) { d in d.height - (size?.depth ?? 0) }
    .allowsHitTesting(false)
  }
}

/// The web view for `KatexRenderer`: `katex/equation.html`, which renders
/// the equation and posts its size to the `size` handler.
private struct KatexWebView: PlatformViewRepresentable {
  let equation: String
  let inline: Bool
  let fontSize: CGFloat
  let onSize: (KatexSize) -> Void

  final class Coordinator: NSObject, WKScriptMessageHandler, WKNavigationDelegate {
    var shown: (String, Bool, CGFloat)?
    var loaded = false
    var pending: (String, Bool, CGFloat)?
    var onSize: ((KatexSize) -> Void)?

    func userContentController(
      _ userContentController: WKUserContentController, didReceive message: WKScriptMessage
    ) {
      guard let body = message.body as? [String: Any],
        let width = body["width"] as? Double, let height = body["height"] as? Double
      else { return }
      let size = KatexSize(
        width: width, height: height, depth: (body["depth"] as? Double).map { CGFloat($0) } ?? 0)
      MainActor.assumeIsolated { onSize?(size) }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
      MainActor.assumeIsolated {
        loaded = true
        if let pending { show(webView, pending.0, pending.1, pending.2) }
      }
    }

    @MainActor func show(
      _ webView: WKWebView, _ equation: String, _ inline: Bool, _ fontSize: CGFloat
    ) {
      guard loaded else {
        pending = (equation, inline, fontSize)
        return
      }
      pending = nil
      shown = (equation, inline, fontSize)
      let data = try? JSONSerialization.data(withJSONObject: [equation])
      let argument = data.flatMap { String(data: $0, encoding: .utf8) } ?? "[\"\"]"
      webView.evaluateJavaScript("show(\(argument)[0], \(inline), \(fontSize))")
    }
  }

  func makeCoordinator() -> Coordinator { Coordinator() }

  private func makeWebView(_ coordinator: Coordinator) -> WKWebView {
    let configuration = WKWebViewConfiguration()
    configuration.userContentController.add(coordinator, name: "size")
    let webView = WKWebView(frame: .zero, configuration: configuration)
    webView.navigationDelegate = coordinator
    #if os(macOS)
      webView.setValue(false, forKey: "drawsBackground")
    #else
      webView.isOpaque = false
      webView.backgroundColor = .clear
      webView.scrollView.isScrollEnabled = false
      webView.scrollView.backgroundColor = .clear
    #endif
    webView.loadFileURL(
      katexDirectory.appendingPathComponent("equation.html"), allowingReadAccessTo: katexDirectory)
    return webView
  }

  private func update(_ webView: WKWebView, _ coordinator: Coordinator) {
    coordinator.onSize = onSize
    if let shown = coordinator.shown, shown == (equation, inline, fontSize) { return }
    coordinator.show(webView, equation, inline, fontSize)
  }

  #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { makeWebView(context.coordinator) }
    func updateNSView(_ webView: WKWebView, context: Context) {
      update(webView, context.coordinator)
    }
  #else
    func makeUIView(context: Context) -> WKWebView { makeWebView(context.coordinator) }
    func updateUIView(_ webView: WKWebView, context: Context) {
      update(webView, context.coordinator)
    }
  #endif
}

/// `InsertEquationDialog` (`KatexEquationAlterer`): Inline, the equation
/// (a field inline, a text area for a block), its visualization as a
/// block, and Confirm.
struct InsertEquationDialog: View {
  let editor: Editor
  @Environment(\.dismiss) private var dismiss
  @State private var equation = ""
  @State private var inline = true

  var body: some View {
    NavigationStack {
      Form {
        Toggle("Inline", isOn: $inline)
          #if os(macOS)
            .toggleStyle(.checkbox)
          #endif
          .accessibilityIdentifier("equation-inline-checkbox")
        Section("Equation") {
          TextField("", text: $equation, axis: inline ? .horizontal : .vertical)
            .lineLimit(inline ? 1...1 : 3...6)
            .font(.system(.body, design: .monospaced))
            .labelsHidden()
            .multilineTextAlignment(.leading)
            .texInput()
            .accessibilityIdentifier("equation-input")
        }
        Section("Visualization") {
          KatexRenderer(equation: equation, inline: false)
            .frame(minHeight: 40)
        }
        Button("Confirm") {
          _ = try? editor.dispatchCommand(
            INSERT_EQUATION_COMMAND, EquationPayload(equation: equation, inline: inline))
          dismiss()
        }
        .accessibilityIdentifier("equation-submit-btn")
      }
      .formStyle(.grouped)
      .navigationTitle("Insert Equation")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
      }
    }
    .frame(minWidth: 420, minHeight: 320)
  }
}

extension View {
  /// TeX is not prose: no autocorrection, spell check or capitalization.
  fileprivate func texInput() -> some View {
    #if os(iOS)
      autocorrectionDisabled().textInputAutocapitalization(.never)
    #else
      autocorrectionDisabled()
    #endif
  }
}
