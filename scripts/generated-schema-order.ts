import ts from "typescript";

type SchemaDeclaration = {
  name: string;
  statement: ts.VariableStatement;
  initializer: ts.Expression;
  dependencies: Set<string>;
};

function valueReferences(node: ts.Node, names: Set<string>): Set<string> {
  const references = new Set<string>();
  const visit = (current: ts.Node) => {
    // Type queries/generics and deferred refinement callbacks do not read a
    // schema while its enclosing declaration is initialized.
    if (
      ts.isTypeNode(current) ||
      ts.isArrowFunction(current) ||
      ts.isFunctionExpression(current)
    )
      return;
    if (ts.isIdentifier(current) && names.has(current.text)) {
      const parent = current.parent;
      const propertyName =
        (ts.isPropertyAccessExpression(parent) && parent.name === current) ||
        ((ts.isPropertyAssignment(parent) || ts.isMethodDeclaration(parent)) &&
          parent.name === current);
      if (!propertyName) references.add(current.text);
    }
    ts.forEachChild(current, visit);
  };
  visit(node);
  return references;
}

/** Order eager generated schema values without printing or changing their AST. */
export function orderGeneratedSchemas(source: string): string {
  const diagnostics = ts.transpileModule(source, {
    fileName: "generated-types.ts",
    reportDiagnostics: true,
    compilerOptions: {
      target: ts.ScriptTarget.Latest,
      module: ts.ModuleKind.ESNext,
    },
  }).diagnostics;
  const syntaxError = diagnostics?.find(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
  );
  if (syntaxError)
    throw new Error(
      `Invalid generated TypeScript: ${ts.flattenDiagnosticMessageText(syntaxError.messageText, " ")}`,
    );
  const file = ts.createSourceFile(
    "generated-types.ts",
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TS,
  );
  const schemas: SchemaDeclaration[] = [];
  const names = new Set<string>();
  for (const statement of file.statements) {
    if (
      !ts.isVariableStatement(statement) ||
      !statement.modifiers?.some(
        (modifier) => modifier.kind === ts.SyntaxKind.ExportKeyword,
      )
    )
      continue;
    const declarations = statement.declarationList.declarations;
    const schema = declarations.find(
      (declaration) =>
        ts.isIdentifier(declaration.name) &&
        declaration.name.text.endsWith("Schema"),
    );
    if (!schema) continue;
    if (
      declarations.length !== 1 ||
      !(statement.declarationList.flags & ts.NodeFlags.Const) ||
      !ts.isIdentifier(schema.name) ||
      !schema.initializer
    )
      throw new Error("Unsupported generated schema declaration");
    const name = schema.name.text;
    if (names.has(name))
      throw new Error(`Duplicate generated schema declaration ${name}`);
    names.add(name);
    schemas.push({
      name,
      statement,
      initializer: schema.initializer,
      dependencies: new Set(),
    });
  }
  for (const schema of schemas)
    schema.dependencies = valueReferences(schema.initializer, names);

  // Source-order priority among ready declarations makes unrelated choices
  // deterministic. An eager cycle cannot be repaired by declaration ordering.
  const remaining = new Set(names);
  const ordered: SchemaDeclaration[] = [];
  while (remaining.size) {
    const ready = schemas.find(
      (schema) =>
        remaining.has(schema.name) &&
        [...schema.dependencies].every((name) => !remaining.has(name)),
    );
    if (!ready)
      throw new Error(
        `Generated schema dependency cycle: ${[...remaining].join(", ")}`,
      );
    remaining.delete(ready.name);
    ordered.push(ready);
  }

  // Replace only schema statement slots. Imports, helpers, type aliases and
  // declaration/initializer tokens remain verbatim; type aliases may precede
  // their values safely because TypeScript erases them at runtime.
  let result = "";
  let cursor = 0;
  for (const [index, schema] of schemas.entries()) {
    const replacement = ordered[index].statement;
    result += source.slice(cursor, schema.statement.getFullStart());
    result += source.slice(replacement.getFullStart(), replacement.end);
    cursor = schema.statement.end;
  }
  return result + source.slice(cursor);
}
