---
'lexical-ui': patch
---

Fixed typing slowing down as documents grow. An update no longer allocates a copy of every node key in the editor state: 500 typing updates in a 100×20 table went from 123 ms to 27 ms. With a root element, a commit now reuses the cached text of every element the update didn't dirty, instead of walking the whole document for the root's text (655 ms to 63 ms for the same table). `get_children` and the reconciler's walk also allocate less per child.
