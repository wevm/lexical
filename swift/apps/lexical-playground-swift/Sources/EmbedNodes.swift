import Foundation
import Lexical
import LexicalDecoratorBlockNode
import LexicalExtension
import LexicalHTML
import LexicalUtils
import SwiftUI
import WebKit

// Ports of packages/lexical-playground/src/nodes/YouTubeNode.tsx,
// TweetNode.tsx and FigmaNode.tsx, and of plugins/YouTubeExtension,
// TwitterExtension and FigmaExtension: block embeds on `DecoratorBlockNode`,
// with their JSON, HTML and commands. Each draws its iframe (or, for a
// tweet, Twitter's widget) in a `WKWebView`, inside
// `BlockWithAlignableContents`' container, which aligns it and shows the
// theme's `embedBlockFocus` outline while it is selected.

/// `.PlaygroundEditorTheme__embedBlockFocus`: `outline: 2px solid rgb(60,
/// 132, 244)`.
private let embedBlockFocusColor = Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255)

/// `YouTubeNode`.
final class YouTubeNode: DecoratorBlockNode {
  var __id = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      YouTubeNode.self, type: "youtube", extends: DecoratorBlockNode.self,
      json: [SchemaField("videoID", stringValue(), field: \YouTubeNode.__id)])
  }

  /// The Swift playground imports HTML with `generateNodesFromDOM`, which
  /// `YouTubeExtension`'s rule does not reach, so the node converts its
  /// `<iframe>` itself as well.
  override class func importDOM() -> DOMConversionMap? {
    [
      "iframe": { node in
        guard let id = (node as? Lexical.DOMElement)?.getAttribute("data-lexical-youtube"),
          !id.isEmpty
        else { return nil }
        return DOMConversion(priority: 1) { _ in
          DOMConversionOutput(node: [try createYouTubeNode(id)])
        }
      }
    ]
  }

  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let element = getDocument().createElement("iframe")
    try element.setAttribute("data-lexical-youtube", __id)
    try element.setAttribute("width", "560")
    try element.setAttribute("height", "315")
    try element.setAttribute("src", "https://www.youtube-nocookie.com/embed/\(__id)")
    try element.setAttribute("frameborder", "0")
    try element.setAttribute(
      "allow",
      "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture")
    try element.setAttribute("allowfullscreen", "true")
    try element.setAttribute("title", "YouTube video")
    return DOMExportOutput(element: element)
  }

  func getId() throws -> String { try getLatest().__id }

  @discardableResult
  func setVideoID(_ videoID: String) throws -> Self {
    let writable = try getWritable()
    writable.__id = videoID
    return writable
  }

  override func getTextContent() throws -> String { "https://www.youtube.com/watch?v=\(__id)" }

  override func decorate(_ editor: Editor) throws -> Any? {
    let (key, format, id) = (getKey(), __format, __id)
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated {
      BlockWithAlignableContents(editor: editor, nodeKey: key, format: format) {
        EmbedWebView(
          html: iframeHTML(
            src: "https://www.youtube-nocookie.com/embed/\(id)",
            allow:
              "accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture",
            title: "YouTube video")
        )
        .frame(maxWidth: 560).frame(height: 315)
        .accessibilityLabel("YouTube video")
      }
    }
  }
}

/// `$createYouTubeNode(videoID)`.
func createYouTubeNode(_ videoID: String) throws -> YouTubeNode {
  try create(YouTubeNode.self).setVideoID(videoID)
}

/// `TweetNode`.
final class TweetNode: DecoratorBlockNode {
  var __id = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      TweetNode.self, type: "tweet", extends: DecoratorBlockNode.self,
      json: [SchemaField("id", stringValue(), field: \TweetNode.__id)])
  }

  /// As `YouTubeNode`'s, for `TwitterExtension`'s rule.
  override class func importDOM() -> DOMConversionMap? {
    [
      "div": { node in
        guard let id = (node as? Lexical.DOMElement)?.getAttribute("data-lexical-tweet-id"),
          !id.isEmpty
        else { return nil }
        return DOMConversion(priority: 1) { _ in
          DOMConversionOutput(node: [try createTweetNode(id)])
        }
      }
    ]
  }

  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let element = getDocument().createElement("div")
    try element.setAttribute("data-lexical-tweet-id", __id)
    try element.append(getDocument().createTextNode(getTextContent()))
    return DOMExportOutput(element: element)
  }

  func getId() throws -> String { try getLatest().__id }

  @discardableResult
  func setId(_ id: String) throws -> Self {
    let writable = try getWritable()
    writable.__id = id
    return writable
  }

  override func getTextContent() throws -> String { "https://x.com/i/web/status/\(__id)" }

  override func decorate(_ editor: Editor) throws -> Any? {
    let (key, format, id) = (getKey(), __format, __id)
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated {
      BlockWithAlignableContents(editor: editor, nodeKey: key, format: format) {
        TweetComponent(editor: editor, nodeKey: key, tweetID: id)
      }
    }
  }
}

/// `$createTweetNode(tweetID)`.
func createTweetNode(_ tweetID: String) throws -> TweetNode {
  try create(TweetNode.self).setId(tweetID)
}

/// `FigmaNode`.
final class FigmaNode: DecoratorBlockNode {
  var __id = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      FigmaNode.self, type: "figma", extends: DecoratorBlockNode.self,
      json: [SchemaField("documentID", stringValue(), field: \FigmaNode.__id)])
  }

  func getId() throws -> String { try getLatest().__id }

  @discardableResult
  func setDocumentID(_ documentID: String) throws -> Self {
    let writable = try getWritable()
    writable.__id = documentID
    return writable
  }

  override func getTextContent() throws -> String { "https://www.figma.com/file/\(__id)" }

  override func decorate(_ editor: Editor) throws -> Any? {
    let (key, format, id) = (getKey(), __format, __id)
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated {
      BlockWithAlignableContents(editor: editor, nodeKey: key, format: format) {
        // The web's template literal keeps its line continuation's
        // indentation in the URL.
        EmbedWebView(
          html: iframeHTML(
            src:
              "https://www.figma.com/embed?embed_host=lexical&url=        https://www.figma.com/file/\(id)",
            allow: nil, title: nil)
        )
        .frame(maxWidth: 560).frame(height: 315)
        .accessibilityLabel("Figma document")
      }
    }
  }
}

/// `$createFigmaNode(documentID)`.
func createFigmaNode(_ documentID: String) throws -> FigmaNode {
  try create(FigmaNode.self).setDocumentID(documentID)
}

/// `INSERT_YOUTUBE_COMMAND`, `INSERT_TWEET_COMMAND`, `INSERT_FIGMA_COMMAND`.
let INSERT_YOUTUBE_COMMAND = LexicalCommand<String>("INSERT_YOUTUBE_COMMAND")
let INSERT_TWEET_COMMAND = LexicalCommand<String>("INSERT_TWEET_COMMAND")
let INSERT_FIGMA_COMMAND = LexicalCommand<String>("INSERT_FIGMA_COMMAND")

nonisolated(unsafe) private let YouTubeImportRule = defineImportRule(
  name: "@lexical/playground/youtube",
  match: sel.tag("iframe").attr("data-lexical-youtube", try! Regex("^.+$"), capture: "id")
) { ctx, _, _ in [try createYouTubeNode(ctx.captures["id"]?[0] ?? "")] }

nonisolated(unsafe) private let TweetImportRule = defineImportRule(
  name: "@lexical/playground/tweet",
  match: sel.tag("div").attr("data-lexical-tweet-id", try! Regex("^.+$"), capture: "id")
) { ctx, _, _ in [try createTweetNode(ctx.captures["id"]?[0] ?? "")] }

/// `YouTubeExtension`, `TwitterExtension` and `FigmaExtension`, with the
/// `BlockWithAlignableContents` commands their components register.
let EmbedsExtension = defineExtension(
  name: "@lexical/playground-swift/Embeds",
  dependencies: [
    BlockWithAlignableContentsExtension,
    configExtension(DOMImportExtension) {
      $0.rules = [YouTubeImportRule, TweetImportRule].map(\.entry)
    },
  ],
  nodes: { [.klass(YouTubeNode.self), .klass(TweetNode.self), .klass(FigmaNode.self)] },
  register: { editor, _, _ in
    func insert(
      _ command: LexicalCommand<String>, _ create: @escaping (String) throws -> LexicalNode
    ) -> () -> Void {
      editor.registerCommand(
        command,
        { payload, _ in
          _ = try insertNodeToNearestRoot(create(payload))
          return true
        }, COMMAND_PRIORITY_EDITOR)
    }
    return mergeRegister([
      insert(INSERT_YOUTUBE_COMMAND, createYouTubeNode),
      insert(INSERT_TWEET_COMMAND, createTweetNode),
      insert(INSERT_FIGMA_COMMAND, createFigmaNode),
    ])
  })

/// `BlockWithAlignableContents`' container: as wide as the editor, its
/// content aligned by the node's `format` (`text-align`), outlined while
/// the node is selected. A click on it outside the embed selects the node
/// (`BlockWithAlignableContentsExtension`'s `CLICK_COMMAND`).
struct BlockWithAlignableContents<Content: View>: View {
  let editor: Editor
  let nodeKey: NodeKey
  let format: ElementFormatType
  @ViewBuilder let content: () -> Content
  @State private var selection = NodeSelectionState()

  private var alignment: Alignment {
    switch format {
    case "center": .center
    case "right", "end": .trailing
    default: .leading
    }
  }

  var body: some View {
    content()
      .frame(maxWidth: .infinity, alignment: alignment)
      .contentShape(Rectangle())
      .overlay {
        if selection.isSelected {
          Rectangle().strokeBorder(embedBlockFocusColor, lineWidth: 2).padding(-2)
        }
      }
      // The embed's web view takes its own clicks; the rest of the line
      // is the container's, as the web's `ref.current` target.
      .onTapGesture { click() }
      .accessibilityAction { click() }
      .onAppear { selection.observe(editor, nodeKey) }
  }

  private func click() {
    #if os(macOS)
      let shiftKey = NSEvent.modifierFlags.contains(.shift)
    #else
      let shiftKey = false
    #endif
    _ = try? editor.dispatchCommand(
      CLICK_COMMAND, MouseEvent("click", shiftKey: shiftKey, targetKey: nodeKey))
  }
}

/// An `<iframe>` alone in a page whose origin is the playground's, so that
/// players that check the embedding page (YouTube's) accept it.
private func iframeHTML(src: String, allow: String?, title: String?) -> String {
  func escape(_ value: String) -> String {
    value.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(
      of: "\"", with: "&quot;")
  }
  let attributes = [
    #"src="\#(escape(src))""#, #"frameborder="0""#, "allowfullscreen",
    allow.map { #"allow="\#(escape($0))""# }, title.map { #"title="\#(escape($0))""# },
  ].compactMap { $0 }.joined(separator: " ")
  return """
    <!doctype html><meta name="viewport" content="width=device-width,initial-scale=1">\
    <style>html,body{margin:0;height:100%;overflow:hidden;background:transparent}iframe{border:0;width:100%;height:100%}</style>\
    <iframe \(attributes)></iframe>
    """
}

/// The origin the embeds' pages load from.
private let embedBaseURL = URL(string: "https://playground.lexical.dev/")!

/// `TweetComponent`: Twitter's widget for the tweet, up to 550 points wide,
/// with "Loading..." until it has drawn. The widget's height is its own, so
/// it is measured once drawn, with its margins (the body is a block
/// formatting context, so they do not collapse out of `scrollHeight`), and
/// the node is redrawn at it.
struct TweetComponent: View {
  let editor: Editor
  let nodeKey: NodeKey
  let tweetID: String
  @State private var isLoading = true

  /// The measured heights, by tweet, so a redrawn tweet keeps its height.
  @MainActor static var heights: [String: CGFloat] = [:]

  var body: some View {
    ZStack(alignment: .topLeading) {
      EmbedWebView(
        html: """
          <!doctype html><meta name="viewport" content="width=device-width,initial-scale=1">\
          <style>html,body{margin:0;background:transparent}body{display:flow-root}</style>\
          <div id="tweet"></div>\
          <script src="https://platform.twitter.com/widgets.js"></script>\
          <script>twttr.ready(function(t){t.widgets.createTweet(\(jsonString(tweetID)),\
          document.getElementById('tweet')).then(function(){new ResizeObserver(function(){\
          window.webkit.messageHandlers.height.postMessage(document.body.scrollHeight)})\
          .observe(document.body)})})</script>
          """,
        onHeight: { height in
          isLoading = false
          guard height > 0, Self.heights[tweetID] != height else { return }
          Self.heights[tweetID] = height
          // Redraw the node, which measures its view again.
          try? editor.update(tag: HISTORY_MERGE_TAG) { try getNodeByKey(nodeKey)?.markDirty() }
        })
      if isLoading && Self.heights[tweetID] == nil { Text("Loading...") }
    }
    // At most 550 points wide, as the widget is, and narrower on a phone,
    // where the widget reflows (to 250 at the least) and reports its height.
    .frame(maxWidth: 550, alignment: .topLeading)
    .frame(height: Self.heights[tweetID] ?? 200, alignment: .topLeading)
    .accessibilityLabel("Tweet")
  }
}

/// `JSON.stringify(value)` for a string.
private func jsonString(_ value: String) -> String {
  let data = try? JSONSerialization.data(withJSONObject: [value])
  return data.flatMap { String(data: $0, encoding: .utf8) }.map {
    String($0.dropFirst().dropLast())
  }
    ?? "\"\""
}

#if os(macOS)
  typealias PlatformViewRepresentable = NSViewRepresentable
#else
  typealias PlatformViewRepresentable = UIViewRepresentable
#endif

/// A `WKWebView` showing `html` from the playground's origin, plays media
/// inline, and reports what its page posts to the `height` handler.
struct EmbedWebView: PlatformViewRepresentable {
  let html: String
  var onHeight: ((CGFloat) -> Void)?

  final class Coordinator: NSObject, WKScriptMessageHandler {
    var html = ""
    var onHeight: ((CGFloat) -> Void)?

    func userContentController(
      _ userContentController: WKUserContentController, didReceive message: WKScriptMessage
    ) {
      guard let height = message.body as? Double else { return }
      MainActor.assumeIsolated { onHeight?(CGFloat(height)) }
    }
  }

  func makeCoordinator() -> Coordinator { Coordinator() }

  private func makeWebView(_ coordinator: Coordinator) -> WKWebView {
    let configuration = WKWebViewConfiguration()
    #if os(iOS)
      configuration.allowsInlineMediaPlayback = true
    #endif
    configuration.userContentController.add(coordinator, name: "height")
    let webView = WKWebView(frame: .zero, configuration: configuration)
    #if os(macOS)
      webView.setValue(false, forKey: "drawsBackground")
    #else
      webView.isOpaque = false
      webView.backgroundColor = .clear
      webView.scrollView.isScrollEnabled = false
    #endif
    return webView
  }

  private func update(_ webView: WKWebView, _ coordinator: Coordinator) {
    coordinator.onHeight = onHeight
    guard coordinator.html != html else { return }
    coordinator.html = html
    webView.loadHTMLString(html, baseURL: embedBaseURL)
  }

  #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { makeWebView(context.coordinator) }
    func updateNSView(_ webView: WKWebView, context: Context) {
      update(webView, context.coordinator)
    }
  #else
    func makeUIView(context: Context) -> WKWebView { makeWebView(context.coordinator) }
    func updateUIView(_ webView: WKWebView, context: Context) {
      update(webView, context.coordinator)
    }
  #endif
}
