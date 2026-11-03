import Lexical
import LexicalExtension
import Testing

// The signals port has no upstream unit tests of its own. This session's
// expected log was recorded by running the same steps against
// @preact/signals-core 1.14.4 in Node, so it pins effect order, batching,
// computed laziness, untracked reads, watched callbacks and the
// same-value check.
@Suite struct SignalsTests {
  @Test func matchesPreactSession() throws {
    var log: [String] = []
    let a = signal(0)
    try effect { () -> (() -> Void)? in
      log.append("e1:\(a.value)")
      return { log.append("c1") }
    }
    try effect { log.append("e2:\(a.value)") }
    a.value = 1
    try batch {
      a.value = 2
      a.value = 3
      log.append("in-batch")
    }

    var runs = 0
    let b = signal(1)
    let c = computed {
      runs += 1
      return b.value * 2
    }
    log.append("runs0:\(runs)")
    log.append("c:\(c.value)")
    log.append("c:\(c.value)")
    log.append("runs:\(runs)")
    b.value = 2
    log.append("runs-after-set:\(runs)")
    log.append("c:\(c.value) runs:\(runs)")

    let d = signal(1)
    let e = signal(1)
    try effect { log.append("u:\(d.value),\(untracked { e.value })") }
    e.value = 2
    d.value = 2

    let w = signal(
      0,
      SignalOptions(
        watched: { _ in log.append("watched") }, unwatched: { _ in log.append("unwatched") }))
    let dispose = try effect { _ = w.value }
    dispose()

    let f = signal(5)
    try effect { log.append("f:\(f.value)") }
    f.value = 5

    let g = signal(0)
    let h = signal(0)
    try effect { log.append("h:\(h.value)") }
    try effect {
      if g.value > 0 { h.value = g.value * 10 }
      log.append("g:\(g.value)")
    }
    g.value = 1

    #expect(
      log == [
        "e1:0", "e2:0", "c1", "e1:1", "e2:1", "in-batch", "c1", "e1:3", "e2:3", "runs0:0", "c:2",
        "c:2", "runs:1", "runs-after-set:1", "c:4 runs:2", "u:1,1", "u:2,2", "watched",
        "unwatched", "f:5", "h:0", "g:0", "g:1", "h:10",
      ])
  }
}
