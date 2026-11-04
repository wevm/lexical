/// `makeStepwiseIterator`: a lazy iterator whose steps may throw. JS
/// iterators are lazy and re-read the tree at each step, so a loop that
/// moves nodes sees the moves; this keeps that. It is not a `Sequence`,
/// because `IteratorProtocol.next()` can't throw: consume it with
/// `next()`, `forEach`, `map` or `toArray()`.
public final class StepwiseIterator<State, Value> {
  private var state: State?
  private let hasNext: (State) throws -> Bool
  private let step: (State) throws -> State?
  private let mapFn: (State) throws -> Value

  public init(
    initial: State?, hasNext: @escaping (State) throws -> Bool,
    step: @escaping (State) throws -> State?, map: @escaping (State) throws -> Value
  ) {
    self.state = initial
    self.hasNext = hasNext
    self.step = step
    self.mapFn = map
  }

  public func next() throws -> Value? {
    guard let current = state, try hasNext(current) else {
      state = nil
      return nil
    }
    let value = try mapFn(current)
    state = try step(current)
    return value
  }

  public func forEach(_ body: (Value) throws -> Void) throws {
    while let value = try next() { try body(value) }
  }

  public func map<T>(_ transform: (Value) throws -> T) throws -> [T] {
    var result: [T] = []
    while let value = try next() { result.append(try transform(value)) }
    return result
  }

  public func toArray() throws -> [Value] { try map { $0 } }
}

/// A lazy iterator built from a `next` function.
public final class LazyIterator<Value> {
  private let nextFn: () throws -> Value?
  public init(_ next: @escaping () throws -> Value?) { self.nextFn = next }
  public func next() throws -> Value? { try nextFn() }
  public func forEach(_ body: (Value) throws -> Void) throws {
    while let value = try next() { try body(value) }
  }
  public func map<T>(_ transform: (Value) throws -> T) throws -> [T] {
    var result: [T] = []
    while let value = try next() { result.append(try transform(value)) }
    return result
  }
  public func toArray() throws -> [Value] { try map { $0 } }
}
