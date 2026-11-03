# Lexical for Swift

A native Swift port of [Lexical](https://lexical.dev) for iOS and macOS, with
UIKit, AppKit, and SwiftUI editor views. It conforms to the upstream JS
packages: the same documents, the same JSON (byte for byte), and the same
editing behavior.

> **Pre-release.** The API is unstable until 1.0, and most of Lexical isn't
> ported yet. See [DESIGN.md](DESIGN.md) for the plan.

Tracks upstream Lexical just after **0.53.0** (see `UPSTREAM_VERSION` and
[DESIGN.md](DESIGN.md)); the first release will name the upstream release it
conforms to.

## Installation

```swift
.package(url: "https://github.com/wevm/lexical-swift", from: "0.1.0")
```

Requires iOS 27 or macOS 27 and Swift 6.4. The non-UI modules also build on
Linux.

## Development

This package is developed in [wevm/lexical](https://github.com/wevm/lexical),
a fork of [meta/lexical](https://github.com/meta/lexical), under `swift/`.
[wevm/lexical-swift](https://github.com/wevm/lexical-swift) is a read-only
mirror for SwiftPM; open issues and changes against wevm/lexical.

```sh
cd swift
swift build --build-tests && swift test

# Conformance vectors and the parity ledger, from the repository root:
pnpm --filter lexical-conformance run generate      # rewrite vectors/
pnpm --filter lexical-conformance run ledger        # rewrite parity/upstream-tests.json
pnpm --filter lexical-conformance run check-ledger  # ported/skipped counts per package
```

## License

MIT. Lexical is copyright Meta Platforms, Inc. and affiliates.
