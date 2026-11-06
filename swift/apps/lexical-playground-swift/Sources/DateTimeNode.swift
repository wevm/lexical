import Foundation
import Lexical
import LexicalExtension
import LexicalHTML
import LexicalUtils
import SwiftUI

// Ports of packages/lexical-playground/src/nodes/DateTimeNode (the node
// and DateTimeComponent) and plugins/DateTimeExtension: a date, and the
// time when it is not midnight, as an inline pill with the text formats of
// where it was inserted. A click opens the picker for its day and time.
// Dates are JS `Date`s, in the local time zone; `nil` is an Invalid Date.

private let tagToFormat: KeyValuePairs<String, TextFormatType> = [
  "b": "bold", "i": "italic", "mark": "highlight", "s": "strikethrough", "u": "underline",
]

private func posixFormatter(_ format: String) -> DateFormatter {
  let formatter = DateFormatter()
  formatter.locale = Locale(identifier: "en_US_POSIX")
  formatter.dateFormat = format
  return formatter
}

/// `Date.prototype.toDateString()`: `Thu Nov 05 2026`.
func jsDateString(_ date: Date) -> String {
  posixFormatter("EEE MMM dd yyyy").string(from: date)
}

/// `Date.prototype.toString()`: `Thu Nov 05 2026 00:00:00 GMT-0500
/// (Eastern Standard Time)`.
func jsDateTimeString(_ date: Date) -> String {
  let zone = TimeZone.current
  let name =
    zone.localizedName(
      for: zone.isDaylightSavingTime(for: date) ? .daylightSaving : .standard,
      locale: Locale(identifier: "en_US")) ?? zone.identifier
  return posixFormatter("EEE MMM dd yyyy HH:mm:ss 'GMT'xx").string(from: date) + " (\(name))"
}

/// `Date.prototype.toISOString()`: UTC, with milliseconds.
func jsISOString(_ date: Date) -> String {
  let formatter = ISO8601DateFormatter()
  formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
  return formatter.string(from: date)
}

/// `Date.parse(string)`, for the forms the playground writes: ISO 8601
/// (with or without milliseconds) and `toString()`'s.
func jsDateParse(_ string: String) -> Date? {
  let iso = ISO8601DateFormatter()
  for options: ISO8601DateFormatter.Options in [
    [.withInternetDateTime, .withFractionalSeconds], [.withInternetDateTime],
  ] {
    iso.formatOptions = options
    if let date = iso.date(from: string) { return date }
  }
  // `toString()` without the time zone's name.
  let withoutName = string.split(separator: " (", maxSplits: 1).first.map(String.init) ?? string
  return posixFormatter("EEE MMM dd yyyy HH:mm:ss 'GMT'xx").date(from: withoutName)
}

/// The local hour and minute of `date`.
private func hourAndMinute(_ date: Date) -> (Int, Int) {
  let parts = Calendar.current.dateComponents([.hour, .minute], from: date)
  return (parts.hour ?? 0, parts.minute ?? 0)
}

/// `getDateTimeText`: the date, and ` HH:mm` when it is not midnight.
func getDateTimeText(_ dateTime: Date?) -> String {
  guard let dateTime else { return "Invalid Date" }
  let (hours, minutes) = hourAndMinute(dateTime)
  return jsDateString(dateTime)
    + (hours == 0 && minutes == 0 ? "" : String(format: " %02d:%02d", hours, minutes))
}

/// `dateTimeState`: parsed with `new Date(value)`, written with
/// `toISOString()`.
nonisolated(unsafe) private let dateTimeState = createState(
  "dateTime",
  parse: { (value: JSONValue?) -> Date? in value?.stringValue.flatMap(jsDateParse) },
  unparse: { (date: Date?) -> JSONValue? in date.map { .string(jsISOString($0)) } })

/// `DateTimeNode`.
final class DateTimeNode: DecoratorTextNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      DateTimeNode.self, type: "datetime", extends: DecoratorTextNode.self,
      stateConfigs: [RequiredNodeStateConfig(dateTimeState, flat: true)])
  }

  /// The Swift playground imports HTML with `generateNodesFromDOM`, which
  /// `DateTimeExtension`'s rules do not reach, so the node converts its
  /// span itself as well.
  override class func importDOM() -> DOMConversionMap? {
    [
      "span": { node in
        guard let el = node as? DOMElement,
          let value = el.getAttribute("data-lexical-datetime"), let date = jsDateParse(value)
        else { return nil }
        return DOMConversion(priority: 2) { _ in
          DOMConversionOutput(node: [try createDateTimeNode(date)])
        }
      }
    ]
  }

  func getDateTime() throws -> Date? { try getState(self, dateTimeState) }

  @discardableResult
  func setDateTime(_ dateTime: Date?) throws -> Self { try setState(self, dateTimeState, dateTime) }

  override func getTextContent() throws -> String { getDateTimeText(try getDateTime()) }

  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let document = getDocument()
    let element = document.createElement("span")
    let dateTime = try getDateTime()
    element.setAttribute("data-lexical-datetime", dateTime.map(jsDateTimeString) ?? "")
    _ = element.appendChild(
      try applyFormatToDom(self, document.createTextNode(getDateTimeText(dateTime)), tagToFormat))
    return DOMExportOutput(element: element)
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let element = getDocument().createElement("span")
    element.setAttribute(
      "data-lexical-datetime", (try getDateTime()).map(jsDateTimeString) ?? "")
    element.style.setProperty("display", "inline-block")
    return element
  }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }

  override func decorate(_ editor: Editor) throws -> Any? {
    let (key, dateTime, format) = (getKey(), try getDateTime(), try getFormat())
    // The views decorate on the main thread.
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated {
      DateTimeComponent(editor: editor, nodeKey: key, dateTime: dateTime, format: format)
    }
  }
}

/// `$createDateTimeNode(dateTime)`.
func createDateTimeNode(_ dateTime: Date) throws -> DateTimeNode {
  try DateTimeNode().setDateTime(dateTime)
}

func isDateTimeNode(_ node: LexicalNode?) -> Bool { node is DateTimeNode }

/// `INSERT_DATETIME_COMMAND`.
let INSERT_DATETIME_COMMAND = LexicalCommand<Date>("INSERT_DATETIME_COMMAND")

/// Today, tomorrow (`days` 1) or yesterday (-1), at local midnight, as the
/// component picker and the toolbar insert them.
func midnight(daysFromToday days: Int = 0) -> Date {
  let today = Calendar.current.startOfDay(for: Date())
  return Calendar.current.date(byAdding: .day, value: days, to: today) ?? today
}

nonisolated(unsafe) private let DateTimeRule = defineImportRule(
  name: "@lexical/playground/datetime", match: sel.tag("span").attr("data-lexical-datetime")
) { ctx, el, next in
  // An unparseable attribute would build a node around an Invalid Date;
  // keep the element's own content instead.
  guard let date = jsDateParse(el.getAttribute("data-lexical-datetime") ?? "") else {
    return try next()
  }
  let node = try createDateTimeNode(date)
  if let firstChild = try ctx.importChildren(el).first as? TextNode {
    try node.setFormat(firstChild.getFormat())
  }
  return [node]
}

nonisolated(unsafe) private let GoogleDocsDateRule = defineImportRule(
  name: "@lexical/playground/datetime-google-docs",
  match: sel.tag("span").attr("data-rich-links", /"type"\s*:\s*"date"/)
) { _, el, next in
  guard let data = (el.getAttribute("data-rich-links") ?? "{}").data(using: .utf8),
    let parsed = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
    let df = parsed["dat_df"] as? [String: Any]
  else { return try next() }
  let seconds =
    (((df["dfie_ts"] as? [String: Any])?["tv"] as? [String: Any])?["tv_s"] as? Double) ?? 0
  let date =
    seconds != 0
    ? Date(timeIntervalSince1970: seconds) : jsDateParse(df["dfie_dt"] as? String ?? "")
  guard let date else { return try next() }
  return [try applyFormatFromStyle(createDateTimeNode(date), el.style)]
}

/// `DateTimeExtension`.
let DateTimeExtension = defineExtension(
  name: "@lexical/playground/DateTime",
  dependencies: [
    CoreImportExtension,
    configExtension(DOMImportExtension) {
      $0.rules = [DateTimeRule, GoogleDocsDateRule].map(\.entry)
    },
  ],
  nodes: { [.klass(DateTimeNode.self)] },
  register: { editor, _, _ in
    editor.registerCommand(
      INSERT_DATETIME_COMMAND,
      { dateTime, _ in
        let dateTimeNode = try createDateTimeNode(dateTime)
        if let selection = try getSelection() as? RangeSelection {
          try dateTimeNode.setFormat(selection.format)
        }
        try insertNodeIntoLeaf(dateTimeNode)
        if try isRootOrShadowRoot(dateTimeNode.getParent()) {
          _ = try wrapNodeInElement(dateTimeNode) { try createParagraphNode() }.selectEnd()
        }
        return true
      }, COMMAND_PRIORITY_EDITOR)
  })

/// `DateTimeComponent`: the pill (`.dateTimePill`), and the picker a click
/// opens, with the day and, when "include time" is on, the time.
struct DateTimeComponent: View {
  let editor: Editor
  let nodeKey: NodeKey
  let dateTime: Date?
  let format: Int
  @State private var isOpen = false
  @State private var selection = NodeSelectionState()

  private var includesTime: Bool {
    dateTime.map { hourAndMinute($0) != (0, 0) } ?? false
  }

  private func has(_ type: TextFormatType) -> Bool {
    format & (TEXT_TYPE_TO_FORMAT[type] ?? 0) != 0
  }

  /// `withDateTimeNode`: sets the node's date and time.
  private func set(_ date: Date) {
    try? editor.update {
      if let node = try getNodeByKey(nodeKey) as? DateTimeNode { try node.setDateTime(date) }
    }
  }

  var body: some View {
    Text(getDateTimeText(dateTime))
      // An inline decorator wider than the line, as at the largest text
      // sizes, is drawn smaller rather than cut short.
      .lineLimit(1)
      .minimumScaleFactor(0.5)
      .bold(has("bold"))
      .italic(has("italic"))
      .underline(has("underline"))
      .strikethrough(has("strikethrough"))
      .padding(.horizontal, 4)
      .background(
        RoundedRectangle(cornerRadius: 8)
          .fill(
            has("highlight")
              ? Color(red: 1, green: 212 / 255, blue: 0, opacity: 0.14)
              : Color(light: Color(white: 0.867), dark: Color(white: 0.27)))
      )
      .overlay {
        if selection.isSelected {
          RoundedRectangle(cornerRadius: 8).stroke(
            Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255), lineWidth: 2)
        }
      }
      .contentShape(Rectangle())
      .onTapGesture { isOpen = true }
      .popover(isPresented: $isOpen, arrowEdge: .bottom) {
        DateTimePicker(dateTime: dateTime ?? midnight(), includesTime: includesTime, set: set)
      }
      .onAppear { selection.observe(editor, nodeKey) }
      .accessibilityElement(children: .combine)
      .accessibilityAddTraits(.isButton)
  }
}

/// The picker of `DateTimeComponent` (`DayPicker` and the time input).
private struct DateTimePicker: View {
  @State var dateTime: Date
  @State var includesTime: Bool
  let set: (Date) -> Void

  var body: some View {
    VStack(alignment: .leading) {
      DatePicker("Day", selection: $dateTime, displayedComponents: .date)
        .datePickerStyle(.graphical)
        .labelsHidden()
      HStack {
        Toggle("Include time", isOn: $includesTime)
        DatePicker("Time", selection: $dateTime, displayedComponents: .hourAndMinute)
          .labelsHidden()
          .disabled(!includesTime)
      }
    }
    .padding()
    .frame(width: 300)
    .onChange(of: dateTime) {
      set(includesTime ? dateTime : Calendar.current.startOfDay(for: dateTime))
    }
    .onChange(of: includesTime) {
      // Unchecking the time sets it back to midnight.
      if !includesTime {
        dateTime = Calendar.current.startOfDay(for: dateTime)
        set(dateTime)
      }
    }
  }
}
