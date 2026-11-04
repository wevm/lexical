---
'lexical-swift': patch
---

The TextKit reconciler applies an update's edits to its rendered text in one pass instead of splicing the text once per edit, so restyling a whole document (select all and bold, on 5,000 blocks) is about 40% faster. The edits sent to the text storage are unchanged.
