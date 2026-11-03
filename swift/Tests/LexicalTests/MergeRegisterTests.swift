import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/mergeRegister.test.ts.

@Suite struct MergeRegisterTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/mergeRegister.test.ts::mergeRegister > calls all of the clean-up functions"
    ))
  func callsAllCleanups() {
    var calls = 0
    let cleanup = { calls += 1 }
    mergeRegister(cleanup, cleanup)()
    #expect(calls == 2)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/mergeRegister.test.ts::mergeRegister > calls the clean-up functions in reverse order"
    ))
  func callsCleanupsInReverse() {
    var calls: [Int] = []
    let unregister = mergeRegister({ calls.append(1) }, { calls.append(2) })
    unregister()
    #expect(calls == [2, 1])
    unregister()
    #expect(calls == [2, 1])
  }
}
