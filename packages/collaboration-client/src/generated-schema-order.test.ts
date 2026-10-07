import { createRequire } from "node:module";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import { describe, expect, it } from "vitest";
import { orderGeneratedSchemas } from "../../../scripts/generated-schema-order";

const require = createRequire(
  new URL("../../commands/src/types.ts", import.meta.url),
);
type RuntimeSchema = { parse(value: unknown): unknown };
function execute(source: string): Record<string, RuntimeSchema> {
  // Keep schema references as local const bindings so this fixture executes
  // actual temporal-dead-zone semantics; CommonJS export rewrites mask them.
  const file = ts.createSourceFile(
    "fixture.ts",
    source,
    ts.ScriptTarget.Latest,
    true,
  );
  const executable = ts.factory.updateSourceFile(
    file,
    file.statements.map((statement) =>
      ts.isVariableStatement(statement)
        ? ts.factory.updateVariableStatement(
            statement,
            statement.modifiers?.filter(
              (modifier) => modifier.kind !== ts.SyntaxKind.ExportKeyword,
            ),
            statement.declarationList,
          )
        : statement,
    ),
  );
  const compiled = ts.transpileModule(
    ts.createPrinter().printFile(executable),
    {
      compilerOptions: {
        target: ts.ScriptTarget.ES2022,
        module: ts.ModuleKind.CommonJS,
      },
    },
  );
  const exports = {};
  runInNewContext(
    `${compiled.outputText}\nObject.assign(exports, {${schemaNames(source).join(", ")}});`,
    { exports, require },
  );
  return exports;
}
function schemaNames(source: string) {
  const file = ts.createSourceFile(
    "fixture.ts",
    source,
    ts.ScriptTarget.Latest,
    true,
  );
  return file.statements.flatMap((statement) =>
    ts.isVariableStatement(statement)
      ? statement.declarationList.declarations.flatMap((declaration) =>
          ts.isIdentifier(declaration.name) &&
          declaration.name.text.endsWith("Schema")
            ? [declaration.name.text]
            : [],
        )
      : [],
  );
}

const shuffledTask = `import { z } from "zod";
const helper = { retained: true };
export const TaskV1Schema = z.object({
  content: DetailValueSchema,
  observed_content_state: DetailValueStateSchema,
});
export type TaskV1 = z.infer<typeof TaskV1Schema>;
export const NativeDetailPayloadSchema = z.object({ value: TaskV1Schema });
export const DetailValueSchema = z.object({
  state: DetailValueStateSchema,
  text: z.string().nullable(),
});
export const DetailValueStateSchema = z.enum(["known", "omitted"]);
`;

describe("generated schema ordering", () => {
  it("repairs the shuffled eager TaskV1 → DetailValue graph without changing declarations", () => {
    expect(() => execute(shuffledTask)).toThrow(/before initialization/);
    const ordered = orderGeneratedSchemas(shuffledTask);
    expect(schemaNames(ordered)).toEqual([
      "DetailValueStateSchema",
      "DetailValueSchema",
      "TaskV1Schema",
      "NativeDetailPayloadSchema",
    ]);
    const runtime = execute(ordered);
    expect(
      runtime.NativeDetailPayloadSchema.parse({
        value: {
          content: { state: "known", text: "" },
          observed_content_state: "omitted",
        },
      }),
    ).toEqual({
      value: {
        content: { state: "known", text: "" },
        observed_content_state: "omitted",
      },
    });
    const statements = (source: string) => {
      const file = ts.createSourceFile(
        "fixture.ts",
        source,
        ts.ScriptTarget.Latest,
        true,
      );
      return file.statements.map((statement) => statement.getText(file));
    };
    expect(statements(ordered).sort()).toEqual(statements(shuffledTask).sort());
    expect(ordered).toContain('import { z } from "zod";');
    expect(ordered).toContain("const helper = { retained: true };");
    expect(orderGeneratedSchemas(ordered)).toBe(ordered);
  });

  it("keeps unrelated ready schemas stable and ignores types, property names, strings and comments", () => {
    const source = `import { z } from "zod";
export const FirstSchema = z.object({ LaterSchema: z.string() }) as z.ZodType<LaterSchema>;
export type LaterSchema = typeof LastSchema;
// LastSchema is a comment, not an eager read.
export const SecondSchema = z.literal("LastSchema");
export const LaterSchema = z.string();
export const LastSchema = z.string();
`;
    expect(orderGeneratedSchemas(source)).toBe(source);
    expect(execute(source).SecondSchema.parse("LastSchema")).toBe("LastSchema");
  });

  it("recognizes shorthand and computed value references but ignores member names and deferred callbacks", () => {
    const source = `import { z } from "zod";
const member = { ValueSchema: z.string() };
export const ObjectSchema = z.object({ ValueSchema });
export const ComputedSchema = z.object({ [KeySchema]: z.string() });
export const MemberSchema = member.ValueSchema.superRefine(() => ValueSchema);
export const ValueSchema = z.string();
export const KeySchema = "key";
`;
    const ordered = orderGeneratedSchemas(source);
    expect(schemaNames(ordered)).toEqual([
      "MemberSchema",
      "ValueSchema",
      "ObjectSchema",
      "KeySchema",
      "ComputedSchema",
    ]);
    expect(
      execute(ordered).ObjectSchema.parse({ ValueSchema: "saved" }),
    ).toEqual({ ValueSchema: "saved" });
  });

  it.each([
    "export const FirstSchema = SecondSchema; export const SecondSchema = FirstSchema;",
    "export const SelfSchema = SelfSchema;",
  ])("fails closed on eager schema cycles", (source) => {
    expect(() => orderGeneratedSchemas(source)).toThrow(
      /Generated schema dependency cycle/,
    );
  });

  it("rejects duplicate and unsupported combined declarations", () => {
    expect(() =>
      orderGeneratedSchemas(
        "export const SameSchema = 1; export const SameSchema = 2;",
      ),
    ).toThrow(/Duplicate generated schema declaration SameSchema/);
    expect(() =>
      orderGeneratedSchemas("export const OneSchema = 1, TwoSchema = 2;"),
    ).toThrow(/Unsupported generated schema declaration/);
  });

  it("imports the actual generated module with the complete executable schema inventory", async () => {
    const generated = await import("@gitru/commands");
    expect(
      Object.keys(generated).filter((name) => name.endsWith("Schema")),
    ).toHaveLength(360);
    expect(
      generated.TaskV1Schema.shape.content.parse({ state: "known", text: "" }),
    ).toEqual({ state: "known", text: "" });
    expect(generated.NativeDetailPayloadSchema.options).toHaveLength(2);
  });

  it("parses the nullable pull-commit cache and local-navigation wire contract", async () => {
    const generated = await import("@gitru/commands");
    const oid = "a".repeat(40);
    const parsed = generated.PullCommitSnapshotSchema.parse({
      subject_id: "github:pull:repository:67",
      context: {
        base_oid: "b".repeat(40),
        head_oid: oid,
        source_repository_provider_id: "source-1",
        metadata_facet_revision: "10",
      },
      commits: [
        {
          oid,
          position: 0,
          summary: "Cached commit",
          message: { state: "omitted", text: null },
          author: { name: "Author", provider: null },
          committer: null,
          authored_at: null,
          committed_at: null,
          parent_oids: [],
          web_url: null,
        },
      ],
      next_cursor: null,
      completeness: { state: "capped", reason: "provider_limit" },
      coverage: {
        state: "partial",
        validated_at: null,
        remote_has_more: true,
      },
      sync: {
        state: "idle",
        last_success_at: null,
        next_retry_at: null,
        error: null,
      },
      freshness: "unknown",
      facet_revision: null,
      revision: "12",
      authorization_view: "3",
    });
    expect(parsed.completeness).toEqual({
      state: "capped",
      reason: "provider_limit",
    });
    expect(parsed.commits[0]?.committer).toBeNull();
    expect(() =>
      generated.PullCommitCompletenessSchema.parse({
        state: "complete",
        reason: "provider_limit",
      }),
    ).toThrow(/cap reason/);
    expect(
      generated.LocalCloneRequestSchema.parse({
        account_id: "account",
        instance_id: "github:https://github.com/",
        repository_id: "target",
        source_repository_provider_id: null,
        authorization_epoch: "7",
      }).source_repository_provider_id,
    ).toBeNull();
    expect(
      generated.OpenLocalPullCommitRequestSchema.parse({
        account_id: "account",
        authorization_epoch: "7",
        subject_id: "github:pull:repository:67",
        commit_oid: oid,
        facet_revision: "11",
        local_repository_id: "11111111-1111-4111-8111-111111111111",
        link_id: "link",
        link_generation: "9",
      }).commit_oid,
    ).toBe(oid);
  });
});
