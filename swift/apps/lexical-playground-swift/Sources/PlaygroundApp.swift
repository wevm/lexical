import LexicalSwiftUI
import SwiftUI

// The Swift playground, laid out like packages/lexical-playground: the
// toolbar above the editor, the tree view (the "debug view") below it, and
// the actions (import, export, clear, lock, settings) at the bottom. In a
// regular-width window the tree view sits beside the editor.

@main
struct LexicalPlaygroundApp: App {
  var body: some Scene {
    WindowGroup("Lexical Playground") {
      PlaygroundView()
    }
    #if os(macOS)
      .defaultSize(width: 1100, height: 760)
    #endif
  }
}

struct PlaygroundView: View {
  @State private var model = PlaygroundModel()
  @State private var sheet: PlaygroundSheet?
  #if os(iOS)
    @Environment(\.horizontalSizeClass) private var sizeClass
    private var isWide: Bool { sizeClass == .regular }
  #else
    private let isWide = true
  #endif

  var body: some View {
    // One layout that changes axis, so the editor view (and the text view
    // attached to the editor) stays the same.
    let layout = isWide ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))
    VStack(spacing: 0) {
      ToolbarView(model: model)
      Divider()
      layout {
        LexicalEditor(model.editor)
          .id(model.generation)
          .accessibilityIdentifier("editor")
          .frame(maxWidth: .infinity, maxHeight: .infinity)
        if model.showsTreeView {
          Divider()
          TreeView(model: model)
            .frame(width: isWide ? 440 : nil, height: isWide ? nil : 280)
        }
      }
      Divider()
      ActionsView(model: model, sheet: $sheet)
    }
    .sheet(item: $sheet) { sheet in
      switch sheet {
      case .settings: SettingsView(model: model)
      case .importExport(let format): ImportExportView(model: model, format: format)
      }
    }
  }
}

enum PlaygroundSheet: Identifiable {
  case settings
  case importExport(DocumentFormat)

  var id: String {
    switch self {
    case .settings: "settings"
    case .importExport(let format): format.rawValue
    }
  }
}

enum DocumentFormat: String {
  case json = "JSON"
  case html = "HTML"
}

/// The playground's bottom bar (`ActionsPlugin`): import, export, clear,
/// lock and settings.
struct ActionsView: View {
  let model: PlaygroundModel
  @Binding var sheet: PlaygroundSheet?

  var body: some View {
    @Bindable var model = model
    HStack(spacing: 16) {
      Button("JSON", systemImage: "curlybraces") { sheet = .importExport(.json) }
        .help("Import or export the editor state as JSON")
      Button("HTML", systemImage: "chevron.left.forwardslash.chevron.right") {
        sheet = .importExport(.html)
      }
      .help("Import or export the editor state as HTML")
      Button("Clear", systemImage: "trash") { model.clearEditor() }
        .help("Clear")
      Toggle(isOn: $model.isEditable) {
        Label(
          model.isEditable ? "Unlocked" : "Read-Only",
          systemImage: model.isEditable ? "lock.open" : "lock")
      }
      .toggleStyle(.button)
      .help(model.isEditable ? "Make read-only" : "Make editable")
      Spacer()
      Button("Settings", systemImage: "gearshape") { sheet = .settings }
    }
    .labelStyle(.iconOnly)
    .buttonStyle(.borderless)
    .padding(.horizontal, 12)
    .padding(.vertical, 8)
  }
}
