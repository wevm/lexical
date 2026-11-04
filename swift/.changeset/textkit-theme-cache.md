---
'lexical-swift': patch
---

A TextKit render resolves each element's style chain and each distinct text and paragraph look once per update instead of once per run and block, and a full render no longer restyles the runs its one edit already styles. Loading a large document renders about a third faster; rendered attributes are unchanged.
