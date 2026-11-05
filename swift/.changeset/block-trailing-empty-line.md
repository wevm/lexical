---
'lexical-swift': patch
---

A quote's bar and a code block's box and gutter cover the block's last line when it is empty, as after a trailing line break or in an empty block, the way a trailing `<br>` gives the web's block an empty line box. On macOS, a block that changes at the end of the text, such as an empty paragraph that becomes a code block, lays its empty line out again with the block's spacing straight away instead of once something is typed.
