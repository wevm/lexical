import Lexical

/// `NamedSignalsOutput<Config>`: one signal per config property, read as
/// `output.disabled.value`, each starting at the config's value.
@dynamicMemberLookup
public final class NamedSignals<Config> {
  private let defaults: Config
  private let options: Partial<Config>
  private var signals: [AnyKeyPath: AnyObject] = [:]

  init(_ defaults: Config, _ options: Partial<Config>) {
    self.defaults = defaults
    self.options = options
  }

  public subscript<V>(dynamicMember key: WritableKeyPath<Config, V>) -> Signal<V> {
    if let existing = signals[key] {
      return existing as! Signal<V>
    }
    let store = Signal(options[dynamicMember: key] ?? defaults[keyPath: key])
    signals[key] = store
    return store
  }
}

/// `namedSignals(defaults, opts)`: a signal for each property of
/// `defaults`, starting at the value in `opts` when it sets one.
public func namedSignals<Config>(
  _ defaults: Config, _ options: Partial<Config> = Partial()
) -> NamedSignals<Config> {
  NamedSignals(defaults, options)
}

/// `watchedSignal(getSnapshot, register)`: a signal that, while something
/// watches it, takes `getSnapshot()` and keeps what `register` sets up.
public func watchedSignal<T>(
  _ getSnapshot: @escaping () -> T, _ register: @escaping (Signal<T>) -> () -> Void
) -> Signal<T> {
  var dispose: (() -> Void)?
  return signal(
    getSnapshot(),
    SignalOptions(
      watched: { this in
        this.value = getSnapshot()
        dispose = register(this)
      },
      unwatched: { _ in
        if let d = dispose {
          d()
          dispose = nil
        }
      }))
}
