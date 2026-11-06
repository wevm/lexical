---
'lexical-swift': patch
---

Add `LexicalFile`, a port of `@lexical/file` in its own library product: `SerializedDocument`, `serializedDocumentFromEditorState` and `editorStateFromSerializedDocument` write and read `.lexical` documents byte for byte as JS does, with the same `lastSaved`, `source` and `version` handling. `importFile` and `exportFile` take the app's file picker as a hook (`ReadTextFileFromSystem`, `ExportBlob`) in place of upstream's DOM file input and download link.
