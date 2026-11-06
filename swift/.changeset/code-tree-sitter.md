---
'lexical-swift': patch
---

`LexicalCodeTreeSitter` is a new `CodeTokenizer` built on the tree-sitter C core (v0.27.1, vendored, called through a thin wrapper), with one opt-in product per grammar: `LexicalCodeTreeSitterJavaScript`, `TypeScript`, `Swift`, `Python`, `JSON`, `HTML`, `CSS`, `Rust`, `Go` and `Bash`. It parses incrementally over UTF-16, so tokens map straight to Lexical offsets, and maps each grammar's highlight captures to Prism's token names. `LexicalTextKit` now styles a `CodeHighlightNode` with the theme's `codeHighlight.<type>` key. `CodeTokenizer.tokenize(codeNode:)` returns `[LexicalNode]?`, where `nil`, the default, means the highlighter tokenizes the block's text, and the highlighter now builds nodes only for the tokens that changed.
