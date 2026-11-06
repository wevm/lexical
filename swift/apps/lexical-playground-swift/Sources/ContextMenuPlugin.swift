import Lexical
import LexicalLink
import LexicalMarkdown

#if os(macOS)
  import AppKit
  import LexicalAppKit
#else
  import UIKit
  import LexicalUIKit
#endif

// Port of packages/lexical-playground/src/plugins/ContextMenuPlugin: the
// web draws its own menu on `contextmenu` (Remove Link while the node is in
// a link, Cut, Copy, Copy as Markdown, Paste, Paste as Plain Text, Delete
// Node). Natively the menu is the system's: the right-click menu on macOS
// and the edit menu on iOS, which already have Cut, Copy and Paste, so the
// playground adds the web's other items ahead of the system's instead of
// replacing them. `$showOn` is tested against the node under the click on
// macOS, as in JS, and against the selection's anchor node on iOS, whose
// edit menu is the selection's.

/// One of the menu's items (`NodeContextMenuOption`).
struct ContextMenuOption {
  let title: String
  /// An SF Symbol for the playground's `contextMenuItemIcon`.
  let symbol: String
  let onSelect: () -> Void
}

/// The items for the current selection, in the web's order, without the
/// ones the system menu has.
@MainActor
func contextMenuOptions(
  _ editor: Editor, node key: NodeKey?, pasteAsPlainText: @escaping () -> Void
) -> [ContextMenuOption] {
  guard editor.isEditable() else { return [] }
  // `$showOn: node => $isLinkNode(node.getParent())`.
  let isInLink =
    (try? editor.read {
      guard let key, let node = try getNodeByKey(key) else { return false }
      return try node.getParent() is LinkNode
    }) ?? false
  var options: [ContextMenuOption] = []
  if isInLink {
    options.append(
      ContextMenuOption(title: "Remove Link", symbol: "link") {
        _ = try? editor.dispatchCommand(TOGGLE_LINK_COMMAND, nil)
      })
  }
  options.append(
    ContextMenuOption(title: "Copy as Markdown", symbol: "m.square") {
      let markdown =
        (try? editor.read {
          try convertSelectionToMarkdownString(playgroundTransformers, getSelection())
        }) ?? ""
      guard !markdown.isEmpty else { return }
      #if os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(markdown, forType: .string)
      #else
        UIPasteboard.general.string = markdown
      #endif
    })
  options.append(
    ContextMenuOption(
      title: "Paste as Plain Text", symbol: "doc.plaintext", onSelect: pasteAsPlainText))
  options.append(
    ContextMenuOption(title: "Delete Node", symbol: "trash") {
      try? editor.update { try deleteContextMenuNode() }
    })
  return options
}

/// `Delete Node`'s `$onSelect`: the selection's top-level block, or the
/// selected decorators.
func deleteContextMenuNode() throws {
  let selection = try getSelection()
  if let selection = selection as? RangeSelection {
    let parents = try selection.anchor.getNode().getParents()
    // `getParents().at(-2)`: the ancestor whose parent is the root.
    if parents.count >= 2 { try parents[parents.count - 2].remove() }
  } else if let selection = selection as? NodeSelection {
    for node in try selection.getNodes() where node is DecoratorNode {
      try node.remove()
    }
  }
}

#if os(macOS)
  /// A menu item that runs a closure.
  private final class ContextMenuItem: NSMenuItem {
    private let handler: () -> Void

    init(_ option: ContextMenuOption) {
      handler = option.onSelect
      super.init(title: option.title, action: #selector(run), keyEquivalent: "")
      target = self
      image = NSImage(systemSymbolName: option.symbol, accessibilityDescription: nil)
    }

    @available(*, unavailable)
    required init(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    @objc private func run() { handler() }
  }

  /// Adds the items to `root`'s right-click menu.
  @MainActor
  func attachContextMenu(to root: AnyObject?, editor: Editor) {
    guard let textView = root as? LexicalNSTextView else { return }
    textView.contextMenuItems = { [weak textView, weak editor] event in
      guard let editor, let textView else { return [] }
      // The node under the click (`$getNearestNodeFromDOMNode(e.target)`).
      let point = textView.convert(event.locationInWindow, from: nil)
      let index = textView.characterIndexForInsertion(at: point)
      let key = textView.controller.document.point(at: index).0
      return contextMenuOptions(editor, node: key) { [weak textView] in
        textView?.pasteAsPlainText(nil)
      }.map(ContextMenuItem.init)
    }
  }
#else
  /// The items as edit menu elements, after the playground's formats.
  @MainActor
  func contextMenuElements(_ textView: LexicalUITextView) -> [UIMenuElement] {
    let key = try? textView.editor.read {
      (try getSelection() as? RangeSelection)?.anchor.key
    }
    let actions = contextMenuOptions(textView.editor, node: key ?? nil) { [weak textView] in
      textView?.pasteAndMatchStyle(nil)
    }.map { option in
      UIAction(title: option.title, image: UIImage(systemName: option.symbol)) { _ in
        option.onSelect()
      }
    }
    return actions.isEmpty ? [] : [UIMenu(options: .displayInline, children: actions)]
  }
#endif
