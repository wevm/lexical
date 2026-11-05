import SwiftUI

/// The playground's settings (`Settings` in the web playground), for the
/// options the Swift port has.
struct SettingsView: View {
  let model: PlaygroundModel
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    @Bindable var model = model
    NavigationStack {
      Form {
        Toggle("Rich Text", isOn: $model.isRichText)
        Toggle("Debug View", isOn: $model.showsTreeView)
        Toggle("Editable", isOn: $model.isEditable)
        Toggle("Autocomplete", isOn: $model.isAutocomplete)
        Toggle("Char Limit", isOn: $model.isCharLimit)
        Toggle("Char Limit (UTF-8)", isOn: $model.isCharLimitUtf8)
        Toggle("Max Length", isOn: $model.isMaxLength)
        Toggle("Use Brackets for Highlighting", isOn: $model.isSpecialText)
        Section {
          Button("Load the Welcome Document") {
            model.loadWelcome()
            dismiss()
          }
        }
      }
      .navigationTitle("Settings")
      .toolbar {
        ToolbarItem(placement: .confirmationAction) {
          Button("Done") { dismiss() }
        }
      }
    }
    .frame(minWidth: 360, minHeight: 280)
  }
}
