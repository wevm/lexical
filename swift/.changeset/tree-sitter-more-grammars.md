---
'lexical-swift': patch
---

`LexicalCodeTreeSitter` has eight more opt-in grammar products: `LexicalCodeTreeSitterC`, `CPP`, `Java`, `Markdown`, `ObjC`, `PowerShell`, `SQL` and `XML`, for the code languages `c`, `cpp`/`c++`, `java`, `markdown`/`md`, `objc`/`objectivec`, `powershell`/`ps1`, `sql` and `xml`. C++ and Objective-C highlight with C's query as well as their own, as their grammars extend it. Markdown uses the block grammar, so headings, lists, quotes and fences highlight, not emphasis or links. Each grammar's MIT license is vendored with it.
