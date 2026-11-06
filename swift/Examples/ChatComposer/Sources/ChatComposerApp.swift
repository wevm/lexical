import Lexical
import LexicalSwiftUI
import SwiftUI
import UniformTypeIdentifiers

// A chat app's message list and composer. Launch arguments: `-dark` and
// `-light` force an appearance; `-draft <text>` starts the composer with
// `text`, and `-attach <name>`, repeated, with attachments (for captures).

@main
struct ChatComposerApp: App {
  var body: some Scene {
    WindowGroup {
      ChatView()
    }
    #if os(macOS)
      .defaultSize(width: 520, height: 680)
    #endif
  }
}

struct ChatView: View {
  @State private var model = ChatModel()
  private let colorScheme: ColorScheme? = {
    let arguments = ProcessInfo.processInfo.arguments
    if arguments.contains("-dark") { return .dark }
    if arguments.contains("-light") { return .light }
    return nil
  }()

  var body: some View {
    VStack(spacing: 0) {
      MessageList(messages: model.messages)
      Divider()
      ComposerView(model: model)
    }
    .preferredColorScheme(colorScheme)
    .task { model.applyLaunchArguments() }
  }
}

// MARK: Messages

struct MessageList: View {
  let messages: [ChatMessage]

  var body: some View {
    ScrollViewReader { proxy in
      ScrollView {
        LazyVStack(alignment: .leading, spacing: 10) {
          ForEach(messages) { message in
            MessageBubble(message: message).id(message.id)
          }
        }
        .padding(12)
      }
      .onChange(of: messages.count) { _, _ in
        if let last = messages.last {
          withAnimation { proxy.scrollTo(last.id, anchor: .bottom) }
        }
      }
    }
  }
}

/// A sent message: a read-only editor with the composer's theme.
struct MessageBubble: View {
  let message: ChatMessage
  private var isMine: Bool { message.author == .me }

  var body: some View {
    HStack {
      if isMine { Spacer(minLength: 48) }
      VStack(alignment: .leading, spacing: 4) {
        if !message.attachments.isEmpty {
          AttachmentChips(attachments: message.attachments, onRemove: nil)
        }
        LexicalEditor(message.editor)
          .sizedToContent(minHeight: 20)
          .theme(chatTheme)
          .editorBackground(false)
      }
      .padding(6)
      .background(
        RoundedRectangle(cornerRadius: 14)
          .fill(isMine ? Color.accentColor.opacity(0.15) : Color.secondary.opacity(0.12)))
      if !isMine { Spacer(minLength: 48) }
    }
    .accessibilityElement(children: .contain)
    .accessibilityIdentifier(isMine ? "message-me" : "message-assistant")
  }
}

// MARK: Composer

struct ComposerView: View {
  @Bindable var model: ChatModel
  @State private var isImporting = false
  @State private var isEditingLink = false
  @State private var linkURL = ""

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      if model.mentions.isOpen {
        SuggestionList(model: model.mentions)
      } else if model.commands.isOpen {
        SuggestionList(model: model.commands)
      }
      if !model.attachments.isEmpty {
        AttachmentChips(attachments: model.attachments) { model.remove($0) }
      }
      formatBar
      HStack(alignment: .bottom, spacing: 8) {
        Button {
          isImporting = true
        } label: {
          Image(systemName: "paperclip").font(.system(size: 17))
        }
        .buttonStyle(.borderless)
        .disabled(model.isStreaming)
        .accessibilityLabel("Attach a file")
        .padding(.bottom, 8)

        // About 8 lines of 15-point text and the insets, then it scrolls.
        LexicalEditor(model.composer, placeholder: "Message…")
          .sizedToContent(minHeight: 36, maxHeight: 180)
          .theme(chatTheme)
          .editorBackground(false)
          .padding(.horizontal, 4)
          .background(
            RoundedRectangle(cornerRadius: 18).strokeBorder(Color.secondary.opacity(0.35))
          )
          .opacity(model.isStreaming ? 0.6 : 1)
          .accessibilityIdentifier("composer")

        Button {
          model.send()
        } label: {
          Image(systemName: "arrow.up.circle.fill").font(.system(size: 26))
        }
        .buttonStyle(.borderless)
        .foregroundStyle(model.canSend ? Color.accentColor : Color.secondary.opacity(0.5))
        .disabled(!model.canSend)
        .keyboardShortcut(.return, modifiers: .command)
        .accessibilityLabel("Send")
        .accessibilityIdentifier("send")
        .padding(.bottom, 4)
      }
      if model.isStreaming {
        Label("Replying…", systemImage: "ellipsis.bubble")
          .font(.caption).foregroundStyle(.secondary)
      }
    }
    .padding(10)
    .fileImporter(
      isPresented: $isImporting, allowedContentTypes: [.item], allowsMultipleSelection: true
    ) { result in
      if case .success(let urls) = result { model.addFiles(urls) }
    }
    .alert("Link", isPresented: $isEditingLink) {
      TextField("https://", text: $linkURL)
      Button("Apply") { model.setLink(linkURL) }
      if model.isLink {
        Button("Remove", role: .destructive) { model.setLink("") }
      }
      Button("Cancel", role: .cancel) {}
    }
  }

  private var formatBar: some View {
    HStack(spacing: 2) {
      formatButton("bold", "bold", "Bold")
      formatButton("italic", "italic", "Italic")
      formatButton("code", "chevron.left.forwardslash.chevron.right", "Code")
      Button {
        linkURL = "https://"
        isEditingLink = true
      } label: {
        Image(systemName: "link").frame(width: 26, height: 22)
      }
      .buttonStyle(.borderless)
      .background(
        model.isLink ? Color.accentColor.opacity(0.2) : .clear, in: .rect(cornerRadius: 5)
      )
      .accessibilityLabel("Link")
      Spacer()
    }
    .disabled(model.isStreaming)
    .foregroundStyle(.secondary)
  }

  private func formatButton(
    _ format: TextFormatType, _ symbol: String, _ label: String
  )
    -> some View
  {
    let isActive = model.activeFormats.contains(format)
    return Button {
      model.toggleFormat(format)
    } label: {
      Image(systemName: symbol).frame(width: 26, height: 22)
    }
    .buttonStyle(.borderless)
    .foregroundStyle(isActive ? Color.accentColor : .secondary)
    .background(isActive ? Color.accentColor.opacity(0.2) : .clear, in: .rect(cornerRadius: 5))
    .accessibilityLabel(label)
  }
}

/// An open menu's options, above the composer.
struct SuggestionList: View {
  let model: SuggestionsModel

  var body: some View {
    VStack(spacing: 0) {
      ForEach(Array(model.rows.enumerated()), id: \.element.id) { index, row in
        HStack(spacing: 8) {
          Image(systemName: row.symbol).frame(width: 20)
          Text(row.title)
          if !row.detail.isEmpty {
            Text(row.detail).foregroundStyle(.secondary)
          }
          Spacer()
        }
        .font(.system(size: 14))
        .padding(.horizontal, 10).padding(.vertical, 6)
        .background(
          model.selectedIndex == index ? Color.accentColor.opacity(0.18) : .clear
        )
        .contentShape(Rectangle())
        .onTapGesture { model.choose(index) }
        #if os(macOS)
          .onHover { if $0 { model.highlight(index) } }
        #endif
        .accessibilityAddTraits(.isButton)
      }
    }
    .clipShape(RoundedRectangle(cornerRadius: 8))
    .background(RoundedRectangle(cornerRadius: 8).fill(.background).shadow(radius: 3, y: 1))
    .accessibilityIdentifier("suggestions")
  }
}

/// Attachments as chips; `onRemove` adds a remove button to each.
struct AttachmentChips: View {
  let attachments: [Attachment]
  let onRemove: ((Attachment) -> Void)?

  var body: some View {
    ScrollView(.horizontal) {
      HStack(spacing: 6) {
        ForEach(attachments) { attachment in
          HStack(spacing: 4) {
            Image(systemName: attachment.symbol)
            Text(attachment.name).lineLimit(1)
            Text(
              ByteCountFormatter.string(fromByteCount: Int64(attachment.size), countStyle: .file)
            )
            .foregroundStyle(.secondary)
            if let onRemove {
              Button {
                onRemove(attachment)
              } label: {
                Image(systemName: "xmark.circle.fill")
              }
              .buttonStyle(.borderless)
              .foregroundStyle(.secondary)
              .accessibilityLabel("Remove \(attachment.name)")
            }
          }
          .font(.system(size: 12))
          .padding(.horizontal, 8).padding(.vertical, 4)
          .background(Capsule().fill(Color.secondary.opacity(0.15)))
        }
      }
    }
    .scrollIndicators(.hidden)
    .accessibilityIdentifier("attachments")
  }
}
