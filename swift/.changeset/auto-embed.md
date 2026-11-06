---
'lexical-swift': patch
---

`LexicalDecoratorBlockNode` ports `@lexical/react`'s `DecoratorBlockNode`, the base of block embeds (an alignment `format`, not indentable, a `<div>`), and `BlockWithAlignableContentsExtension`, the commands of `BlockWithAlignableContents`: `FORMAT_ELEMENT_COMMAND` aligns a selected block, and a click selects or deselects it. `LexicalAutoEmbed` ports the logic of `LexicalAutoEmbedPlugin`: `registerAutoEmbed` offers a node menu at a link that a paste of one token created when an `EmbedConfig` parses its URL, with `URL_MATCHER`, `INSERT_EMBED_COMMAND` and `AutoEmbedOption`.
