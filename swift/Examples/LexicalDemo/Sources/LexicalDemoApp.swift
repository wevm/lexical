import Lexical
import LexicalSwiftUI
import SwiftUI

#if canImport(UIKit)
  import LexicalUIKit
  import UIKit
#endif

// The demo app: `LexicalEditor` on the demo document. Launch arguments:
// `-empty` starts from an empty paragraph; `-text <text>`, repeated,
// from one paragraph per argument; `-uitest` shows the model probe,
// the editor's text content and JSON as accessibility labels, which the UI
// tests read to check the model rather than the view; `-benchmark` times
// keystrokes on a 5,000-paragraph document (Benchmark.swift) instead;
// `-dark` shows the dark appearance whatever the system's is.

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
  /// The text content of the model's selection, and its focus: the text
  /// of the focus node and the offset in it.
  private(set) var selection = ""
  /// The last commands the views dispatched, newest last.
  private(set) var log: [String] = []
  @ObservationIgnored private var removeListeners: [() -> Void] = []

  init() {
    let arguments = ProcessInfo.processInfo.arguments
    // A demo that cannot build its editor has nothing to show.
    editor = try! makeDemoEditor()
    let lines = arguments.indices.dropLast().filter { arguments[$0] == "-text" }.map {
      arguments[$0 + 1]
    }
    if !lines.isEmpty {
      try? editor.update(discrete: true) {
        let root = try getRoot().clear()
        for line in lines {
          try root.append(createParagraphNode().append(createTextNode(line)))
        }
        _ = try root.selectEnd()
      }
    } else if !arguments.contains("-empty") {
      try? loadDemoDocument(editor)
    }
    if arguments.contains("-uitest"), !arguments.contains("-benchmark") {
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
          CLICK_COMMAND,
          { [weak self] event, _ in
            self?.record("click \(event.detail)")
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

  /// On iOS, what the view shows: its appearance and content size
  /// category, the heading's and the body text's font sizes, and the
  /// decorators' accessibility labels.
  func viewProbe() -> String {
    #if canImport(UIKit)
      guard let view = editor.getRootElement() as? LexicalUITextView else { return "" }
      let text = view.text as NSString
      func size(_ string: String) -> String {
        let range = text.range(of: string)
        guard range.location != NSNotFound,
          let font = view.textStorage.attribute(.font, at: range.location, effectiveRange: nil)
            as? UIFont
        else { return "-" }
        return String(format: "%.1f", font.pointSize)
      }
      var labels: [String] = []
      view.textStorage.enumerateAttribute(
        .attachment, in: NSRange(location: 0, length: view.textStorage.length)
      ) { value, _, _ in
        if let attachment = value as? NSTextAttachment {
          labels.append(attachment.accessibilityLabel ?? "nil")
        }
      }
      let traits = view.traitCollection
      return [
        "dark \(traits.userInterfaceStyle == .dark)",
        "category \(traits.preferredContentSizeCategory.rawValue)",
        "heading \(size("Lexical for Swift"))", "body \(size("Native editing"))",
        "decorators \(labels.joined(separator: " | "))",
      ].joined(separator: "; ")
    #else
      return ""
    #endif
  }

  private func probe(_ state: EditorState) {
    text = (try? state.read { try getRoot().getTextContent() }) ?? ""
    json = (try? state.toJSON(true).stringify()) ?? ""
    selection =
      (try? state.read { () -> String in
        guard let selection = try getSelection() as? RangeSelection else { return "" }
        let focus = selection.focus
        return try
          "\(selection.getTextContent())|focus \(focus.getNode().getTextContent())@\(focus.offset)"
      }) ?? ""
  }
}

struct DemoView: View {
  @State private var model = DemoModel()
  private let showsProbe = ProcessInfo.processInfo.arguments.contains("-uitest")
  private let isBenchmark = ProcessInfo.processInfo.arguments.contains("-benchmark")
  @State private var benchmark = "running"
  @State private var viewProbe = ""
  private let isDark = ProcessInfo.processInfo.arguments.contains("-dark")

  var body: some View {
    VStack(spacing: 0) {
      LexicalEditor(model.editor)
        .accessibilityIdentifier("editor")
      if showsProbe {
        VStack(alignment: .leading) {
          Text("model").accessibilityIdentifier("model-text").accessibilityLabel(model.text)
          Text("json").accessibilityIdentifier("model-json").accessibilityLabel(model.json)
          Text("selection").accessibilityIdentifier("model-selection")
            .accessibilityLabel(model.selection)
          Text("log").accessibilityIdentifier("model-log")
            .accessibilityLabel(model.log.joined(separator: "; "))
          Text("view").accessibilityIdentifier("view-probe").accessibilityLabel(viewProbe)
          if isBenchmark {
            Text(benchmark).accessibilityIdentifier("benchmark")
          }
        }
        .font(.caption2)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(4)
      }
    }
    .preferredColorScheme(isDark ? .dark : nil)
    .task {
      while showsProbe, !isBenchmark, !Task.isCancelled {
        viewProbe = model.viewProbe()
        try? await Task.sleep(for: .milliseconds(300))
      }
    }
    .task {
      guard isBenchmark else { return }
      // After the view is attached as the editor's root.
      try? await Task.sleep(for: .seconds(1))
      benchmark = (try? runKeystrokeBenchmark(model.editor)) ?? "failed"
    }
  }
}
