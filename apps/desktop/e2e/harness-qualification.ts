/** Failures from WDIO afterTest hooks are logged rather than failing the spec.
 * Persist a finite failure and check it again at launcher/outer-runner boundaries. */
import { lstatSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { z } from "zod";

const Phase = z.enum([
  "main",
  "restart",
  "crash-before-commit",
  "crash-after-commit",
]);
const Failure = z
  .object({
    run_nonce: z.uuid(),
    phase: Phase,
    reason: z.literal("crash_evidence_failed"),
  })
  .strict();
const Ack = z
  .object({
    run_nonce: z.uuid(),
    session_id: z.uuid(),
    scenario_generation: z.string().regex(/^(0|[1-9][0-9]*)$/),
    kind: z.enum(["before_commit", "committed_before_hint"]),
    process_id: z.number().int().positive().max(0xffffffff),
  })
  .strict();
const Exit = Ack.extend({
  version: z.literal(1),
  application_id: z.literal("com.ruru.gitru.e2e.collaboration"),
  phase: z.enum(["crash-before-commit", "crash-after-commit"]),
  binary: z.string().min(1),
  requested_signal: z.literal("SIGKILL"),
  observed_signal: z.literal("SIGKILL"),
  exit_code: z.null(),
}).strict();

function boundedJson(path: string) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 4096)
    throw new Error("Invalid retained qualification evidence file");
  return JSON.parse(readFileSync(path, "utf8"));
}

export function recordHarnessQualificationFailure(
  artifacts: string,
  runNonce: string,
  phase: string,
) {
  const receipt = Failure.parse({
    run_nonce: runNonce,
    phase,
    reason: "crash_evidence_failed",
  });
  writeFileSync(
    resolve(artifacts, "qualification-error.json"),
    JSON.stringify(receipt),
    { flag: "wx", mode: 0o600 },
  );
}

export function assertHarnessQualification(
  artifacts: string,
  runNonce: string,
  phase: string,
) {
  try {
    Failure.parse(boundedJson(resolve(artifacts, "qualification-error.json")));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
  // A foreign or malformed failure never permits a successful qualification.
  throw new Error(`Retained qualification failed for ${phase} (${runNonce})`);
}

export function assertHarnessCrashProof(
  artifacts: string,
  runNonce: string,
  phase: string,
  binary: string,
) {
  const ack = Ack.parse(
    boundedJson(resolve(artifacts, "crash-driver-ack.json")),
  );
  const proof = Exit.parse(
    boundedJson(resolve(artifacts, "process-exit.json")),
  );
  const kind =
    phase === "crash-before-commit" ? "before_commit" : "committed_before_hint";
  if (
    proof.run_nonce !== runNonce ||
    proof.phase !== phase ||
    proof.binary !== binary ||
    proof.kind !== kind ||
    Object.entries(ack).some(
      ([key, value]) => proof[key as keyof typeof ack] !== value,
    )
  )
    throw new Error("Retained crash proof does not match its exact driver run");
}
