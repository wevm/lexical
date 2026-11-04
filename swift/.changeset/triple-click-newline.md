---
'lexical-swift': patch
---

`LexicalNSTextView` no longer flashes a triple click's selection across the line's whole width: the paragraph selection it shows while the mouse is down stops before the newline, as `UITextView`'s does.
