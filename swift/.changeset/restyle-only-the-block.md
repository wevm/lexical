---
'lexical-swift': patch
---

Restyling a block's text after a commit walks only that block, not every text run in the document. Enter and joining paragraphs in a 5,000-block document went from about 23 ms to 12 ms (release).
