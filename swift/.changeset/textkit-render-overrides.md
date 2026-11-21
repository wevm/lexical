---
'lexical-swift': patch
---

Added `TextKitRenderExtension` and `textKitOverride`, the TextKit counterpart of `domOverride`'s classes: an app adds theme keys to the elements of the node classes it names, from its own state (a link to a missing page), and the views apply them after the element's theme key. To restyle a node when that state changes, mark it dirty in an update tagged `HISTORY_MERGE_TAG`, which keeps it out of undo history. An inline element marked dirty now restyles the text of its block, as a block marked dirty does.
