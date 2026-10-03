// tauri-typegen 0.4 does not understand Serde rename_all, nullable Option,
// or injected Webview. Apply source-derived corrections for collaboration DTOs.
// The generated package remains generated; Rust is the source of truth.
const root = new URL("../", import.meta.url);
const domain = await Bun.file(
  new URL("crates/collaboration/src/domain.rs", root),
).text();
const error = await Bun.file(
  new URL("crates/collaboration/src/error.rs", root),
).text();
const detail = await Bun.file(
  new URL("crates/collaboration/src/detail.rs", root),
).text();
const contextualCapabilities = await Bun.file(
  new URL("crates/collaboration/src/contextual_capabilities.rs", root),
).text();
const output = new URL("packages/commands/src/types.ts", root);
let generated = await Bun.file(output).text();
const snake = (value: string) =>
  value.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();

for (const source of [domain, error, detail, contextualCapabilities]) {
  for (const match of source.matchAll(
    /#\[serde\(rename_all = "snake_case"\)\]\s*pub enum (\w+)\s*\{([^}]+)\}/g,
  )) {
    const [, name, body] = match;
    const variants = body
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
    const pattern = new RegExp(
      `export const ${name}Schema = z\\.enum\\(\\[[^\\]]+\\]\\);`,
    );
    if (!pattern.test(generated))
      throw new Error(`Missing generated enum ${name}`);
    generated = generated.replace(
      pattern,
      `export const ${name}Schema = z.enum(${JSON.stringify(variants.map(snake))});`,
    );
  }
  for (const match of source.matchAll(/pub struct (\w+)\s*\{([^}]+)\}/g)) {
    const [, name, body] = match;
    if (
      !source.slice(0, match.index).split(/\n\n/).at(-1)?.includes("Serialize")
    )
      continue;
    const pattern = new RegExp(
      `(export const ${name}Schema = z\\.object\\(\\{)([\\s\\S]*?)(\\n\\}\\);)`,
    );
    const schema = generated.match(pattern);
    // Wake hints are event-only; the generator emits command-reachable DTOs.
    if (!schema && name === "ChangeHint") continue;
    if (!schema) throw new Error(`Missing generated schema ${name}`);
    let fields = schema[2];
    for (const field of body.matchAll(/pub (\w+): Option</g)) {
      const fieldPattern = new RegExp(
        `(${field[1]}: [^\\n]+)\\.optional\\(\\)`,
      );
      if (!fieldPattern.test(fields))
        throw new Error(`Missing nullable field ${name}.${field[1]}`);
      fields = fields.replace(fieldPattern, "$1.nullable()");
    }
    generated = generated.replace(pattern, `$1${fields}$3`);
  }
}
generated = generated.replace(
  /(export const Collaboration\w+ParamsSchema = z\.object\(\{)([\s\S]*?)(\n\}\);)/g,
  (_, open, fields, close) =>
    `${open}${fields.replace(/view: WebviewSchema,?/g, "")}${close}`,
);
if (generated.includes("WebviewSchema"))
  throw new Error("Unexpected injected Webview remains in generated types");
generated = generated.replace(/[\t ]+$/gm, "");
await Bun.write(output, generated);
