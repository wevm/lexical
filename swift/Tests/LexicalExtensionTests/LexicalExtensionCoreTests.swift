import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical/src/__tests__/unit/LexicalExtensionCore.test.ts.
// The upstream cases check TypeScript inference; here each assigns the
// result to a variable of the expected type, so the compiler checks it.
// JS `undefined` output and `never` init are `Void`; `{}` config is
// `EmptyConfig`.

@Suite struct LexicalExtensionCoreTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > does not change identity"
    ))
  func doesNotChangeIdentity() {
    let extensionArg: LexicalExtension<EmptyConfig, Void, Void> = LexicalExtension(
      name: "test", config: EmptyConfig(), init: nil, build: nil)
    // `defineExtension` builds the extension from its fields, so identity
    // is that of the value it returns, which it keeps.
    let extension_ = safeCast(extensionArg)
    #expect(extension_ === extensionArg)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the expected type (base case)"
    ),
    .upstream(
      "lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > can define an extension without config"
    ))
  func infersBaseCase() {
    let extension_: LexicalExtension<EmptyConfig, Void, Void> = defineExtension(name: "test")
    #expect(extension_.name == "test")
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the expected type (config inference)"
    ))
  func infersConfig() {
    struct NumberConfig { var number = 123 }
    let extension_: LexicalExtension<NumberConfig, Void, Void> = defineExtension(
      name: "test", config: NumberConfig())
    #expect(extension_.config.number == 123)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the expected type (output inference)"
    ))
  func infersOutput() throws {
    struct Output { var output: Int }
    let extension_: LexicalExtension<EmptyConfig, Output, Void> = defineExtension(
      name: "test", build: { _, _, _ in Output(output: 321) })
    let editor = try buildEditorFromExtensions(extension_)
    defer { editor.dispose() }
    #expect(try getExtensionDependencyFromEditor(editor, extension_).output.output == 321)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::defineExtension > infers the correct init type"
    ))
  func infersInit() {
    let extension_: LexicalExtension<EmptyConfig, Void, String> = defineExtension(
      name: "test", init: { _, _, _ in "string" })
    #expect(extension_.hasInit)
  }

  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalExtensionCore.test.ts::declarePeerDependency > validates the type argument"
    ))
  func declarePeerDependencyValidates() throws {
    struct OtherConfig { var other = true }
    let dep = declarePeerDependency("other")
    #expect(dep.name == "other")
    #expect(dep.config == nil)
    let configured = declarePeerDependency("other", config: OtherConfig.self) { $0.other = false }
    #expect(configured.name == "other")
    let partial = try #require(configured.config as? Partial<OtherConfig>)
    #expect(partial.other == false)
    // The config type is checked when the peer's configs merge.
  }
}
