import Lexical
import LexicalExtension
import LexicalRichText
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/
// BlockIdExtension.handles.property.test.ts. Upstream draws its documents
// with fast-check; Swift draws them from the same distributions with the
// seeded mulberry32, so the run is the same every time. The typo-rate
// measurement, run upstream only when BLOCK_HANDLE_TYPO_DOCS is set, is not
// ported.

/// An id, resolved against the ids before it.
private enum IdSpec {
  case random(String)
  case short(String)
  case shared(base: Int, prefix: Int, tail: String)
}

private enum Edit {
  case insert(at: Int, id: IdSpec)
  case delete(at: Int)
  case replace(at: Int)
  case move(at: Int, to: Int)
}

private struct Draw {
  let random: () -> Double

  func int(_ min: Int, _ max: Int) -> Int {
    min + Int((random() * Double(max - min + 1)).rounded(.down))
  }
  func nat() -> Int { int(0, 2_147_483_647) }
  func idString(_ min: Int, _ max: Int) -> String {
    String((0..<int(min, max)).map { _ in HANDLE_ALPHABET[int(0, HANDLE_ALPHABET.count - 1)] })
  }
  /// fast-check's `oneof` with weights 3 (random), 1 (short), 4 (shared).
  func idSpec() -> IdSpec {
    let pick = int(0, 7)
    if pick < 3 {
      return .random(idString(21, 21))
    }
    if pick < 4 {
      return .short(idString(1, 3))
    }
    return .shared(base: nat(), prefix: int(1, 10), tail: idString(1, 17))
  }
  func edit() -> Edit {
    switch int(0, 3) {
    case 0: .insert(at: nat(), id: idSpec())
    case 1: .delete(at: nat())
    case 2: .replace(at: nat())
    default: .move(at: nat(), to: nat())
    }
  }
}

private func resolveId(_ spec: IdSpec, _ earlier: [String]) -> String {
  switch spec {
  case .random(let id), .short(let id): id
  case .shared(let base, let prefix, let tail):
    earlier.isEmpty ? tail : String(earlier[base % earlier.count].prefix(prefix)) + tail
  }
}

/// Checks every invariant on the current state, the typo property included.
private func check(_ editor: Editor, _ context: String) throws {
  try editor.read {
    let blocks = try getRoot().getChildren()
    let ids = try blocks.map { try #require(try getBlockId($0)) }
    #expect(Set(ids).count == ids.count, "\(context)")
    let handles = try getBlockHandles()
    #expect(handles.size == ids.count, "\(context)")
    var handleOfId: [String: String] = [:]
    for (i, block) in blocks.enumerated() {
      let id = ids[i]
      let handle = try #require(try getBlockHandle(block))
      #expect(handle == bruteForceHandle(id, ids), "\(context) handle of \(id)")
      #expect(handles.get(id) == handle, "\(context)")
      handleOfId[id] = handle
      // A whole id is accepted when it is longer than any handle.
      for input in id.count > HANDLE_KEY_LENGTH ? [handle, id] : [handle] {
        let result = try resolveBlockHandle(input)
        #expect(result.node?.is(block) == true, "\(context): \(input) → \(result.status)")
      }
    }
    let allHandles = Set(handleOfId.values)
    for (i, block) in blocks.enumerated() {
      let handle = handleOfId[ids[i]]!
      for s in oneCharTypos(handle) {
        let result = try resolveBlockHandle(s)
        if case .found(let node) = result, !node.is(block) {
          let otherId = try #require(try getBlockId(node))
          let otherHandle = handleOfId[otherId]!
          #expect(
            s == otherId || (handleKey(otherId).hasPrefix(s) && s.hasPrefix(otherHandle)),
            "\(context): typo \(s) of \(handle) named \(otherId) (handle \(otherHandle))")
        } else if case .ambiguous(_, let candidates) = result {
          for candidate in candidates {
            #expect(allHandles.contains(candidate), "\(context)")
          }
        }
      }
    }
  }
}

private func runDocument(_ specs: [IdSpec], _ batches: [[Edit]]) throws {
  var ids: [String] = []
  for spec in specs {
    let id = resolveId(spec, ids)
    if !ids.contains(id) {
      ids.append(id)
    }
  }
  var n = 0
  let editor = try asEdited(
    buildEditorFromExtensions(
      defineExtension(
        name: "[block-handle-property]",
        dependencies: [
          RichTextExtension,
          // Only for blocks whose id is taken (edits bring their own ids):
          // longer than any generated id, so never one.
          configExtension(BlockIdExtension) {
            $0.generateId = {
              n += 1
              return String(repeating: "F", count: 28) + String(n)
            }
          },
        ], editorConfig: InitialEditorConfig(initialEditorState: InitialEditorStateType.none))))
  defer { editor.dispose() }
  try discrete(editor) {
    for id in ids {
      let block = try paragraph(id)
      try setState(block, blockIdState, id)
      try getRoot().append(block)
    }
  }
  var previous = try editor.read { try getBlockHandles() }
  try check(editor, "initial \(ids)")
  for (step, batch) in batches.enumerated() {
    // Each batch of edits is one update.
    try discrete(editor) {
      for op in batch {
        let blocks = try getRoot().getChildren()
        let index =
          switch op {
          case .insert(let at, _), .delete(let at), .replace(let at), .move(let at, _): at
          }
        let at = blocks[index % blocks.count]
        if case .insert = op {
        } else if blocks.count >= 3 {
          switch op {
          case .delete: try at.remove()
          case .move(_, let to):
            let target = blocks[to % blocks.count]
            if !target.is(at) {
              try target.insertAfter(at)
            }
          default: try at.replace(paragraph("r\(step)"))
          }
          continue
        }
        // Blocks inserted earlier in this update have no id yet.
        let current = try blocks.compactMap { try getBlockId($0) }
        let block = try paragraph("t\(step)")
        if case .insert(_, let id) = op {
          try setState(block, blockIdState, resolveId(id, current))
        }
        try at.insertAfter(block)
      }
    }
    // The handles of the previous read name the blocks they named then,
    // while those exist.
    let read = previous
    try editor.read {
      for (id, handle) in read {
        let result = try resolveBlockHandle(handle, BlockHandleOptions(read: read))
        if let node = try getBlockById(id) {
          #expect(result.node?.is(node) == true, "step \(step) \(handle) → \(result.status)")
        } else {
          #expect(result.status == "not-found", "step \(step) \(handle)")
        }
      }
    }
    previous = try editor.read { try getBlockHandles() }
    try check(editor, "step \(step)")
  }
}

@Suite struct BlockHandlePropertyTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/BlockIdExtension.handles.property.test.ts::block handles: property > handles, resolution and typo safety under random ids and edits"
    ))
  func randomIdsAndEdits() throws {
    // Upstream's example: an insert that repeats an id (its copy gets a
    // fallback id), with every short counter-like id already taken.
    try runDocument(
      (1...32).map { .random("zz\($0)") }, [[.insert(at: 0, id: .short("zz1"))]])
    let draw = Draw(random: mulberry32(2024))
    for _ in 0..<15 {
      let specs = (0..<draw.int(3, 40)).map { _ in draw.idSpec() }
      let batches = (0..<draw.int(0, 10)).map { _ in (0..<draw.int(1, 3)).map { _ in draw.edit() } }
      try runDocument(specs, batches)
    }
  }
}
