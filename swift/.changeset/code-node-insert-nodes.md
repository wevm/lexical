---
'lexical-swift': patch
---

Fixed `RangeSelection.insertNodes` in a `CodeNode`, which now inserts the nodes in the code block, or the text of a pasted code block, as JS does. `CodeNode` is now a `CodeLanguageNode`, so it takes `insertNodes`' code block case; before, a pasted paragraph's text merged into the code's next line.
