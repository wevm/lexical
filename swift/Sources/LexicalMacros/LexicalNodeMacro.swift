import SwiftCompilerPlugin
import SwiftDiagnostics
import SwiftSyntax
import SwiftSyntaxBuilder
import SwiftSyntaxMacros

// `@LexicalNode` (Q30): generates a node class's `config()` from its
// `@Field` properties, and their `get`/`set` accessors. Clone, JSON import
// and export in both forms, and schema key order come from that config at
// run time, as they come from `$config` in JS.

struct MacroError: Error, CustomStringConvertible {
  let description: String
  init(_ description: String) { self.description = description }
}

/// One `@Field` property of a node class.
struct FieldDecl {
  /// The stored property, such as `__label`.
  let name: String
  /// Its type as written, for the accessors; `nil` when it has none.
  let type: String?
  /// The JSON key: the attribute's string, or the name without leading
  /// underscores.
  let key: String
  /// The schema expression, as written.
  let schema: String
  /// The name accessors are made from: the name without leading underscores.
  let accessorBase: String
  let accessors: Bool
}

public struct LexicalNodeMacro: MemberMacro {
  public static func expansion(
    of node: AttributeSyntax,
    providingMembersOf declaration: some DeclGroupSyntax,
    conformingTo protocols: [TypeSyntax],
    in context: some MacroExpansionContext
  ) throws -> [DeclSyntax] {
    guard let classDecl = declaration.as(ClassDeclSyntax.self) else {
      throw MacroError("@LexicalNode applies to a class that extends a node class")
    }
    let className = classDecl.name.trimmedDescription
    let isPublic = classDecl.modifiers.contains {
      ["public", "open"].contains($0.name.text)
    }
    let access = isPublic ? "public " : ""

    var leading: [String] = []
    var trailing: [String] = []
    for argument in node.arguments?.as(LabeledExprListSyntax.self) ?? [] {
      let text = argument.trimmedDescription.trimmingTrailingComma()
      if let label = argument.label?.text, label == "type" || label == "extends" {
        leading.append(text)
      } else {
        trailing.append(text)
      }
    }

    let fields = try classDecl.memberBlock.members.compactMap { member in
      try field(member.decl)
    }
    var arguments = ["\(className).self"] + leading
    if !fields.isEmpty {
      let entries = fields.map {
        "SchemaField(\(literal($0.key)), \($0.schema), field: \\\(className).\($0.name))"
      }
      arguments.append(
        "json: [\n" + entries.map { "      \($0)," }.joined(separator: "\n") + "\n    ]")
    }
    arguments += trailing

    var decls: [DeclSyntax] = [
      """
      \(raw: access)override class func config() throws -> NodeConfig? {
        try NodeConfig(
          \(raw: arguments.joined(separator: ",\n    ")))
      }
      """
    ]
    for field in fields where field.accessors {
      guard let type = field.type else {
        throw MacroError(
          "@Field var \(field.name) needs a type annotation for its accessors, or accessors: false")
      }
      let base = field.accessorBase.prefix(1).uppercased() + field.accessorBase.dropFirst()
      decls.append(
        """
        \(raw: access)func get\(raw: base)() throws -> \(raw: type) {
          try getLatest().\(raw: field.name)
        }
        """)
      decls.append(
        """
        @discardableResult
        \(raw: access)func set\(raw: base)(_ value: \(raw: type)) throws -> Self {
          let writable = try getWritable()
          writable.\(raw: field.name) = value
          return writable
        }
        """)
    }
    return decls
  }

  /// The `@Field` property `decl` declares, if any.
  static func field(_ decl: DeclSyntax) throws -> FieldDecl? {
    guard let variable = decl.as(VariableDeclSyntax.self) else { return nil }
    guard
      let attribute = variable.attributes.compactMap({ $0.as(AttributeSyntax.self) }).first(
        where: { $0.attributeName.trimmedDescription == "Field" })
    else { return nil }
    guard variable.bindings.count == 1, let binding = variable.bindings.first,
      let name = binding.pattern.as(IdentifierPatternSyntax.self)?.identifier.text
    else {
      throw MacroError("@Field applies to one stored property")
    }
    let arguments = Array(attribute.arguments?.as(LabeledExprListSyntax.self) ?? [])
    let unlabeled = arguments.filter { $0.label == nil }
    let base = String(name.drop { $0 == "_" })
    let key: String
    let schema: ExprSyntax
    if unlabeled.count >= 2,
      let literal = unlabeled[0].expression.as(StringLiteralExprSyntax.self),
      let value = literal.representedLiteralValue
    {
      key = value
      schema = unlabeled[1].expression
    } else if let first = unlabeled.first {
      key = base
      schema = first.expression
    } else {
      throw MacroError("@Field needs a schema, such as @Field(stringValue())")
    }
    let accessors =
      arguments.first { $0.label?.text == "accessors" }?.expression.trimmedDescription != "false"
    return FieldDecl(
      name: name, type: binding.typeAnnotation?.type.trimmedDescription, key: key,
      schema: schema.trimmedDescription, accessorBase: base, accessors: accessors)
  }

  static func literal(_ s: String) -> String {
    StringLiteralExprSyntax(content: s).description
  }
}

/// `@Field`: marks a stored property for `@LexicalNode`, which reads it.
public struct FieldMacro: PeerMacro {
  public static func expansion(
    of node: AttributeSyntax,
    providingPeersOf declaration: some DeclSyntaxProtocol,
    in context: some MacroExpansionContext
  ) throws -> [DeclSyntax] {
    []
  }
}

extension String {
  fileprivate func trimmingTrailingComma() -> String {
    hasSuffix(",") ? String(dropLast()).trimmingCharacters() : self
  }

  fileprivate func trimmingCharacters() -> String {
    var s = Substring(self)
    while s.last == " " { s = s.dropLast() }
    return String(s)
  }
}

@main
struct LexicalMacrosPlugin: CompilerPlugin {
  let providingMacros: [Macro.Type] = [LexicalNodeMacro.self, FieldMacro.self]
}
