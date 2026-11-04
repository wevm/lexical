---
'lexical-swift': patch
---

`getWritable()` on a node already marked dirty in the current pass returns it without marking it and its ancestors again. Setting several properties of one node, as importing JSON does, is faster; the dirty nodes an update reports are unchanged.
