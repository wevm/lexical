// The code languages the playground offers, in one place so a highlighter
// can replace the list with its own: the web toolbar's
// `CODE_LANGUAGE_OPTIONS_PRISM`, with `@lexical/code-prism`'s
// `CODE_LANGUAGE_FRIENDLY_NAME_MAP` names and `CODE_LANGUAGE_MAP` aliases
// (FacadePrism.ts).

/// The language picker's options, `[value, friendly name]`, in the web's order.
let codeLanguageOptions: [(value: String, name: String)] = [
  ("c", "C"), ("clike", "C-like"), ("cpp", "C++"), ("css", "CSS"), ("go", "Go"),
  ("html", "HTML"), ("java", "Java"), ("js", "JavaScript"), ("markdown", "Markdown"),
  ("objc", "Objective-C"), ("plain", "Plain Text"), ("powershell", "PowerShell"),
  ("py", "Python"), ("rust", "Rust"), ("sql", "SQL"), ("swift", "Swift"),
  ("typescript", "TypeScript"), ("xml", "XML"),
]

private let codeLanguageAliases = [
  "cpp": "cpp", "golang": "go", "java": "java", "javascript": "js", "md": "markdown",
  "plaintext": "plain", "python": "py", "text": "plain", "ts": "typescript",
]

/// `normalizeCodeLanguage`: a language's canonical name.
func normalizeCodeLanguage(_ language: String) -> String {
  codeLanguageAliases[language] ?? language
}

/// `getLanguageFriendlyName`: the display name of a language, or the
/// language itself where the list has none.
func getLanguageFriendlyName(_ language: String) -> String {
  let language = normalizeCodeLanguage(language)
  return codeLanguageOptions.first { $0.value == language }?.name ?? language
}
