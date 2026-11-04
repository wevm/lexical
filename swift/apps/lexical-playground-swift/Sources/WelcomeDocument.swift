import Lexical
import LexicalLink
import LexicalList
import LexicalRichText

/// The playground's welcome document (`prepopulatedRichText` in
/// packages/lexical-playground/src/App.tsx), without the nodes the Swift
/// port does not have yet (hashtags and mentions).
func loadWelcomeDocument(_ editor: Editor) throws {
  try editor.update(discrete: true) {
    let root = try getRoot().clear()
    try root.append(createHeadingNode("h1").append(createTextNode("Welcome to the playground")))
    try root.append(
      createQuoteNode().append(
        createTextNode(
          "In case you were wondering what the black box at the bottom is – it's the debug view, showing the current state of the editor. You can disable it by toggling it in the settings."
        )))
    try root.append(
      createParagraphNode().append(
        createTextNode("The playground is a demo environment built with "),
        createTextNode("Lexical for Swift").setFormat("code"),
        createTextNode(". Try typing in "),
        createTextNode("some text").setFormat("bold"),
        createTextNode(" with "),
        createTextNode("different").setFormat("italic"),
        createTextNode(" formats.")))
    try root.append(
      createParagraphNode().append(
        createTextNode(
          "Make sure to check out the various plugins in the toolbar. You can also type Markdown, such as # for a heading or - for a list, and import and export the document as JSON, HTML or Markdown."
        )))
    try root.append(
      createParagraphNode().append(
        createTextNode("If you'd like to find out more about Lexical, you can:")))
    func item(
      _ before: String, _ url: String, _ label: String, _ after: String
    ) throws
      -> ListItemNode
    {
      try createListItemNode().append(
        createTextNode(before), createLinkNode(url).append(createTextNode(label)),
        createTextNode(after))
    }
    try root.append(
      createListNode("bullet").append(
        item(
          "Visit the ", "https://lexical.dev/", "Lexical website",
          " for documentation and more information."),
        item(
          "Check out the code on our ", "https://github.com/meta/lexical", "GitHub repository", "."),
        item(
          "The Swift port lives in the swift directory of ",
          "https://github.com/wevm/lexical/tree/main/swift", "wevm/lexical", "."),
        item(
          "Join our ", "https://discord.com/invite/KmG4wQnnD9", "Discord Server",
          " and chat with the team.")))
    try root.append(
      createParagraphNode().append(
        createTextNode(
          "Lastly, we're constantly adding cool new features to this playground. So make sure you check back here when you next get a chance :)."
        )))
    _ = try root.selectEnd()
  }
}
