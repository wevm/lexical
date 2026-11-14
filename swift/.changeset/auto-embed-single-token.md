---
'lexical-swift': patch
---

Fixed `registerAutoEmbed`'s single-token test of a pasted text (JS's `/^\S+$/`) to use JS's white space: a paste with U+0085 in it now counts as one token, as in JS, so the link it makes can open the embed menu.
