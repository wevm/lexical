import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalRefCountedRegistry.test.ts.

@Suite struct LexicalRefCountedRegistryTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > activates on the first registration and passes options through"
    ))
  func activatesWithOptions() {
    var calls: [(String, Int?)] = []
    let registry = createRefCountedRegistry { (key: String, options: Int?) in
      calls.append((key, options))
      return {}
    }
    _ = registry.register("a", 1)
    #expect(calls.count == 1)
    #expect(calls.first?.0 == "a" && calls.first?.1 == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > reference counts: activates once, disposes only after the last release"
    ))
  func referenceCounts() {
    var activations = 0
    var disposals = 0
    let registry = createRefCountedRegistry { (_: String) in
      activations += 1
      return { disposals += 1 }
    }
    let releaseA = registry.register("k")
    let releaseB = registry.register("k")
    #expect(activations == 1)
    releaseA()
    #expect(disposals == 0)
    releaseB()
    #expect(disposals == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > distinct keys activate and dispose independently"
    ))
  func distinctKeys() {
    var disposed: [String] = []
    let registry = createRefCountedRegistry { (key: String) in { disposed.append(key) } }
    _ = registry.register("a")
    let releaseB = registry.register("b")
    releaseB()
    #expect(disposed == ["b"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > the disposer is idempotent"
    ))
  func disposerIsIdempotent() {
    var disposals = 0
    let registry = createRefCountedRegistry { (_: String) in { disposals += 1 } }
    let release = registry.register("k")
    release()
    release()
    #expect(disposals == 1)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > re-registering a released key creates a fresh activation"
    ))
  func reRegisteringActivatesAgain() {
    var activations = 0
    var disposals = 0
    let registry = createRefCountedRegistry { (_: String) in
      activations += 1
      return { disposals += 1 }
    }
    registry.register("k")()
    #expect(disposals == 1)
    _ = registry.register("k")
    #expect(activations == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > a stale disposer does not affect a fresh entry for the same key"
    ))
  func staleDisposerIgnored() {
    var disposed: [Int] = []
    var call = 0
    let registry = createRefCountedRegistry { (_: String) in
      call += 1
      let n = call
      return { disposed.append(n) }
    }
    let staleRelease = registry.register("k")
    staleRelease()
    _ = registry.register("k")
    staleRelease()
    #expect(disposed == [1])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > dispose() tears down every live registration"
    ))
  func disposeTearsDownAll() {
    var disposed: [String] = []
    let registry = createRefCountedRegistry { (key: String) in { disposed.append(key) } }
    _ = registry.register("a")
    _ = registry.register("b")
    _ = registry.register("b")
    registry.dispose()
    #expect(disposed == ["a", "b"])
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalRefCountedRegistry.test.ts::createRefCountedRegistry > a disposer held after registry.dispose() is a no-op"
    ))
  func disposerAfterDisposeIsNoOp() {
    var disposals = 0
    let registry = createRefCountedRegistry { (_: String) in { disposals += 1 } }
    let release = registry.register("k")
    registry.dispose()
    #expect(disposals == 1)
    release()
    #expect(disposals == 1)
  }
}
