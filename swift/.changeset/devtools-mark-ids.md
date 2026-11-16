---
'lexical-swift': patch
---

Fixed `generateContent` in `LexicalDevtoolsCore` printing nothing for a `MarkNode`: it prints the mark's ids (`ids: [ a, b ]`), as `@lexical/devtools-core` does. `LexicalDevtoolsCore` now depends on `LexicalMark`.
