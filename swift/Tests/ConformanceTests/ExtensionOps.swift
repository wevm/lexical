import Foundation
import Lexical
import LexicalA11y
import LexicalCodeCore
import LexicalExtension
import LexicalHashtag
import LexicalHeadless
import LexicalHistory
import LexicalLink
import LexicalList
import LexicalMark
import LexicalOverflow
import LexicalPlainText
import LexicalRichText

// Scripts with `extensions` (ops.ts): the editor is built with
// `buildEditorFromExtensions` from test extensions that ops.ts's
// `testExtension` makes from JSON (`ExtensionSpec`), and a few of
// @lexical/extension's own (`REAL_EXTENSIONS`). Each test extension logs
// every phase it runs, so the log is the order the builder ran them in.
// The packages' own extensions (RichTextExtension, HistoryExtension...) are
// real extensions too, whose JSON overrides name their functions by ops.ts's
// CONFIG_PRESETS (`realConfigured`).

/// A test extension's config: JSON, whose overrides are a
/// `Partial<JSONObject>` of `\JSONObject[key]` key paths.
typealias TestConfig = JSONObject

/// A test extension. Its output is the `namedSignals` of its config (with
/// `signals`), its init result `{init: name}`; both are `Any`, as an
/// extension without `build` or `init` has none.
typealias TestExtension = LexicalExtension<TestConfig, Any, Any>

/// The overrides `config` writes, key by key.
func testPartial(_ config: JSONObject) -> Partial<TestConfig> {
  Partial { partial in
    for (key, value) in config {
      partial.set(\TestConfig[key], value)
    }
  }
}

/// `REAL_EXTENSIONS` in ops.ts, without config overrides, but for
/// `HistoryExtension`, which `TestExtensions.real` gives the session's
/// clock.
private func realExtension(_ key: String) -> (any AnyLexicalExtensionArgument)? {
  switch key {
  case "ClearEditorExtension": ClearEditorExtension
  case "DecoratorTextExtension": DecoratorTextExtension
  case "EditorStateExtension": EditorStateExtension
  case "HorizontalRuleExtension": HorizontalRuleExtension
  case "NodeSelectionExtension": NodeSelectionExtension
  case "AutoLinkExtension": AutoLinkExtension
  case "BlockIdExtension": BlockIdExtension
  case "CheckListExtension": CheckListExtension
  case "CodeExtension": CodeExtension
  case "CodeIndentExtension": CodeIndentExtension
  case "HashtagExtension": HashtagExtension
  case "LinkExtension": LinkExtension
  case "ListExtension": ListExtension
  case "NestedEditorExtension": NestedEditorExtension
  case "NodeStructureExtension": NodeStructureExtension
  case "NormalizeInlineElementsExtension": NormalizeInlineElementsExtension
  case "OverflowExtension": OverflowExtension
  case "PlainTextExtension": PlainTextExtension
  case "RichTextExtension": RichTextExtension
  case "SelectBlockExtension": SelectBlockExtension
  case "SharedHistoryExtension": SharedHistoryExtension
  case "TabIndentationExtension": TabIndentationExtension
  case "AriaLiveRegionExtension": AriaLiveRegionExtension
  case "AutoLinkAnnounceExtension": AutoLinkAnnounceExtension
  case "EditorModeAnnounceExtension": EditorModeAnnounceExtension
  case "HeadingAnnounceExtension": HeadingAnnounceExtension
  case "HistoryAnnounceExtension": HistoryAnnounceExtension
  case "MarkExtension": MarkExtension
  case "MarkBaseExtension": MarkBaseExtension
  case "HeadlessExtension": HeadlessExtension
  case "KeyboardShortcutsExtension": KeyboardShortcutsExtension
  case "ClickAfterLastBlockExtension": ClickAfterLastBlockExtension
  case "NormalizeTripleClickSelectionExtension": NormalizeTripleClickSelectionExtension
  case "IMEExtension": IMEExtension
  case "AutoFocusExtension": AutoFocusExtension
  case "ClickableLinkExtension": ClickableLinkExtension
  default: markFlavor(key)
  }
}

/// `MARK_FLAVORS` in ops.ts: the mark flavors an `ExtensionArg` or a
/// MarkState action's `flavor` may name. `ClashingMarks` shares
/// CommentMarks' `stateKey`, and `AttributeClashMarks` its attribute.
private let markFlavors: [String: MarkFlavorExtension] = [
  "MarkStateExtension": MarkStateExtension,
  "CommentMarks": try! defineMarkExtension(
    name: "@conformance/CommentMarks", stateKey: "commentIds", className: "comment",
    overlapClassName: "comment-overlap"),
  "SuggestionMarks": try! defineMarkExtension(
    name: "@conformance/SuggestionMarks", stateKey: "suggestionIds",
    attribute: "data-Suggestion"),
  "ClashingMarks": try! defineMarkExtension(
    name: "@conformance/ClashingMarks", stateKey: "commentIds"),
  "AttributeClashMarks": try! defineMarkExtension(
    name: "@conformance/AttributeClashMarks", stateKey: "otherIds",
    attribute: "data-lexical-comment-ids"),
]

/// The mark flavor of `MARK_FLAVORS` named `key`, if any.
func markFlavor(_ key: String) -> MarkFlavorExtension? {
  markFlavors[key]
}

/// `markFlavor(action.flavor)` in ops.ts: the flavor a MarkState action
/// names, if it names one.
func markFlavor(of action: JSONValue) throws -> MarkFlavorExtension? {
  guard let key = action.objectValue?["flavor"]?.stringValue else { return nil }
  guard let flavor = markFlavor(key) else { throw ScriptError("unknown mark flavor \(key)") }
  return flavor
}

/// JS's default sort of strings: by UTF-16 code unit.
private func jsSorted<S: Sequence<String>>(_ strings: S) -> JSONValue {
  .array(strings.sorted { $0.utf16.lexicographicallyPrecedes($1.utf16) }.map { .string($0) })
}

/// `PREFILLED_HISTORY_STATE` in ops.ts.
private let prefilledHistoryState =
  #"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"prefilled","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#

/// `configExtension(extension, ...partials)`, which Swift spells
/// variadically.
private func configured<C, O, I>(
  _ extension_: LexicalExtension<C, O, I>, _ partials: [Partial<C>]
) throws -> any AnyLexicalExtensionArgument {
  switch partials.count {
  case 0: return extension_
  case 1: return configExtension(extension_, partials[0])
  case 2: return configExtension(extension_, partials[0], partials[1])
  case 3: return configExtension(extension_, partials[0], partials[1], partials[2])
  default: throw ScriptError("too many configs of \(extension_.name)")
  }
}

/// Each JSON config as a `Partial<C>`, field by field: `set` returns false
/// for a field it doesn't know.
private func partials<C>(
  _ key: String, _ configs: [JSONObject],
  _ set: (inout Partial<C>, String, JSONValue) throws -> Bool
) throws -> [Partial<C>] {
  try configs.map { config in
    var partial = Partial<C>()
    for (field, value) in config where try !set(&partial, field, value) {
      throw ScriptError("bad config of \(key): \(field) \(value.stringify())")
    }
    return partial
  }
}

private func bool(_ value: JSONValue) throws -> Bool {
  guard let b = value.boolValue else { throw ScriptError("not a boolean: \(value.stringify())") }
  return b
}

private func string(_ value: JSONValue) throws -> String {
  guard let s = value.stringValue else { throw ScriptError("not a string: \(value.stringify())") }
  return s
}

private func politeness(_ value: JSONValue) throws -> AriaPoliteness {
  guard let p = value.stringValue.flatMap(AriaPoliteness.init(rawValue:)) else {
    throw ScriptError("not a politeness: \(value.stringify())")
  }
  return p
}

private func number(_ value: JSONValue) throws -> Double {
  guard let n = value.numberValue else { throw ScriptError("not a number: \(value.stringify())") }
  return n
}

/// A JSON number or `null`, as an `Int?`.
private func intOrNull(_ value: JSONValue) throws -> Int? {
  if case .null = value { return nil }
  return Int(try number(value))
}

/// `LinkAttributes` from JSON: `rel`, `target` and `title`, each absent,
/// `null` or a string.
private func linkAttributes(_ value: JSONValue) throws -> LinkAttributes {
  guard let object = value.objectValue else { throw ScriptError("bad attributes") }
  func attribute(_ key: String) throws -> String?? {
    switch object[key] {
    case nil: return .none
    case .null?: return .some(nil)
    case .string(let s)?: return .some(s)
    default: throw ScriptError("bad attribute \(key)")
    }
  }
  return LinkAttributes(
    rel: try attribute("rel"), target: try attribute("target"), title: try attribute("title"))
}

/// A `ModifiersJson` as its mask.
private func modifiers(_ value: JSONValue) throws -> KeyboardEventModifierMask {
  let json = value.objectValue ?? JSONObject()
  func expectation(_ name: String) throws -> ModifierExpectation {
    switch json[name] {
    case nil, .bool(false)?: return .released
    case .bool(true)?: return .pressed
    case .string("any")?: return .any
    case let other?: throw ScriptError("bad modifier \(name): \(other.stringify())")
    }
  }
  var mask = KeyboardEventModifierMask(
    altKey: try expectation("altKey"), ctrlKey: try expectation("ctrlKey"),
    metaKey: try expectation("metaKey"), shiftKey: try expectation("shiftKey"))
  switch json["controlOrOther"]?.stringValue {
  case nil: break
  case "metaKey": mask.controlOrOther = .metaKey
  case "altKey": mask.controlOrOther = .altKey
  case let other?: throw ScriptError("bad controlOrOther \(other)")
  }
  return mask
}

/// `scriptShortcutMatch`: a `ShortcutJson` as a `KeyboardShortcut` without
/// functions, its command the keyboard command it names (`KEY_DOWN_COMMAND`
/// for a match that names none).
func shortcutMatch(_ value: JSONValue) throws -> KeyboardShortcut {
  let json = value.objectValue ?? JSONObject()
  guard let key = json["key"]?.stringValue else {
    throw ScriptError("a shortcut needs a key: \(value.stringify())")
  }
  return KeyboardShortcut(
    key: key, modifiers: try json["modifiers"].map(modifiers) ?? KeyboardEventModifierMask(),
    unshiftedKey: json["unshiftedKey"]?.stringValue,
    command: try json["command"]?.stringValue.map(keyboardCommand) ?? KEY_DOWN_COMMAND,
    description: json["description"]?.stringValue,
    bubbleFromNestedEditors: json["bubbleFromNestedEditors"]?.boolValue == true)
}

/// `getWindow: 'log'`'s window.
private final class LogWindow: LinkWindow {
  let log: (JSONValue) -> Void

  init(_ log: @escaping (JSONValue) -> Void) { self.log = log }

  func open(_ url: String, _ target: String, _ features: String?) {
    log(.array(["open", .string(url), .string(target), features.map { .string($0) } ?? .null]))
  }
}

private let extensionPhases = ["init", "build", "register", "afterRegistration"]

/// The test extensions of one script: ops.ts's `extensionOf`, which makes
/// each on first use.
final class TestExtensions {
  let defs: JSONObject
  var made: [String: TestExtension] = [:]
  /// The effects `watchNodeKey` made, by key.
  var watchers: [String: () -> Void] = [:]
  /// The disposers `registerMarkIDsListener` returned, by name.
  var markIDsListeners: [String: () -> Void] = [:]
  var log: (JSONValue) -> Void = { _ in }
  var command: (String) -> LexicalCommand<JSONValue> = { LexicalCommand($0) }
  /// The session's clock, which `HistoryExtension` reads as JS's reads
  /// `Date.now()` while a step runs.
  var clock: () -> Double = { 0 }
  /// The script's `parent` editor, which NestedEditorExtension's
  /// `$getParentEditor: 'parent'` returns.
  var parent: Editor?

  init(_ defs: JSONObject) { self.defs = defs }

  func of(_ key: String) throws -> TestExtension {
    if let made = made[key] { return made }
    guard let spec = defs[key]?.objectValue else {
      throw ScriptError("unknown extension \(key)")
    }
    let extension_ = try make(spec)
    made[key] = extension_
    return extension_
  }

  /// `extensionArg(ctx, arg)`.
  func arg(_ arg: JSONValue) throws -> any AnyLexicalExtensionArgument {
    if let key = arg.stringValue {
      if key == "HistoryExtension" { return try realConfigured(key, []) }
      return try realExtension(key) ?? of(key)
    }
    guard let parts = arg.arrayValue, let key = parts.first?.stringValue else {
      throw ScriptError("bad extension argument \(arg.stringify())")
    }
    if key == "HistoryExtension" || realExtension(key) != nil {
      return try realConfigured(
        key,
        parts.dropFirst().map { config in
          guard let object = config.objectValue else { throw ScriptError("bad config") }
          return object
        })
    }
    let configs = try parts.dropFirst().map { config in
      guard let object = config.objectValue else { throw ScriptError("bad config") }
      return testPartial(object)
    }
    let extension_ = try of(key)
    // `configExtension(extension, ...configs)`, which Swift spells variadically.
    switch configs.count {
    case 0: return extension_
    case 1: return configExtension(extension_, configs[0])
    case 2: return configExtension(extension_, configs[0], configs[1])
    default: throw ScriptError("too many configs of \(key)")
    }
  }

  /// `configExtension(extension, ...configs.map(realConfig))` of a real
  /// extension: each JSON config as its typed overrides, with the functions
  /// `CONFIG_PRESETS` names.
  private func realConfigured(
    _ key: String, _ configs: [JSONObject]
  ) throws
    -> any AnyLexicalExtensionArgument
  {
    switch key {
    case "HistoryExtension":
      // The session's clock first; no JSON config can set `now`.
      let clock = self.clock
      let now = Partial<HistoryConfig> { $0.set(\.now, clock) }
      return try configured(
        HistoryExtension,
        [now]
          + partials(key, configs) { (p: inout Partial<HistoryConfig>, field, value) in
            switch field {
            case "delay": p.set(\.delay, try number(value))
            case "disabled": p.set(\.disabled, try bool(value))
            case "maxDepth": p.set(\.maxDepth, try intOrNull(value))
            case "createInitialHistoryState" where value.stringValue == "prefilled":
              p.set(\.createInitialHistoryState) { editor in
                let editorState = try! editor.parseEditorState(prefilledHistoryState)
                return HistoryState(
                  undoStack: [HistoryStateEntry(editor: editor, editorState: editorState)])
              }
            default: return false
            }
            return true
          })
    case "ListExtension":
      return try configured(
        ListExtension,
        partials(key, configs) { (p: inout Partial<ListConfig>, field, value) in
          switch field {
          case "hasStrictIndent": p.set(\.hasStrictIndent, try bool(value))
          case "shouldPreserveNumbering": p.set(\.shouldPreserveNumbering, try bool(value))
          default: return false
          }
          return true
        })
    case "CheckListExtension":
      return try configured(
        CheckListExtension,
        partials(key, configs) { (p: inout Partial<CheckListConfig>, field, value) in
          guard field == "disableTakeFocusOnClick" else { return false }
          p.set(\.disableTakeFocusOnClick, try bool(value))
          return true
        })
    case "LinkExtension":
      return try configured(
        LinkExtension,
        partials(key, configs) { (p: inout Partial<LinkConfig>, field, value) in
          switch field {
          case "attributes": p.set(\.attributes, try linkAttributes(value))
          case "validateUrl" where value.stringValue == "https":
            p.set(\.validateUrl) { $0.hasPrefix("https://") }
          default: return false
          }
          return true
        })
    case "AutoLinkExtension":
      let log = self.log
      return try configured(
        AutoLinkExtension,
        partials(key, configs) { (p: inout Partial<AutoLinkConfig>, field, value) in
          let names = (value.arrayValue ?? []).map { $0.stringValue ?? "" }
          switch field {
          case "matchers":
            p.set(
              \.matchers,
              try names.map { name -> LinkMatcher in
                switch name {
                case "url": return autoLinkUrlMatcher
                case "email": return autoLinkEmailMatcher
                default: throw ScriptError("unknown matcher \(name)")
                }
              })
          case "changeHandlers":
            p.set(
              \.changeHandlers,
              try names.map { name -> ChangeHandler in
                guard name == "log" else { throw ScriptError("unknown change handler \(name)") }
                return { url, prevUrl in
                  log(
                    .array([
                      "autoLink", url.map { .string($0) } ?? .null,
                      prevUrl.map { .string($0) } ?? .null,
                    ]))
                }
              })
          default: return false
          }
          return true
        })
    case "RichTextExtension":
      return try configured(
        RichTextExtension,
        partials(key, configs) { (p: inout Partial<RichTextConfig>, field, value) in
          switch field {
          case "escapeFormatTriggers":
            p.set(\.escapeFormatTriggers, try escapeFormatTriggers(value))
          // `shouldHandlePasteAsFiles: 'always'` pastes the files of a paste
          // that has any.
          case "shouldHandlePasteAsFiles" where value.stringValue == "always":
            p.set(\.shouldHandlePasteAsFiles) { files, _ in !files.isEmpty }
          default: return false
          }
          return true
        })
    case "CodeIndentExtension":
      return try configured(
        CodeIndentExtension,
        partials(key, configs) { (p: inout Partial<CodeIndentConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          case "tabSize": p.set(\.tabSize, try intOrNull(value))
          case "escapeWithArrows": p.set(\.escapeWithArrows, try bool(value))
          default: return false
          }
          return true
        })
    case "NormalizeInlineElementsExtension":
      return try configured(
        NormalizeInlineElementsExtension,
        partials(key, configs) {
          (p: inout Partial<NormalizeInlineElementsConfig>, field, value) in
          guard field == "disabled" else { return false }
          p.set(\.disabled, try bool(value))
          return true
        })
    case "TabIndentationExtension":
      return try configured(
        TabIndentationExtension,
        partials(key, configs) { (p: inout Partial<TabIndentationConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          case "maxIndent": p.set(\.maxIndent, try intOrNull(value))
          case "$canIndent" where value.stringValue == "never": p.set(\.canIndent) { _ in false }
          default: return false
          }
          return true
        })
    case "NestedEditorExtension":
      return try configured(
        NestedEditorExtension,
        partials(key, configs) {
          [unowned self] (p: inout Partial<NestedEditorConfig>, field, value) in
          switch field {
          case "inheritEditableFromParent": p.set(\.inheritEditableFromParent, try bool(value))
          case "$getParentEditor" where value.stringValue == "parent":
            p.set(\.getParentEditor) { [unowned self] in
              guard let parent = self.parent else {
                throw ScriptError("the script has no parent editor")
              }
              return parent
            }
          default: return false
          }
          return true
        })
    case "SharedHistoryExtension":
      return try configured(
        SharedHistoryExtension,
        partials(key, configs) { (p: inout Partial<SharedHistoryConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          default: return false
          }
          return true
        })
    case "NodeStructureExtension":
      return try configured(
        NodeStructureExtension,
        partials(key, configs) { (p: inout Partial<NodeStructureConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          default: return false
          }
          return true
        })
    case "BlockIdExtension":
      return try configured(
        BlockIdExtension,
        partials(key, configs) { (p: inout Partial<BlockIdConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          case "repairTransforms": p.set(\.repairTransforms, try bool(value))
          case "generateId" where value.stringValue == "counter":
            // `id1`, `id2`...
            var n = 0
            p.set(\.generateId) {
              n += 1
              return "id\(n)"
            }
          case "generateId" where value.stringValue == "stutter":
            // Each of `id1`, `id2`... twice in a row.
            var n = 0
            p.set(\.generateId) {
              defer { n += 1 }
              return "id\((n >> 1) + 1)"
            }
          case "generateId" where value.stringValue == "invalid":
            p.set(\.generateId) { "not valid" }
          case "$isIdentifiedBlock" where value.stringValue == "rootChild":
            p.set(\.isIdentifiedBlock, isRootChildBlock)
          default: return false
          }
          return true
        })
    case "SelectBlockExtension":
      return try configured(
        SelectBlockExtension,
        partials(key, configs) { (p: inout Partial<SelectBlockConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          case "cascadeSelection": p.set(\.cascadeSelection, try bool(value))
          default: return false
          }
          return true
        })
    case "AriaLiveRegionExtension":
      return try configured(
        AriaLiveRegionExtension,
        partials(key, configs) { (p: inout Partial<AriaLiveRegionExtensionConfig>, field, value) in
          switch field {
          case "politeness": p.set(\.politeness, try politeness(value))
          default: return false
          }
          return true
        })
    case "HistoryAnnounceExtension":
      return try configured(
        HistoryAnnounceExtension,
        partials(key, configs) {
          (p: inout Partial<HistoryAnnounceExtensionConfig>, field, value) in
          switch field {
          case "undone": p.set(\.undone, try string(value))
          case "redone": p.set(\.redone, try string(value))
          case "disabled": p.set(\.disabled, try bool(value))
          default: return false
          }
          return true
        })
    case "EditorModeAnnounceExtension":
      return try configured(
        EditorModeAnnounceExtension,
        partials(key, configs) {
          (p: inout Partial<EditorModeAnnounceExtensionConfig>, field, value) in
          switch field {
          case "editable": p.set(\.editable, try string(value))
          case "readOnly": p.set(\.readOnly, try string(value))
          case "disabled": p.set(\.disabled, try bool(value))
          default: return false
          }
          return true
        })
    case "HeadingAnnounceExtension":
      return try configured(
        HeadingAnnounceExtension,
        partials(key, configs) {
          (p: inout Partial<HeadingAnnounceExtensionConfig>, field, value) in
          switch field {
          case "created": p.set(\.created, try string(value))
          case "destroyed": p.set(\.destroyed, try string(value))
          case "disabled": p.set(\.disabled, try bool(value))
          default: return false
          }
          return true
        })
    case "AutoLinkAnnounceExtension":
      return try configured(
        AutoLinkAnnounceExtension,
        partials(key, configs) {
          (p: inout Partial<AutoLinkAnnounceExtensionConfig>, field, value) in
          switch field {
          case "created": p.set(\.created, try string(value))
          case "createdMany": p.set(\.createdMany, try string(value))
          case "destroyed": p.set(\.destroyed, try string(value))
          case "destroyedMany": p.set(\.destroyedMany, try string(value))
          case "disabled": p.set(\.disabled, try bool(value))
          default: return false
          }
          return true
        })
    case "KeyboardShortcutsExtension":
      return try configured(
        KeyboardShortcutsExtension,
        partials(key, configs) { (p: inout Partial<KeyboardShortcutsConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          case "priority": p.set(\.priority, Int(try number(value)))
          case "shortcuts": p.set(\.shortcuts, try shortcuts(value))
          default: return false
          }
          return true
        })
    case "ClickAfterLastBlockExtension":
      return try configured(
        ClickAfterLastBlockExtension,
        partials(key, configs) { (p: inout Partial<ClickAfterLastBlockConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          case "$shouldInsertAfter" where value.stringValue == "always":
            p.set(\.shouldInsertAfter) { _ in true }
          default: return false
          }
          return true
        })
    case "AutoFocusExtension":
      return try configured(
        AutoFocusExtension,
        partials(key, configs) { (p: inout Partial<AutoFocusConfig>, field, value) in
          switch field {
          case "disabled": p.set(\.disabled, try bool(value))
          case "defaultSelection": p.set(\.defaultSelection, try string(value))
          default: return false
          }
          return true
        })
    case "ClickableLinkExtension":
      let log = self.log
      return try configured(
        ClickableLinkExtension,
        partials(key, configs) { (p: inout Partial<ClickableLinkConfig>, field, value) in
          switch (field, value.stringValue) {
          case ("disabled", _): p.set(\.disabled, try bool(value))
          case ("newTab", _): p.set(\.newTab, try bool(value))
          // `getWindow: 'log'`: a window whose `open` logs `['open', url,
          // target, features]`.
          case ("getWindow", "log"):
            let window = LogWindow(log)
            p.set(\.getWindow) { _ in window }
          case ("getWindow", "none"): p.set(\.getWindow) { _ in nil }
          default: return false
          }
          return true
        })
    default:
      if let flavor = markFlavor(key) {
        return try configured(
          flavor,
          partials(key, configs) { (p: inout Partial<MarkStateConfig>, field, value) in
            switch (field, value.stringValue) {
            case ("disabled", _): p.set(\.disabled, try bool(value))
            case ("exportIDsToHTML", _): p.set(\.exportIDsToHTML, try bool(value))
            // CONFIG_PRESETS' `markableNodes: 'text'` is `[TextNode]`, and
            // `'decorators'` `[...DEFAULT_MARKABLE_NODES, $isDecoratorNode]`.
            case ("markableNodes", "text"): p.set(\.markableNodes, [.klass(TextNode.self)])
            case ("markableNodes", "decorators"):
              p.set(\.markableNodes, DEFAULT_MARKABLE_NODES + [.predicate { $0 is DecoratorNode }])
            default: return false
            }
            return true
          })
      }
      throw ScriptError("\(key) takes no config overrides")
    }
  }

  /// `scriptShortcuts(ctx, json)`: a `ShortcutsJson` as a table, each
  /// shortcut's `$disabled` and `$dispatch` by name.
  private func shortcuts(_ json: JSONValue) throws -> NamedKeyboardShortcuts {
    guard let object = json.objectValue else {
      throw ScriptError("bad shortcuts \(json.stringify())")
    }
    var table = NamedKeyboardShortcuts()
    for (name, value) in object {
      switch value {
      case .null: table.set(name, nil)
      case .array(let list): table.set(name, try list.map(shortcut))
      default: table.set(name, [try shortcut(value)])
      }
    }
    return table
  }

  private func shortcut(_ value: JSONValue) throws -> KeyboardShortcut {
    var shortcut = try shortcutMatch(value)
    let json = value.objectValue ?? JSONObject()
    switch json["$disabled"]?.stringValue {
    case nil: break
    case "always": shortcut.disabled = { _, _ in true }
    case "notRange": shortcut.disabled = { selection, _ in !(selection is RangeSelection) }
    case let other?: throw ScriptError("bad $disabled \(other)")
    }
    if let name = json["$dispatch"]?.stringValue {
      guard name == "log" || name == "unhandled" else {
        throw ScriptError("bad $dispatch \(name)")
      }
      let handled = name == "log"
      let log = self.log
      shortcut.dispatch = { [unowned self] _, event, next, fromEditor in
        // `fromEditor === ctx.editor`: every editor is the session's but
        // the script's `parent`.
        log(.array(["shortcutDispatch", .string(event.key), .bool(self.parent !== fromEditor)]))
        return handled ? try next() : false
      }
    }
    return shortcut
  }

  /// `output[name].value = value` for a signal of a real extension's
  /// output; false when `key` isn't a real extension.
  private func setRealSignal(
    _ editor: Editor, _ key: String, _ name: String, _ value: JSONValue
  ) throws -> Bool {
    switch (key, name) {
    case ("HistoryExtension", "disabled"):
      try getExtensionDependencyFromEditor(editor, HistoryExtension).output.disabled.set(
        try bool(value))
    case ("HistoryExtension", "delay"):
      try getExtensionDependencyFromEditor(editor, HistoryExtension).output.delay.set(
        try number(value))
    case ("HistoryExtension", "maxDepth"):
      try getExtensionDependencyFromEditor(editor, HistoryExtension).output.maxDepth.set(
        try intOrNull(value))
    case ("ListExtension", "hasStrictIndent"):
      try getExtensionDependencyFromEditor(editor, ListExtension).output.hasStrictIndent.set(
        try bool(value))
    case ("ListExtension", "shouldPreserveNumbering"):
      try getExtensionDependencyFromEditor(editor, ListExtension).output.shouldPreserveNumbering
        .set(try bool(value))
    case ("NormalizeInlineElementsExtension", "disabled"):
      try getExtensionDependencyFromEditor(editor, NormalizeInlineElementsExtension).output
        .disabled.set(try bool(value))
    case ("CodeIndentExtension", "disabled"):
      try getExtensionDependencyFromEditor(editor, CodeIndentExtension).output.disabled.set(
        try bool(value))
    case ("CodeIndentExtension", "tabSize"):
      try getExtensionDependencyFromEditor(editor, CodeIndentExtension).output.tabSize.set(
        try intOrNull(value))
    case ("TabIndentationExtension", "disabled"):
      try getExtensionDependencyFromEditor(editor, TabIndentationExtension).output.disabled.set(
        try bool(value))
    case ("TabIndentationExtension", "maxIndent"):
      try getExtensionDependencyFromEditor(editor, TabIndentationExtension).output.maxIndent.set(
        try intOrNull(value))
    case ("BlockIdExtension", "disabled"):
      try getExtensionDependencyFromEditor(editor, BlockIdExtension).output.disabled.set(
        try bool(value))
    case ("NodeStructureExtension", "disabled"):
      try getExtensionDependencyFromEditor(editor, NodeStructureExtension).output.disabled.set(
        try bool(value))
    case ("SelectBlockExtension", "disabled"):
      try getExtensionDependencyFromEditor(editor, SelectBlockExtension).output.disabled.set(
        try bool(value))
    case ("SelectBlockExtension", "cascadeSelection"):
      try getExtensionDependencyFromEditor(editor, SelectBlockExtension).output.cascadeSelection
        .set(try bool(value))
    case ("NestedEditorExtension", "inheritEditableFromParent"):
      try getExtensionDependencyFromEditor(editor, NestedEditorExtension).output
        .inheritEditableFromParent.set(try bool(value))
    case ("AriaLiveRegionExtension", "politeness"):
      try getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension).output.politeness.set(
        try politeness(value))
    case ("HistoryAnnounceExtension", let field):
      let o = try getExtensionDependencyFromEditor(editor, HistoryAnnounceExtension).output
      switch field {
      case "undone": try o.undone.set(try string(value))
      case "redone": try o.redone.set(try string(value))
      case "disabled": try o.disabled.set(try bool(value))
      default: throw ScriptError("no signal \(name) of \(key)")
      }
    case ("EditorModeAnnounceExtension", let field):
      let o = try getExtensionDependencyFromEditor(editor, EditorModeAnnounceExtension).output
      switch field {
      case "editable": try o.editable.set(try string(value))
      case "readOnly": try o.readOnly.set(try string(value))
      case "disabled": try o.disabled.set(try bool(value))
      default: throw ScriptError("no signal \(name) of \(key)")
      }
    case ("HeadingAnnounceExtension", let field):
      let o = try getExtensionDependencyFromEditor(editor, HeadingAnnounceExtension).output
      switch field {
      case "created": try o.created.set(try string(value))
      case "destroyed": try o.destroyed.set(try string(value))
      case "disabled": try o.disabled.set(try bool(value))
      default: throw ScriptError("no signal \(name) of \(key)")
      }
    case ("AutoLinkAnnounceExtension", let field):
      let o = try getExtensionDependencyFromEditor(editor, AutoLinkAnnounceExtension).output
      switch field {
      case "created": try o.created.set(try string(value))
      case "createdMany": try o.createdMany.set(try string(value))
      case "destroyed": try o.destroyed.set(try string(value))
      case "destroyedMany": try o.destroyedMany.set(try string(value))
      case "disabled": try o.disabled.set(try bool(value))
      default: throw ScriptError("no signal \(name) of \(key)")
      }
    case (let flavorKey, let field) where markFlavor(flavorKey) != nil:
      let o = try getExtensionDependencyFromEditor(editor, markFlavor(flavorKey)!).output
      switch field {
      case "disabled": try o.disabled.set(try bool(value))
      case "exportIDsToHTML": try o.exportIDsToHTML.set(try bool(value))
      default: throw ScriptError("no signal \(name) of \(key)")
      }
    default:
      if key == "HistoryExtension" || realExtension(key) != nil {
        throw ScriptError("no signal \(name) of \(key)")
      }
      return false
    }
    return true
  }

  private func strings(_ value: JSONValue?) -> [String] {
    (value?.arrayValue ?? []).compactMap(\.stringValue)
  }

  /// `testExtension(ctx, spec)`.
  private func make(_ spec: JSONObject) throws -> TestExtension {
    guard let name = spec["name"]?.stringValue else {
      throw ScriptError("an extension needs a name")
    }
    let phases = spec["phases"] == nil ? extensionPhases : strings(spec["phases"])
    let flag = { (key: String) in spec[key]?.boolValue == true }
    let log = { [unowned self] (entry: [JSONValue]) in self.log(.array(entry)) }
    let rootText = { (editor: Editor) -> JSONValue in
      .string(try editor.getEditorState().read { try getRoot().getTextContent() })
    }
    var editorConfig = InitialEditorConfig(
      namespace: spec["namespace"]?.stringValue, editable: spec["editable"]?.boolValue)
    switch spec["initialEditorState"] {
    case nil: break
    case .null?: editorConfig.initialEditorState = InitialEditorStateType.none
    case .object(let initial)?:
      if let json = initial["json"] {
        editorConfig.initialEditorState = .json(json.stringify())
      } else if let text = initial["text"]?.stringValue {
        editorConfig.initialEditorState = .update { _ in
          try getRoot().append(try createParagraphNode().append(try createTextNode(text)))
        }
      } else {
        throw ScriptError("bad initialEditorState")
      }
    default: throw ScriptError("bad initialEditorState")
    }
    if let theme = spec["theme"]?.objectValue {
      editorConfig.theme = Dictionary(theme.map { ($0.0, $0.1) }, uniquingKeysWith: { $1 })
    }
    var nodes: [LexicalNodeConfig] = []
    for type in strings(spec["nodes"]) {
      switch type {
      case "decorator-text": nodes.append(.klass(DecoratorTextNode.self))
      case "horizontalrule": nodes.append(.klass(HorizontalRuleNode.self))
      case "autolink": nodes.append(.klass(AutoLinkNode.self))
      case "mark": nodes.append(.klass(MarkNode.self))
      case "overflow": nodes.append(.klass(OverflowNode.self))
      case "test-box": nodes.append(.klass(TestBoxNode.self))
      case "test-sub-box": nodes.append(.klass(TestSubBoxNode.self))
      case "test-tag": nodes.append(.klass(TestTagNode.self))
      case "test-sub-tag": nodes.append(.klass(TestSubTagNode.self))
      case "test-text": nodes.append(.klass(TestTextNode.self))
      // `textReplacement`'s.
      case "replace-text": nodes.append(.replacement(textReplacement(false)))
      case "replace-text-legacy": nodes.append(.replacement(textReplacement(true)))
      default: throw ScriptError("unknown node type \(type)")
      }
    }
    var mergeConfig: ((TestConfig, Partial<TestConfig>) throws -> TestConfig)?
    if spec["mergeConfig"]?.stringValue == "concat" {
      mergeConfig = { config, overrides in
        var merged = shallowMergeConfig(config, overrides)
        for key in config.keys where overrides.has(\TestConfig[key]) {
          if case .array(let a)? = config[key],
            case .array(let b)?? = overrides[dynamicMember: \TestConfig[key]]
          {
            merged[key] = .array(a + b)
          }
        }
        return merged
      }
    }
    let inspect = flag("inspect")
    let initPeers = strings(spec["initPeers"])
    let buildPeers = strings(spec["buildPeers"])
    let lookups = strings(spec["lookups"])
    let initFn: ((inout InitialEditorConfig, TestConfig, ExtensionInitState) throws -> Any)? =
      phases.contains("init")
      ? { [unowned self] editorConfig, config, state in
        log(["init", .string(name), .object(config)])
        if inspect {
          let types = try getKnownTypesAndNodes(editorConfig.nodes ?? []).types
          log(["knownTypes", .string(name), .array(types.array.map { .string($0) })])
          log([
            "dependents", .string(name),
            .array(state.getDirectDependentNames().array.map { .string($0) }),
          ])
          log([
            "peerNames", .string(name), .array(state.getPeerNameSet().array.map { .string($0) }),
          ])
        }
        for peer in initPeers {
          let dep = try state.getPeer(peer, config: TestConfig.self)
          log(["initPeer", .string(name), .string(peer), dep.map { .object($0.config) } ?? .null])
        }
        for key in lookups {
          let dep = try state.getDependency(try self.of(key))
          log(["initDependency", .string(name), .string(key), .object(dep.config)])
        }
        return JSONValue.object(JSONObject([("init", .string(name))]))
      } : nil
    let signals = flag("signals")
    let buildFn: ((Editor, TestConfig, ExtensionBuildState<Any>) throws -> Any)? =
      phases.contains("build")
      ? { [unowned self] _, config, state in
        log(["build", .string(name), .object(config)])
        for peer in buildPeers {
          let dep = try state.getPeer(peer)
          log([
            "buildPeer", .string(name), .string(peer),
            (dep?.config as? TestConfig).map { .object($0) } ?? .null,
          ])
        }
        for key in lookups {
          let dep = try state.getDependency(try self.of(key))
          log(["buildDependency", .string(name), .string(key), .object(dep.config)])
        }
        return signals ? namedSignals(config) : ()
      } : nil
    let (initResult, abort, listen, editorState) = (
      flag("initResult"), flag("abort"), flag("listen"), flag("editorState")
    )
    let registerFn: ((Editor, TestConfig, ExtensionRegisterState<Any, Any>) throws -> () -> Void)? =
      phases.contains("register")
      ? { [unowned self] editor, config, state in
        log(["register", .string(name), .object(config), try rootText(editor)])
        if initResult {
          log(["initResult", .string(name), try state.getInitResult() as? JSONValue ?? .null])
        }
        var cleanups: [() -> Void] = []
        if abort {
          try state.getSignal().addAbortListener { log(["aborted", .string(name)]) }
        }
        if listen {
          cleanups.append(
            editor.registerUpdateListener { (payload: UpdateListenerPayload) in
              log([
                "update", .string(name), .array(Array(payload.tags).sorted().map { .string($0) }),
              ])
            })
        }
        let command = self.command("EXTENSION_COMMAND")
        if signals {
          guard let output = state.getOutput() as? NamedSignals<TestConfig> else {
            throw ScriptError("no output")
          }
          let disabled = output[dynamicMember: \TestConfig["disabled"]]
          let label = output[dynamicMember: \TestConfig["label"]]
          let summary = computed { () -> String in
            if truthy(disabled.value) { return "off" }
            return "on:\(template(label.value))"
          }
          cleanups.append(
            try effect { log(["summary", .string(name), .string(summary.value)]) })
          cleanups.append(
            try effect { () -> (() -> Void)? in
              if truthy(disabled.value) { return nil }
              let current = untracked { label.value } ?? .null
              log(["enable", .string(name), current])
              let unregister = editor.registerCommand(
                command,
                { payload, _ in
                  log(["handled", .string(name), current, payload])
                  return false
                }, COMMAND_PRIORITY_EDITOR)
              return {
                log(["disable", .string(name)])
                unregister()
              }
            })
        } else {
          cleanups.append(
            editor.registerCommand(
              command,
              { payload, _ in
                log(["handled", .string(name), payload])
                return false
              }, COMMAND_PRIORITY_EDITOR))
        }
        if editorState {
          let signal = try state.getDependency(EditorStateExtension).output
          cleanups.append(
            try effect {
              let text = try signal.value.read { try getRoot().getTextContent() }
              log(["editorState", .string(name), .string(text)])
            })
        }
        return {
          log(["unregister", .string(name)])
          for cleanup in cleanups.reversed() { cleanup() }
        }
      } : nil
    let afterRegistrationFn:
      ((Editor, TestConfig, ExtensionRegisterState<Any, Any>) throws -> () -> Void)? =
        phases.contains("afterRegistration")
        ? { editor, config, _ in
          log(["afterRegistration", .string(name), .object(config), try rootText(editor)])
          return { log(["afterUnregister", .string(name)]) }
        } : nil
    return TestExtension(
      name: name, config: spec["config"]?.objectValue ?? JSONObject(),
      conflictsWith: strings(spec["conflictsWith"]),
      dependencies: try (spec["dependencies"]?.arrayValue ?? []).map { try arg($0) },
      peerDependencies: try (spec["peerDependencies"]?.arrayValue ?? []).map { peer in
        guard let peerName = peer.arrayValue?.first?.stringValue else {
          throw ScriptError("bad peer dependency")
        }
        guard let config = peer.arrayValue?.dropFirst().first?.objectValue else {
          return declarePeerDependency(peerName)
        }
        return declarePeerDependency(peerName, config: TestConfig.self) { partial in
          partial = testPartial(config)
        }
      },
      mergeConfig: mergeConfig, nodes: nodes.isEmpty ? nil : { nodes },
      editorConfig: editorConfig, init: initFn, build: buildFn, register: registerFn,
      afterRegistration: afterRegistrationFn)
  }

  /// `runExtensionStep(step, ctx)`.
  func runStep(_ step: JSONValue, _ editor: Editor) throws {
    let object = step.objectValue ?? JSONObject()
    if object["dispose"] != nil {
      editor.dispose()
    } else if let entries = object["setSignals"]?.arrayValue {
      try batch {
        for entry in entries {
          guard let parts = entry.arrayValue, parts.count == 3, let key = parts[0].stringValue,
            let signalName = parts[1].stringValue
          else { throw ScriptError("bad setSignals") }
          if try setRealSignal(editor, key, signalName, parts[2]) { continue }
          let output = try getExtensionDependencyFromEditor(editor, try of(key)).output
          guard let signals = output as? NamedSignals<TestConfig> else {
            throw ScriptError("no signals of \(key)")
          }
          try signals[dynamicMember: \TestConfig[signalName]].set(parts[2])
        }
      }
    } else if let names = object["hasExtensions"]?.arrayValue {
      let builder = try LexicalBuilder.fromEditor(editor)
      for name in names.compactMap(\.stringValue) {
        log(.array(["hasExtension", .string(name), .bool(builder.hasExtensionByName(name))]))
      }
    } else if object["logHistory"] != nil {
      let output = try getExtensionDependencyFromEditor(editor, HistoryExtension).output
      let historyState = output.historyState.value
      log(
        .array([
          "history", .bool(output.canUndo.value), .bool(output.canRedo.value),
          .number(Double(historyState.undoStack.count)),
          .number(Double(historyState.redoStack.count)),
        ]))
    } else if let key = object["watchNodeKey"]?.stringValue {
      guard watchers[key] == nil else { throw ScriptError("already watching \(key)") }
      let selected = try getExtensionDependencyFromEditor(editor, NodeSelectionExtension).output
        .watchNodeKey(key)
      let log = self.log
      watchers[key] = try effect { log(["nodeSelected", .string(key), .bool(selected.value)]) }
    } else if let spec = object["registerMarkIDsListener"]?.objectValue {
      let name = spec["name"]?.stringValue ?? ""
      let key = spec["flavor"]?.stringValue ?? ""
      guard markIDsListeners[name] == nil else { throw ScriptError("already listening as \(name)") }
      guard let flavor = markFlavor(key) else { throw ScriptError("unknown mark flavor \(key)") }
      let log = self.log
      markIDsListeners[name] = try registerMarkIDsListener(editor, flavor) { change in
        log(
          .array([
            "markIDsChange", .string(name), jsSorted(change.added), jsSorted(change.removed),
            jsSorted(change.ids), jsSorted(change.tags),
          ]))
      }
    } else if let name = object["unregisterMarkIDsListener"]?.stringValue {
      guard let dispose = markIDsListeners.removeValue(forKey: name) else {
        throw ScriptError("no listener \(name)")
      }
      dispose()
    } else if let key = object["logMarkIDNodeKeys"]?.stringValue {
      guard let flavor = markFlavor(key) else { throw ScriptError("unknown mark flavor \(key)") }
      let keys = try getExtensionDependencyFromEditor(editor, flavor).output.markIDNodeKeys.peek()
      let ids = keys.keysArray.sorted { $0.utf16.lexicographicallyPrecedes($1.utf16) }
      log(
        .array([
          "markIDNodeKeys", .string(key),
          .array(ids.map { .array([.string($0), jsSorted(keys.get($0) ?? [])]) }),
        ]))
    } else if let key = object["unwatchNodeKey"]?.stringValue {
      guard let dispose = watchers.removeValue(forKey: key) else {
        throw ScriptError("not watching \(key)")
      }
      dispose()
    } else if object["logIME"] != nil {
      let output = try getExtensionDependencyFromEditor(editor, IMEExtension).output
      log(
        .array([
          "ime", output.compositionKey.peek().map { .string($0) } ?? .null,
          output.composingTextNode.peek().map { .string($0.getKey()) } ?? .null,
        ]))
    } else if let key = object["logDependency"]?.stringValue {
      let dep = try getExtensionDependencyFromEditor(editor, try of(key))
      log(.array(["dependency", .string(key), .object(dep.config)]))
    } else if let peerName = object["logPeer"]?.stringValue {
      let dep = try getPeerDependencyFromEditor(editor, peerName)
      log(
        .array([
          "peer", .string(peerName), (dep?.config as? TestConfig).map { .object($0) } ?? .null,
        ]))
    } else {
      throw ScriptError("unknown step \(step.stringify())")
    }
  }
}

/// JS's truthiness of a JSON value (`undefined` is `nil`).
private func truthy(_ value: JSONValue?) -> Bool {
  switch value {
  case nil, .null?: false
  case .bool(let b)?: b
  case .number(let n)?: n != 0 && !n.isNaN
  case .string(let s)?: !s.isEmpty
  default: true
  }
}

/// `${value}` of a JSON value.
private func template(_ value: JSONValue?) -> String {
  guard let value else { return "undefined" }
  if case .string(let s) = value { return s }
  return value.stringify()
}

/// `RichTextConfig['escapeFormatTriggers']` from JSON: each format's
/// `TriggerConfig` (booleans by trigger), or `null`.
private func escapeFormatTriggers(_ value: JSONValue) throws -> EscapeFormatTriggerConfig {
  guard let object = value.objectValue else { throw ScriptError("not triggers: \(value)") }
  var config: EscapeFormatTriggerConfig = [:]
  for (format, triggers) in object {
    if triggers == .null {
      config.updateValue(nil, forKey: format)
      continue
    }
    guard let triggers = triggers.objectValue else { throw ScriptError("not triggers: \(value)") }
    var trigger = TriggerConfig()
    for (name, on) in triggers {
      guard let on = on.boolValue else { throw ScriptError("not a trigger: \(on)") }
      switch name {
      case "enter": trigger.enter = on
      case "click": trigger.click = on
      case "arrow": trigger.arrow = on
      case "space": trigger.space = on
      case "tab": trigger.tab = on
      case "onlyAtBoundary": trigger.onlyAtBoundary = on
      default: throw ScriptError("unknown trigger \(name)")
      }
    }
    config.updateValue(trigger, forKey: format)
  }
  return config
}
