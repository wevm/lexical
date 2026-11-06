---
'lexical-swift': patch
---

`LexicalCodeCore` gains pluggable syntax highlighting: the `CodeTokenizer` protocol (Prism-style `CodeToken`s, `tokenize(codeNode:)` for `$tokenize`, and `languageState`/`loadLanguage` for grammars that load later), `CodeHighlighterExtension` with the tokenizer in its config, and `registerCodeHighlighting`. It ports the highlighting loop of `@lexical/code-prism`: the code-node transform, the token diff that replaces only changed `CodeHighlightNode`s, and caret retention. `ComposedCodeTokenizer` falls back from one tokenizer to another per language, and `PlainCodeTokenizer` highlights nothing.
