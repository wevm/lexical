import Lexical
import LexicalTesting
import LexicalUtils
import Testing

// Ports of LexicalUtilsEventFiles.test.ts and LexicalUtilsKlassEqual.test.ts
// in packages/lexical-utils/src/__tests__/unit.

private final class MyEvent: DOMEvent {}
private final class MyEvent2: DOMEvent {}
private enum Shadow {
  final class MyEvent: DOMEvent {}
}

private let pngFile = DataTransferFile(
  name: "image.png", type: "image/png", data: Array("img".utf8))

private func dataTransfer(_ types: [String], _ files: [DataTransferFile]) -> DataTransfer {
  let transfer = DataTransfer()
  for type in types where type != "Files" { transfer.setData(type, "") }
  transfer.files = files
  return transfer
}

private func clipboardEvent(_ types: [String], _ files: [DataTransferFile]) -> ClipboardEvent {
  ClipboardEvent("paste", clipboardData: dataTransfer(types, files))
}

@Suite struct EventHelpersTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns [false, [], false] when event has no DataTransfer"
    ))
  func noDataTransfer() {
    let result = eventFiles(KeyboardEvent(key: "a", code: "KeyA"))
    #expect(!result.0 && result.1.isEmpty && !result.2)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns hasContent=true for text/html only clipboard (no files)"
    ))
  func htmlOnly() {
    let (hasFiles, files, hasContent) = eventFiles(clipboardEvent(["text/html"], []))
    #expect(!hasFiles && files.isEmpty && hasContent)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns hasContent=true for text/plain only clipboard (no files)"
    ))
  func plainOnly() {
    let (hasFiles, files, hasContent) = eventFiles(clipboardEvent(["text/plain"], []))
    #expect(!hasFiles && files.isEmpty && hasContent)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > returns hasFiles=true and hasContent=false when only Files present"
    ))
  func filesOnly() {
    let (hasFiles, files, hasContent) = eventFiles(clipboardEvent(["Files"], [pngFile]))
    #expect(hasFiles && files.count == 1 && !hasContent)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > browser-copied image: Files + text/html, no text/plain > returns hasFiles=true and hasContent=true (text/html present)"
    ))
  func filesAndHTML() {
    let (hasFiles, files, hasContent) = eventFiles(
      clipboardEvent(["Files", "text/html"], [pngFile]))
    #expect(hasFiles && files.count == 1 && hasContent)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > browser-copied image: Files + text/html, no text/plain > DragEvent with files and text/html behaves the same way"
    ))
  func dragFilesAndHTML() {
    let event = DragEvent("drop", dataTransfer: dataTransfer(["Files", "text/html"], [pngFile]))
    let (hasFiles, files, hasContent) = eventFiles(event)
    #expect(hasFiles && files.count == 1 && hasContent)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsEventFiles.test.ts::eventFiles > Word-copied text: Files + text/html + text/plain > returns hasFiles=true and hasContent=true (all types present)"
    ))
  func wordCopy() {
    let (hasFiles, files, hasContent) = eventFiles(
      clipboardEvent(["text/plain", "text/html", "Files"], [pngFile]))
    #expect(hasFiles && files.count == 1 && hasContent)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsKlassEqual.test.ts::LexicalUtilsKlassEqual tests > objectKlassEquals"
    ))
  func klassEquals() {
    let eventInstance: DOMEvent = MyEvent("")
    #expect(eventInstance is MyEvent)
    #expect(objectKlassEquals(eventInstance, MyEvent.self))
    #expect(!(eventInstance is MyEvent2))
    #expect(!objectKlassEquals(eventInstance, MyEvent2.self))
    #expect(!(eventInstance is Shadow.MyEvent))
    #expect(objectKlassEquals(eventInstance, Shadow.MyEvent.self))
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/LexicalUtilsKlassEqual.test.ts::LexicalUtilsKlassEqual tests > objectKlassEquals with a nullish or prototype-less value"
    ))
  func klassEqualsNil() {
    #expect(!objectKlassEquals(nil, MyEvent.self))
    #expect(!objectKlassEquals(Optional<DOMEvent>.none as Any?, MyEvent.self))
    #expect(!objectKlassEquals((), MyEvent.self))
  }
}
