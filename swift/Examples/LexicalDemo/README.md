# Lexical demo app

`LexicalEditor` on a small document (headings, a quote, bold, italic, code
and two SwiftUI decorators), for trying the editor on an iPhone, an iPad or
the simulator, and for the UI tests in `UITests/`. It builds the package
from `../..`.

The Xcode project is generated with [XcodeGen](https://github.com/yonaskolb/XcodeGen)
and not checked in:

```sh
cd swift/Examples/LexicalDemo
xcodegen generate
```

Launch arguments: `-empty` starts from an empty paragraph; `-text
<paragraph>`, repeated once per paragraph, starts from those paragraphs;
`-uitest` shows the model probe (the editor's text content, JSON and the
last commands the view dispatched, as accessibility labels) that the UI
tests read; `-benchmark` types 200 keystrokes into a 5,000-paragraph
document and shows their p95 (`testReportsKeystrokeTiming` prints it; pass
`-configuration Release` for release numbers); `-dark` shows the dark
appearance. With `-uitest` the probe also reports the view's appearance,
content size category, font sizes and decorator labels, which the Dynamic
Type, Dark Mode and decorator label tests read (Dynamic Type through
`-UIPreferredContentSizeCategoryName`).

## Simulator

```sh
xcodebuild test -project LexicalDemo.xcodeproj -scheme LexicalDemo \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
  -skipMacroValidation -skipPackagePluginValidation
```

The IME tests switch to the Japanese (Romaji), Simplified Chinese (Pinyin
QWERTY) and Korean keyboards, and skip when one is not enabled. On a
simulator, enable them and reboot it:

```sh
xcrun simctl spawn <udid> defaults write -g AppleKeyboards -array \
  "en_US@sw=QWERTY;hw=Automatic" "ja_JP-Romaji@sw=QWERTY-Japanese;hw=Automatic" \
  "zh_Hans-Pinyin@sw=Pinyin-Simplified;hw=Automatic" "ko_KR@sw=Korean;hw=Automatic" \
  "emoji@sw=Emoji"
xcrun simctl shutdown <udid> && xcrun simctl boot <udid>
```

## Device

Signing is automatic with the team passed on the command line, never
committed. `LEXICAL_BUNDLE_PREFIX` changes the bundle identifiers when the
default ones are taken for the team (free teams need unique ones).

```sh
xcodebuild test -project LexicalDemo.xcodeproj -scheme LexicalDemo \
  -destination 'platform=iOS,id=<device udid>' -allowProvisioningUpdates \
  DEVELOPMENT_TEAM="$DEVELOPMENT_TEAM" \
  -skipMacroValidation -skipPackagePluginValidation
```

Before the first run, on the device: Developer Mode on (Settings > Privacy
& Security), the keyboards above added (Settings > General > Keyboard >
Keyboards), and, with a free team, the developer trusted after the first
install (Settings > General > VPN & Device Management).

On a device, the paste test skips, because the test runner cannot write the
general pasteboard there; paste from another app is a manual check below.

## Manual checks

What the UI tests cannot drive. Run the app without `-uitest` from the home
screen, or with it (from Xcode's scheme) to watch the model probe at the
bottom.

Software keyboard
- [ ] Type a sentence; autocorrection replaces a misspelled word and the
  caret stays after the next character typed.
- [ ] Tap a QuickType prediction; it replaces the current word.
- [ ] Undo with shake and with the three-finger swipe left; redo with the
  three-finger swipe right. Each undoes the last typing run, not one key.
- [ ] Hold the space bar and move the caret (trackpad mode).
- [ ] Slide to type (QuickPath) a word.
- [ ] Return makes a new paragraph; Shift-Return on a hardware keyboard
  makes a line break.

IME
- [ ] Japanese Romaji and Kana: compose, pick a candidate from the bar,
  convert a long phrase with the space bar and move between clauses with the
  arrows; the underline follows the composition and nothing commits early.
- [ ] Chinese Pinyin (QWERTY and 9-key) and Handwriting: candidates insert
  where the caret is.
- [ ] Korean: syllables form as you type, and delete takes one jamo at a
  time.
- [ ] Switch keyboards in the middle of a composition; the marked text
  commits.

Dictation and handwriting
- [ ] Dictate a sentence with punctuation ("comma", "period"); dictate
  again over a selection.
- [ ] Scribble with Apple Pencil on an iPad (write, scratch out to delete,
  circle to select).

Selection
- [ ] Double-tap selects a word, triple-tap a paragraph; drag the handles
  across a bold word and the quote.
- [ ] The loupe appears while dragging the caret and the handles.
- [ ] The edit menu's Bold / Italic / Underline format the selection;
  Select All selects the document.
- [ ] Tap a decorator (the SwiftUI badge and card); the caret does not jump
  inside it.

Clipboard
- [ ] Copy rich text from Notes, Safari and Mail and paste it: formatting
  (bold, italic, links, headings) survives. Allow the paste prompt once.
- [ ] Copy from the demo and paste into Notes and Messages.
- [ ] Drag text from Safari into the editor, and from the editor to Notes.

Hardware keyboard
- [ ] Command-B / I / U, Command-Z / Shift-Command-Z, Command-A, Command-C
  / X / V.
- [ ] Arrows, Option-arrows (words), Command-arrows (line and document
  ends), with Shift to extend.
- [ ] Option-Delete deletes a word, Command-Delete to the line start.
- [ ] Tab and Escape do what the editor's extensions say.
