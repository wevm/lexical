---
'lexical-swift': patch
---

Added `LexicalNSTextView.contentHeight(atWidth:)` to the public API: the height the view's content would have at another width, measured without resizing the view, for an app that sizes the view itself (`NSView` has no `sizeThatFits` for a width). It wraps the text around the container's exclusion paths, which measuring the text storage on its own misses. On iOS, `LexicalUITextView.sizeThatFits(_:)`, inherited from `UITextView`, already answers this.
