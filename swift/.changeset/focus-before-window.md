---
'lexical-swift': patch
---

`LexicalUITextView` and `LexicalNSTextView` take a focus asked for before they are in a window (by `AutoFocusExtension`, `editor.focus()` or `focus()`) once they land in one, as a contenteditable is focused when it mounts. On macOS that focus was lost; a `blur()` in between now cancels it on both platforms.
