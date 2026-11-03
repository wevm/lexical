#if os(macOS)
  import AppKit
  import Lexical
  import LexicalTextKit
  import UniformTypeIdentifiers

  // `NSPasteboard` as a `DataTransfer`: Lexical's JSON, HTML, plain text,
  // URLs and files, by the types of `TextKitPasteboard`.

  extension NSPasteboard {
    /// The pasteboard's contents as a `DataTransfer`. Rich text without
    /// HTML (RTF, from native apps) is converted to HTML, which the
    /// rich-text importer reads, styles included.
    @MainActor
    public func lexicalDataTransfer(plainTextOnly: Bool = false) -> DataTransfer {
      let dataTransfer = DataTransfer()
      for (mime, type) in TextKitPasteboard.types {
        if plainTextOnly && mime != "text/plain" { continue }
        if let string = string(forType: NSPasteboard.PasteboardType(type)) {
          dataTransfer.setData(mime, string)
        }
      }
      if plainTextOnly { return dataTransfer }
      if dataTransfer.getData("text/html").isEmpty, let html = htmlFromRichText() {
        dataTransfer.setData("text/html", html)
      }
      let urls =
        readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true])
        as? [URL] ?? []
      for url in urls {
        guard let data = try? Data(contentsOf: url) else { continue }
        let type =
          UTType(filenameExtension: url.pathExtension)?.preferredMIMEType
          ?? TextKitPasteboard.mimeType(forPathExtension: url.pathExtension)
        dataTransfer.files.append(
          DataTransferFile(name: url.lastPathComponent, type: type, data: Array(data)))
      }
      if dataTransfer.files.isEmpty, dataTransfer.types.isEmpty {
        // An image copied from an app, with no file behind it.
        for (type, ext) in [(NSPasteboard.PasteboardType.png, "png"), (.tiff, "tiff")] {
          if let data = data(forType: type) {
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

    private func htmlFromRichText() -> String? {
      for type in [NSPasteboard.PasteboardType.rtf, .rtfd] {
        guard let data = data(forType: type),
          let attributed = NSAttributedString(rtf: data, documentAttributes: nil)
            ?? NSAttributedString(rtfd: data, documentAttributes: nil),
          let html = try? attributed.data(
            from: NSRange(location: 0, length: attributed.length),
            documentAttributes: [.documentType: NSAttributedString.DocumentType.html])
        else { continue }
        return String(decoding: html, as: UTF8.self)
      }
      return nil
    }

    /// Replaces the pasteboard's contents with `dataTransfer`'s.
    public func write(_ dataTransfer: DataTransfer) {
      clearContents()
      _ = writeObjects([dataTransfer.pasteboardItem()])
    }
  }

  extension DataTransfer {
    /// A pasteboard item with this transfer's data, for the clipboard or a
    /// drag.
    public func pasteboardItem() -> NSPasteboardItem {
      let item = NSPasteboardItem()
      for (mime, type) in TextKitPasteboard.types {
        let value = getData(mime)
        if !value.isEmpty { item.setString(value, forType: NSPasteboard.PasteboardType(type)) }
      }
      return item
    }
  }
#endif
