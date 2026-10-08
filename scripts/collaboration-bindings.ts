// tauri-typegen 0.4 does not understand Serde rename_all, nullable Option,
// adjacent-tag payload dependencies, or injected Webview. Apply source-derived
// corrections for collaboration DTOs.
// The generated package remains generated; Rust is the source of truth.
import { orderGeneratedSchemas } from "./generated-schema-order";

const root = new URL("../", import.meta.url);
const domain = await Bun.file(
  new URL("crates/collaboration/src/domain.rs", root),
).text();
const error = await Bun.file(
  new URL("crates/collaboration/src/error.rs", root),
).text();
const effective = await Bun.file(
  new URL("crates/collaboration/src/effective.rs", root),
).text();
const providerInboxActions = await Bun.file(
  new URL("crates/collaboration/src/provider_inbox_actions.rs", root),
).text();
const commandRecovery = await Bun.file(
  new URL("crates/collaboration/src/command_recovery.rs", root),
).text();
const textEdits = await Bun.file(
  new URL("crates/collaboration/src/text_edits.rs", root),
).text();
const guardedMerge = await Bun.file(
  new URL("crates/collaboration/src/guarded_merge.rs", root),
).text();
const workflowState = await Bun.file(
  new URL("crates/collaboration/src/workflow_state.rs", root),
).text();
const commentSend = await Bun.file(
  new URL("crates/collaboration/src/comment_send.rs", root),
).text();
const pullCreation = await Bun.file(
  new URL("crates/collaboration/src/pull_creation.rs", root),
).text();
const reviewSubmission = await Bun.file(
  new URL("crates/collaboration/src/review_submission.rs", root),
).text();
const issueCreation = await Bun.file(
  new URL("crates/collaboration/src/issue_creation.rs", root),
).text();
const detail = await Bun.file(
  new URL("crates/collaboration/src/detail.rs", root),
).text();
const participants = await Bun.file(
  new URL("crates/collaboration/src/participants.rs", root),
).text();
const tasks = await Bun.file(
  new URL("crates/collaboration/src/tasks.rs", root),
).text();
const checks = await Bun.file(
  new URL("crates/collaboration/src/checks.rs", root),
).text();
const activity = await Bun.file(
  new URL("crates/collaboration/src/activity.rs", root),
).text();
const reviewNative = await Bun.file(
  new URL("crates/collaboration/src/review_native.rs", root),
).text();
const reviews = await Bun.file(
  new URL("crates/collaboration/src/reviews.rs", root),
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
const diagnostics = await Bun.file(
  new URL("crates/collaboration/src/diagnostics.rs", root),
).text();
const localLinks = await Bun.file(
  new URL("crates/collaboration/src/local_links.rs", root),
).text();
const notificationSubjects = await Bun.file(
  new URL("crates/collaboration/src/notification_subjects.rs", root),
).text();
const pullCommits = await Bun.file(
  new URL("crates/collaboration/src/pull_commits.rs", root),
).text();
const recovery = await Bun.file(
  new URL("crates/collaboration/src/recovery.rs", root),
).text();
const recoveryCommands = await Bun.file(
  new URL(
    "apps/desktop/src-tauri/src/commands/collaboration_recovery.rs",
    root,
  ),
).text();
const pullFiles = await Bun.file(
  new URL("crates/collaboration/src/pull_files.rs", root),
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
const harnessDomain = await Bun.file(
  new URL("crates/collaboration/src/test_harness/domain.rs", root),
).text();
const nativeHarnessDomain = await Bun.file(
  new URL("apps/desktop/src-tauri/src/collaboration_harness/domain.rs", root),
).text();
const output = new URL("packages/commands/src/types.ts", root);
let generated = await Bun.file(output).text();
const snake = (value: string) =>
  value.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();
// These serialized cache/parser types are native-only and deliberately absent
// from renderer command signatures. If reachable later, correct them normally.
const nativeOnlyTypes = new Set([
  "PullCreationLocalObservation",
  "PullCreationOwner",
  "CommandRecoveryExport",
  "NotificationSubjectKind",
  "NotificationSubjectRepresentation",
  "NotificationSubjectFallbackReason",
  "NotificationSubjectSelector",
  "DetailEnumeration",
  "DetailHeadScope",
  "DetailReconciliation",
  "ProviderPullCommit",
  "PullCommitProviderOrder",
  "PullCommitSource",
  "PullCommitBinding",
  "PullCommitMembershipRequest",
  "PullCommitMembershipReceipt",
  "PullFileLease",
  "PullFileBinding",
  "PullFileGenerationReceipt",
  "PullFileProvenance",
  "RecoveryCategoryCount",
  "SyncDiagnosticsExport",
  "HarnessCoreRequest",
  "HarnessCoreReceipt",
  "CheckContext",
  "CheckAggregateState",
  "CheckAggregate",
]);

// The pinned generator flattens Serde tuple variants into string literals and
// never visits their payload structs. Emit that missing graph from Rust before
// applying the ordinary Option/null correction below. Deliberately fail on an
// unsupported shape rather than inventing a renderer-owned wire model.
const payloadStructs = new Map(
  [
    ...`${participants}\n${tasks}\n${checks}\n${activity}\n${reviews}\n${reviewNative}\n${reviewSubmission}`.matchAll(
      /pub struct (\w+)\s*\{([^}]+)\}/g,
    ),
  ].map(([, name, body]) => [name, body] as const),
);
const payloadEnums = new Map(
  [
    ...`${participants}\n${tasks}\n${checks}\n${activity}\n${reviews}\n${reviewNative}\n${reviewSubmission}`.matchAll(
      /#\[serde\(rename_all = "snake_case"\)\]\s*pub enum (\w+)\s*\{([^}]+)\}/g,
    ),
  ].map(
    ([, name, body]) =>
      [
        name,
        body
          .split(",")
          .map((variant) => variant.trim())
          .filter(Boolean),
      ] as const,
  ),
);
const nativePayloadKinds = new Set<string>();
for (const [
  ,
  tag,
  content,
  renameAll,
  name,
  body,
] of `${reviewNative}\n${participants}\n${reviewSubmission}`.matchAll(
  /#\[serde\(tag = "([^"]+)", content = "([^"]+)"(?:, rename_all = "(snake_case)")?\)\]\s*pub enum (\w+)\s*\{([^}]+)\}/g,
)) {
  const dependencies: string[] = [];
  const visiting = new Set<string>();
  const emitted = new Set<string>();
  const schemaFor = (rustType: string): string => {
    const optional = rustType.match(/^Option<(.+)>$/);
    if (optional) return `${schemaFor(optional[1])}.optional()`;
    const boxed = rustType.match(/^Box<(.+)>$/);
    if (boxed) return schemaFor(boxed[1]);
    rustType = rustType.replace(/^crate::/, "");
    if (rustType === "String") return "z.string()";
    if (rustType === "bool") return "z.boolean()";
    if (
      ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64", "usize"].includes(
        rustType,
      )
    )
      return "z.number()";
    if (!/^\w+$/.test(rustType))
      throw new Error(`Unsupported tagged payload type ${rustType}`);
    if (generated.includes(`export const ${rustType}Schema =`))
      return `${rustType}Schema`;
    if (emitted.has(rustType)) return `${rustType}Schema`;
    if (visiting.has(rustType))
      throw new Error(`Recursive tagged payload ${rustType}`);
    if (rustType === "CheckKind") {
      dependencies.push(
        'export const CheckKindSchema = z.enum(["check_run", "commit_status"]);\n\nexport type CheckKind = z.infer<typeof CheckKindSchema>;',
      );
      emitted.add(rustType);
      return "CheckKindSchema";
    }
    if (rustType === "CheckStateV1") {
      dependencies.push(
        'export const CheckStateV1Schema = z.discriminatedUnion("kind", [z.object({ kind: z.literal("check_run"), status: z.string(), conclusion: z.string().nullable() }), z.object({ kind: z.literal("commit_status"), state: z.string() })]);\n\nexport type CheckStateV1 = z.infer<typeof CheckStateV1Schema>;',
      );
      emitted.add(rustType);
      return "CheckStateV1Schema";
    }
    const variants = payloadEnums.get(rustType);
    if (variants) {
      dependencies.push(
        `export const ${rustType}Schema = z.enum(${JSON.stringify(variants.map(snake))});\n\nexport type ${rustType} = z.infer<typeof ${rustType}Schema>;`,
      );
      emitted.add(rustType);
      return `${rustType}Schema`;
    }
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
      `z.object({ ${JSON.stringify(tag)}: z.literal(${JSON.stringify(renamed ?? (renameAll === "snake_case" ? snake(variant) : variant))}), ${JSON.stringify(content)}: ${schemaFor(type)} })`,
  );
  const pattern = new RegExp(
    `export const ${name}Schema = z\\.enum\\(\\[[^\\]]+\\]\\);`,
  );
  const declaration = `${dependencies.join("\n\n")}\n\nexport const ${name}Schema = z.discriminatedUnion(${JSON.stringify(tag)}, [${schemas.join(", ")}]);\n\nexport type ${name} = z.infer<typeof ${name}Schema>;`;
  if (pattern.test(generated))
    generated = generated.replace(pattern, declaration);
  else if (name === "ReviewThreadNativeV1") generated += `\n\n${declaration}`;
  else throw new Error(`Missing flattened tagged schema ${name}`);
}

// The pull-file boundary uses explicit unknown facts, plus source-specific
// validation receipts. Recover these Serde unions from the Rust declaration;
// reject schema growth that the pinned generator cannot safely represent.
for (const name of ["PullFileCount", "PullFileFlag"]) {
  const declaration = pullFiles.match(
    new RegExp(
      `#\\[serde\\(tag = "state", content = "value", rename_all = "snake_case"\\)\\]\\s*pub enum ${name} \\{\\s*Unknown,\\s*Known\\((String|bool)\\),\\s*\\}`,
    ),
  );
  if (!declaration) throw new Error(`Unsupported pull-file fact union ${name}`);
  const pattern = new RegExp(
    `export const ${name}Schema = z\\.enum\\(\\[[^\\]]+\\]\\);`,
  );
  if (!pattern.test(generated))
    throw new Error(`Missing pull-file fact schema ${name}`);
  generated = generated.replace(
    pattern,
    `export const ${name}Schema = z.discriminatedUnion("state", [z.object({ state: z.literal("unknown") }), z.object({ state: z.literal("known"), value: z.${declaration[1] === "String" ? "string" : "boolean"}() })]);`,
  );
}
const validationDeclaration = pullFiles.match(
  /#\[serde\(tag = "kind", rename_all = "snake_case"\)\]\s*pub enum PullFileArtifactValidation \{([\s\S]*?)\n\}/,
);
if (!validationDeclaration)
  throw new Error("Missing Rust pull-file validation union");
const validationVariants = [
  ...validationDeclaration[1].matchAll(/(\w+)\s*\{([^}]+)\},/g),
];
if (
  validationVariants.length !== 2 ||
  validationDeclaration[1].replace(/(\w+)\s*\{([^}]+)\},/g, "").trim()
)
  throw new Error("Unsupported pull-file validation variants");
const validationSchemas = validationVariants.map(([, variant, fields]) => {
  const declarations = [...fields.matchAll(/(\w+): String,/g)];
  if (!declarations.length || fields.replace(/(\w+): String,/g, "").trim())
    throw new Error(`Unsupported pull-file validation fields ${variant}`);
  return `z.object({ kind: z.literal(${JSON.stringify(snake(variant))}), ${declarations.map(([, field]) => `${field}: z.string()`).join(", ")} })`;
});
const validationPattern =
  /export const PullFileArtifactValidationSchema = z\.enum\(\[[^\]]+\]\);/;
if (!validationPattern.test(generated))
  throw new Error("Missing flattened pull-file validation");
generated = generated.replace(
  validationPattern,
  `export const PullFileArtifactValidationSchema = z.discriminatedUnion("kind", [${validationSchemas.join(", ")}]);`,
);

// Persisted inbox payloads have native completion/read semantics. The pinned
// generator flattens internally tagged variants, so rebuild only this closed
// Rust-declared shape and fail if the model grows without a matching codec.
const inboxDeclaration = domain.match(
  /#\[serde\(tag = "source", rename_all = "snake_case"\)\]\s*pub enum NativeInboxState \{([\s\S]*?)\n\}/,
);
if (!inboxDeclaration) throw new Error("Missing Rust native inbox union");
const inboxVariants = [
  ...inboxDeclaration[1].matchAll(/(\w+)\s*\{([^}]+)\},/g),
];
if (
  inboxVariants.length !== 2 ||
  inboxDeclaration[1].replace(/(\w+)\s*\{([^}]+)\},/g, "").trim()
)
  throw new Error("Unsupported native inbox variants");
if (!generated.includes("export const TodoCompletionSchema =")) {
  const completion = domain.match(/pub enum TodoCompletion \{([^}]+)\}/);
  if (!completion || completion[1].replace(/\w+,/g, "").trim())
    throw new Error("Unsupported todo completion enum");
  const states = [...completion[1].matchAll(/(\w+),/g)].map(([, state]) =>
    snake(state),
  );
  generated += `\nexport const TodoCompletionSchema = z.enum(${JSON.stringify(states)});\nexport type TodoCompletion = z.infer<typeof TodoCompletionSchema>;\n`;
}
const inboxSchemas = inboxVariants.map(([, variant, fields]) => {
  const declarations = [
    ...fields.matchAll(/(\w+): (String|bool|TodoCompletion),/g),
  ];
  if (
    !declarations.length ||
    fields.replace(/(\w+): (String|bool|TodoCompletion),/g, "").trim()
  )
    throw new Error(`Unsupported native inbox fields ${variant}`);
  return `z.object({ source: z.literal(${JSON.stringify(snake(variant))}), ${declarations.map(([, field, type]) => `${field}: ${type === "TodoCompletion" ? "TodoCompletionSchema" : type === "bool" ? "z.boolean()" : "z.string()"}`).join(", ")} })`;
});
const inboxPattern =
  /export const NativeInboxStateSchema = z\.enum\(\[[^\]]+\]\);/;
if (!inboxPattern.test(generated))
  throw new Error("Missing flattened native inbox schema");
generated = generated.replace(
  inboxPattern,
  `export const NativeInboxStateSchema = z.discriminatedUnion("source", [${inboxSchemas.join(", ")}]);\nexport type NativeInboxState = z.infer<typeof NativeInboxStateSchema>;`,
);

for (const source of [
  effective,
  commandRecovery,
  textEdits,
  workflowState,
  guardedMerge,
  commentSend,
  issueCreation,
  pullCreation,
  reviewSubmission,
  providerInboxActions,
  domain,
  error,
  detail,
  participants,
  tasks,
  checks,
  activity,
  reviews,
  reviewNative,
  contextualCapabilities,
  resourceMetadata,
  demand,
  diagnostics,
  localLinks,
  notificationSubjects,
  pullCommits,
  pullFiles,
  recovery,
  recoveryCommands,
  gitRemotes,
  linkCommands,
  repositoryInfo,
  repositoryCommands,
  harnessDomain,
  nativeHarnessDomain,
]) {
  for (const match of source.matchAll(
    /#\[serde\(rename_all = "snake_case"\)\]\s*pub enum (\w+)\s*\{([^}]+)\}/g,
  )) {
    const [, name, body] = match;
    const variants = body
      .replace(/\/\/[^\n]*/g, "")
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean);
    const pattern = new RegExp(
      `export const ${name}Schema = z\\.enum\\(\\[[^\\]]+\\]\\);`,
    );
    if (!pattern.test(generated) && nativeOnlyTypes.has(name)) continue;
    if (!pattern.test(generated))
      throw new Error(`Missing generated enum ${name}`);
    const schema = `export const ${name}Schema = z.enum(${JSON.stringify(variants.map(snake))});`;
    const hasType = new RegExp(`export type ${name}\\b`).test(generated);
    generated = generated.replace(
      pattern,
      hasType
        ? schema
        : `${schema}\n\nexport type ${name} = z.infer<typeof ${name}Schema>;`,
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
const checkV1Schema =
  /(export const CheckV1Schema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!checkV1Schema.test(generated))
  throw new Error("Missing generated check payload schema");
generated = generated.replace(
  checkV1Schema,
  `$1.superRefine((check, context) => {
  if ((check.kind === "check_run") !== (check.state.kind === "check_run") || (check.kind === "commit_status") !== (check.state.kind === "commit_status") || (check.kind === "check_run" && check.allow_failure !== null)) {
    context.addIssue({ code: "custom", path: ["state"], message: "Check kind and native state family must match" });
  }
  });`,
);
const textEditSnapshotSchema =
  /(export const TextEditSnapshotSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!textEditSnapshotSchema.test(generated))
  throw new Error("Missing generated text-edit snapshot schema");
generated = generated.replace(
  textEditSnapshotSchema,
  `$1.superRefine((snapshot, context) => {
  const available = snapshot.availability === "available";
  const completeBase = snapshot.context !== null && snapshot.title !== null && snapshot.reason === null && snapshot.pending_intent === null;
  const unavailableShape = snapshot.context === null && snapshot.title === null && snapshot.body === null && snapshot.reason !== null;
  const pendingMatches = (snapshot.reason === "pending_intent") === (snapshot.pending_intent !== null);
  if ((available && !completeBase) || (!available && !unavailableShape) || !pendingMatches) {
    context.addIssue({ code: "custom", path: ["availability"], message: "Text edit availability and native evidence must agree" });
  }
});`,
);
const textEditRequestSchema =
  /(export const TextEditRequestSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!textEditRequestSchema.test(generated))
  throw new Error("Missing generated text-edit request schema");
generated = generated.replace(
  textEditRequestSchema,
  `$1.superRefine((request, context) => {
  if (!request.accept_best_effort || (request.title === null && request.body === null)) {
    context.addIssue({ code: "custom", path: ["accept_best_effort"], message: "Text edits require explicit best-effort acceptance and at least one changed field" });
  }
});`,
);
// Online consent is literal and provider preview evidence is internally coherent.
generated = generated.replace(
  /(export const GuardedMergeRequestSchema = z\.object\(\{[\s\S]*?)confirm_inspected_head: z\.coerce\.boolean\(\)/,
  "$1confirm_inspected_head: z.literal(true)",
);
const guardedMergePreviewSchema =
  /(export const GuardedMergePreviewSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!guardedMergePreviewSchema.test(generated))
  throw new Error("Missing generated guarded merge preview");
generated = generated.replace(
  guardedMergePreviewSchema,
  `$1.superRefine((preview, context) => {
  const available = preview.context !== null;
  if (available ? (preview.reason !== null || preview.can_push !== true || preview.mergeable !== true || preview.provider_mergeability !== "clean" || preview.methods.length === 0 || preview.expected_head !== preview.context?.expected_head || preview.authorization_view !== preview.context?.authorization_view || preview.expires_in_seconds !== 60) : (preview.reason === null || preview.expires_in_seconds !== 0)) {
    context.addIssue({code:"custom", path:["context"], message:"Online merge evidence must agree with its grant"});
  }
});`,
);
const workflowStateSnapshotSchema =
  /(export const WorkflowStateSnapshotSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!workflowStateSnapshotSchema.test(generated))
  throw new Error("Missing generated workflow-state snapshot schema");
generated = generated.replace(
  workflowStateSnapshotSchema,
  `$1.superRefine((snapshot, context) => {
  const available = snapshot.availability === "available";
  const availableShape = snapshot.context !== null && snapshot.current_state !== null && snapshot.reason === null && snapshot.pending_intent === null;
  const unavailableShape = snapshot.context === null && snapshot.current_state === null && snapshot.reason !== null;
  const pendingMatches = (snapshot.reason === "pending_intent") === (snapshot.pending_intent !== null);
  if ((available && !availableShape) || (!available && !unavailableShape) || !pendingMatches) {
    context.addIssue({ code: "custom", path: ["availability"], message: "Workflow-state availability and native evidence must agree" });
  }
});`,
);
const workflowStateRequestSchema =
  /(export const WorkflowStateRequestSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!workflowStateRequestSchema.test(generated))
  throw new Error("Missing generated workflow-state request schema");
generated = generated.replace(
  workflowStateRequestSchema,
  `$1.superRefine((request, context) => {
  if (!request.accept_best_effort) {
    context.addIssue({ code: "custom", path: ["accept_best_effort"], message: "Workflow-state changes require explicit best-effort acceptance" });
  }
});`,
);
const commentDraftSnapshotSchema =
  /(export const CommentDraftSnapshotSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!commentDraftSnapshotSchema.test(generated))
  throw new Error("Missing generated comment-draft snapshot schema");
generated = generated.replace(
  commentDraftSnapshotSchema,
  `$1.superRefine((snapshot, context) => {
  const available = snapshot.availability === "available";
  const availableShape = snapshot.context !== null && snapshot.reason === null && snapshot.submission === null;
  const unavailableShape = snapshot.context === null && snapshot.reason !== null;
  if ((available && !availableShape) || (!available && !unavailableShape)) {
    context.addIssue({ code: "custom", path: ["availability"], message: "Comment send availability and native evidence must agree" });
  }
});`,
);
const sendCommentRequestSchema =
  /(export const SendCommentRequestSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!sendCommentRequestSchema.test(generated))
  throw new Error("Missing generated send-comment request schema");
generated = generated.replace(
  sendCommentRequestSchema,
  `$1.superRefine((request, context) => {
  if (!request.accept_background_delivery) {
    context.addIssue({ code: "custom", path: ["accept_background_delivery"], message: "Comment send requires explicit background delivery acceptance" });
  }
});`,
);
const issueDraftSnapshotSchema =
  /(export const IssueDraftSnapshotSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!issueDraftSnapshotSchema.test(generated))
  throw new Error("Missing generated issue-draft snapshot schema");
generated = generated.replace(
  issueDraftSnapshotSchema,
  `$1.superRefine((snapshot, context) => {
  const available = snapshot.availability === "available";
  const availableShape = snapshot.context !== null && snapshot.reason === null && snapshot.submission === null && snapshot.published === null;
  const unavailableShape = snapshot.context === null && snapshot.reason !== null;
  const currentSubmission = snapshot.submission !== null && snapshot.submission.draft_generation === snapshot.generation;
  const submissionReason = currentSubmission ? "already_submitted" : snapshot.submission !== null ? "pending_submission" : null;
  const publishedMatches = snapshot.published === null || (snapshot.reason === "already_submitted" && snapshot.submission !== null && snapshot.published.command_id === snapshot.submission.command_id);
  if ((available && !availableShape) || (!available && !unavailableShape) || ((snapshot.reason === "already_submitted" || snapshot.reason === "pending_submission") && snapshot.reason !== submissionReason) || !publishedMatches) {
    context.addIssue({ code: "custom", path: ["availability"], message: "Issue draft availability and submission evidence must agree" });
  }
});`,
);
const submitIssueRequestSchema =
  /(export const SubmitIssueRequestSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!submitIssueRequestSchema.test(generated))
  throw new Error("Missing generated submit-issue request schema");
generated = generated.replace(
  submitIssueRequestSchema,
  `$1.superRefine((request, context) => {
  if (!request.accept_background_delivery) {
    context.addIssue({ code: "custom", path: ["accept_background_delivery"], message: "Issue creation requires explicit background delivery acceptance" });
  }
});`,
);
// Qualify every native family explicitly; enum growth cannot silently borrow
// another payload's authority or widen ordinary entry limits.
if (
  nativePayloadKinds.size !== 6 ||
  !nativePayloadKinds.has("participant.v1") ||
  !nativePayloadKinds.has("task.v1") ||
  !nativePayloadKinds.has("check.v1") ||
  !nativePayloadKinds.has("activity.v1") ||
  !nativePayloadKinds.has("review.v1") ||
  !nativePayloadKinds.has("review_thread.v1")
)
  throw new Error("Extend native detail field-family guards for this payload");
const familyFields = (name: string, maximum: number): string[] => {
  const family = detail.match(
    new RegExp(`fn is_${name}\\(self\\) -> bool \\{([\\s\\S]*?)\\n    \\}`),
  );
  if (!family) throw new Error(`Missing Rust ${name} field-family declaration`);
  const fields = [...family[1].matchAll(/Self::(\w+)/g)].map(([, field]) =>
    snake(field),
  );
  if (fields.length !== maximum || new Set(fields).size !== fields.length)
    throw new Error(`Extend qualified Rust ${name} field-family bound`);
  return fields;
};
const participantFields = familyFields("participant", 6);
const taskFields = familyFields("task", 12);
const checkOnlyFields = familyFields("check", 1);
const checkFields = [...checkOnlyFields, "head_oid"];
const reviewSummaryFields = familyFields("review_summary", 6);
const reviewThreadFields = familyFields("review_thread", 5);
const activityFields = ["activity", "body", "author", "updated_at"];
if (
  !/DetailFacet::Activity\s*=>\s*matches!\([\s\S]*?Self::Activity\s*\|\s*Self::Body\s*\|\s*Self::Author\s*\|\s*Self::UpdatedAt[\s\S]*?\)/.test(
    detail,
  )
)
  throw new Error("Extend qualified Rust activity field-family bound");
const fieldsEnum = detail.match(/pub enum DetailField\s*\{([^}]+)\}/);
if (!fieldsEnum) throw new Error("Missing Rust detail field enum");
const fields = fieldsEnum[1]
  .split(",")
  .map((field) => field.trim())
  .filter(Boolean)
  .map(snake);
const nativeFields = new Set([
  ...participantFields,
  ...taskFields,
  ...checkOnlyFields,
  "review",
  "review_thread",
  "activity",
]);
const genericFields = fields.filter((field) => !nativeFields.has(field));
if (
  nativeFields.size !== 22 ||
  genericFields.length !== 6 ||
  checkFields.length !== 2 ||
  fields.length !== 28 ||
  new Set(fields).size !== fields.length ||
  [...nativeFields].some((field) => !fields.includes(field))
)
  throw new Error("Extend qualified disjoint detail field families");
const entrySchema =
  /(export const DetailEntrySchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!entrySchema.test(generated))
  throw new Error("Missing generated detail entry schema for family guard");
generated = generated.replace(
  entrySchema,
  `const detailFieldFamilies = {\n  generic: new Set<string>(${JSON.stringify(genericFields)}),\n  "participant.v1": new Set<string>(${JSON.stringify(participantFields)}),\n  "task.v1": new Set<string>(${JSON.stringify(taskFields)}),\n  "check.v1": new Set<string>(${JSON.stringify(checkFields)}),\n  "activity.v1": new Set<string>(${JSON.stringify(activityFields)}),\n  "review.v1": new Set<string>(${JSON.stringify(reviewSummaryFields)}),\n  "review_thread.v1": new Set<string>(${JSON.stringify(reviewThreadFields)}),\n};\n\n$1.superRefine((entry, context) => {\n  const family = detailFieldFamilies[entry.native?.kind ?? "generic"];\n  const mask = new Set<string>(entry.field_mask);\n  const validations = entry.field_validations.map((validation) => validation.field);\n  const validated = new Set<string>(validations);\n  const exactNativeEvidence = entry.native?.kind === "check.v1" || entry.native?.kind === "review.v1" || entry.native?.kind === "review_thread.v1";\n  const invalidExactEvidence = exactNativeEvidence && (mask.size !== family.size || validated.size !== family.size || [...family].some((field) => !mask.has(field) || !validated.has(field)));\n  if (invalidExactEvidence || entry.field_mask.length > family.size || validations.length > family.size || mask.size !== entry.field_mask.length || validated.size !== validations.length || entry.field_mask.some((field) => !family.has(field)) || validations.some((field) => !family.has(field))) {\n    context.addIssue({ code: "custom", path: ["native"], message: "Detail entry fields do not match its native payload" });\n  }\n});`,
);
generated = generated.replace(
  /(export const Collaboration\w+ParamsSchema = z\.object\(\{)([\s\S]*?)(\n\}\);)/g,
  (_, open, fields, close) =>
    `${open}${fields.replace(/view: WebviewSchema,?/g, "")}${close}`,
);
if (generated.includes("WebviewSchema"))
  throw new Error("Unexpected injected Webview remains in generated types");
const pullCommitCompletenessSchema =
  /(export const PullCommitCompletenessSchema = z\.object\(\{[\s\S]*?\n\}\));/;
if (!pullCommitCompletenessSchema.test(generated))
  throw new Error("Missing generated pull-commit completeness schema");
generated = generated.replace(
  pullCommitCompletenessSchema,
  `$1.superRefine((value, context) => {
  if ((value.state === "capped") !== (value.reason !== null)) {
    context.addIssue({ code: "custom", path: ["reason"], message: "A pull-commit cap reason is valid only for capped snapshots" });
  }
});`,
);
generated = orderGeneratedSchemas(generated.replace(/[\t ]+$/gm, ""));
await Bun.write(output, generated);

// Event URLs remain unchanged on the wire. tauri-typegen 0.4 leaves ':' and
// other event-name separators in its function identifier, producing invalid TS.
const eventsOutput = new URL("packages/commands/src/events.ts", root);
const eventsFile = Bun.file(eventsOutput);
if (await eventsFile.exists()) {
  const identifiers = new Set<string>();
  // Feature-exclusive relays can emit the same typed event as production.
  // The scanner ignores cfg and emits the exact listener twice. Collapse only
  // identical definitions; differing payloads still fail the collision check.
  const definitions = new Set<string>();
  let events = (await eventsFile.text()).replace(
    /\/\*\*\n \* Listen for '[^\n]+' events[\s\S]*?export async function [^\s(]+\([\s\S]*?\n\}/g,
    (definition) => {
      if (definitions.has(definition)) return "";
      definitions.add(definition);
      return definition;
    },
  );
  events = events.replace(
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
