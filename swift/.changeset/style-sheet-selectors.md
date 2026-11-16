---
'lexical-swift': patch
---

Fixed `$inlineStylesFromStyleSheets` (`inlineStylesFromStyleSheets`, and the legacy `generateNodesFromDOM` that runs it) to match JS's on more style sheets: rules with descendant, child and sibling combinators and every attribute selector operator now apply, property names in a rule match in any case, and a `}` inside a quoted value no longer ends the rule. Added `parseStyleSheet` and `DOMNode.querySelectorAll`/`DOMElement.matches`, which it uses.
