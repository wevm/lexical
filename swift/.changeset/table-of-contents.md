---
'lexical-swift': patch
---

Add `registerTableOfContents` to `LexicalRichText`, a port of `@lexical/react`'s `TableOfContentsPlugin` logic: it reports the document's headings (`TableOfContentsEntry`: key, text and tag), in order, at once and after each change to them, for a view to list.
