import Lexical
import LexicalExtension

// The TextKit counterpart of `domOverride`'s classes (#247): an app adds
// theme keys to elements of its choosing, from its own state, as a web app
// adds a class to a node's DOM element. The views draw no DOM, so the
// override returns the keys, which `TextKitElement(_:)` reads inside the
// render's read. See DESIGN.md, "Theme classes from the app".

/// An override of the theme keys the elements it matches add: its node
/// classes (and their subclasses), or every element when `nodes` is `nil`.
/// `classes` gets the element, `$next` (the keys its own rendering and the
/// overrides before this one add) and the editor, and returns the keys to
/// apply after the element's `themeKey`, in cascade order. It runs inside
/// the render's read.
public struct TextKitRenderMatch {
  public let nodes: [LexicalNode.Type]?
  public let classes: (ElementNode, () throws -> [String], Editor) throws -> [String]

  public init(
    nodes: [LexicalNode.Type]?,
    classes: @escaping (ElementNode, () throws -> [String], Editor) throws -> [String]
  ) {
    self.nodes = nodes
    self.classes = classes
  }

  func matches(_ node: ElementNode) -> Bool {
    nodes?.contains { isSubclass(type(of: node), of: $0) } ?? true
  }
}

/// `textKitOverride([Klass, ...], classes)`, the counterpart of
/// `domOverride([Klass, ...], {createDOM})` for the theme keys an element
/// adds.
public func textKitOverride(
  _ klasses: [LexicalNode.Type],
  classes: @escaping (ElementNode, () throws -> [String], Editor) throws -> [String]
) -> TextKitRenderMatch {
  TextKitRenderMatch(nodes: klasses, classes: classes)
}

/// `textKitOverride('*', classes)`: an override of every element.
public func textKitOverride(
  all classes: @escaping (ElementNode, () throws -> [String], Editor) throws -> [String]
) -> TextKitRenderMatch {
  TextKitRenderMatch(nodes: nil, classes: classes)
}

/// `TextKitRenderExtension`'s config. Configuring it again appends its
/// overrides to those already there, which run after them.
public struct TextKitRenderConfig {
  public var overrides: [TextKitRenderMatch]
  public init(overrides: [TextKitRenderMatch] = []) {
    self.overrides = overrides
  }
}

let TextKitRenderExtensionName = "@lexical/textkit/TextKitRender"

/// Adds theme keys to the elements the views draw, from the app's state,
/// as `DOMRenderExtension`'s overrides add classes to their DOM. To restyle
/// a node when that state changes, mark it dirty in an update tagged
/// `HISTORY_MERGE_TAG`, which keeps it out of undo history.
public let TextKitRenderExtension = defineExtension(
  name: TextKitRenderExtensionName, config: TextKitRenderConfig(),
  mergeConfig: { config, partial in
    var merged = partial.applied(to: config)
    if let overrides = partial.overrides { merged.overrides = config.overrides + overrides }
    return merged
  })

/// The editor's overrides, empty when it was not built with
/// `TextKitRenderExtension`.
func textKitOverrides(_ editor: Editor?) throws -> [TextKitRenderMatch] {
  guard let editor else { return [] }
  return try getPeerDependencyFromEditor(
    editor, TextKitRenderExtensionName, as: type(of: TextKitRenderExtension))?.config.overrides
    ?? []
}
