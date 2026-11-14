---
'lexical-swift': patch
---

On macOS, `LexicalNSTextView` takes files dragged in from Finder or another app while the editor is editable, as a contenteditable takes a dropped file, so the drop reaches `DROP_COMMAND` and `DRAG_DROP_PASTE`. Before, `NSTextView` refused the drag, so a dropped file never reached the editor.
