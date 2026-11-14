#if canImport(UIKit)
  import Lexical
  import LexicalTextKit
  import UIKit
  import UniformTypeIdentifiers

  // `UIPasteboard` and drag items as a `DataTransfer`: Lexical's JSON,
  // HTML, plain text, URLs and images, by the types of `TextKitPasteboard`.

  /// A string from a pasteboard value, which may come as text or data.
  private func pasteboardString(_ value: Any?) -> String? {
    switch value {
    case let string as String: string
    case let data as Data: String(data: data, encoding: .utf8)
    case let url as URL: url.absoluteString
    case let attributed as NSAttributedString: attributed.string
    default: nil
    }
  }

  /// HTML for rich text from a native app (RTF), which the rich-text
  /// importer reads, styles included.
  func htmlFromRTF(_ data: Data) -> String? {
    guard
      let attributed = try? NSAttributedString(
        data: data, options: [.documentType: NSAttributedString.DocumentType.rtf],
        documentAttributes: nil),
      let html = try? attributed.data(
        from: NSRange(location: 0, length: attributed.length),
        documentAttributes: [.documentType: NSAttributedString.DocumentType.html])
    else { return nil }
    return String(decoding: html, as: UTF8.self)
  }

  extension UIPasteboard {
    /// The first item's contents as a `DataTransfer`, reading only the
    /// types it carries. When it has no text, the items' images and other
    /// files are its `files`, which start loading off the main thread now
    /// (UIKit lets a paste's items load only while it happens) and which
    /// `bytes()` waits for.
    public func lexicalDataTransfer(plainTextOnly: Bool = false) -> DataTransfer {
      let dataTransfer = DataTransfer()
      guard numberOfItems > 0 else { return dataTransfer }
      let first = IndexSet(integer: 0)
      func value(_ type: String) -> Any? {
        values(forPasteboardType: type, inItemSet: first)?.first
      }
      for (mime, type) in TextKitPasteboard.types {
        if plainTextOnly && mime != "text/plain" { continue }
        if let value = pasteboardString(value(type)) { dataTransfer.setData(mime, value) }
      }
      if dataTransfer.getData("text/plain").isEmpty,
        let value = pasteboardString(value("public.plain-text"))
      {
        dataTransfer.setData("text/plain", value)
      }
      if plainTextOnly { return dataTransfer }
      if dataTransfer.getData("text/html").isEmpty, let rtf = value("public.rtf") as? Data,
        let html = htmlFromRTF(rtf)
      {
        dataTransfer.setData("text/html", html)
      }
      if dataTransfer.types.isEmpty {
        dataTransfer.files = itemProviders.compactMap(DataTransferFile.loading)
      }
      return dataTransfer
    }

    /// Replaces the pasteboard's contents with `dataTransfer`'s.
    public func write(_ dataTransfer: DataTransfer) {
      setItems([dataTransfer.pasteboardItem()])
    }
  }

  extension DataTransfer {
    /// The pasteboard item of this transfer's data.
    public func pasteboardItem() -> [String: Any] {
      var item: [String: Any] = [:]
      for (mime, type) in TextKitPasteboard.types {
        let value = getData(mime)
        if !value.isEmpty { item[type] = value }
      }
      return item
    }

    /// An item provider with this transfer's data, for a drag.
    public func itemProvider() -> NSItemProvider {
      let provider = NSItemProvider()
      for (mime, type) in TextKitPasteboard.types {
        let value = getData(mime)
        if value.isEmpty { continue }
        provider.registerDataRepresentation(forTypeIdentifier: type, visibility: .all) {
          completion in
          completion(Data(value.utf8), nil)
          return nil
        }
      }
      return provider
    }

    /// Loads the text of drag items, calling `completion` on the main queue.
    /// The items' images and other files, such as a PDF from Files, are its
    /// `files`: they start loading off the main thread now, since UIKit lets
    /// a drop's items load only while it is performed, and `completion`
    /// doesn't wait for them; `bytes()` does.
    @MainActor
    public static func load(
      _ providers: [NSItemProvider], completion: @escaping @MainActor (DataTransfer) -> Void
    ) {
      let dataTransfer = DataTransfer()
      let group = DispatchGroup()
      nonisolated(unsafe) var strings: [(String, String)] = []
      let lock = NSLock()
      for provider in providers {
        for (mime, type) in TextKitPasteboard.types
        where provider.hasItemConformingToTypeIdentifier(type) {
          group.enter()
          provider.loadDataRepresentation(forTypeIdentifier: type) { data, _ in
            if let data, let value = String(data: data, encoding: .utf8) {
              lock.withLock { strings.append((mime, value)) }
            }
            group.leave()
          }
        }
      }
      // Started here: UIKit lets a drop's items load only while it is
      // performed.
      let files = providers.compactMap(DataTransferFile.loading)
      group.notify(queue: .main) {
        MainActor.assumeIsolated {
          for (mime, value) in strings where dataTransfer.getData(mime).isEmpty {
            dataTransfer.setData(mime, value)
          }
          dataTransfer.files = files
          completion(dataTransfer)
        }
      }
    }
  }

  extension DataTransferFile {
    /// The file an item provider carries: an image, or an item with none
    /// of `TextKitPasteboard`'s types, such as a PDF from Files; `nil` for
    /// an item of text. UIKit lets a drop's or a paste's items load only
    /// while it happens, so the file starts loading now, off the main
    /// thread, into a temporary file that `bytes()` waits for and reads.
    /// `size` is `nil`, since it is known only once the file has loaded.
    static func loading(_ provider: NSItemProvider) -> DataTransferFile? {
      let types = provider.registeredTypeIdentifiers
      let isText = TextKitPasteboard.types.contains { types.contains($0.type) }
      guard
        let type = types.first(where: { UTType($0)?.conforms(to: .image) == true })
          ?? (isText ? nil : types.first { UTType($0)?.conforms(to: .data) == true })
      else { return nil }
      let utType = UTType(type)
      var name =
        provider.suggestedName ?? (utType?.conforms(to: .image) == true ? "image" : "file")
      if let ext = utType?.preferredFilenameExtension,
        UTType(filenameExtension: (name as NSString).pathExtension) != utType
      {
        name += ".\(ext)"
      }
      let file = ItemProviderFile(provider, type)
      return DataTransferFile(
        name: name,
        type: utType?.preferredMIMEType
          ?? TextKitPasteboard.mimeType(forPathExtension: (name as NSString).pathExtension),
        size: nil
      ) { try await file.bytes() }
    }
  }

  /// An item provider's file, moved into a temporary directory as it
  /// loads, and removed once no `DataTransferFile` holds it.
  private final class ItemProviderFile: Sendable {
    private let location: Task<URL, any Error>

    init(_ provider: NSItemProvider, _ type: String) {
      let (loaded, continuation) = AsyncThrowingStream<URL, any Error>.makeStream()
      // The provider removes its file when this returns, so it is moved.
      _ = provider.loadFileRepresentation(forTypeIdentifier: type) { url, error in
        do {
          guard let url else { throw error ?? CocoaError(.fileReadUnknown) }
          let directory = FileManager.default.temporaryDirectory
            .appending(path: "lexical-\(UUID().uuidString)", directoryHint: .isDirectory)
          try FileManager.default.createDirectory(
            at: directory, withIntermediateDirectories: true)
          let destination = directory.appending(path: url.lastPathComponent)
          try FileManager.default.moveItem(at: url, to: destination)
          continuation.yield(destination)
          continuation.finish()
        } catch {
          continuation.finish(throwing: error)
        }
      }
      location = Task.detached {
        for try await url in loaded { return url }
        throw CocoaError(.fileReadUnknown)
      }
    }

    deinit {
      let location = location
      Task.detached {
        if let url = try? await location.value {
          try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
        }
      }
    }

    /// The file's contents, once it has loaded, read off the main thread.
    @concurrent
    func bytes() async throws -> [UInt8] {
      Array(try Data(contentsOf: try await location.value))
    }
  }
#endif
