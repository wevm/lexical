// Port of packages/lexical/src/LexicalCommands.ts and LexicalUpdateTags.ts.
//
// Commands whose payload is a DOM event (keyboard, pointer, clipboard, drag)
// are declared by the native input layer, with the native event as payload.

/// A command: dispatched with `editor.dispatchCommand` and handled by
/// listeners registered with `editor.registerCommand`, by priority. Commands
/// are compared by identity, like the objects `createCommand` returns.
public final class LexicalCommand<Payload>: Sendable {
  public let type: String?

  public init(_ type: String? = nil) {
    self.type = type
  }
}

/// `createCommand(type)`.
public func createCommand<Payload>(
  _ type: String? = nil, payload _: Payload.Type = Payload.self
)
  -> LexicalCommand<Payload>
{
  LexicalCommand(type)
}

/// The payload of `SELECTION_INSERT_CLIPBOARD_NODES_COMMAND`.
public struct InsertClipboardNodesPayload {
  public var nodes: [LexicalNode]
  public var selection: BaseSelection

  public init(nodes: [LexicalNode], selection: BaseSelection) {
    self.nodes = nodes
    self.selection = selection
  }
}

public let SELECTION_CHANGE_COMMAND = LexicalCommand<Void>("SELECTION_CHANGE_COMMAND")
public let SELECTION_INSERT_CLIPBOARD_NODES_COMMAND = LexicalCommand<InsertClipboardNodesPayload>(
  "SELECTION_INSERT_CLIPBOARD_NODES_COMMAND")
public let DELETE_CHARACTER_COMMAND = LexicalCommand<Bool>("DELETE_CHARACTER_COMMAND")
public let INSERT_LINE_BREAK_COMMAND = LexicalCommand<Bool>("INSERT_LINE_BREAK_COMMAND")
public let INSERT_PARAGRAPH_COMMAND = LexicalCommand<Void>("INSERT_PARAGRAPH_COMMAND")
/// Text to insert; in JS the payload may also be the `beforeinput` event.
public let CONTROLLED_TEXT_INSERTION_COMMAND = LexicalCommand<String>(
  "CONTROLLED_TEXT_INSERTION_COMMAND")
public let REMOVE_TEXT_COMMAND = LexicalCommand<Void>("REMOVE_TEXT_COMMAND")
public let DELETE_WORD_COMMAND = LexicalCommand<Bool>("DELETE_WORD_COMMAND")
public let DELETE_LINE_COMMAND = LexicalCommand<Bool>("DELETE_LINE_COMMAND")
public let FORMAT_TEXT_COMMAND = LexicalCommand<TextFormatType>("FORMAT_TEXT_COMMAND")
/// The formats to set (`true`) or clear (`false`), in order, as the keys of
/// the JS payload object are; others are left alone.
public let SET_TEXT_FORMAT_COMMAND = LexicalCommand<[(TextFormatType, Bool)]>(
  "SET_TEXT_FORMAT_COMMAND")
public let UNDO_COMMAND = LexicalCommand<Void>("UNDO_COMMAND")
public let REDO_COMMAND = LexicalCommand<Void>("REDO_COMMAND")
public let INSERT_TAB_COMMAND = LexicalCommand<Void>("INSERT_TAB_COMMAND")
public let INDENT_CONTENT_COMMAND = LexicalCommand<Void>("INDENT_CONTENT_COMMAND")
public let OUTDENT_CONTENT_COMMAND = LexicalCommand<Void>("OUTDENT_CONTENT_COMMAND")
public let FORMAT_ELEMENT_COMMAND = LexicalCommand<ElementFormatType>("FORMAT_ELEMENT_COMMAND")
public let CLEAR_EDITOR_COMMAND = LexicalCommand<Void>("CLEAR_EDITOR_COMMAND")
public let CLEAR_HISTORY_COMMAND = LexicalCommand<Void>("CLEAR_HISTORY_COMMAND")
public let CAN_REDO_COMMAND = LexicalCommand<Bool>("CAN_REDO_COMMAND")
public let CAN_UNDO_COMMAND = LexicalCommand<Bool>("CAN_UNDO_COMMAND")

// MARK: - Update tags

/// An update tag, as passed to `editor.update(..., tag:)` and
/// `$addUpdateTag`.
public typealias UpdateTag = String

public let HISTORIC_TAG: UpdateTag = "historic"
public let HISTORY_PUSH_TAG: UpdateTag = "history-push"
public let HISTORY_MERGE_TAG: UpdateTag = "history-merge"
public let PASTE_TAG: UpdateTag = "paste"
public let CUT_TAG: UpdateTag = "cut"
public let COLLABORATION_TAG: UpdateTag = "collaboration"
public let SKIP_COLLAB_TAG: UpdateTag = "skip-collab"
public let DEFER_COMPOSITION_MERGE_TAG: UpdateTag = "defer-composition-merge"
public let SKIP_SCROLL_INTO_VIEW_TAG: UpdateTag = "skip-scroll-into-view"
public let SKIP_DOM_SELECTION_TAG: UpdateTag = "skip-dom-selection"
public let SKIP_SELECTION_FOCUS_TAG: UpdateTag = "skip-selection-focus"
public let FOCUS_TAG: UpdateTag = "focus"
public let COMPOSITION_START_TAG: UpdateTag = "composition-start"
public let COMPOSITION_END_TAG: UpdateTag = "composition-end"
