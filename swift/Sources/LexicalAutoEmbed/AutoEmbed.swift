import Lexical
import LexicalLink
import LexicalMenu
import LexicalUtils

// Port of packages/lexical-react/src/LexicalAutoEmbedPluginUtils.ts and the
// logic of LexicalAutoEmbedPlugin.tsx: a pasted link that an embed config
// recognizes (a YouTube video, a tweet) opens a node menu at it that offers
// to replace it with the embed. React keeps the match in component state
// and renders `LexicalNodeMenuPlugin`; here `registerAutoEmbed` keeps it
// and drives a `NodeMenu`, which the app's view draws.

/// `EmbedMatchResult`: the matched `url`, the `id` of the embedded
/// resource, and optional provider data.
public struct EmbedMatchResult: Sendable {
  public var url: String
  public var id: String
  public var data: (any Sendable)?

  public init(url: String, id: String, data: (any Sendable)? = nil) {
    self.url = url
    self.id = id
    self.data = data
  }
}

/// `EmbedConfig`: a kind of embed, with its `type`, `parseUrl`, which
/// decides whether a URL is one and extracts its data (JS may return a
/// promise; here it is `async`), and `insertNode`, which inserts its node.
@MainActor
public protocol EmbedConfig {
  var type: String { get }
  func parseUrl(_ url: String) async -> EmbedMatchResult?
  func insertNode(_ editor: Editor, _ result: EmbedMatchResult) throws
}

/// `URL_MATCHER`: a general-purpose URL expression for a `parseUrl`.
nonisolated(unsafe) public let URL_MATCHER = try! Regex(
  #"((https?:\/\/(www\.)?)|(www\.))[-a-zA-Z0-9@:%._+~#=]{1,256}\.[a-zA-Z0-9()]{1,6}\b([-a-zA-Z0-9()@:%_+.~#?&\/\/=]*)"#
)

/// `INSERT_EMBED_COMMAND`: starts inserting the embed whose config has
/// the payload as its `type`.
public let INSERT_EMBED_COMMAND = LexicalCommand<String>("INSERT_EMBED_COMMAND")

/// `AutoEmbedOption`: an item of the embed menu, with what choosing it does.
open class AutoEmbedOption: MenuOption {
  public let onSelect: (LexicalNode?) throws -> Void

  public init(_ title: String, onSelect: @escaping (LexicalNode?) throws -> Void) {
    self.onSelect = onSelect
    super.init(title)
    self.title = title
  }
}

/// `/^\S+$/`: JS's `\s` is Unicode's white space and U+FEFF.
private func isSingleToken(_ text: String) -> Bool {
  !text.isEmpty && !text.unicodeScalars.contains { $0.properties.isWhitespace || $0 == "\u{FEFF}" }
}

/// `LexicalAutoEmbedPlugin`: watches for links that a paste of one token
/// created and, when one of `embedConfigs` parses the URL (the last one
/// that does wins), opens the returned menu at the link with
/// `getMenuOptions`' options. Its `embed` replaces the link with the embed
/// (unless it was removed or its URL changed); `dismiss` closes the menu.
/// `onOpenEmbedModalForConfig` handles `INSERT_EMBED_COMMAND`, and
/// `disableMenuFor` suppresses the menu for a link where it returns true.
/// Returns the menu and a function that unregisters it.
@MainActor
public func registerAutoEmbed<Config: EmbedConfig>(
  _ editor: Editor, embedConfigs: [Config],
  getMenuOptions:
    @escaping (
      _ activeEmbedConfig: Config, _ embed: @escaping () -> Void, _ dismiss: @escaping () -> Void
    ) -> [AutoEmbedOption],
  onOpenEmbedModalForConfig: ((Config) -> Void)? = nil,
  menuCommandPriority: Int = COMMAND_PRIORITY_LOW,
  disableMenuFor: ((LexicalNode) throws -> Bool)? = nil
) throws -> (menu: NodeMenu<AutoEmbedOption>, unregister: () -> Void) {
  let state = AutoEmbedState<Config>(
    editor, embedConfigs: embedConfigs, getMenuOptions: getMenuOptions,
    disableMenuFor: disableMenuFor, commandPriority: menuCommandPriority)
  var removers = [
    editor.registerCommand(
      PASTE_COMMAND,
      { [weak state] payload, _ in
        guard case .clipboard(let event) = payload, let data = event.clipboardData else {
          state?.isSingleTokenPaste = false
          return false
        }
        let isSingleTokenPaste = isSingleToken(data.getData("text/plain"))
        state?.isSingleTokenPaste = isSingleTokenPaste
        if isSingleTokenPaste {
          try onUpdate { [weak state] in state?.isSingleTokenPaste = false }
        }
        return false
      }, COMMAND_PRIORITY_BEFORE_EDITOR)
  ]
  for klass in [LinkNode.self, AutoLinkNode.self] as [LexicalNode.Type] {
    removers.append(
      try editor.registerMutationListener(
        klass,
        { [weak state] mutations, payload in
          MainActor.assumeIsolated { state?.mutated(mutations, payload.updateTags) }
        }, skipInitialization: true))
  }
  if let onOpenEmbedModalForConfig {
    removers.append(
      editor.registerCommand(
        INSERT_EMBED_COMMAND,
        { type, _ in
          MainActor.assumeIsolated {
            guard let config = embedConfigs.first(where: { $0.type == type }) else { return false }
            onOpenEmbedModalForConfig(config)
            return true
          }
        }, COMMAND_PRIORITY_EDITOR))
  }
  return (
    state.nodeMenu,
    {
      removers.forEach { $0() }
      state.reset()
    }
  )
}

/// The plugin's state: the current match, and the menu it drives.
@MainActor
private final class AutoEmbedState<Config: EmbedConfig> {
  struct Match {
    let embedConfig: Config
    let key: NodeKey
    let result: EmbedMatchResult
    let url: String
  }

  let editor: Editor
  let embedConfigs: [Config]
  let getMenuOptions: (Config, @escaping () -> Void, @escaping () -> Void) -> [AutoEmbedOption]
  let disableMenuFor: ((LexicalNode) throws -> Bool)?
  var nodeMenu: NodeMenu<AutoEmbedOption>!
  var embedMatch: Match?
  var isSingleTokenPaste = false
  /// Only the latest paste may open the menu, even when an older
  /// `parseUrl` returns after a newer one.
  var latestCheck = 0

  init(
    _ editor: Editor, embedConfigs: [Config],
    getMenuOptions:
      @escaping (Config, @escaping () -> Void, @escaping () -> Void) ->
      [AutoEmbedOption],
    disableMenuFor: ((LexicalNode) throws -> Bool)?, commandPriority: Int
  ) {
    self.editor = editor
    self.embedConfigs = embedConfigs
    self.getMenuOptions = getMenuOptions
    self.disableMenuFor = disableMenuFor
    nodeMenu = NodeMenu(
      editor,
      onSelectOption: { option, targetNode, closeMenu, _ in
        try option.onSelect(targetNode)
        closeMenu()
      }, commandPriority: commandPriority)
    nodeMenu.menu.onClose = { [weak self] in self?.reset() }
  }

  func reset() {
    guard embedMatch != nil else { return }
    embedMatch = nil
    nodeMenu.setNodeKey(nil)
    nodeMenu.menu.setOptions([])
  }

  func setEmbedMatch(_ match: Match) {
    embedMatch = match
    nodeMenu.setNodeKey(match.key)
    nodeMenu.menu.setOptions(
      getMenuOptions(
        match.embedConfig, { [weak self] in self?.embedLinkViaActiveEmbedConfig() },
        { [weak self] in self?.reset() }))
  }

  func mutated(_ mutations: JSMap<NodeKey, NodeMutation>, _ updateTags: JSSet<UpdateTag>) {
    for (key, mutation) in mutations {
      if mutation == "created" && updateTags.has(PASTE_TAG) && isSingleTokenPaste {
        Task { await checkIfLinkNodeIsEmbeddable(key) }
      } else if key == embedMatch?.key {
        reset()
      }
    }
  }

  func isMenuDisabled(_ linkNode: LexicalNode) throws -> Bool {
    try disableMenuFor?(linkNode) ?? false
  }

  /// `$getCurrentLink`: the link `key`, while it is attached and its URL is
  /// still `url`.
  func currentLink(_ key: NodeKey, _ url: String) throws -> LinkNode? {
    guard let link = try getNodeByKey(key) as? LinkNode, try link.isAttached(),
      try link.getURL() == url
    else { return nil }
    return link
  }

  func checkIfLinkNodeIsEmbeddable(_ key: NodeKey) async {
    latestCheck += 1
    let check = latestCheck
    let url: String? = try? editor.read(.latest) {
      guard let link = try getNodeByKey(key) as? LinkNode, try !isMenuDisabled(link) else {
        return nil
      }
      return try link.getURL()
    }
    guard let url else { return }
    // When several configs match, the last one wins, so they are checked
    // from the end, stopping at the first match.
    for embedConfig in embedConfigs.reversed() {
      let result = await embedConfig.parseUrl(url)
      if check != latestCheck { return }
      guard let result else { continue }
      // Edits made while `parseUrl` ran may not be committed yet, and the
      // link may have moved where the menu is disabled.
      let stillEmbeddable =
        (try? editor.read(.pending) {
          guard let link = try currentLink(key, url) else { return false }
          return try !isMenuDisabled(link)
        }) ?? false
      if stillEmbeddable {
        setEmbedMatch(Match(embedConfig: embedConfig, key: key, result: result, url: url))
      }
      return
    }
  }

  func embedLinkViaActiveEmbedConfig() {
    guard let match = embedMatch else { return }
    do {
      try editor.update { [self] in
        guard let link = try currentLink(match.key, match.url) else { return }
        if try getSelection() == nil { _ = try link.selectEnd() }
        try match.embedConfig.insertNode(editor, match.result)
        if try link.isAttached() { try link.remove() }
      }
    } catch {
      try? editor._onError(error)
    }
  }
}
