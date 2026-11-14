---
'lexical-swift': patch
---

[fix] NodeState reads node JSON the way Lexical JS does in two more cases. A `$`
that is a string or an array, not an object, now gives the node a state
keyed by index, as JS's `Object.entries` does (for a string, one entry per
UTF-16 code unit), where Swift used to drop it. And a state value no config
has read yet (an object or array) now compares by identity, as JS's `===`
does: two text nodes made from separate JSON that carry equal unread
objects no longer merge, and a node and its copies or split pieces still
do. `registerLexicalTextEntity` applies the same rule when it decides
whether a text node merges with a neighboring entity.
