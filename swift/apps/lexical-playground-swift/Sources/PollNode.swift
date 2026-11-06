import Foundation
import Lexical
import LexicalExtension
import LexicalHTML
import LexicalUtils
import SwiftUI

// Ports of packages/lexical-playground/src/nodes/PollNode.tsx,
// PollComponent.tsx and plugins/PollExtension: an inline poll with a
// question and options anyone can vote for, kept in two flat NodeStates,
// with its HTML, `INSERT_POLL_COMMAND` and the Insert dialog.

/// One of a poll's options. Parsed options keep their JSON as it was, as
/// upstream keeps the parsed row itself; an edit writes a fresh one
/// (`cloneOption`).
struct PollOption: Equatable {
  fileprivate(set) var json: JSONObject

  var text: String { json["text"]?.stringValue ?? "" }
  var uid: String { json["uid"]?.stringValue ?? "" }
  var votes: [String] { json["votes"]?.arrayValue?.compactMap(\.stringValue) ?? [] }

  init(text: String, uid: String, votes: [String]) {
    json = JSONObject([
      ("text", .string(text)), ("uid", .string(uid)), ("votes", .array(votes.map { .string($0) })),
    ])
  }

  fileprivate init(json: JSONObject) { self.json = json }
}

/// `createUID`: five lowercase letters.
private func createUID() -> String {
  String((0..<5).map { _ in "abcdefghijklmnopqrstuvwxyz".randomElement()! })
}

/// `createPollOption(text)`.
func createPollOption(_ text: String = "") -> PollOption {
  PollOption(text: text, uid: createUID(), votes: [])
}

/// `cloneOption(option, text, votes)`.
private func cloneOption(
  _ option: PollOption, _ text: String, _ votes: [String]? = nil
) -> PollOption {
  PollOption(text: text, uid: option.uid, votes: votes ?? option.votes)
}

/// `parseOptions`: the rows with a string `text` and `uid` and string
/// `votes`.
private func parseOptions(_ json: JSONValue?) -> [PollOption] {
  (json?.arrayValue ?? []).compactMap { row in
    guard let object = row.objectValue, object["text"]?.stringValue != nil,
      object["uid"]?.stringValue != nil,
      let votes = object["votes"]?.arrayValue, votes.allSatisfy({ $0.stringValue != nil })
    else { return nil }
    return PollOption(json: object)
  }
}

private func optionsJSON(_ options: [PollOption]) -> JSONValue {
  .array(options.map { .object($0.json) })
}

nonisolated(unsafe) private let questionState = createState(
  "question", parse: { (value: JSONValue?) -> String in value?.stringValue ?? "" },
  unparse: { (value: String) -> JSONValue? in .string(value) })

/// Equal when they stringify the same, as upstream compares them.
nonisolated(unsafe) private let optionsState = createState(
  "options", parse: parseOptions, unparse: { optionsJSON($0) },
  isEqual: { $0.count == $1.count && optionsJSON($0).stringify() == optionsJSON($1).stringify() })

/// `PollNode`.
final class PollNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(
      PollNode.self, type: "poll", extends: DecoratorNode.self,
      stateConfigs: [
        RequiredNodeStateConfig(questionState, flat: true),
        RequiredNodeStateConfig(optionsState, flat: true),
      ])
  }

  /// The Swift playground imports HTML with `generateNodesFromDOM`, which
  /// `PollExtension`'s rule does not reach, so the node converts its span
  /// itself as well.
  override class func importDOM() -> DOMConversionMap? {
    [
      "span": { node in
        guard let el = node as? DOMElement, el.getAttribute("data-lexical-poll-question") != nil
        else { return nil }
        return DOMConversion(priority: 1) { node in
          DOMConversionOutput(node: [try convertPollElement(node as! DOMElement)].compactMap { $0 })
        }
      }
    ]
  }

  func getQuestion() throws -> String { try getState(self, questionState) }

  @discardableResult
  func setQuestion(_ question: String) throws -> Self {
    try setState(self, questionState, question)
  }

  func getOptions() throws -> [PollOption] { try getState(self, optionsState) }

  @discardableResult
  func setOptions(_ options: [PollOption]) throws -> Self {
    try setState(self, optionsState, options)
  }

  @discardableResult
  func setOptions(_ updater: ([PollOption]) throws -> [PollOption]) throws -> Self {
    try setState(self, optionsState, updater)
  }

  @discardableResult
  func addOption(_ option: PollOption) throws -> Self { try setOptions { $0 + [option] } }

  @discardableResult
  func deleteOption(_ option: PollOption) throws -> Self {
    try setOptions { options in
      guard let index = options.firstIndex(of: option) else { return options }
      var options = options
      options.remove(at: index)
      return options
    }
  }

  @discardableResult
  func setOptionText(_ option: PollOption, _ text: String) throws -> Self {
    try setOptions { options in
      guard let index = options.firstIndex(of: option) else { return options }
      var options = options
      options[index] = cloneOption(option, text)
      return options
    }
  }

  @discardableResult
  func toggleVote(_ option: PollOption, _ username: String) throws -> Self {
    try setOptions { options in
      guard let index = options.firstIndex(of: option) else { return options }
      var votes = option.votes
      if let voteIndex = votes.firstIndex(of: username) {
        votes.remove(at: voteIndex)
      } else {
        votes.append(username)
      }
      var options = options
      options[index] = cloneOption(option, option.text, votes)
      return options
    }
  }

  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let element = getDocument().createElement("span")
    try element.setAttribute("data-lexical-poll-question", getQuestion())
    try element.setAttribute("data-lexical-poll-options", optionsJSON(getOptions()).stringify())
    return DOMExportOutput(element: element)
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let element = getDocument().createElement("span")
    element.style.setProperty("display", "inline-block")
    return element
  }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }

  override func decorate(_ editor: Editor) throws -> Any? {
    let (key, question, options) = (getKey(), try getQuestion(), try getOptions())
    // The views decorate on the main thread.
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated {
      PollComponent(editor: editor, nodeKey: key, question: question, options: options)
    }
  }
}

/// `$createPollNode(question, options)`.
func createPollNode(_ question: String, _ options: [PollOption]) throws -> PollNode {
  try PollNode().setQuestion(question).setOptions(options)
}

func isPollNode(_ node: LexicalNode?) -> Bool { node is PollNode }

/// `$convertPollElement`.
private func convertPollElement(_ el: DOMElement) throws -> PollNode? {
  guard let question = el.getAttribute("data-lexical-poll-question"),
    let options = el.getAttribute("data-lexical-poll-options")
  else { return nil }
  return try createPollNode(question, parseOptions(try JSONValue.parse(options)))
}

/// `INSERT_POLL_COMMAND`.
let INSERT_POLL_COMMAND = LexicalCommand<String>("INSERT_POLL_COMMAND")

nonisolated(unsafe) private let PollImportRule = defineImportRule(
  name: "@lexical/playground/poll",
  match: sel.tag("span").attr("data-lexical-poll-question")
) { _, el, next in
  if let node = try convertPollElement(el) { return [node] }
  return try next()
}

/// `PollExtension`, with `PollComponent`'s `CLICK_COMMAND` handler: the
/// views report a click on a decorator with its key; a click selects only
/// the poll, or deselects it, and a Shift click adds it to the selection.
let PollExtension = defineExtension(
  name: "@lexical/playground/Poll",
  dependencies: [configExtension(DOMImportExtension) { $0.rules = [PollImportRule.entry] }],
  nodes: { [.klass(PollNode.self)] },
  register: { editor, _, _ in
    mergeRegister([
      editor.registerCommand(
        INSERT_POLL_COMMAND,
        { question, _ in
          let pollNode = try createPollNode(question, [createPollOption(), createPollOption()])
          try insertNodes([pollNode])
          if try isRootOrShadowRoot(pollNode.getParentOrThrow()) {
            try wrapNodeInElement(pollNode, createParagraphNode).selectEnd()
          }
          return true
        }, COMMAND_PRIORITY_EDITOR),
      editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          guard let key = event.targetKey, try getNodeByKey(key) is PollNode else { return false }
          let current = try getSelection() as? NodeSelection
          let isSelected = current?.has(key) ?? false
          let selection =
            event.shiftKey ? (current ?? createNodeSelection()) : createNodeSelection()
          if isSelected {
            selection.delete(key)
          } else {
            selection.add(key)
          }
          try setSelection(selection)
          return true
        }, COMMAND_PRIORITY_LOW),
    ])
  })

/// `useCollaborationContext().name`: one of upstream's animal names, picked
/// once.
let collaborationUsername =
  [
    "Cat", "Dog", "Rabbit", "Frog", "Fox", "Hedgehog", "Pigeon", "Squirrel", "Bear", "Tiger",
    "Leopard", "Zebra", "Wolf", "Owl", "Gull", "Squid",
  ].randomElement()!

private let pollBlue = Color(red: 61 / 255, green: 135 / 255, blue: 245 / 255)
private let focusBlue = Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255)

/// `.PollNode__container`'s `min-width: 400px`; an iPhone's editor is
/// narrower.
@MainActor private var pollWidth: CGFloat {
  #if os(iOS)
    UIDevice.current.userInterfaceIdiom == .phone ? 320 : 400
  #else
    400
  #endif
}

/// `PollComponent`, with the look of `PollNode.css`.
struct PollComponent: View {
  let editor: Editor
  let nodeKey: NodeKey
  let question: String
  let options: [PollOption]
  @State private var selection = NodeSelectionState()

  private var totalVotes: Int { options.reduce(0) { $0 + $1.votes.count } }

  var body: some View {
    VStack(spacing: 0) {
      // `.PollNode__heading`.
      Text(question)
        .font(.system(size: 18, weight: .bold))
        .foregroundStyle(Color(white: 0.267))
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity)
        .padding(.bottom, 15)
        .accessibilityIdentifier("poll-question")
      ForEach(Array(options.enumerated()), id: \.element.uid) { index, option in
        PollOptionView(
          option: option, index: index, canDelete: options.count >= 3, totalVotes: totalVotes,
          withPollNode: withPollNode)
      }
      // `.PollNode__footer`'s small button.
      Button("Add Option") { withPollNode { try $0.addOption(createPollOption()) } }
        .buttonStyle(PlaygroundSmallButtonStyle())
        .accessibilityIdentifier("poll-add-option")
    }
    .padding(15)
    .frame(width: pollWidth)
    .background(Color(white: 0.988), in: RoundedRectangle(cornerRadius: 10))
    .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color(white: 0.933), lineWidth: 1))
    .overlay {
      if selection.isSelected {
        RoundedRectangle(cornerRadius: 10).inset(by: -1).stroke(focusBlue, lineWidth: 2)
      }
    }
    .environment(\.colorScheme, .light)
    .ignoresSafeArea(.keyboard)
    .onAppear { selection.observe(editor, nodeKey) }
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier("poll")
  }

  private func withPollNode(_ body: @escaping (PollNode) throws -> Void) {
    try? editor.update {
      if let node = try getNodeByKey(nodeKey) as? PollNode { try body(node) }
    }
  }
}

/// `PollOptionComponent`: the vote checkbox, the option's text over the
/// share of the votes it has, and the delete button.
private struct PollOptionView: View {
  let option: PollOption
  let index: Int
  let canDelete: Bool
  let totalVotes: Int
  let withPollNode: (@escaping (PollNode) throws -> Void) -> Void
  @State private var text = ""
  @FocusState private var isFocused: Bool

  var body: some View {
    let checked = option.votes.contains(collaborationUsername)
    let votes = option.votes.count
    HStack(spacing: 0) {
      // `.PollNode__optionCheckboxWrapper`.
      Button {
        withPollNode { try $0.toggleVote(option, collaborationUsername) }
      } label: {
        RoundedRectangle(cornerRadius: 5)
          .fill(checked ? pollBlue : .clear)
          .overlay(
            RoundedRectangle(cornerRadius: 5).stroke(checked ? pollBlue : Color(white: 0.6))
          )
          .overlay {
            if checked {
              Image(systemName: "checkmark").font(.system(size: 11, weight: .bold))
                .foregroundStyle(.white)
            }
          }
          .frame(width: 22, height: 22)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .padding(.trailing, 10)
      .accessibilityLabel(checked ? "Remove vote" : "Vote")
      .accessibilityAddTraits(checked ? .isSelected : [])
      .accessibilityIdentifier("poll-option-\(index)-vote")
      // `.PollNode__optionInputWrapper`.
      ZStack(alignment: .leading) {
        TextField(
          "", text: $text,
          prompt: Text("Option \(index + 1)").font(.system(size: 13)).foregroundStyle(
            Color(white: 0.6))
        )
        .textFieldStyle(.plain)
        .font(.system(size: 13, weight: .bold))
        .foregroundStyle(pollBlue)
        .padding(7)
        .padding(.trailing, votes > 0 ? 60 : 0)
        .focused($isFocused)
        .onChange(of: text) { _, value in
          guard value != option.text else { return }
          withPollNode { try $0.setOptionText(option, value) }
        }
        .accessibilityIdentifier("poll-option-\(index)")
        if votes > 0 {
          Text(votes == 1 ? "1 vote" : "\(votes) votes")
            .font(.system(size: 12))
            .foregroundStyle(pollBlue)
            .frame(maxWidth: .infinity, alignment: .trailing)
            .padding(.trailing, 15)
            .allowsHitTesting(false)
        }
      }
      // `.PollNode__optionInputVotes`: the option's share of the votes.
      .background(alignment: .leading) {
        GeometryReader { geometry in
          Color(red: 236 / 255, green: 243 / 255, blue: 254 / 255)
            .frame(
              width: totalVotes == 0 ? 0 : geometry.size.width * Double(votes) / Double(totalVotes)
            )
            .animation(.easeInOut(duration: 1), value: votes)
        }
      }
      .clipShape(RoundedRectangle(cornerRadius: 5))
      .overlay(RoundedRectangle(cornerRadius: 5).stroke(pollBlue))
      // `.PollNode__optionDelete`: a grey X, disabled below three options.
      Button {
        withPollNode { try $0.deleteOption(option) }
      } label: {
        Image(systemName: "xmark").font(.system(size: 12, weight: .semibold))
          .foregroundStyle(Color(white: 0.6))
          .frame(width: 28, height: 28)
          .contentShape(Rectangle())
      }
      .buttonStyle(.plain)
      .opacity(0.3)
      .disabled(!canDelete)
      .padding(.leading, 6)
      .accessibilityLabel("Remove")
      .accessibilityIdentifier("poll-option-\(index)-remove")
    }
    .padding(.bottom, 10)
    .onAppear { text = option.text }
    // The node's text, unless the field is being edited: its own text is
    // newer.
    .onChange(of: option.text) { _, value in if !isFocused, value != text { text = value } }
  }
}

/// `Button` with `small`: `.Button__root.Button__small`.
struct PlaygroundSmallButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(.system(size: 13))
      .foregroundStyle(.black)
      .padding(.vertical, 5).padding(.horizontal, 10)
      .background(
        Color(white: configuration.isPressed ? 0.867 : 0.933), in: RoundedRectangle(cornerRadius: 5)
      )
  }
}

/// `InsertPollDialog`: the question, and Confirm once it is not blank.
struct InsertPollDialog: View {
  let editor: Editor
  @Environment(\.dismiss) private var dismiss
  @State private var question = ""

  var body: some View {
    NavigationStack {
      Form {
        TextField("Question", text: $question)
          .accessibilityIdentifier("poll-question-input")
        Button("Confirm") {
          _ = try? editor.dispatchCommand(INSERT_POLL_COMMAND, question)
          dismiss()
        }
        .disabled(question.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        .accessibilityIdentifier("poll-confirm")
      }
      .navigationTitle("Insert Poll")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
      }
    }
    .frame(minWidth: 420, minHeight: 140)
  }
}
