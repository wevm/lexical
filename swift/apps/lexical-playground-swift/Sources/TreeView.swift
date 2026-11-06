import SwiftUI

/// The playground's debug view (`TreeViewPlugin`): the text
/// `generateContent` prints for the editor, on a dark background, with a
/// switch between the tree and the exported HTML.
struct TreeView: View {
  let model: PlaygroundModel

  var body: some View {
    @Bindable var model = model
    VStack(alignment: .leading, spacing: 0) {
      HStack {
        Picker("Tree view", selection: $model.treeViewShowsHTML) {
          Text("Tree").tag(false)
          Text("Export DOM").tag(true)
        }
        .pickerStyle(.segmented)
        .labelsHidden()
        .fixedSize()
        Spacer()
      }
      .padding(8)
      ScrollView([.vertical, .horizontal]) {
        Text(model.tree)
          .scaledFont(size: 11, design: .monospaced, relativeTo: .caption2)
          .foregroundStyle(Color.white)
          .textSelection(.enabled)
          .frame(maxWidth: .infinity, alignment: .topLeading)
          .padding(.horizontal, 10)
          .padding(.bottom, 10)
          .accessibilityIdentifier("tree-view")
      }
      // A short tree starts at the top, not centered in the pane.
      .defaultScrollAnchor(.topLeading, for: .alignment)
    }
    .background(Color(red: 0.13, green: 0.13, blue: 0.14))
    .environment(\.colorScheme, .dark)
  }
}
