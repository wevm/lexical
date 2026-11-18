---
'lexical-swift': patch
---

Added `SelectionAlwaysOnDisplayExtension` and `selectionAlwaysOnDisplay(editor)`, which keep the selection shown while the editor does not have the focus, as in JS. `LexicalUITextView` highlights its selection while it is not the first responder; `LexicalNSTextView` already draws an unfocused selection. Added `LexicalRootElement.selectionAlwaysOnDisplay()`, which does nothing by default. `LexicalExtension` now depends on `LexicalUtils`, as `@lexical/extension` does on `@lexical/utils`.
