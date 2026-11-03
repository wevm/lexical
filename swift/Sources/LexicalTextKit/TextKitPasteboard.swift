import Lexical

// The pasteboard types that stand for a `DataTransfer`'s MIME types, shared
// by `NSPasteboard`, `UIPasteboard` and drag items. Lexical's own types use
// a reverse-DNS identifier, since pasteboards key their items by uniform
// type identifier.

public enum TextKitPasteboard {
  /// MIME type and pasteboard type, in the order the views write them.
  public static let types: [(mime: String, type: String)] = [
    ("application/x-lexical-editor", "dev.lexical.editor"),
    ("application/x-lexical-drag", "dev.lexical.drag"),
    ("text/html", "public.html"),
    ("text/plain", "public.utf8-plain-text"),
    ("text/uri-list", "public.url"),
  ]

  /// The pasteboard type of `mime`, or `nil` when the views don't carry it.
  public static func type(forMIME mime: String) -> String? {
    types.first { $0.mime == mime }?.type
  }

  /// The MIME type of a file with the filename extension `pathExtension`,
  /// for a `DataTransferFile` (`application/octet-stream` when unknown).
  public static func mimeType(forPathExtension pathExtension: String) -> String {
    switch pathExtension.lowercased() {
    case "png": "image/png"
    case "jpg", "jpeg": "image/jpeg"
    case "gif": "image/gif"
    case "webp": "image/webp"
    case "heic": "image/heic"
    case "tif", "tiff": "image/tiff"
    case "svg": "image/svg+xml"
    case "pdf": "application/pdf"
    case "txt": "text/plain"
    case "md": "text/markdown"
    case "html", "htm": "text/html"
    case "json": "application/json"
    case "mp4", "m4v": "video/mp4"
    case "mov": "video/quicktime"
    case "mp3": "audio/mpeg"
    default: "application/octet-stream"
    }
  }
}
