---
'lexical-swift': patch
---

`LexicalUITextView` and `LexicalNSTextView` follow `editor.setEditable(_:)`: they are editable only while the editor is, as `ContentEditable` sets `contentEditable`, and a read-only editor takes no typing, deleting, cutting, pasting, dropping or check box toggles, only clicks, copies and drags of its selection, as on the web. Before, `setEditable(false)` left the views editable and input still changed the document.
