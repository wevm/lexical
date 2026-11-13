import Foundation
import Lexical
import LexicalExtension
import LexicalList
import LexicalRichText
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts.
// Upstream checks the derivation against Node's crypto. Swift has no
// independent SHA-256 on every platform, so the values that Node's crypto
// gives for the same inputs are written here as literals.

/// `/^[A-Za-z0-9_-]{21}$/`.
private func isId(_ string: String) -> Bool {
  string.count == 21 && string.allSatisfy { HANDLE_ALPHABET.contains($0) }
}

/// A document stored without ids, as compact JSON (what a server might keep).
private let STORED = JSONObject([
  (
    "root",
    [
      "children": [
        ["children": [["text": "Hello, wörld 👋", "type": "text"]], "type": "paragraph"],
        ["children": [["text": "Second", "type": "text"]], "tag": "h2", "type": "heading"],
        ["children": [], "type": "paragraph"],
      ],
      "type": "root",
    ]
  )
]).stringify()

/// `createHash('sha256')` of STORED's canonical document, in hex.
private let STORED_KEY = "a5b08ad0c2466a1e221f46f78d5aa99a46566c9196b447369d279e01725d6cd3"

private func loaded() throws -> Editor {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[format]", dependencies: [RichTextExtension, BlockIdExtension],
      editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
  try editor.setEditorState(editor.parseEditorState(STORED))
  return editor
}

private func childIds(_ editor: Editor) throws -> [String?] {
  try editor.getEditorState().toJSON()["root"]?.objectValue?["children"]?.arrayValue?.map {
    $0.objectValue?["$"]?.objectValue?["blockId"]?.stringValue
  } ?? []
}

@Suite struct BlockIdFormatTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > the documented end-to-end vector"
    ))
  func documentedEndToEndVector() throws {
    let stored =
      #"{"root":{"children":["#
      + #"{"$":{"10":1.5,"9":"x","note":"tab\tquote\""},"#
      + #""children":[{"text":"Hello, wörld 👋","type":"text"}],"type":"paragraph"},"#
      + #"{"children":["#
      + #"{"children":[{"text":"one","type":"text"}],"type":"listitem","value":1},"#
      + #"{"children":[{"text":"two","type":"text"}],"type":"listitem","value":2}],"#
      + #""listType":"bullet","start":1,"tag":"ul","type":"list"},"#
      + #"{"$":{"blockId":"kept-id"},"children":[],"type":"paragraph"}"#
      + #"],"type":"root"}}"#
    let canonical =
      #"{"children":["#
      + #"{"$":{"9":"x","10":1.5,"note":"tab\tquote\""},"#
      + #""children":[{"text":"Hello, wörld 👋","type":"text"}],"type":"paragraph"},"#
      + #"{"children":["#
      + #"{"children":[{"text":"one","type":"text"}],"type":"listitem"},"#
      + #"{"children":[{"text":"two","type":"text"}],"type":"listitem","value":2}],"#
      + #""listType":"bullet","type":"list"},"#
      + #"{"children":[],"type":"paragraph"}"#
      + #"],"type":"root"}"#
    // Node's sha256 of `canonical`, and of the key and each place.
    let key = "9fa59b03bb40d3d4ac45b31a472f965a0825ffc10693d4165b1e4dcf956b5dfa"
    let expected: [(String?, String, String)] = [
      ("0", "paragraph", "qfDxX6r4J9KOyXNjM0x2X"),
      ("1", "list", "nWeVEnzogQyDvzB4_4gMw"),
      ("1.0", "listitem", "L9_QRFvuohaGhhqAlzLEB"),
      ("1.1", "listitem", "BCreOq5xUSiXSxTd8xMLr"),
      (nil, "paragraph", "kept-id"),
    ]
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[format-vector]",
        dependencies: [RichTextExtension, ListExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    try editor.setEditorState(editor.parseEditorState(stored))
    #expect(try editor.read { try canonicalDocument() } == canonical)
    let storedKey = try editor.read { try documentKey() }
    #expect(hex(storedKey) == key)
    var actual: [String] = []
    func visit(_ node: JSONValue) {
      guard let object = node.objectValue else { return }
      if let id = object["$"]?.objectValue?["blockId"]?.stringValue {
        actual.append("\(object["type"]?.stringValue ?? ""):\(id)")
      }
      object["children"]?.arrayValue?.forEach(visit)
    }
    try editor.getEditorState().toJSON()["root"]?.objectValue?["children"]?.arrayValue?.forEach(
      visit)
    #expect(actual == expected.map { "\($0.1):\($0.2)" })
    for (place, _, id) in expected {
      if let place {
        #expect(derivePositionalId(storedKey, place) == id, "\(place)")
      }
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > sha256 matches the standard on every padding boundary and beyond"
    ))
  func sha256MatchesTheStandard() {
    // Node's createHash('sha256') of the same bytes.
    let expected: [(Int, String)] = [
      (0, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
      (1, "7da59d0dfbe21f43e842e8afb43e12a6445bbac07c2fc26984c71d0de3f99c9c"),
      (3, "ad3f4a8472e5343338f443d76cd1ce802541e80aacd6367ba7fa5c1904bf56ae"),
      (55, "937365c24868838e86c7ef3617fee9ede9fd9660c15813bd9c634c5cbd2f5d57"),
      (56, "23bf3d748309a100dfded953f30bbd1828d41ebf69ac13fc32b6b0382d70ac24"),
      (57, "3d0f0f4881f99fc1bcbe4885965558b971b51644994f4e986a5278fb55c6448d"),
      (63, "a74c568993e4152d2292bf0893adc6bce3f44e625f846c5d57466e96c859b016"),
      (64, "9f65f39d0ebb8a31bf11cca25d3e3908ec400dce4ac5a8be1ab143638e570ae7"),
      (65, "2e9d96235f30cc008d6909575b1c258b234c1b37cd7828313f6d1a34a8167c86"),
      (119, "d7b0bc3c7434b203a51b87d734103162546be61f4a681dffaec95e320b20c909"),
      (120, "4cabcbe839cb42c11343f621b58755f9949a485e8253a03087c437de3f858d05"),
      (128, "967bee342ef500f4ee0b83b4ecaaf32ce55c73d44c03c85ffe556325c32611bf"),
      (1000, "d13ba0485e4f5f3bce9492c2263737aa00950a6d8cabd3b1e361e092c3d06c46"),
      (100_000, "6263d936874538d6ef59ba9ab7ee1f28cd5ab1f23b89af04d0d0de1ed2d60249"),
    ]
    var seed = 7
    func next() -> UInt8 {
      seed = (seed * 16807) % 2_147_483_647
      return UInt8(seed & 0xff)
    }
    for (length, digest) in expected {
      let bytes = (0..<length).map { _ in next() }
      #expect(hex(sha256(bytes)) == digest, "length \(length)")
    }
    #expect(
      hex(sha256(utf8("abc"))) == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    )
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > utf8 matches the standard, lone surrogates included"
    ))
  func utf8MatchesTheStandard() {
    // A Swift String holds no lone surrogates, so the cases that have one
    // upstream have no equivalent.
    for text in ["", "ascii", "wörld", "€ and 👋"] {
      #expect(utf8(text) == Array(text.utf8))
    }
    #expect(utf8("€ and 👋") == [226, 130, 172, 32, 97, 110, 100, 32, 240, 159, 145, 139])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > canonical JSON sorts keys at every level and leaves out undefined"
    ))
  func canonicalJSONSortsKeys() {
    // Unsorted keys. A Swift JSON object has no `undefined` member to leave
    // out.
    let unsorted: JSONValue = .object(
      JSONObject([
        ("b", [.object(JSONObject([("z", 1), ("m", "x")]))]), ("a", .null), ("c", "é\""),
      ]))
    #expect(canonicalJSON(unsorted) == #"{"a":null,"b":[{"m":"x","z":1}],"c":"é\""}"#)
    // Keys that are array indices come first, in numeric order, as
    // ECMAScript orders them, then the others by UTF-16 code units.
    let indices: JSONValue = .object(
      JSONObject(["b", "10", "a", "9", "B", ""].map { ($0, JSONValue(1)) }))
    #expect(canonicalJSON(indices) == #"{"9":1,"10":1,"":1,"B":1,"a":1,"b":1}"#)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > the document hashed is the compact export without ids"
    ))
  func hashedDocumentIsCompactExportWithoutIds() throws {
    let editor = try loaded()
    defer { editor.dispose() }
    let canonical = try editor.read { try canonicalDocument() }
    // The load named every block, but ids are not part of what is hashed.
    #expect(!canonical.contains("blockId"))
    let root = try #require(try editor.getEditorState().toJSON(true)["root"])
    #expect(
      canonical
        == canonicalJSON(root).replacing(
          try Regex(#""\$":\{"blockId":"[A-Za-z0-9_-]{21}"\},"#), with: ""))
    #expect(hex(try editor.read { try documentKey() }) == STORED_KEY)
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > positional ids follow the specification and are pinned"
    ))
  func positionalIdsArePinned() throws {
    let editor = try loaded()
    defer { editor.dispose() }
    let key = try editor.read { try documentKey() }
    // Node's sha256(documentKey ‖ place) in base64url, cut to 21.
    let spec: [(String, String)] = [
      ("0", "Obj4Jnk2Uq2o1Jcgsuvda"), ("1", "m81kLbLYKGetZFUBn8iGg"),
      ("2", "tTZ_qAifsgwmKg2wFCDwn"),
      ("0.0", "01M9btZTmuIzFg2o2pgz9"), ("12.3.4", "Axzfcx8mMHfz1TXtDMogg"),
      ("0#1", "mFBw6txrzaNCXLhCe_CLM"), ("1#2", "c5PdYRr4fc3Byt2qkDanR"),
    ]
    for (place, id) in spec {
      #expect(derivePositionalId(key, place) == id, "\(place)")
    }
    let ids = try childIds(editor)
    #expect(ids == ["0", "1", "2"].map { derivePositionalId(key, $0) })
    // Frozen: these are the ids every reader gives this stored document.
    #expect(hex(key) == STORED_KEY)
    #expect(ids == ["Obj4Jnk2Uq2o1Jcgsuvda", "m81kLbLYKGetZFUBn8iGg", "tTZ_qAifsgwmKg2wFCDwn"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > ids of later holders of a repeated id follow the specification and are pinned"
    ))
  func duplicateIdsArePinned() {
    // Node's sha256("dup\0" + id + "\0" + ordinal) in base64url, cut to 21.
    for (id, ordinal, expected) in [
      ("x", "1", "-rK-MX7NTIVuS6OIwyd26"), ("x", "2", "I8N4JHaH_LC1SSZw_HJ8r"),
      ("V1StGXR8_Z5jdHi6B-myT", "1", "atPwgUJO7TzlWVO7K9Scw"),
      ("wörld 👋", "3", "MTjPC2mYL6Yd8hRaD8fNq"),
      (String(repeating: "y", count: 64), "1#1", "9Ei_ZS5wIhrsxyA-qNJdY"),
    ] {
      let derived = deriveDuplicateId(id, ordinal)
      #expect(derived == expected)
      #expect(isId(derived))
    }
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > a loaded document with repeated ids: the first holder keeps the id, the later ones are renamed by ordinal"
    ))
  func repeatedIdsRenamedByOrdinal() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[format-dup]", dependencies: [RichTextExtension, BlockIdExtension],
        editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none)))
    defer { editor.dispose() }
    func paragraph(_ text: String, _ id: String) -> JSONValue {
      [
        "$": ["blockId": .string(id)], "children": [["text": .string(text), "type": "text"]],
        "type": "paragraph",
      ]
    }
    try editor.setEditorState(
      editor.parseEditorState(
        JSONObject([
          (
            "root",
            [
              "children": [
                paragraph("a", "x"), paragraph("b", "x"), paragraph("c", "y"), paragraph("d", "x"),
              ],
              "type": "root",
            ]
          )
        ]).stringify()))
    #expect(try childIds(editor) == ["x", "-rK-MX7NTIVuS6OIwyd26", "y", "I8N4JHaH_LC1SSZw_HJ8r"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.format.test.ts::block id format > positional ids are well formed, use every symbol, and do not repeat"
    ))
  func positionalIdsWellFormed() {
    let key = sha256(utf8("{}"))
    var seen = Set<String>()
    var symbols = Set<Character>()
    for i in 0..<20_000 {
      let id = derivePositionalId(key, "\(i % 200).\(i)")
      #expect(isId(id))
      #expect(blockIdState.parse(.string(id)) == id)
      seen.insert(id)
      symbols.formUnion(id)
    }
    #expect(seen.count == 20_000)
    #expect(symbols.count == 64)
  }
}
