import Lexical
import LexicalSwiftUI
import SwiftUI

// The demo app: `LexicalEditor` on the demo document. Launch arguments:
// `-empty` starts from an empty paragraph; `-uitest` shows the model probe,
// the editor's text content and JSON as accessibility labels, which the UI
// tests read to check the model rather than the view.

@main
struct LexicalDemoApp: App {
  var body: some Scene {
    WindowGroup {
      DemoView()
    }
  }
}

@MainActor
@Observable
final class DemoModel {
  let editor: Editor
  private(set) var text = ""
  private(set) var json = ""
  /// The last commands the views dispatched, newest last.
  private(set) var log: [String] = []
  @ObservationIgnored private var removeListeners: [() -> Void] = []

  init() {
    let arguments = ProcessInfo.processInfo.arguments
    // A demo that cannot build its editor has nothing to show.
    editor = try! makeDemoEditor()
    if !arguments.contains("-empty") {
      try? loadDemoDocument(editor)
    }
    if arguments.contains("-uitest") {
      removeListeners = [
        editor.registerUpdateListener { [weak self] payload in
          let state = payload.editorState
          MainActor.assumeIsolated { self?.probe(state) }
        },
        editor.registerCommand(
          KEY_DOWN_COMMAND,
          { [weak self] event, _ in
            self?.record("keydown \(event.metaKey ? "meta+" : "")\(event.key)")
            return false
          }, COMMAND_PRIORITY_CRITICAL),
        editor.registerCommand(
          BEFORE_INPUT_COMMAND,
          { [weak self] event, _ in
            self?.record(
              "beforeinput \(event.inputType) \(event.data.map { "\($0.debugDescription)" } ?? "")")
            return false
          }, COMMAND_PRIORITY_CRITICAL),
        editor.registerCommand(
          PASTE_COMMAND,
          { [weak self] _, _ in
            self?.record("paste"); return false
          },
          COMMAND_PRIORITY_CRITICAL),
        editor.registerCommand(
          UNDO_COMMAND,
          { [weak self] _, _ in
            self?.record("undo"); return false
          },
          COMMAND_PRIORITY_CRITICAL),
      ]
      probe(editor.getEditorState())
    }
  }

  private nonisolated func record(_ entry: String) {
    MainActor.assumeIsolated {
      log = Array((log + [entry]).suffix(12))
    }
  }

  private func probe(_ state: EditorState) {
    text = (try? state.read { try getRoot().getTextContent() }) ?? ""
    json = (try? state.toJSON(true).stringify()) ?? ""
  }
}

struct DemoView: View {
  @State private var model = DemoModel()
  private let showsProbe = ProcessInfo.processInfo.arguments.contains("-uitest")

  var body: some View {
    VStack(spacing: 0) {
      LexicalEditor(model.editor)
        .accessibilityIdentifier("editor")
      if showsProbe {
        VStack(alignment: .leading) {
          Text("model").accessibilityIdentifier("model-text").accessibilityLabel(model.text)
          Text("json").accessibilityIdentifier("model-json").accessibilityLabel(model.json)
          Text("log").accessibilityIdentifier("model-log")
            .accessibilityLabel(model.log.joined(separator: "; "))
        }
        .font(.caption2)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(4)
      }
    }
  }
}
