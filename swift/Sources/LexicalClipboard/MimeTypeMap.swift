/// A map from MIME type to `Value` that keeps its keys in insertion order,
/// as the JS objects keyed by MIME type in @lexical/clipboard do
/// (`LexicalClipboardData`, `ExportMimeTypeConfig`, `ImportMimeTypeConfig`,
/// `ImportMimeTypePriority`). Setting a key that is present keeps its
/// position; setting `nil` deletes it.
public struct MimeTypeMap<Value>: Sequence, ExpressibleByDictionaryLiteral {
  public private(set) var keys: [String] = []
  private var storage: [String: Value] = [:]

  public init() {}

  public init(dictionaryLiteral elements: (String, Value)...) {
    for (key, value) in elements { self[key] = value }
  }

  public subscript(key: String) -> Value? {
    get { storage[key] }
    set {
      if let newValue {
        if storage.updateValue(newValue, forKey: key) == nil { keys.append(key) }
      } else if storage.removeValue(forKey: key) != nil {
        keys.removeAll { $0 == key }
      }
    }
  }

  public var isEmpty: Bool { keys.isEmpty }

  /// The entries in insertion order (`Object.entries`).
  public func makeIterator() -> IndexingIterator<[(key: String, value: Value)]> {
    keys.map { (key: $0, value: storage[$0]!) }.makeIterator()
  }
}
