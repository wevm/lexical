---
'lexical-swift': patch
---

`LexicalNSTextView` registers for the Lexical pasteboard types as drag types, so a drag that carries only a Lexical payload reaches its `DRAGOVER_COMMAND` and `DROP_COMMAND` handlers: `TextKitPasteboard.types` gains `application/x-lexical-drag-block` (`dev.lexical.drag-block`), the payload of a block dragged by its handle, and the view takes it from itself as a move.
