import Lexical
import LexicalRichText

/// The playground's welcome document (`prepopulatedRichText` in
/// packages/lexical-playground/src/App.tsx), with the v0.1.0 nodes.
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
          "Make sure to check out the various plugins in the toolbar. You can also import and export the editor state as JSON or HTML."
        )))
    try root.append(
      createParagraphNode().append(
        createTextNode(
          "If you'd like to find out more about Lexical, you can visit lexical.dev; the Swift port lives in the swift directory of wevm/lexical."
        )))
    _ = try root.selectEnd()
  }
}
