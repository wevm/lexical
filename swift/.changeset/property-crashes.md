---
'lexical-swift': patch
---

Parsing a document no longer traps on a `format`, `detail`, `textFormat` or `indent` number outside `Int`'s range: the flags read as JS's bitwise operators read them (`ToInt32`) and indents are clamped. Enter in an empty code-highlight node now throws and leaves the editor unchanged, as in JS, instead of trapping.
