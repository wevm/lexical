---
'lexical-swift': patch
---

A named font family draws at the weight a theme or inline `font-weight` asks for, picking the family's face as CSS font matching does: the faces nearest the normal width, then of the slant, then of the weight. On macOS every weight drew as Regular or Bold before: `font-weight: 500` in Avenir Next now draws Medium and 600 Demi Bold. On iOS, 500 drew Bold in a family without a Medium (Georgia, Menlo, Optima, Baskerville), 900 switched Helvetica Neue and Futura to their condensed faces, and an italic lost its weight; each now draws as on the web.
