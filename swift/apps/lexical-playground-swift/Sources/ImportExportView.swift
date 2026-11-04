import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

/// Import and export as JSON, HTML or Markdown: the editor's current
/// document in an editable text field, which Import loads back into the
/// editor.
struct ImportExportView: View {
  let model: PlaygroundModel
  let format: DocumentFormat
  @Environment(\.dismiss) private var dismiss
  @State private var text = ""
  @State private var error: String?

  var body: some View {
    NavigationStack {
      VStack(alignment: .leading, spacing: 8) {
        TextEditor(text: $text)
          .font(.system(size: 12, design: .monospaced))
          .autocorrectionDisabled()
          .accessibilityIdentifier("import-export-text")
        if let error {
          Text(error).font(.caption).foregroundStyle(.red)
        }
      }
      .padding()
      .navigationTitle(format.rawValue)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button("Close") { dismiss() }
        }
        ToolbarItem(placement: .primaryAction) {
          Button("Import") { importText() }
            .disabled(!model.isEditable)
        }
        ToolbarItem {
          Button("Copy", systemImage: "doc.on.doc") { copy() }
        }
      }
    }
    #if os(macOS)
      // A macOS sheet sizes to its content; an iOS sheet has the screen's
      // width, which a minimum wider than an iPhone would overflow.
      .frame(minWidth: 560, minHeight: 480)
    #endif
    .onAppear {
      text =
        switch format {
        case .json: model.exportJSON()
        case .html: model.exportHTML()
        case .markdown: model.exportMarkdown()
        }
    }
  }

  private func importText() {
    do {
      switch format {
      case .json: try model.importJSON(text)
      case .html: try model.importHTML(text)
      case .markdown: try model.importMarkdown(text)
      }
      dismiss()
    } catch {
      self.error = "\(error)"
    }
  }

  private func copy() {
    #if os(macOS)
      NSPasteboard.general.clearContents()
      NSPasteboard.general.setString(text, forType: .string)
    #else
      UIPasteboard.general.string = text
    #endif
  }
}
