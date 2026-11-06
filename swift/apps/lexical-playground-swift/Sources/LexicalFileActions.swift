import Lexical
import LexicalFile
import Observation
import SwiftUI
import UniformTypeIdentifiers

#if os(macOS)
  import AppKit
#endif

// `ActionsPlugin`'s Import and Export buttons: `importFile` and `exportFile`
// from LexicalFile, with the file hooks a native app supplies in place of
// the web's file input and download link, the open and save panels on macOS
// and SwiftUI's file importer and exporter on iOS.

/// A `.lexical` file, by its extension. The app declares no type of its
/// own, so this is the dynamic type the system gives such a file (one that
/// conforms to `.json` would not match the files on disk).
let lexicalDocumentType = UTType(filenameExtension: "lexical") ?? .data

/// The open and save requests the views present on iOS, and the error a
/// file reports.
@MainActor
@Observable
final class LexicalFileModel {
  /// `importFile`'s callback while the file importer is up (iOS).
  @ObservationIgnored private var pendingImport: ((String) throws -> Void)?
  var isImporting = false
  /// The document `exportFile` saves, while the file exporter is up (iOS).
  var pendingExport: LexicalFileDocument?
  var isExporting: Bool {
    get { pendingExport != nil }
    set { if !newValue { pendingExport = nil } }
  }
  /// A file that could not be read or saved, as the alert shows it.
  var error: String?

  /// The Import button: `importFile(editor)`.
  func importDocument(_ editor: Editor) {
    importFile(editor) { [weak self] _, callback in
      MainActor.assumeIsolated { self?.readTextFile(callback) }
    }
  }

  /// The Export button, with the web playground's name and source.
  func exportDocument(_ editor: Editor) {
    do {
      try exportFile(
        editor, fileName: "Playground \(isoNow())", source: "Playground"
      ) { [weak self] text, fileName in
        MainActor.assumeIsolated { self?.save(text, fileName) }
      }
    } catch {
      self.error = "\(error)"
    }
  }

  /// `readTextFileFromSystem`.
  private func readTextFile(_ callback: @escaping (String) throws -> Void) {
    #if os(macOS)
      let panel = NSOpenPanel()
      panel.allowedContentTypes = [lexicalDocumentType]
      panel.allowsMultipleSelection = false
      guard panel.runModal() == .OK, let url = panel.url else { return }
      read(url, callback)
    #else
      pendingImport = callback
      isImporting = true
    #endif
  }

  /// The file importer's result (iOS).
  func finishImport(_ result: Result<URL, any Error>) {
    guard let callback = pendingImport else { return }
    pendingImport = nil
    switch result {
    case .success(let url):
      let scoped = url.startAccessingSecurityScopedResource()
      defer { if scoped { url.stopAccessingSecurityScopedResource() } }
      read(url, callback)
    case .failure(let error):
      self.error = error.localizedDescription
    }
  }

  private func read(_ url: URL, _ callback: (String) throws -> Void) {
    do {
      try callback(String(contentsOf: url, encoding: .utf8))
    } catch {
      self.error = "\(url.lastPathComponent) is not a Lexical document: \(error)"
    }
  }

  /// `exportBlob`.
  private func save(_ text: String, _ fileName: String) {
    #if os(macOS)
      let panel = NSSavePanel()
      panel.allowedContentTypes = [lexicalDocumentType]
      panel.nameFieldStringValue = fileName
      guard panel.runModal() == .OK, let url = panel.url else { return }
      do {
        try text.write(to: url, atomically: true, encoding: .utf8)
      } catch {
        self.error = error.localizedDescription
      }
    #else
      pendingExport = LexicalFileDocument(text: text, fileName: fileName)
    #endif
  }

  /// `new Date().toISOString()`.
  private func isoNow() -> String {
    let formatter = ISO8601DateFormatter()
    formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return formatter.string(from: Date())
  }
}

/// A `.lexical` file for the file exporter.
struct LexicalFileDocument: FileDocument {
  static let readableContentTypes = [lexicalDocumentType]
  var text: String
  var fileName: String

  init(text: String, fileName: String) {
    self.text = text
    self.fileName = fileName
  }

  init(configuration: ReadConfiguration) throws {
    text = String(decoding: configuration.file.regularFileContents ?? Data(), as: UTF8.self)
    fileName = configuration.file.filename ?? "document.lexical"
  }

  func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
    FileWrapper(regularFileWithContents: Data(text.utf8))
  }
}

extension View {
  /// The iOS file importer and exporter `LexicalFileModel` asks for, and
  /// the alert for a file it couldn't read or save.
  func lexicalFileDialogs(_ files: LexicalFileModel) -> some View {
    @Bindable var files = files
    return
      self
      .fileImporter(isPresented: $files.isImporting, allowedContentTypes: [lexicalDocumentType]) {
        files.finishImport($0)
      }
      .fileExporter(
        isPresented: $files.isExporting, document: files.pendingExport,
        contentType: lexicalDocumentType,
        defaultFilename: files.pendingExport?.fileName
      ) { result in
        if case .failure(let error) = result { files.error = error.localizedDescription }
      }
      .alert(
        "Couldn't open the file",
        isPresented: Binding(get: { files.error != nil }, set: { if !$0 { files.error = nil } })
      ) {
        Button("OK") { files.error = nil }
      } message: {
        Text(files.error ?? "")
      }
  }
}
