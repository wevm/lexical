---
'lexical-swift': patch
---

Fixed `basicTypeaheadTriggerMatch` to match as JS's `useBasicTypeaheadTriggerMatch` does: it counts `maxLength` in UTF-16 code units, as a JS regular expression without the `u` flag does, so a query of one emoji no longer fits a `maxLength` of 1, and its white space is JS's `\s` (U+FEFF ends a query, U+0085 and U+180E do not). The trigger and `punctuation` are still read as the bodies of character classes.
