---
'lexical-swift': patch
---

On macOS, `LexicalNSTextView` no longer reads every dragged file into memory each time the pointer moves during a drag. As a browser's `dragover` does, `DRAGOVER_COMMAND`'s `DataTransfer` lists the `Files` type with empty `files`; the files are read once, on the drop.
