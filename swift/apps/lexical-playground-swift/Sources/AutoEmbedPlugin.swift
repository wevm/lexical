import Foundation
import Lexical
import LexicalAutoEmbed
import LexicalMenu
import SwiftUI

// Port of packages/lexical-playground/src/plugins/AutoEmbedPlugin: the
// playground's embed configs (YouTube, X and Figma), the menu that offers
// to embed a pasted link (`LexicalAutoEmbedPlugin`, from `LexicalAutoEmbed`,
// drawn as the default node menu), and `AutoEmbedDialog`, which the Insert
// menu and the component picker open through `INSERT_EMBED_COMMAND`. The
// web hides the menu in a pull quote's attribution and a review's author,
// which the port does not have.

/// `PlaygroundEmbedConfig`.
struct PlaygroundEmbedConfig: EmbedConfig {
  let type: String
  /// Human readable name of the embedded content, e.g. Tweet or Google Map.
  let contentName: String
  /// An SF Symbol standing in for the playground's `i.icon` image.
  let symbol: String
  /// An example of a matching url.
  let exampleUrl: String
  /// For extra searching.
  let keywords: [String]
  let parse: @MainActor (String) -> EmbedMatchResult?
  let command: LexicalCommand<String>

  func parseUrl(_ url: String) async -> EmbedMatchResult? { parse(url) }

  func insertNode(_ editor: Editor, _ result: EmbedMatchResult) throws {
    _ = try editor.dispatchCommand(command, result.id)
  }
}

/// `regExp.exec(text)`: the first match's groups, `nil` for one that did
/// not take part.
private func exec(_ pattern: String, _ text: String) -> [String?]? {
  guard let regex = try? NSRegularExpression(pattern: pattern),
    let match = regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text))
  else { return nil }
  return (0..<match.numberOfRanges).map {
    Range(match.range(at: $0), in: text).map { String(text[$0]) }
  }
}

let YoutubeEmbedConfig = PlaygroundEmbedConfig(
  type: "youtube-video", contentName: "Youtube Video", symbol: "play.rectangle",
  exampleUrl: "https://www.youtube.com/watch?v=jNQXAC9IVRw", keywords: ["youtube", "video"],
  parse: { url in
    let match = exec(#"^.*(youtu\.be\/|v\/|u\/\w\/|embed\/|watch\?v=|&v=)([^#&?]*).*"#, url)
    guard let id = match?[2] ?? nil, id.utf16.count == 11 else { return nil }
    return EmbedMatchResult(url: url, id: id)
  }, command: INSERT_YOUTUBE_COMMAND)

let TwitterEmbedConfig = PlaygroundEmbedConfig(
  type: "tweet", contentName: "X(Tweet)", symbol: "xmark",
  exampleUrl: "https://x.com/jack/status/20", keywords: ["tweet", "twitter", "x"],
  parse: { text in
    guard let match = exec(#"^https:\/\/(twitter|x)\.com\/(#!\/)?(\w+)\/status(es)*\/(\d+)"#, text),
      let id = match[5], let url = match[1]
    else { return nil }
    return EmbedMatchResult(url: url, id: id)
  }, command: INSERT_TWEET_COMMAND)

let FigmaEmbedConfig = PlaygroundEmbedConfig(
  type: "figma", contentName: "Figma Document", symbol: "pencil.and.outline",
  exampleUrl: "https://www.figma.com/file/LKQ4FJ4bTnCSjedbRpk931/Sample-File",
  keywords: ["figma", "figma.com", "mock-up"],
  parse: { text in
    guard
      let match = exec(
        #"https:\/\/([\w.-]+\.)?figma.com\/(file|proto)\/([0-9a-zA-Z]{22,128})(?:\/.*)?$"#, text),
      let id = match[3], let url = match[0]
    else { return nil }
    return EmbedMatchResult(url: url, id: id)
  }, command: INSERT_FIGMA_COMMAND)

/// `EmbedConfigs`, in the web's order.
let EmbedConfigs = [TwitterEmbedConfig, YoutubeEmbedConfig, FigmaEmbedConfig]

/// `AutoEmbedPlugin`: the menu at a pasted link, with Dismiss and Embed,
/// and `INSERT_EMBED_COMMAND`, which opens the dialog for its config
/// through `showModal`.
@MainActor
func registerAutoEmbedPlugin(
  _ editor: Editor, showModal: @escaping (PlaygroundEmbedConfig) -> Void
) throws -> (host: TypeaheadMenuHost<AutoEmbedOption>, unregister: () -> Void) {
  let (menu, unregister) = try registerAutoEmbed(
    editor, embedConfigs: EmbedConfigs,
    getMenuOptions: { config, embed, dismiss in
      [
        AutoEmbedOption("Dismiss", onSelect: { _ in dismiss() }),
        AutoEmbedOption("Embed \(config.contentName)", onSelect: { _ in embed() }),
      ]
    }, onOpenEmbedModalForConfig: showModal)
  let host = TypeaheadMenuHost(menu: menu.menu) {
    TypeaheadMenuRow(id: $0.key, title: $0.title ?? "", symbol: nil, glyph: nil)
  }
  return (host, unregister)
}

/// `AutoEmbedDialog`: a URL field with the config's example as its
/// placeholder, checked 200 ms after the last keystroke (`debounce`), and
/// Embed, enabled once `parseUrl` accepts the URL.
struct AutoEmbedDialog: View {
  let embedConfig: PlaygroundEmbedConfig
  let editor: Editor
  @Environment(\.dismiss) private var dismiss
  @State private var text = ""
  @State private var embedResult: EmbedMatchResult?
  @State private var validation: Task<Void, Never>?

  var body: some View {
    NavigationStack {
      Form {
        TextField("URL", text: $text, prompt: Text(embedConfig.exampleUrl))
          .accessibilityIdentifier("\(embedConfig.type)-embed-modal-url")
          #if os(iOS)
            .textInputAutocapitalization(.never)
            .keyboardType(.URL)
          #endif
          .autocorrectionDisabled()
          .onChange(of: text) { _, value in validate(value) }
        Button("Embed") {
          guard let embedResult else { return }
          try? embedConfig.insertNode(editor, embedResult)
          dismiss()
        }
        .disabled(embedResult == nil)
        .accessibilityIdentifier("\(embedConfig.type)-embed-modal-submit-btn")
      }
      .navigationTitle("Embed \(embedConfig.contentName)")
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
      }
    }
    .frame(minWidth: 420, minHeight: 160)
  }

  private func validate(_ inputText: String) {
    validation?.cancel()
    validation = Task {
      try? await Task.sleep(for: .milliseconds(200))
      guard !Task.isCancelled else { return }
      if inputText.firstMatch(of: URL_MATCHER) != nil {
        embedResult = await embedConfig.parseUrl(inputText)
      } else {
        embedResult = nil
      }
    }
  }
}
