---
'lexical-swift': patch
---

Node, editor and editor-state fields, the `JSMap` storage and the per-thread context skip Swift's dynamic exclusivity checks, which every node read and write paid for. Updates and loads are faster; behavior is unchanged.
