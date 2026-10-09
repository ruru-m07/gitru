import { randomUUID } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import {
  assertHarnessCrashProof,
  assertHarnessQualification,
  recordHarnessQualificationFailure,
} from "./harness-qualification";

const roots: string[] = [];
// Resolve the actual utils used by our installed CLI without adding a direct
// product dependency or depending on Bun's physical node_modules layout.
const require = createRequire(import.meta.url);
const cliRequire = createRequire(require.resolve("@wdio/cli"));
const utilsDirectory = cliRequire.resolve
  .paths("@wdio/utils")
  ?.map((directory) => join(directory, "@wdio/utils"))
  .find((directory) => existsSync(join(directory, "package.json")));
if (!utilsDirectory) throw new Error("Installed WDIO utils not found");
const metadata = JSON.parse(
  readFileSync(join(utilsDirectory, "package.json"), "utf8"),
);
if (metadata.version !== "9.31.7")
  throw new Error("Unqualified WDIO hook version");
const moduleUrl = pathToFileURL(
  join(utilsDirectory, metadata.exports["."].import),
).href;
const { executeHooksWithArgs } = (await import(
  /* @vite-ignore */ moduleUrl
)) as {
  executeHooksWithArgs: (
    name: string,
    hooks: Array<() => Promise<void>>,
  ) => Promise<unknown[]>;
};
function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "gitru-proof-test-"));
  roots.push(directory);
  return { directory, runNonce: randomUUID(), phase: "crash-before-commit" };
}
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true });
});

describe("retained driver qualification failures", () => {
  it("fails at the outer boundary when installed WDIO swallows an afterTest error", async () => {
    const { directory, runNonce, phase } = fixture();
    const errors = await executeHooksWithArgs("afterTest", [
      async () => {
        recordHarnessQualificationFailure(directory, runNonce, phase);
        throw new Error("invalid checkpoint");
      },
    ]);
    expect(errors).toHaveLength(1);
    expect(errors[0]).toBeInstanceOf(Error);
    expect(() =>
      assertHarnessQualification(directory, runNonce, phase),
    ).toThrow("Retained qualification failed");
  });

  it("rejects an exit proof with the correct nonce but a different native process", () => {
    const { directory, runNonce, phase } = fixture();
    const ack = {
      run_nonce: runNonce,
      session_id: randomUUID(),
      scenario_generation: "1",
      kind: "before_commit",
      process_id: 1234,
    };
    writeFileSync(
      join(directory, "crash-driver-ack.json"),
      JSON.stringify(ack),
    );
    writeFileSync(
      join(directory, "process-exit.json"),
      JSON.stringify({
        ...ack,
        process_id: 5678,
        version: 1,
        application_id: "com.ruru.gitru.e2e.collaboration",
        phase,
        binary: "/fixture/gitru",
        requested_signal: "SIGKILL",
        observed_signal: "SIGKILL",
        exit_code: null,
      }),
    );
    expect(() =>
      assertHarnessCrashProof(directory, runNonce, phase, "/fixture/gitru"),
    ).toThrow("exact driver run");
  });
});
