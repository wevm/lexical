---
'lexical-ui': patch
---

Ported the children fast paths of `$reconcileChildren`. With a root element, an edit confined to the last children of a block of four or more (typing at the end, appending, Enter or merging at the end) now reconciles only those children and updates the root's cached text from them, instead of visiting every child. Other same-size edits skip the clean children. Typing at the end of 2,000 paragraphs went from 630 ms to 252 ms for 500 updates.
