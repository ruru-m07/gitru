// tauri-typegen 0.4 does not understand Serde rename_all, nullable Option,
// adjacent-tag payload dependencies, or injected Webview. Apply source-derived
// corrections for collaboration DTOs.
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
const participants = await Bun.file(
  new URL("crates/collaboration/src/participants.rs", root),
).text();
const contextualCapabilities = await Bun.file(
  new URL("crates/collaboration/src/contextual_capabilities.rs", root),
).text();
const resourceMetadata = await Bun.file(
  new URL("crates/collaboration/src/resource_metadata.rs", root),
).text();
const demand = await Bun.file(
  new URL("crates/collaboration/src/demand.rs", root),
).text();
const localLinks = await Bun.file(
  new URL("crates/collaboration/src/local_links.rs", root),
).text();
const notificationSubjects = await Bun.file(
  new URL("crates/collaboration/src/notification_subjects.rs", root),
).text();
const gitRemotes = await Bun.file(
  new URL("crates/git/models/remotes.rs", root),
).text();
const linkCommands = await Bun.file(
  new URL(
    "apps/desktop/src-tauri/src/commands/collaboration_local_links.rs",
    root,
  ),
).text();
// Repository registration receives the serialized local metadata back from
// add_local_git_repo. Rust Option is null here too, including safe missing origin.
const repositoryInfo = await Bun.file(
  new URL("crates/ipc/src/repo_manager.rs", root),
).text();
const repositoryCommands = await Bun.file(
  new URL("crates/ipc/src/commands.rs", root),
).text();
const output = new URL("packages/commands/src/types.ts", root);
let generated = await Bun.file(output).text();
const snake = (value: string) =>
  value.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
// These serialized cache/parser types are native-only and deliberately absent
// from renderer command signatures. If reachable later, correct them normally.
const nativeOnlyTypes = new Set([
  "NotificationSubjectKind",
  "NotificationSubjectRepresentation",
  "NotificationSubjectFallbackReason",
  "NotificationSubjectSelector",
  "DetailEnumeration",
  "DetailHeadScope",
  "DetailReconciliation",
]);

// The pinned generator flattens Serde tuple variants into string literals and
// never visits their payload structs. Emit that missing graph from Rust before
// applying the ordinary Option/null correction below. Deliberately fail on an
// unsupported shape rather than inventing a renderer-owned wire model.
const payloadStructs = new Map(
  [...participants.matchAll(/pub struct (\w+)\s*\{([^}]+)\}/g)].map(
    ([, name, body]) => [name, body] as const,
  ),
);
const nativePayloadKinds = new Set<string>();
for (const [, tag, content, name, body] of participants.matchAll(
  /#\[serde\(tag = "([^"]+)", content = "([^"]+)"\)\]\s*pub enum (\w+)\s*\{([^}]+)\}/g,
)) {
  const dependencies: string[] = [];
  const visiting = new Set<string>();
  const emitted = new Set<string>();
  const schemaFor = (rustType: string): string => {
    const optional = rustType.match(/^Option<(.+)>$/);
    if (optional) return `${schemaFor(optional[1])}.optional()`;
    if (rustType === "String") return "z.string()";
    if (rustType === "bool") return "z.boolean()";
    if (!/^\w+$/.test(rustType))
      throw new Error(`Unsupported tagged payload type ${rustType}`);
    if (generated.includes(`export const ${rustType}Schema =`))
      return `${rustType}Schema`;
    if (emitted.has(rustType)) return `${rustType}Schema`;
    if (visiting.has(rustType))
      throw new Error(`Recursive tagged payload ${rustType}`);
    const struct = payloadStructs.get(rustType);
    if (!struct) throw new Error(`Missing tagged payload struct ${rustType}`);
    visiting.add(rustType);
    const fields = [...struct.matchAll(/pub (\w+): ([^,\n]+),/g)];
    if (
      !fields.length ||
      struct.includes("#[") ||
      fields.length !== [...struct.matchAll(/\bpub\b/g)].length
    )
      throw new Error(`Unsupported tagged payload fields ${rustType}`);
    const properties = fields.map(
      ([, field, type]) => `  ${field}: ${schemaFor(type.trim())},`,
    );
    dependencies.push(
      `export const ${rustType}Schema = z.object({\n${properties.join("\n")}\n});\n\nexport type ${rustType} = z.infer<typeof ${rustType}Schema>;`,
    );
    visiting.delete(rustType);
    emitted.add(rustType);
    return `${rustType}Schema`;
  };
  const variants = [
    ...body.matchAll(
      /(?:#\[serde\(rename = "([^"]+)"\)\]\s*)?(\w+)\((\w+)\),/g,
    ),
  ];
  const remainder = body
    .replace(/(?:#\[serde\(rename = "([^"]+)"\)\]\s*)?(\w+)\((\w+)\),/g, "")
    .trim();
  if (!variants.length || remainder)
    throw new Error(`Unsupported adjacent-tag variants ${name}`);
  if (name === "NativeDetailPayload")
    for (const [, renamed, variant] of variants)
      nativePayloadKinds.add(renamed ?? variant);
  const schemas = variants.map(
    ([, renamed, variant, type]) =>
      `z.object({ ${JSON.stringify(tag)}: z.literal(${JSON.stringify(renamed ?? variant)}), ${JSON.stringify(content)}: ${schemaFor(type)} })`,
  );
  const pattern = new RegExp(
    `export const ${name}Schema = z\\.enum\\(\\[[^\\]]+\\]\\);`,
  );
  if (!pattern.test(generated))
    throw new Error(`Missing flattened tagged schema ${name}`);
  generated = generated.replace(
    pattern,
    `${dependencies.join("\n\n")}\n\nexport const ${name}Schema = z.discriminatedUnion(${JSON.stringify(tag)}, [${schemas.join(", ")}]);\n\nexport type ${name} = z.infer<typeof ${name}Schema>;`,
  );
}

for (const source of [
  domain,
  error,
  detail,
  participants,
  contextualCapabilities,
  resourceMetadata,
  demand,
  localLinks,
  notificationSubjects,
  gitRemotes,
  linkCommands,
  repositoryInfo,
  repositoryCommands,
]) {
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
    if (!pattern.test(generated) && nativeOnlyTypes.has(name)) continue;
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
    if (!schema && nativeOnlyTypes.has(name)) continue;
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
// Only this native family is admitted today. Future families must qualify an
// explicit guard extension rather than silently borrowing participant fields.
if (nativePayloadKinds.size !== 1 || !nativePayloadKinds.has("participant.v1"))
  throw new Error("Extend native detail field-family guards for this payload");
const participantFamily = detail.match(
  /fn is_participant\(self\) -> bool \{([\s\S]*?)\n    \}/,
);
if (!participantFamily)
  throw new Error("Missing Rust participant field-family declaration");
const participantFields = [
  ...participantFamily[1].matchAll(/Self::(\w+)/g),
].map(([, name]) => snake(name));
if (
  !participantFields.length ||
  new Set(participantFields).size !== participantFields.length
)
  throw new Error("Invalid Rust participant field-family declaration");
const entrySchema =
  /(export const DetailEntrySchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!entrySchema.test(generated))
  throw new Error("Missing generated detail entry schema for family guard");
generated = generated.replace(
  entrySchema,
  `const detailParticipantFields = new Set<string>(${JSON.stringify(participantFields)});\n\n$1.superRefine((entry, context) => {\n  const participant = entry.native !== null;\n  if (entry.field_mask.some((field) => detailParticipantFields.has(field) !== participant) || entry.field_validations.some((validation) => detailParticipantFields.has(validation.field) !== participant)) {\n    context.addIssue({ code: "custom", path: ["native"], message: "Detail entry fields do not match its native payload" });\n  }\n});`,
);
generated = generated.replace(
  /(export const Collaboration\w+ParamsSchema = z\.object\(\{)([\s\S]*?)(\n\}\);)/g,
  (_, open, fields, close) =>
    `${open}${fields.replace(/view: WebviewSchema,?/g, "")}${close}`,
);
if (generated.includes("WebviewSchema"))
  throw new Error("Unexpected injected Webview remains in generated types");
generated = generated.replace(/[\t ]+$/gm, "");
await Bun.write(output, generated);

// Event URLs remain unchanged on the wire. tauri-typegen 0.4 leaves ':' and
// other event-name separators in its function identifier, producing invalid TS.
const eventsOutput = new URL("packages/commands/src/events.ts", root);
const eventsFile = Bun.file(eventsOutput);
if (await eventsFile.exists()) {
  const identifiers = new Set<string>();
  let events = (await eventsFile.text()).replace(
    /^export async function ([^\s(]+)(?=\s*\()/gm,
    (_, original: string) => {
      const identifier = original.replace(
        /[^A-Za-z0-9_$]+([A-Za-z0-9_$]?)/g,
        (_separator, next: string) => next.toUpperCase(),
      );
      if (
        !/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(identifier) ||
        identifiers.has(identifier)
      )
        throw new Error("Invalid or colliding generated event identifier");
      identifiers.add(identifier);
      return `export async function ${identifier}`;
    },
  );
  // This generator imports Event even when all callbacks project just payload.
  const eventUses = events.replace(
    /^import[^\n]+from ["']@tauri-apps\/api\/event["'];?$/m,
    "",
  );
  if (!/\bEvent\b/.test(eventUses.replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*/g, "")))
    events = events.replace(/,\s*type Event(?=\s*[,}])/, "");
  await Bun.write(eventsOutput, `${events.trimEnd()}\n`);
}
