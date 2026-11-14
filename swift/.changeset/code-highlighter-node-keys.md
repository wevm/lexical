---
'lexical-swift': patch
---

Fixed the code highlighter's node keys, which now follow JS's: JS's `$tokenize` makes a node of every token before comparing them with the block's children, so each token takes a key. `CodeHighlighterExtension` and `registerCodeHighlighting` still make nodes only of the tokens that changed, but now give them the keys JS gives them and skip the keys of the rest, so the nodes made after a highlight have JS's keys too.
