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
    .commands { PlaygroundCommands() }
    #if os(macOS)
      // The web playground's Keyboard shortcuts dialog, as a floating panel.
      Window("Keyboard Shortcuts", id: "keyboard-shortcuts") { ShortcutsHelpView() }
        .windowResizability(.contentSize)
        .windowLevel(.floating)
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
    @Bindable var model = model
    // One layout that changes axis, so the editor view (and the text view
    // attached to the editor) stays the same.
    let layout = isWide ? AnyLayout(HStackLayout(spacing: 0)) : AnyLayout(VStackLayout(spacing: 0))
    VStack(spacing: 0) {
      ToolbarView(model: model)
      Divider()
      // A compact width puts the find panel above the editor, which it
      // would cover.
      if model.findReplace.isOpen && !isWide {
        FindReplacePanel(model: model.findReplace).padding(8)
      }
      layout {
        // Editor.tsx's placeholder.
        LexicalEditor(
          model.editor,
          placeholder: model.isRichText ? "Enter some rich text..." : "Enter some plain text..."
        )
        .id(model.generation)
        .accessibilityIdentifier("editor")
        .frame(maxWidth: .infinity, minHeight: isWide ? nil : 120, maxHeight: .infinity)
        .overlay(alignment: .topTrailing) {
          // `CommentPlugin`'s Show Comments button.
          ShowCommentsButton(comments: model.comments).padding(10)
        }
        .overlay(alignment: .topTrailing) {
          // A compact width has no room beside the editor.
          if model.tableOfContents.isShown && !isWide {
            TableOfContentsPanel(model: model.tableOfContents, isCompact: true)
              .background(RoundedRectangle(cornerRadius: 8).fill(.background).shadow(radius: 6))
              .padding(.top, 60).padding(.trailing, 10)
          }
        }
        .overlay(alignment: .topTrailing) {
          // `FindReplacePanel`, at the top right as on the web.
          if model.findReplace.isOpen && isWide {
            FindReplacePanel(model: model.findReplace).padding(12)
          }
        }
        .overlay(alignment: .bottomLeading) {
          // `CharacterLimitPlugin`'s counter (`.characters-limit`).
          if let remaining = model.remainingCharacters {
            Text("\(remaining)")
              .font(.system(size: 12))
              .foregroundStyle(remaining < 0 ? .red : Color(white: 0.53))
              .padding(.leading, 12).padding(.bottom, 5)
              .accessibilityIdentifier("characters-limit")
          }
        }
        // `TableOfContentsPlugin`, at the right of the page as on the web.
        if model.tableOfContents.isShown && isWide {
          TableOfContentsPanel(model: model.tableOfContents).padding(.top, 60)
            .frame(maxHeight: .infinity, alignment: .top)
        }
        if model.showsTreeView {
          Divider()
          // Below the editor, it shrinks with it when the find panel and
          // the keyboard leave little room.
          TreeView(model: model)
            .frame(width: isWide ? 440 : nil)
            .frame(minHeight: isWide ? nil : 80, maxHeight: isWide ? nil : 280)
        }
      }
      .overlay(alignment: .trailing) {
        // `CommentsPanel`, over the right of the page as on the web.
        if model.comments.showComments {
          CommentsPanel(comments: model.comments)
            .transition(.move(edge: .trailing).combined(with: .opacity))
        }
      }
      .animation(.easeOut(duration: 0.2), value: model.comments.showComments)
      Divider()
      ActionsView(model: model, sheet: $sheet)
    }
    .onChange(of: isWide, initial: true) { _, isWide in model.comments.isWide = isWide }
    .focusedSceneValue(\.playgroundModel, model)
    .sheet(isPresented: $model.showsShortcuts) {
      NavigationStack {
        ShortcutsHelpView()
          .toolbar { Button("Done") { model.showsShortcuts = false } }
      }
    }
    .sheet(isPresented: $model.isInsertingImage) { InsertImageDialog(model: model) }
    // A compact width has no room below the selection for the comment
    // input box, so it is a sheet there.
    .sheet(
      isPresented: Binding(
        get: { model.comments.showCommentInput && !isWide },
        set: { if !$0 && model.comments.showCommentInput { model.comments.cancelAddComment() } })
    ) {
      CommentInputBox(comments: model.comments)
        .padding()
        .presentationDetents([.height(200)])
    }
    .sheet(
      isPresented: Binding(
        get: { model.embedConfig != nil }, set: { if !$0 { model.embedConfig = nil } })
    ) {
      if let embedConfig = model.embedConfig {
        AutoEmbedDialog(embedConfig: embedConfig, editor: model.editor)
      }
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
  case markdown = "Markdown"
}

/// The playground's bottom bar (`ActionsPlugin`): import, export, clear,
/// lock and settings.
struct ActionsView: View {
  let model: PlaygroundModel
  @Binding var sheet: PlaygroundSheet?

  #if os(macOS)
    @Environment(\.openWindow) private var openWindow
  #endif

  var body: some View {
    @Bindable var model = model
    HStack(spacing: 16) {
      // `ActionsPlugin`'s Keyboard shortcuts button.
      Button("Keyboard shortcuts", systemImage: "keyboard") {
        #if os(macOS)
          openWindow(id: "keyboard-shortcuts")
        #else
          model.showsShortcuts = true
        #endif
      }
      .help("Show keyboard shortcuts")
      Button("JSON", systemImage: "curlybraces") { sheet = .importExport(.json) }
        .help("Import or export the editor state as JSON")
      Button("HTML", systemImage: "chevron.left.forwardslash.chevron.right") {
        sheet = .importExport(.html)
      }
      .help("Import or export the editor state as HTML")
      if model.isRichText {
        Button("Markdown", systemImage: "m.square") { sheet = .importExport(.markdown) }
          .help("Import or export the document as Markdown")
      }
      Button("Clear", systemImage: "trash") { model.clearEditor() }
        .help("Clear")
      #if os(iOS)
        // Without a hardware keyboard there is no ⌘F, which is the web's
        // only way to open the find panel.
        Button("Find and Replace", systemImage: "magnifyingglass") {
          model.findReplace.dispatch(TOGGLE_FIND_REPLACE_COMMAND)
        }
      #endif
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
