---
'lexical-swift': patch
---

Made `contentHeight(atWidth:)` on `LexicalNSTextView` and `LexicalUITextView` public, so an app that wraps the view in its own `NSViewRepresentable` or `UIViewRepresentable` can size it in `sizeThatFits` as `LexicalEditor` does: it measures the text at a width without changing the view, with the text container's exclusion paths.
