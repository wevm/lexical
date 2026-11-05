// Port of packages/lexical-react/src/LexicalMenuOption.ts.
//
// JS keeps a `ref` to each option's rendered element, which the menu
// scrolls into view, and a JSX `icon` and `title`. The menus here render
// natively, so the element and the icon belong to the view that draws the
// option; `title` is its text.

/// `MenuOption`: an item of a typeahead or node menu. Each option has a
/// unique `key`; subclass it to carry what selecting it needs.
open class MenuOption {
  public let key: String
  /// The text the menu shows for the option.
  open var title: String?

  public init(_ key: String) {
    self.key = key
  }
}
