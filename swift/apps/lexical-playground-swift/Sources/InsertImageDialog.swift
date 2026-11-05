import Foundation
import SwiftUI
import UniformTypeIdentifiers

// Port of `InsertImageDialog` in packages/lexical-playground/src/plugins/
// ImagesExtension: Sample, URL or File, then the URL or file with its alt
// text and Confirm, which dispatches `INSERT_IMAGE_COMMAND`. A file becomes
// a `data:` URL, as `FileReader.readAsDataURL` makes it. The sample is the
// web's `images/yellow-flower.jpg`, bundled.

struct InsertImageDialog: View {
  let model: PlaygroundModel
  @Environment(\.dismiss) private var dismiss
  @State private var mode: Mode?
  @State private var src = ""
  @State private var altText = ""
  @State private var isImporting = false
  @State private var fileName = ""

  enum Mode { case url, file }

  var body: some View {
    NavigationStack {
      Form {
        switch mode {
        case nil:
          Section {
            Button("Sample") {
              insert(
                ImagePayload(altText: "Yellow flower in tilt shift lens", src: "yellow-flower.jpg"))
            }
            .accessibilityIdentifier("image-modal-option-sample")
            Button("URL") { mode = .url }
              .accessibilityIdentifier("image-modal-option-url")
            Button("File") { mode = .file }
              .accessibilityIdentifier("image-modal-option-file")
          }
        case .url:
          TextField(
            "Image URL", text: $src, prompt: Text("i.e. https://source.unsplash.com/random")
          )
          .accessibilityIdentifier("image-modal-url-input")
          #if os(iOS)
            .textInputAutocapitalization(.never)
            .keyboardType(.URL)
          #endif
          .autocorrectionDisabled()
          TextField("Alt Text", text: $altText, prompt: Text("Random unsplash image"))
            .accessibilityIdentifier("image-modal-alt-text-input")
          confirm("image-modal-confirm-btn")
        case .file:
          Button(fileName.isEmpty ? "Image Upload" : fileName) { isImporting = true }
            .accessibilityIdentifier("image-modal-file-upload")
          TextField("Alt Text", text: $altText, prompt: Text("Descriptive alternative text"))
            .accessibilityIdentifier("image-modal-alt-text-input")
          confirm("image-modal-file-upload-btn")
        }
      }
      .navigationTitle("Insert Image")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
      }
      .fileImporter(isPresented: $isImporting, allowedContentTypes: [.image]) { result in
        guard case .success(let url) = result else { return }
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let data = try? Data(contentsOf: url) else { return }
        let type = UTType(filenameExtension: url.pathExtension)?.preferredMIMEType ?? "image/png"
        src = "data:\(type);base64,\(data.base64EncodedString())"
        fileName = url.lastPathComponent
      }
    }
    .frame(minWidth: 360, minHeight: 240)
  }

  private func confirm(_ identifier: String) -> some View {
    Button("Confirm") { insert(ImagePayload(altText: altText, src: src)) }
      .disabled(src.isEmpty)
      .accessibilityIdentifier(identifier)
  }

  private func insert(_ payload: ImagePayload) {
    model.insertImage(payload)
    dismiss()
  }
}
