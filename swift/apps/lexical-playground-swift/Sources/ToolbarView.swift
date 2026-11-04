import Lexical
import SwiftUI

/// The playground's toolbar (`ToolbarPlugin`): undo and redo, the block
/// type, the lists, the text formats, the link, clear formatting, alignment
/// and indent. The buttons reflect the selection, as the web toolbar does.
struct ToolbarView: View {
  let model: PlaygroundModel
  @State private var isEditingLink = false

  var body: some View {
    ScrollView(.horizontal, showsIndicators: false) {
      HStack(spacing: 4) {
        Button("Undo", systemImage: "arrow.uturn.backward") { model.undo() }
          .disabled(!model.canUndo || !model.isEditable)
        Button("Redo", systemImage: "arrow.uturn.forward") { model.redo() }
          .disabled(!model.canRedo || !model.isEditable)
        Divider().frame(height: 20)
        if model.isRichText {
          Menu {
            ForEach(BlockType.allCases) { type in
              Button {
                model.setBlockType(type)
              } label: {
                if type == model.blockType {
                  Label(type.name, systemImage: "checkmark")
                } else {
                  Text(type.name)
                }
              }
            }
          } label: {
            Text(model.blockType.name).frame(minWidth: 90, alignment: .leading)
          }
          .fixedSize()
          .accessibilityLabel("Formatting options for text style")
          .accessibilityValue(model.blockType.name)
          ForEach(BlockType.lists, id: \.type) { item in
            FormatButton(
              title: item.type.name, symbol: item.symbol, isOn: model.blockType == item.type
            ) {
              model.setBlockType(item.type)
            }
          }
          Divider().frame(height: 20)
        }
        ForEach(toolbarFormats, id: \.format) { item in
          FormatButton(
            title: item.title, symbol: item.symbol, isOn: model.formats.contains(item.format)
          ) {
            model.toggleFormat(item.format)
          }
        }
        FormatButton(title: "Insert Link", symbol: "link", isOn: model.isLink) {
          isEditingLink = true
        }
        .popover(isPresented: $isEditingLink) {
          LinkEditorView(model: model, isPresented: $isEditingLink)
        }
        Button("Clear Formatting", systemImage: "eraser") { model.clearFormatting() }
        if model.isRichText {
          Divider().frame(height: 20)
          Menu {
            ForEach(toolbarAlignments, id: \.format) { item in
              Button(item.title, systemImage: item.symbol) { model.align(item.format) }
            }
            Divider()
            Button("Outdent", systemImage: "decrease.indent") { model.outdent() }
            Button("Indent", systemImage: "increase.indent") { model.indent() }
          } label: {
            Label(
              "Alignment",
              systemImage: toolbarAlignments.first { $0.format == model.alignment }?.symbol
                ?? "text.alignleft")
          }
          .fixedSize()
          .accessibilityLabel("Formatting options for text alignment")
        }
      }
      .labelStyle(.iconOnly)
      .buttonStyle(.borderless)
      .disabled(!model.isEditable)
      .padding(.horizontal, 8)
      .padding(.vertical, 6)
    }
  }
}

/// A format toggle, highlighted while the selection has the format.
private struct FormatButton: View {
  let title: String
  let symbol: String
  let isOn: Bool
  let action: () -> Void

  var body: some View {
    Button(title, systemImage: symbol, action: action)
      .padding(4)
      .background(
        RoundedRectangle(cornerRadius: 6).fill(isOn ? Color.accentColor.opacity(0.18) : .clear)
      )
      .accessibilityAddTraits(isOn ? .isSelected : [])
      .help(title)
  }
}

/// The link editor (`FloatingLinkEditorPlugin`): the selection's link URL,
/// editable, with Apply and Remove. For a selection outside a link, Apply
/// links it to the URL typed.
private struct LinkEditorView: View {
  let model: PlaygroundModel
  @Binding var isPresented: Bool
  @State private var url = ""

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Text(model.isLink ? "Edit Link" : "Insert Link").font(.headline)
      TextField("https://", text: $url)
        .textFieldStyle(.roundedBorder)
        .autocorrectionDisabled()
        #if os(iOS)
          .textInputAutocapitalization(.never)
          .keyboardType(.URL)
        #endif
        .accessibilityIdentifier("link-url")
        .onSubmit(apply)
      HStack {
        if model.isLink {
          Button("Remove", role: .destructive) {
            model.setLink(nil)
            isPresented = false
          }
        }
        Spacer()
        Button("Cancel") { isPresented = false }
        Button("Apply", action: apply)
          .keyboardShortcut(.defaultAction)
          .disabled(!validateUrl(url))
      }
    }
    .padding()
    .frame(minWidth: 320)
    .onAppear { url = model.isLink ? model.linkURL : "https://" }
  }

  private func apply() {
    guard validateUrl(url) else { return }
    model.setLink(url)
    isPresented = false
  }
}
