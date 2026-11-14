---
'lexical-swift': minor
---

**Breaking:** pasted and dropped files are no longer read into memory on the main thread. As a web `File` is a lazy `Blob`, a `DataTransferFile` now has a `size` and reads its contents only when a handler awaits `bytes()`, off the main thread, so a paste or drop of large files no longer blocks the view, even when no handler wants the files. `DataTransferFile.data` is gone, and the type is no longer `Equatable`.

To migrate, read a file's contents with `try await file.bytes()` where you read `file.data`, as upstream's `mediaFileReader` awaits each `File`. A `DRAG_DROP_PASTE` handler that needs the bytes returns `true` and reads them in a `Task`. `DataTransferFile(name:type:data:)` still makes a file in memory, and `DataTransferFile(name:type:size:load:)` makes a lazy one. Compare files by `name` and `type` where you compared them with `==`.

On macOS, a dropped or pasted file is read from its URL; an image on the pasteboard with no file behind it is read from the pasteboard and throws once the pasteboard has changed. On iOS, UIKit lets items load only during the paste or drop, so each file starts loading into a temporary file then, `bytes()` waits for it, and `size` is `nil`. iOS drops and pastes now also carry files that aren't images, such as a PDF from Files, which were lost before.
