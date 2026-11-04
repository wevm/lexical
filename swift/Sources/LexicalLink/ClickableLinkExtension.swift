import Lexical
import LexicalExtension

// Port of packages/lexical-link/src/ClickableLinkExtension.ts.
//
// JS listens to `click` and `auxclick` on the root element and opens the
// link with `window.open`. The native views report a click as
// `CLICK_COMMAND`, with the node under the pointer as `targetKey`, so the
// extension handles that command, and `LinkWindow` stands in for the
// window. JS also opens a plain `<a>` that is not a LinkNode; a native view
// has no such element, so that branch is not ported.

/// What `ClickableLinkExtension` opens a link with: JS `Window.open`.
public protocol LinkWindow: AnyObject {
  /// `window.open(url, target, features)`: `target` is `_blank` (with
  /// `features` `noopener`) for a new tab, else `_self`.
  func open(_ url: String, _ target: String, _ features: String?)
}

/// `ClickableLinkConfig`.
public struct ClickableLinkConfig {
  /// Open every link in a new tab.
  public var newTab: Bool
  public var disabled: Bool
  /// The window to open links in, or `nil` to not open them. JS defaults to
  /// the global `window`; the native views have none, so the app supplies
  /// it.
  public var getWindow: (Editor) -> LinkWindow?

  public init(
    newTab: Bool = false, disabled: Bool = false,
    getWindow: @escaping (Editor) -> LinkWindow? = { _ in nil }
  ) {
    self.newTab = newTab
    self.disabled = disabled
    self.getWindow = getWindow
  }
}

/// `registerClickableLink(editor, stores)`: opens a clicked link, unless
/// it is an unlinked autolink or the click ends a non-collapsed selection.
/// A middle click, a click with Command or Control, a `_blank` target or
/// `newTab` opens it in a new tab.
public func registerClickableLink(
  _ editor: Editor, _ stores: NamedSignals<ClickableLinkConfig>
) -> () -> Void {
  editor.registerCommand(
    CLICK_COMMAND,
    { event, _ in
      let isMiddle = event.button == 1
      if event.type == "auxclick" && !isMiddle {
        return false
      }
      guard let targetKey = event.targetKey else { return false }
      // The command runs in an update, which JS's listener opens itself; the
      // selection it reads is the one the click left.
      guard let clickedNode = try getNodeByKey(targetKey), !stores.disabled.peek(),
        let link = try findMatchingParent(clickedNode, { $0 is ElementNode }) as? LinkNode
      else { return false }
      if try (link as? AutoLinkNode)?.getIsUnlinked() ?? false { return false }
      let url = try link.sanitizeUrl(link.getURL())
      let urlTarget = try link.getTarget()
      if url.isEmpty { return false }
      if let selection = try getSelection() as? RangeSelection, !selection.isCollapsed() {
        event.preventDefault()
        return false
      }
      let newTab =
        stores.newTab.peek() || isMiddle || event.metaKey || event.ctrlKey || urlTarget == "_blank"
      guard let targetWindow = stores.getWindow.peek()(editor) else { return false }
      if newTab {
        targetWindow.open(url, "_blank", "noopener")
      } else {
        targetWindow.open(url, "_self", nil)
      }
      event.preventDefault()
      return false
    }, COMMAND_PRIORITY_LOW)
}

/// `ClickableLinkExtension`: opens links when they are clicked.
public let ClickableLinkExtension = defineExtension(
  name: "@lexical/link/ClickableLink", config: ClickableLinkConfig(),
  dependencies: [LinkExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in registerClickableLink(editor, state.getOutput()) })
