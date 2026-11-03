import Testing

/// Marks a Swift test as the port of an upstream Lexical unit test.
///
/// `id` is the test's entry in `swift/parity/upstream-tests.json`:
/// `<file under packages/>::<describe > ... > title>`. The parity ledger
/// (`pnpm run check-ledger` in packages/lexical-conformance) finds these by
/// reading the Swift sources, so write `id` as one string literal.
///
/// ```swift
/// @Test(.upstream("lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor > read"))
/// func read() { ... }
/// ```
public struct Upstream: TestTrait, Sendable {
  public let id: String
}

extension Trait where Self == Upstream {
  public static func upstream(_ id: String) -> Self { Upstream(id: id) }
}
