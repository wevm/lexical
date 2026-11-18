---
'lexical-swift': patch
---

Changed `ClickableLinkExtension` to open clicked links by default, as JS opens them in the global window: `ClickableLinkConfig.getWindow` now defaults to the editor's root element, and `LexicalNSTextView` and `LexicalUITextView` open the link with the system's handler for its URL. Added `LexicalRootElement.open(_:_:_:)`, which does nothing by default. An app that configured `getWindow` is unaffected.
