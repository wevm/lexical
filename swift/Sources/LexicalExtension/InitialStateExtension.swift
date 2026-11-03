import Lexical

/// `EditorSetOptions`: the options of `setEditorState`.
public struct EditorSetOptions {
  public var tag: UpdateTag?
  public init(tag: UpdateTag? = nil) { self.tag = tag }
}

/// `InitialStateConfig`.
public struct InitialStateConfig {
  public var updateOptions: EditorUpdateOptions
  public var setOptions: EditorSetOptions

  public init(
    updateOptions: EditorUpdateOptions = EditorUpdateOptions(tag: [HISTORY_MERGE_TAG]),
    setOptions: EditorSetOptions = EditorSetOptions(tag: HISTORY_MERGE_TAG)
  ) {
    self.updateOptions = updateOptions
    self.setOptions = setOptions
  }
}

/// The init result of `InitialStateExtension`: mutable, as in JS.
public final class InitialStateInit {
  public let initialEditorState: InitialEditorStateType
  public var initialized = false

  init(_ initialEditorState: InitialEditorStateType) {
    self.initialEditorState = initialEditorState
  }
}

private func defaultInitializer(_: Editor) throws {
  let root = try getRoot()
  if try root.isEmpty() {
    try root.append(try createParagraphNode())
  }
}

/// `InitialStateExtension`: registers the core nodes and fills a new
/// editor from `initialEditorState` (by default, one empty paragraph).
public let InitialStateExtension = defineExtension(
  name: "@lexical/extension/InitialState",
  config: InitialStateConfig(),
  nodes: {
    [
      .klass(RootNode.self), .klass(TextNode.self), .klass(LineBreakNode.self),
      .klass(TabNode.self), .klass(ParagraphNode.self),
    ]
  },
  init: { editorConfig, _, _ in
    InitialStateInit(editorConfig.initialEditorState ?? .update(defaultInitializer))
  },
  afterRegistration: { editor, config, state in
    let initResult = try state.getInitResult()
    if !initResult.initialized {
      initResult.initialized = true
      switch initResult.initialEditorState {
      case .none:
        break
      case .editorState(let editorState):
        try editor.setEditorState(editorState, tag: config.setOptions.tag)
      case .update(let fn):
        try editor.update({ try fn(editor) }, config.updateOptions)
      case .json(let json):
        try editor.setEditorState(editor.parseEditorState(json), tag: config.setOptions.tag)
      case .serialized(let object):
        try editor.setEditorState(editor.parseEditorState(object), tag: config.setOptions.tag)
      }
    }
    return {}
  })
