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
    /// The first item's contents as a `DataTransfer`.
    public func lexicalDataTransfer(plainTextOnly: Bool = false) -> DataTransfer {
      let dataTransfer = DataTransfer()
      guard let item = items.first else { return dataTransfer }
      for (mime, type) in TextKitPasteboard.types {
        if plainTextOnly && mime != "text/plain" { continue }
        if let value = pasteboardString(item[type]) { dataTransfer.setData(mime, value) }
      }
      if dataTransfer.getData("text/plain").isEmpty,
        let value = pasteboardString(item["public.plain-text"])
      {
        dataTransfer.setData("text/plain", value)
      }
      if plainTextOnly { return dataTransfer }
      if dataTransfer.getData("text/html").isEmpty, let rtf = item["public.rtf"] as? Data,
        let html = htmlFromRTF(rtf)
      {
        dataTransfer.setData("text/html", html)
      }
      if dataTransfer.types.isEmpty {
        for (type, ext) in [("public.png", "png"), ("public.jpeg", "jpg")] {
          if let data = (item[type] as? Data) ?? (item[type] as? UIImage).flatMap({ $0.pngData() })
          {
            dataTransfer.files.append(
              DataTransferFile(
                name: "image.\(ext)", type: TextKitPasteboard.mimeType(forPathExtension: ext),
                data: Array(data)))
            break
          }
        }
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

    /// Loads the data of drag items, calling `completion` on the main queue.
    @MainActor
    public static func load(
      _ providers: [NSItemProvider], completion: @escaping @MainActor (DataTransfer) -> Void
    ) {
      let dataTransfer = DataTransfer()
      let group = DispatchGroup()
      nonisolated(unsafe) var strings: [(String, String)] = []
      nonisolated(unsafe) var files: [DataTransferFile] = []
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
        if let type = provider.registeredTypeIdentifiers.first(where: {
          UTType($0)?.conforms(to: .image) == true
        }) {
          group.enter()
          provider.loadDataRepresentation(forTypeIdentifier: type) { data, _ in
            if let data {
              let utType = UTType(type)
              let ext = utType?.preferredFilenameExtension ?? "png"
              let file = DataTransferFile(
                name: (provider.suggestedName ?? "image") + ".\(ext)",
                type: utType?.preferredMIMEType ?? "image/png", data: Array(data))
              lock.withLock { files.append(file) }
            }
            group.leave()
          }
        }
      }
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
#endif
