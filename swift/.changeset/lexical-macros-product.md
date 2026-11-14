---
'lexical-swift': minor
---

[breaking] The `@LexicalNode` and `@Field` macros move out of `Lexical` into a new, optional `LexicalMacros` product, so an app that doesn't use them no longer builds swift-syntax, and Xcode no longer asks to trust a macro (or needs `-skipMacroValidation`) for `Lexical`. A node can declare its `config()` without them, with a `NodeConfig` and a `SchemaField` per property, as JS's `$config` does; the README's Custom nodes section shows both forms. To keep using the macros, add `.product(name: "LexicalMacros", package: "lexical-swift")` to the target (in Xcode, the LexicalMacros package product) and `import LexicalMacros` next to `import Lexical` in each file that uses them.
