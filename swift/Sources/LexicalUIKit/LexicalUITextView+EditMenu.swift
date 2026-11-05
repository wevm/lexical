#if canImport(UIKit)
  import UIKit

  // The edit menu: an app adds its own actions (the playground's text
  // formats) to what UIKit suggests for the selection, as a page can't do
  // with Safari's selection menu.

  extension LexicalUITextView {
    public func textView(
      _ textView: UITextView, editMenuForTextIn range: NSRange,
      suggestedActions: [UIMenuElement]
    ) -> UIMenu? {
      guard let elements = editMenuElements?(range), !elements.isEmpty else { return nil }
      return UIMenu(children: elements + suggestedActions)
    }
  }
#endif
