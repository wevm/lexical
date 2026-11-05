---
'lexical-swift': patch
---

Add `LexicalText`, a port of `@lexical/text`: `registerLexicalTextEntity(_:_:_:_:)`, which turns the text a matcher finds into entity nodes (such as hashtags) and back, `EntityMatch`, `rootTextContent()`, `isRootTextContentEmpty(_:trim:)`, `canShowPlaceholder(_:)`, their curried forms, and `findTextIntersectionFromCharacters(_:_:)`.
