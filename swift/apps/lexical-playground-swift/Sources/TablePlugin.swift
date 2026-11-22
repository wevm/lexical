import Lexical
import LexicalTable
import SwiftUI

// Port of packages/lexical-playground/src/plugins/TablePlugin.tsx's
// `InsertTableDialog`: the rows and columns of the table to insert, 5 by 5
// to start, confirmed when there are 1–500 rows and 1–50 columns.

/// `InsertTableDialog`.
struct InsertTableDialog: View {
  let editor: Editor
  @Environment(\.dismiss) private var dismiss
  @State private var rows = "5"
  @State private var columns = "5"

  private var isDisabled: Bool {
    guard let row = Int(rows), let column = Int(columns) else { return true }
    return !(1...500).contains(row) || !(1...50).contains(column)
  }

  var body: some View {
    NavigationStack {
      Form {
        TextField("Rows", text: $rows, prompt: Text("# of rows (1-500)"))
          .accessibilityIdentifier("table-modal-rows")
        TextField("Columns", text: $columns, prompt: Text("# of columns (1-50)"))
          .accessibilityIdentifier("table-modal-columns")
        Button("Confirm") {
          _ = try? editor.dispatchCommand(
            INSERT_TABLE_COMMAND, InsertTableCommandPayload(columns: columns, rows: rows))
          dismiss()
        }
        .disabled(isDisabled)
        .accessibilityIdentifier("table-model-confirm-insert")
      }
      #if os(iOS)
        .keyboardType(.numberPad)
      #endif
      .navigationTitle("Insert Table")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
      }
    }
    .frame(minWidth: 420, minHeight: 180)
  }
}
