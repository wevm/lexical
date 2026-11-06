---
'lexical-swift': patch
---

A line break in the TextKit views takes its block's text look (font, size and color), as the web's `<br>` takes its block's font. Before, it took the editor's base look, so in a code block an empty line after Enter got a body-size caret drawn off its line, and the line above it drew its text lower once it ended in a break. Every code line, empty or not, now has the code style's line box and caret on macOS and iOS.
