import Lexical
import SwiftUI

/// The playground's toolbar (`ToolbarPlugin`): undo and redo, the block
/// type, the lists, the font family and size, the text formats, the text
/// and background colors, the link, clear formatting, the Insert menu,
/// alignment and indent. The buttons reflect the selection, as the web toolbar does.
struct ToolbarView: View {
  let model: PlaygroundModel

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
        if model.blockType == .code && model.isRichText {
          CodeLanguageMenu(model: model)
        } else {
          FontFamilyMenu(model: model)
          Divider().frame(height: 20)
          FontSizeStepper(model: model)
          Divider().frame(height: 20)
          ForEach(toolbarFormats, id: \.format) { item in
            FormatButton(
              title: item.title, symbol: item.symbol, isOn: model.formats.contains(item.format)
            ) {
              model.toggleFormat(item.format)
            }
          }
          ColorPickerButton(
            title: "Formatting text color", symbol: "character", color: model.fontColor
          ) { model.applyStyleText("color", $0) }
          ColorPickerButton(
            title: "Formatting background color", symbol: "paintbrush.fill",
            color: model.backgroundColor
          ) { model.applyStyleText("background-color", $0) }
          FormatButton(title: "Insert Link", symbol: "link", isOn: model.isLink) {
            model.insertLink()
          }
          Button("Clear Formatting", systemImage: "eraser") { model.clearFormatting() }
          if model.isRichText {
            Divider().frame(height: 20)
            // The web toolbar's Insert dropdown, with the nodes the Swift
            // port has.
            Menu {
              Button("Horizontal Rule", systemImage: "minus") { model.insertHorizontalRule() }
              Button("Page Break", systemImage: "scissors") { model.insertPageBreak() }
              Button("Image", systemImage: "photo") { model.isInsertingImage = true }
              Button("Poll", systemImage: "chart.bar.xaxis") { model.isInsertingPoll = true }
              Button("Equation", systemImage: "function") { model.isInsertingEquation = true }
              Button("Sticky Note", systemImage: "note.text") { model.insertStickyNote() }
              Button("Date", systemImage: "calendar") { model.insertDate() }
              ForEach(EmbedConfigs, id: \.type) { config in
                Button(config.contentName, systemImage: config.symbol) {
                  model.insertEmbed(config.type)
                }
              }
            } label: {
              Label("Insert", systemImage: "plus").labelStyle(.titleAndIcon)
            }
            .fixedSize()
            .accessibilityLabel("Insert specialized editor node")
          }
        }
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
    .accessibilityIdentifier("toolbar")
  }
}

/// The web toolbar's code language dropdown (`CODE_LANGUAGE_OPTIONS_PRISM`),
/// shown instead of the font and format controls in a code block.
private struct CodeLanguageMenu: View {
  let model: PlaygroundModel

  var body: some View {
    let current = normalizeCodeLanguage(model.codeLanguage)
    Menu {
      item("(No language)", isOn: model.codeLanguage.isEmpty) { model.setCodeLanguage(nil) }
      ForEach(codeLanguageOptions, id: \.value) { option in
        item(option.name, isOn: option.value == current) { model.setCodeLanguage(option.value) }
      }
    } label: {
      Text(
        model.codeLanguage.isEmpty
          ? "(No language)"
          : codeLanguageOptions.first { $0.value == current }?.name ?? ""
      )
      .frame(minWidth: 90, alignment: .leading)
    }
    .fixedSize()
    .accessibilityLabel("Select language")
    .accessibilityIdentifier("code-language")
  }

  private func item(_ name: String, isOn: Bool, action: @escaping () -> Void) -> some View {
    Button(action: action) {
      if isOn { Label(name, systemImage: "checkmark") } else { Text(name) }
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
