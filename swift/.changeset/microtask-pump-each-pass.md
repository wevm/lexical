---
'lexical-swift': patch
---

Fixed `LexicalUITextView` and `LexicalNSTextView` committing an update made outside the view (from a toolbar, a sheet's button or a main-queue block) only once the main run loop went idle. While events kept the main thread busy, such as during a sheet's dismissal, the commit waited for all of them; on a loaded iOS Simulator an image inserted from a closing sheet showed seconds late. The views now run queued microtasks after each pass of the run loop, as a browser runs them after each task.
