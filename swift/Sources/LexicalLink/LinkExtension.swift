import Lexical
import LexicalExtension
import LexicalHTML

// Port of packages/lexical-link/src/LexicalLinkExtension.ts.

/// `LinkConfig`.
public struct LinkConfig {
  /// When set, a paste of a URL over a non-collapsed selection links the
  /// selection where `validateUrl(url)` holds, and `TOGGLE_LINK_COMMAND`
  /// rejects URLs it rejects. When `nil`, every URL is accepted and paste
  /// is left alone.
  public var validateUrl: ((String) -> Bool)?
  /// The default anchor attributes `TOGGLE_LINK_COMMAND` uses.
  public var attributes: LinkAttributes?

  public init(validateUrl: ((String) -> Bool)? = nil, attributes: LinkAttributes? = nil) {
    self.validateUrl = validateUrl
    self.attributes = attributes
  }
}

/// `registerLink(editor, stores)`: the link transform, `TOGGLE_LINK_COMMAND`,
/// and, while `validateUrl` is set, linking the selection on a URL paste.
public func registerLink(_ editor: Editor, _ stores: NamedSignals<LinkConfig>) throws -> () -> Void
{
  try mergeRegister(
    editor.registerNodeTransform(LinkNode.self) { try linkNodeTransform($0) },
    editor.registerCommand(
      TOGGLE_LINK_COMMAND,
      { payload, _ in
        let validateUrl = stores.validateUrl.peek()
        let attributes = stores.attributes.peek() ?? LinkAttributes()
        switch payload {
        case nil:
          try toggleLink(nil)
          return true
        case .url(let url)?:
          if validateUrl == nil || validateUrl!(url) {
            try toggleLink(url, attributes)
            return true
          }
          return false
        case .attributes(let url, let payloadAttributes)?:
          // Only the attributes the payload carries override the configured
          // defaults; validateUrl gates both payload shapes.
          if let validateUrl, !validateUrl(url) {
            return false
          }
          try toggleLink(url, attributes.merging(payloadAttributes))
          return true
        }
      }, COMMAND_PRIORITY_EDITOR),
    effect { () -> (() -> Void)? in
      guard let validateUrl = stores.validateUrl.value else { return nil }
      let attributes = stores.attributes.value ?? LinkAttributes()
      return editor.registerCommand(
        PASTE_COMMAND,
        { event, _ in
          guard let selection = try getSelection() as? RangeSelection, !selection.isCollapsed(),
            case .clipboard(let clipboardEvent) = event,
            let clipboardData = clipboardEvent.clipboardData
          else {
            return false
          }
          let clipboardText = clipboardData.getData("text")
          if !validateUrl(clipboardText) {
            return false
          }
          // Skip link wrapping for non-simple text nodes (e.g. code blocks).
          let nodes = try selection.getNodes()
          if try !nodes.contains(where: {
            try $0 is ElementNode || (($0 as? TextNode).map { try !$0.isSimpleText() } ?? false)
          }) {
            _ = try editor.dispatchCommand(
              TOGGLE_LINK_COMMAND, .attributes(url: clipboardText, attributes))
            clipboardEvent.preventDefault()
            return true
          }
          return false
        }, COMMAND_PRIORITY_LOW)
    })
}

/// `LinkExtension`: registers `LinkNode`, its transform, the
/// `TOGGLE_LINK_COMMAND` handler and `LinkImportRules`.
public let LinkExtension = defineExtension(
  name: "@lexical/link/Link", config: LinkConfig(),
  dependencies: [
    // DOMImportExtension support for the nodes registered here. Inert
    // unless the editor imports HTML through the DOMImportExtension
    // pipeline.
    CoreImportExtension,
    configExtension(DOMImportExtension) { $0.rules = LinkImportRules.map(\.entry) },
  ],
  mergeConfig: { config, partial in
    var merged = partial.applied(to: config)
    // Configured attributes merge with the overriding ones.
    if let base = config.attributes, partial.has(\LinkConfig.attributes) {
      merged.attributes = base.merging(merged.attributes ?? LinkAttributes())
    }
    return merged
  },
  nodes: { [.klass(LinkNode.self)] },
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in try registerLink(editor, state.getOutput()) })
