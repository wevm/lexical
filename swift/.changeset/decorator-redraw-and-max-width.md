---
'lexical-swift': patch
---

The TextKit views attach a decorator again when its node changes, so its view shows the node's new properties, as React renders a decorator again. An inline decorator wider than its line is scaled down to the line's width, keeping its aspect, as an image's `max-width: 100%` scales it.
