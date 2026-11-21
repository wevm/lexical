import Lexical

// Port of packages/lexical-table/src/LexicalTableCommands.ts.

/// `InsertTableCommandPayloadHeaders`: `{rows, columns}`, or a `boolean`
/// for both.
public enum InsertTableCommandPayloadHeaders: Equatable, ExpressibleByBooleanLiteral {
  case boolean(Bool)
  case rowsAndColumns(rows: Bool, columns: Bool)

  public init(booleanLiteral value: Bool) { self = .boolean(value) }
}

/// `InsertTableCommandPayload`.
public struct InsertTableCommandPayload {
  public var columns: String
  public var rows: String
  public var includeHeaders: InsertTableCommandPayloadHeaders?

  public init(
    columns: String, rows: String, includeHeaders: InsertTableCommandPayloadHeaders? = nil
  ) {
    self.columns = columns
    self.rows = rows
    self.includeHeaders = includeHeaders
  }
}

public let INSERT_TABLE_COMMAND = createCommand(
  "INSERT_TABLE_COMMAND", payload: InsertTableCommandPayload.self)
