# ``Lexical``

A native Swift port of the Lexical editor framework's core.

## Overview

`Lexical` is the model of the editor: an immutable ``EditorState`` holding
a tree of nodes and a selection, changed through `Editor.update`
and read through `Editor.read`. It is a port of the `lexical` npm
package, function by function, and produces the same states and the same
JSON, byte for byte, as the JS code it conforms to.

The views live in `LexicalUIKit`, `LexicalAppKit` and `LexicalSwiftUI`; the
features (rich text, history, clipboard, HTML) live in the module named
after their upstream package. Build an editor from extensions with
`buildEditorFromExtensions` in `LexicalExtension`.

## Topics

### Essentials

- <doc:JSParity>
- ``Editor``
- ``EditorState``
