---
'lexical-swift': patch
---

On macOS, a named font family draws at the weight a theme or inline `font-weight` asks for, picking the family's face as CSS font matching does, where it drew every weight as Regular or Bold before. `font-weight: 500` in Avenir Next draws Medium, 600 draws Demi Bold, and an italic keeps its weight; a family without a Medium draws 500 at Regular, and no weight switches to a condensed face.
