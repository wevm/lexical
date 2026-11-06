---
'lexical-swift': patch
---

[feature] `LexicalUITextView` has `willPresentEditMenu` and `willDismissEditMenu`, called as UIKit presents and dismisses the edit menu (the view is its own `UITextViewDelegate`), so an app can move its floating views out of the menu's way.
