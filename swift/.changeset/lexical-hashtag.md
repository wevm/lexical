---
'lexical-swift': patch
---

Add `LexicalHashtag`, a port of `@lexical/hashtag`: `HashtagNode`, `createHashtagNode(_:)`, `isHashtagNode(_:)`, `HashtagExtension`, `HashtagConfig` and `registerLexicalHashtag(_:_:)`, with JS's matcher. The TextKit views style a text node whose type is not `text` with the theme style of its type, and the default theme gives hashtags the web playground's background.
