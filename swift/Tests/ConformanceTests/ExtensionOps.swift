import Foundation
import Lexical
import LexicalExtension

// Scripts with `extensions` (ops.ts): the editor is built with
// `buildEditorFromExtensions` from test extensions that ops.ts's
// `testExtension` makes from JSON (`ExtensionSpec`), and a few of
// @lexical/extension's own (`REAL_EXTENSIONS`). Each test extension logs
// every phase it runs, so the log is the order the builder ran them in.

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

/// `REAL_EXTENSIONS` in ops.ts.
private func realExtension(_ key: String) -> (any AnyLexicalExtensionArgument)? {
  switch key {
  case "ClearEditorExtension": ClearEditorExtension
  case "DecoratorTextExtension": DecoratorTextExtension
  case "EditorStateExtension": EditorStateExtension
  default: nil
  }
}

private let extensionPhases = ["init", "build", "register", "afterRegistration"]

/// The test extensions of one script: ops.ts's `extensionOf`, which makes
/// each on first use.
final class TestExtensions {
  let defs: JSONObject
  var made: [String: TestExtension] = [:]
  var log: (JSONValue) -> Void = { _ in }
  var command: (String) -> LexicalCommand<JSONValue> = { LexicalCommand($0) }

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
      return try realExtension(key) ?? of(key)
    }
    guard let parts = arg.arrayValue, let key = parts.first?.stringValue else {
      throw ScriptError("bad extension argument \(arg.stringify())")
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
    var editorConfig = InitialEditorConfig()
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
    var nodes: [LexicalNodeConfig] = []
    for type in strings(spec["nodes"]) {
      switch type {
      case "decorator-text": nodes.append(.klass(DecoratorTextNode.self))
      case "horizontalrule": nodes.append(.klass(HorizontalRuleNode.self))
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
          let output = try getExtensionDependencyFromEditor(editor, try of(key)).output
          guard let signals = output as? NamedSignals<TestConfig> else {
            throw ScriptError("no signals of \(key)")
          }
          try signals[dynamicMember: \TestConfig[signalName]].set(parts[2])
        }
      }
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
