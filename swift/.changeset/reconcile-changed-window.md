---
'lexical-swift': patch
---

The TextKit reconciler skips the unchanged children around an inserted, removed or moved block instead of walking and reconciling every child of its parent. Enter and joining paragraphs in a 5,000-block document now take about 4 ms (release), under the 8 ms target.
