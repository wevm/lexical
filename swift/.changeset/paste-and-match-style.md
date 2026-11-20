---
'lexical-swift': patch
---

Fixed Paste and Match Style (⌥⇧⌘V) staying disabled in `LexicalNSTextView`: its readable pasteboard types now include `NSTextView`'s own, which AppKit checks to enable the menu item.
