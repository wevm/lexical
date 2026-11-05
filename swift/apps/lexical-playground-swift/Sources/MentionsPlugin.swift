import Foundation
import Lexical
import LexicalExtension
import LexicalHTML
import LexicalMenu

// Port of packages/lexical-playground/src/plugins/MentionsExtension: an "@"
// before a name opens a menu of matching people from a dummy lookup that
// answers after 500 ms, as on the web.

nonisolated(unsafe) private let MentionImportRule = defineImportRule(
  name: "@lexical/playground/mention", match: sel.tag("span").attr("data-lexical-mention")
) { _, el, _ in [try importMention(el)] }

/// `MentionsExtension`: `MentionNode` and its import rule. Depends on
/// `CoreImportExtension` so the `<span data-lexical-mention>` rule is merged
/// after, and out-prioritizes, the core inline-format `<span>` rule.
let MentionsExtension = defineExtension(
  name: "@lexical/playground/Mentions",
  dependencies: [
    CoreImportExtension,
    configExtension(DOMImportExtension) { $0.rules = [MentionImportRule.entry] },
  ],
  nodes: { [.klass(MentionNode.self)] })

private let PUNC = PUNCTUATION

private let TRIGGERS = "@"

/// Chars we expect to see in a mention (non-space, non-punctuation).
private let VALID_CHARS = "[^" + TRIGGERS + PUNC + #"\s]"#

/// Non-standard series of chars. Each series must be preceded and followed
/// by a valid char.
private let VALID_JOINS =
  "(?:"
  + #"\.[ |$]|"#  // E.g. "r. " in "Mr. Smith"
  + " |"  // E.g. " " in "Josh Duck"
  + "[" + PUNC + "]|"  // E.g. "-' in "Salier-Hellendag"
  + ")"

private let LENGTH_LIMIT = 75

nonisolated(unsafe) private let AtSignMentionsRegex = try! Regex(
  #"(^|\s|\()("# + "[" + TRIGGERS + "]" + "((?:" + VALID_CHARS + VALID_JOINS + "){0,"
    + String(LENGTH_LIMIT) + "})" + ")$"
).matchingSemantics(.unicodeScalar)

/// 50 is the longest alias length limit.
private let ALIAS_LENGTH_LIMIT = 50

/// Regex used to match alias.
nonisolated(unsafe) private let AtSignMentionsRegexAliasRegex = try! Regex(
  #"(^|\s|\()("# + "[" + TRIGGERS + "]" + "((?:" + VALID_CHARS + "){0,"
    + String(ALIAS_LENGTH_LIMIT) + "})" + ")$"
).matchingSemantics(.unicodeScalar)

/// At most, 5 suggestions are shown in the popup.
private let SUGGESTION_LIST_LENGTH_LIMIT = 5

let dummyMentionsData = [
  "Aayla Secura", "Adi Gallia", "Admiral Dodd Rancit", "Admiral Firmus Piett",
  "Admiral Gial Ackbar", "Admiral Ozzel", "Admiral Raddus", "Admiral Terrinald Screed",
  "Admiral Trench", "Admiral U.O. Statura", "Agen Kolar", "Agent Kallus",
  "Aiolin and Morit Astarte", "Aks Moe", "Almec", "Alton Kastle", "Amee", "AP-5", "Armitage Hux",
  "Artoo", "Arvel Crynyd", "Asajj Ventress", "Aurra Sing", "AZI-3", "Bala-Tik", "Barada",
  "Bargwill Tomder", "Baron Papanoida", "Barriss Offee", "Baze Malbus", "Bazine Netal", "BB-8",
  "BB-9E", "Ben Quadinaros", "Berch Teller", "Beru Lars", "Bib Fortuna", "Biggs Darklighter",
  "Black Krrsantan", "Bo-Katan Kryze", "Boba Fett", "Bobbajo", "Bodhi Rook", "Borvo the Hutt",
  "Boss Nass", "Bossk", "Breha Antilles-Organa", "Bren Derlin", "Brendol Hux", "BT-1", "C-3PO",
  "C1-10P", "Cad Bane", "Caluan Ematt", "Captain Gregor", "Captain Phasma",
  "Captain Quarsh Panaka", "Captain Rex", "Carlist Rieekan", "Casca Panzoro", "Cassian Andor",
  "Cassio Tagge", "Cham Syndulla", "Che Amanwe Papanoida", "Chewbacca", "Chi Eekway Papanoida",
  "Chief Chirpa", "Chirrut Îmwe", "Ciena Ree", "Cin Drallig", "Clegg Holdfast", "Cliegg Lars",
  "Coleman Kcaj", "Coleman Trebor", "Colonel Kaplan", "Commander Bly", "Commander Cody (CC-2224)",
  "Commander Fil (CC-3714)", "Commander Fox", "Commander Gree", "Commander Jet",
  "Commander Wolffe", "Conan Antonio Motti", "Conder Kyl", "Constable Zuvio", "Cordé",
  "Cpatain Typho", "Crix Madine", "Cut Lawquane", "Dak Ralter", "Dapp", "Darth Bane",
  "Darth Maul", "Darth Tyranus", "Daultay Dofine", "Del Meeko", "Delian Mors", "Dengar",
  "Depa Billaba", "Derek Klivian", "Dexter Jettster", "Dineé Ellberger", "DJ", "Doctor Aphra",
  "Doctor Evazan", "Dogma", "Dormé", "Dr. Cylo", "Droidbait", "Droopy McCool", "Dryden Vos",
  "Dud Bolt", "Ebe E. Endocott", "Echuu Shen-Jon", "Eeth Koth", "Eighth Brother", "Eirtaé",
  "Eli Vanto", "Ellé", "Ello Asty", "Embo", "Eneb Ray", "Enfys Nest", "EV-9D9", "Evaan Verlaine",
  "Even Piell", "Ezra Bridger", "Faro Argyus", "Feral", "Fifth Brother", "Finis Valorum", "Finn",
  "Fives", "FN-1824", "FN-2003", "Fodesinbeed Annodue", "Fulcrum", "FX-7", "GA-97", "Galen Erso",
  "Gallius Rax", "Garazeb \"Zeb\" Orrelios", "Gardulla the Hutt", "Garrick Versio",
  "Garven Dreis", "Gavyn Sykes", "Gideon Hask", "Gizor Dellso", "Gonk droid", "Grand Inquisitor",
  "Greeata Jendowanian", "Greedo", "Greer Sonnel", "Grievous", "Grummgar", "Gungi", "Hammerhead",
  "Han Solo", "Harter Kalonia", "Has Obbit", "Hera Syndulla", "Hevy", "Hondo Ohnaka", "Huyang",
  "Iden Versio", "IG-88", "Ima-Gun Di", "Inquisitors", "Inspector Thanoth", "Jabba",
  "Jacen Syndulla", "Jan Dodonna", "Jango Fett", "Janus Greejatus", "Jar Jar Binks", "Jas Emari",
  "Jaxxon", "Jek Tono Porkins", "Jeremoch Colton", "Jira", "Jobal Naberrie", "Jocasta Nu",
  "Joclad Danva", "Joh Yowza", "Jom Barell", "Joph Seastriker", "Jova Tarkin", "Jubnuk",
  "Jyn Erso", "K-2SO", "Kanan Jarrus", "Karbin", "Karina the Great", "Kes Dameron", "Ketsu Onyo",
  "Ki-Adi-Mundi", "King Katuunko", "Kit Fisto", "Kitster Banai", "Klaatu", "Klik-Klak",
  "Korr Sella", "Kylo Ren", "L3-37", "Lama Su", "Lando Calrissian", "Lanever Villecham",
  "Leia Organa", "Letta Turmond", "Lieutenant Kaydel Ko Connix", "Lieutenant Thire", "Lobot",
  "Logray", "Lok Durd", "Longo Two-Guns", "Lor San Tekka", "Lorth Needa", "Lott Dod",
  "Luke Skywalker", "Lumat", "Luminara Unduli", "Lux Bonteri", "Lyn Me", "Lyra Erso",
  "Mace Windu", "Malakili", "Mama the Hutt", "Mars Guo", "Mas Amedda", "Mawhonic", "Max Rebo",
  "Maximilian Veers", "Maz Kanata", "ME-8D9", "Meena Tills", "Mercurial Swift", "Mina Bonteri",
  "Miraj Scintel", "Mister Bones", "Mod Terrik", "Moden Canady", "Mon Mothma", "Moradmin Bast",
  "Moralo Eval", "Morley", "Mother Talzin", "Nahdar Vebb", "Nahdonnis Praji", "Nien Nunb",
  "Niima the Hutt", "Nines", "Norra Wexley", "Nute Gunray", "Nuvo Vindi", "Obi-Wan Kenobi",
  "Odd Ball", "Ody Mandrell", "Omi", "Onaconda Farr", "Oola", "OOM-9", "Oppo Rancisis",
  "Orn Free Taa", "Oro Dassyne", "Orrimarko", "Osi Sobeck", "Owen Lars", "Pablo-Jill",
  "Padmé Amidala", "Pagetti Rook", "Paige Tico", "Paploo", "Petty Officer Thanisson",
  "Pharl McQuarrie", "Plo Koon", "Po Nudo", "Poe Dameron", "Poggle the Lesser", "Pong Krell",
  "Pooja Naberrie", "PZ-4CO", "Quarrie", "Quay Tolsite", "Queen Apailana", "Queen Jamillia",
  "Queen Neeyutnee", "Qui-Gon Jinn", "Quiggold", "Quinlan Vos", "R2-D2", "R2-KT", "R3-S6",
  "R4-P17", "R5-D4", "RA-7", "Rabé", "Rako Hardeen", "Ransolm Casterfo", "Rappertunie",
  "Ratts Tyerell", "Raymus Antilles", "Ree-Yees", "Reeve Panzoro", "Rey", "Ric Olié",
  "Riff Tamson", "Riley", "Rinnriyin Di", "Rio Durant", "Rogue Squadron", "Romba", "Roos Tarpals",
  "Rose Tico", "Rotta the Hutt", "Rukh", "Rune Haako", "Rush Clovis", "Ruwee Naberrie",
  "Ryoo Naberrie", "Sabé", "Sabine Wren", "Saché", "Saelt-Marae", "Saesee Tiin",
  "Salacious B. Crumb", "San Hill", "Sana Starros", "Sarco Plank", "Sarkli", "Satine Kryze",
  "Savage Opress", "Sebulba", "Senator Organa", "Sergeant Kreel", "Seventh Sister", "Shaak Ti",
  "Shara Bey", "Shmi Skywalker", "Shu Mai", "Sidon Ithano", "Sifo-Dyas", "Sim Aloo",
  "Siniir Rath Velus", "Sio Bibble", "Sixth Brother", "Slowen Lo", "Sly Moore", "Snaggletooth",
  "Snap Wexley", "Snoke", "Sola Naberrie", "Sora Bulq", "Strono Tuggs", "Sy Snootles",
  "Tallissan Lintra", "Tarfful", "Tasu Leech", "Taun We", "TC-14", "Tee Watt Kaa", "Teebo",
  "Teedo", "Teemto Pagalies", "Temiri Blagg", "Tessek", "Tey How", "Thane Kyrell", "The Bendu",
  "The Smuggler", "Thrawn", "Tiaan Jerjerrod", "Tion Medon", "Tobias Beckett", "Tulon Voidgazer",
  "Tup", "U9-C4", "Unkar Plutt", "Val Beckett", "Vanden Willard", "Vice Admiral Amilyn Holdo",
  "Vober Dand", "WAC-47", "Wag Too", "Wald", "Walrus Man", "Warok", "Wat Tambor", "Watto",
  "Wedge Antilles", "Wes Janson", "Wicket W. Warrick", "Wilhuff Tarkin", "Wollivan", "Wuher",
  "Wullf Yularen", "Xamuel Lennox", "Yaddle", "Yarael Poof", "Yoda", "Zam Wesell", "Zev Senesca",
  "Ziro the Hutt", "Zuckuss",
]

/// `dummyLookupService`: the names containing `string`, ignoring case,
/// after 500 ms.
@MainActor
private func dummyLookup(_ string: String, _ callback: @escaping @MainActor ([String]) -> Void) {
  DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
    let lowered = string.lowercased()
    callback(dummyMentionsData.filter { $0.lowercased().contains(lowered) })
  }
}

/// `checkForAtSignMentions`.
func checkForAtSignMentions(_ text: String, _ minMatchLength: Int) -> MenuTextMatch? {
  guard
    let match = (try? AtSignMentionsRegex.firstMatch(in: text))
      ?? (try? AtSignMentionsRegexAliasRegex.firstMatch(in: text))
  else { return nil }
  // The strategy ignores leading whitespace but we need to know its length
  // to add it to the leadOffset.
  let maybeLeadingWhitespace = match.output[1].substring.map(String.init) ?? ""
  let matchingString = match.output[3].substring.map(String.init) ?? ""
  guard matchingString.utf16.count >= minMatchLength else { return nil }
  return MenuTextMatch(
    leadOffset: text.utf16.distance(from: text.startIndex, to: match.range.lowerBound)
      + maybeLeadingWhitespace.utf16.count,
    matchingString: matchingString,
    replaceableString: match.output[2].substring.map(String.init) ?? "")
}

/// `getPossibleQueryMatch`.
func getPossibleQueryMatch(_ text: String) -> MenuTextMatch? {
  checkForAtSignMentions(text, 1)
}

/// `MentionTypeaheadOption`.
final class MentionTypeaheadOption: MenuOption {
  let name: String

  override init(_ name: String) {
    self.name = name
    super.init(name)
    title = name
  }
}

/// `MentionsPlugin`.
@MainActor
func registerMentions(
  _ editor: Editor
) -> (
  host: TypeaheadMenuHost<MentionTypeaheadOption>, unregister: () -> Void
) {
  // `mentionsCache`: a query's results, or `nil` while its lookup runs.
  var cache: [String: [String]?] = [:]
  var queryString: String?
  var setResults: ([String]) -> Void = { _ in }
  let checkForSlashTriggerMatch = basicTypeaheadTriggerMatch("/", minLength: 0)
  let (menu, unregister) = registerTypeaheadMenu(
    editor,
    triggerFn: { text, editor in
      if checkForSlashTriggerMatch(text, editor) != nil { return nil }
      return getPossibleQueryMatch(text)
    },
    onQueryChange: { mentionString in
      // `useMentionLookupService`.
      queryString = mentionString
      guard let mentionString else {
        setResults([])
        return
      }
      if let cached = cache[mentionString] {
        if let cached { setResults(cached) }
        return
      }
      cache[mentionString] = .some(nil)
      dummyLookup(mentionString) { results in
        cache[mentionString] = results
        if queryString == mentionString { setResults(results) }
      }
    },
    onSelectOption: { (option: MentionTypeaheadOption, nodeToReplace, closeMenu, _) in
      let mentionNode = try createMentionNode(option.name)
      try nodeToReplace?.replace(mentionNode)
      try mentionNode.select()
      closeMenu()
    })
  setResults = { [weak menu] results in
    menu?.setOptions(
      results.prefix(SUGGESTION_LIST_LENGTH_LIMIT).map { MentionTypeaheadOption($0) })
  }
  let host = TypeaheadMenuHost(menu: menu) {
    TypeaheadMenuRow(id: $0.key, title: $0.name, symbol: "person.crop.circle.fill", glyph: nil)
  }
  return (host, unregister)
}
