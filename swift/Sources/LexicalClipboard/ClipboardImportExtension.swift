import Lexical
import LexicalExtension
import LexicalHTML

// Port of packages/lexical-clipboard/src/ClipboardImportExtension.ts
// (`@experimental` upstream).

/// The `$next` of an `ImportMimeTypeFunction`: defers to the handler
/// registered before it. Omitted arguments default to the caller's, so
/// `next()` is `next(data, selection)`. Returns whether that handler
/// claimed the data.
public struct ImportNext {
  let call: (String?, (any BaseSelection)?) throws -> Bool

  public func callAsFunction(
    _ nextData: String? = nil, _ nextSelection: (any BaseSelection)? = nil
  ) throws -> Bool {
    try call(nextData, nextSelection)
  }
}

/// `ImportMimeTypeFunction`: a handler in a per-MIME-type import stack.
/// It gets the (non-empty) data, the insertion selection, `next`, and the
/// whole `DataTransfer`; it returns `true` when it consumed the data.
public typealias ImportMimeTypeFunction =
  (
    _ data: String, _ selection: any BaseSelection, _ next: ImportNext,
    _ dataTransfer: DataTransfer
  ) throws -> Bool

/// `ImportMimeTypeConfig`: a stack of `ImportMimeTypeFunction` per MIME
/// type; the last function runs first.
public typealias ImportMimeTypeConfig = MimeTypeMap<[ImportMimeTypeFunction]>

/// `ImportMimeTypePriority`: per-MIME-type ordering weights, lower first.
/// Types without a weight run after the weighted ones, in lexical order.
public typealias ImportMimeTypePriority = MimeTypeMap<Double>

/// `ClipboardImportConfig`.
public struct ClipboardImportConfig {
  /// `$importMimeType`: merged with `[...prev, ...override]` per MIME type.
  public var importMimeType: ImportMimeTypeConfig
  /// Spread-merged across configs.
  public var priority: ImportMimeTypePriority

  public init(
    importMimeType: ImportMimeTypeConfig = DEFAULT_IMPORT_MIME_TYPE,
    priority: ImportMimeTypePriority = DEFAULT_IMPORT_MIME_TYPE_PRIORITY
  ) {
    self.importMimeType = importMimeType
    self.priority = priority
  }
}

/// `DEFAULT_IMPORT_MIME_TYPE_PRIORITY`: `application/x-lexical-editor` (0),
/// `text/html` (10), `text/plain` (20), `text/uri-list` (30).
nonisolated(unsafe) public let DEFAULT_IMPORT_MIME_TYPE_PRIORITY: ImportMimeTypePriority = [
  "application/x-lexical-editor": 0,
  "text/html": 10,
  "text/plain": 20,
  "text/uri-list": 30,
]

// JS passes the HTML through a Trusted Types policy (`trustHTML`) first;
// there is no Trusted Types outside a browser, so the string is parsed as
// is.

/// Default handler for `application/x-lexical-editor`: parse the JSON,
/// check the namespace, and insert the serialized nodes.
private func defaultLexicalEditorImporter(
  _ data: String, _ selection: any BaseSelection, _ next: ImportNext, _: DataTransfer
) throws -> Bool {
  do {
    let editor = try getEditor()
    if case .object(let payload) = try JSONValue.parse(data),
      payload["namespace"]?.stringValue == editor._config.namespace,
      case .array(let serializedNodes)? = payload["nodes"]
    {
      let nodes = try generateNodesFromSerializedNodes(
        serializedNodes.map { value in
          guard case .object(let node) = value else {
            throw LexicalError("parseEditorState: a node is not a serialized node")
          }
          return node
        })
      try insertGeneratedNodes(editor, nodes, selection)
      return true
    }
  } catch {
    consoleWarn("\(error)")
  }
  return try next()
}

/// Default handler for `text/html`: parse the HTML and run the legacy
/// `generateNodesFromDOM`.
private func defaultHtmlImporter(
  _ data: String, _ selection: any BaseSelection, _ next: ImportNext, _: DataTransfer
) throws -> Bool {
  do {
    let editor = try getEditor()
    let dom = DOMParser().parseFromString(data, "text/html")
    let nodes = try generateNodesFromDOM(editor, dom)
    try insertGeneratedNodes(editor, nodes, selection)
    return true
  } catch {
    consoleWarn("\(error)")
    return try next()
  }
}

/// Default handler for `text/plain`: on a RangeSelection each `\n` becomes
/// a paragraph break and each tab a TabNode; other selections use
/// `insertRawText`.
private func defaultPlainTextImporter(
  _ data: String, _ selection: any BaseSelection, _: ImportNext, _: DataTransfer
) throws -> Bool {
  guard selection is RangeSelection else {
    try selection.insertRawText(data)
    return true
  }
  // Each insertion reports the caret position that follows it by writing
  // to the editor's selection, so re-read `getSelection()` between tokens.
  // Honoring a caller-supplied selection (#6278) means promoting it to the
  // editor's selection first.
  if try getSelection() !== selection {
    try setSelection(selection)
  }
  func withCurrentRange(_ fn: (RangeSelection) throws -> Void) throws {
    if let cur = try getSelection() as? RangeSelection {
      try fn(cur)
    }
  }
  try tokenizeRawText(
    data,
    RawTextVisitor(
      linebreak: { try withCurrentRange { _ = try $0.insertParagraph() } },
      tab: { try withCurrentRange { try $0.insertNodes([createTabNode()]) } },
      text: { part in try withCurrentRange { try $0.insertText(part) } }))
  return true
}

/// `DEFAULT_IMPORT_MIME_TYPE`: the handler stacks reproducing the legacy
/// `insertDataTransferForRichText` behavior.
nonisolated(unsafe) public let DEFAULT_IMPORT_MIME_TYPE: ImportMimeTypeConfig = [
  "application/x-lexical-editor": [defaultLexicalEditorImporter],
  "text/html": [defaultHtmlImporter],
  "text/plain": [defaultPlainTextImporter],
  // `text/uri-list` drops behave like text.
  "text/uri-list": [defaultPlainTextImporter],
]

/// `ClipboardImportOutput`: the merged configuration, and
/// `insertDataTransfer`, which tries each MIME type in priority order.
public struct ClipboardImportOutput {
  public let importMimeType: ImportMimeTypeConfig
  public let priority: ImportMimeTypePriority

  /// `$insertDataTransfer`: invokes the stack of each MIME type present in
  /// `dataTransfer`, in `priority` order, until one claims the data.
  /// Returns whether one did.
  @discardableResult
  public func insertDataTransfer(
    _ dataTransfer: DataTransfer, _ selection: any BaseSelection
  ) throws -> Bool {
    try runImport(
      ClipboardImportConfig(importMimeType: importMimeType, priority: priority), dataTransfer,
      selection)
  }
}

private func callImportMimeTypeFunctionStack(
  _ fns: [ImportMimeTypeFunction]?, _ data: String, _ selection: any BaseSelection,
  _ dataTransfer: DataTransfer
) throws -> Bool {
  guard let fns else {
    return false
  }
  func callAt(
    _ i: Int, _ currentData: String, _ currentSelection: any BaseSelection
  ) throws
    -> Bool
  {
    guard fns.indices.contains(i) else { return false }
    return try fns[i](
      currentData, currentSelection,
      ImportNext { nextData, nextSelection in
        try callAt(i - 1, nextData ?? currentData, nextSelection ?? currentSelection)
      }, dataTransfer)
  }
  return try callAt(fns.count - 1, data, selection)
}

/// `orderedMimeTypes`: the MIME types with a stack, by weight (a stable
/// sort, as JS's); unweighted types last, in lexical order.
private func orderedMimeTypes(_ config: ClipboardImportConfig) -> [String] {
  let mimes = config.importMimeType.keys
  return mimes.enumerated().sorted { lhs, rhs in
    let (a, b) = (lhs.element, rhs.element)
    let order: Int
    switch (config.priority[a], config.priority[b]) {
    case (nil, nil): order = a < b ? -1 : a > b ? 1 : 0
    case (nil, _): order = 1
    case (_, nil): order = -1
    case (let wa?, let wb?): order = wa < wb ? -1 : wa > wb ? 1 : 0
    }
    return order != 0 ? order < 0 : lhs.offset < rhs.offset
  }.map(\.element)
}

private func runImport(
  _ config: ClipboardImportConfig, _ dataTransfer: DataTransfer, _ selection: any BaseSelection
) throws -> Bool {
  // Read once for the iOS Safari heuristic that skips text/html when it
  // matches text/plain verbatim (autocorrect produces such a payload).
  let plainString = dataTransfer.getData("text/plain")
  for mime in orderedMimeTypes(config) {
    let data = dataTransfer.getData(mime)
    if data.isEmpty {
      continue
    }
    if mime == "text/html" && data == plainString {
      continue
    }
    if try callImportMimeTypeFunctionStack(
      config.importMimeType[mime], data, selection, dataTransfer)
    {
      return true
    }
  }
  return false
}

nonisolated(unsafe) private let DEFAULT_OUTPUT = ClipboardImportOutput(
  importMimeType: DEFAULT_IMPORT_MIME_TYPE, priority: DEFAULT_IMPORT_MIME_TYPE_PRIORITY)

/// `$getImportOutput` (`@internal`): the active editor's
/// `ClipboardImportExtension` output, or the defaults without it.
public func getImportOutput() throws -> ClipboardImportOutput {
  let dep = try getPeerDependency(
    ClipboardImportExtension.name, as: type(of: ClipboardImportExtension))
  return dep?.output ?? DEFAULT_OUTPUT
}

/// `ClipboardImportExtension` (`@experimental`): the import direction of
/// `GetClipboardDataExtension`, a per-MIME-type stack of
/// `ImportMimeTypeFunction`s with priority weights.
public let ClipboardImportExtension = defineExtension(
  name: "@lexical/clipboard/Import",
  config: safeCast(ClipboardImportConfig()),
  mergeConfig: { config, partial in
    var merged = shallowMergeConfig(config, partial)
    if let partialMimeType = partial.importMimeType {
      var importMimeType = config.importMimeType
      for (k, v) in partialMimeType {
        importMimeType[k] = (importMimeType[k] ?? []) + v
      }
      merged.importMimeType = importMimeType
    }
    if let partialPriority = partial.priority {
      // Spread-merge weights: a key in `partial` overrides the same key,
      // and new keys are added.
      var priority = config.priority
      for (k, v) in partialPriority {
        priority[k] = v
      }
      merged.priority = priority
    }
    return merged
  },
  build: { _, config, _ in
    ClipboardImportOutput(importMimeType: config.importMimeType, priority: config.priority)
  })

/// `ClipboardDOMImportExtension` (`@experimental`): routes `text/html`
/// pastes and drops through `DOMImportExtension`, with the `paste` source
/// kind and the `DataTransfer` in the import context.
public let ClipboardDOMImportExtension = defineExtension(
  name: "@lexical/clipboard/DOMImport",
  dependencies: [
    CoreImportExtension,
    configExtension(ClipboardImportExtension) {
      $0.importMimeType = [
        "text/html": [
          { html, selection, _, dataTransfer in
            let dom = DOMParser().parseFromString(html, "text/html")
            let nodes = try generateNodesFromDOMViaExtension(
              dom,
              GenerateNodesFromDOMOptions(context: [
                contextValue(ImportSource, .paste),
                contextValue(ImportSourceDataTransfer, dataTransfer),
              ]))
            try insertGeneratedNodes(getEditor(), nodes, selection)
            return true
          }
        ]
      ]
    },
  ])
