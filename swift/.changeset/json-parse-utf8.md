---
'lexical-swift': patch
---

`JSONValue.parse` (and so `parseEditorState`) is about five times faster: it reads the text's UTF-8 instead of copying it to UTF-16, decodes strings without escapes in one step, and keeps a `JSONObject`'s entries in one ordered array, which it searches linearly up to 16 keys. Parsed values, key order and error messages are unchanged; error positions are still UTF-16 offsets.
