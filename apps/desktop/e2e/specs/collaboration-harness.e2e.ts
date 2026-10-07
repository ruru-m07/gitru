import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { browser } from "@wdio/globals";
import {
  type HarnessScenario,
  type HarnessScenarioResult,
  HarnessScenarioResultSchema,
  HarnessScenarioSchema,
} from "../protocol/collaboration-harness";

const phase = process.env.GITRU_COLLABORATION_HARNESS_PHASE ?? "main";
const allowedPhases = [
  "main",
  "crash-before-commit",
  "crash-after-commit",
  "restart",
];
const pullCommitFixture = {
  accountId: "ruru103:primary",
  subjectId: "github:pull:9007199254742993",
  baseOid: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  headOid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  sourceRepositoryProviderId: "9007199254741994",
  oids: [
    "cccccccccccccccccccccccccccccccccccccccc",
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  ],
} as const;
if (!allowedPhases.includes(phase))
  throw new Error("Unknown retained native harness runner phase");

async function scenario(name: HarnessScenario) {
  const result = await browser.executeAsync(
    (
      scenarioName: HarnessScenario,
      done: (receipt: HarnessScenarioResult | null) => void,
    ) => {
      const api = window.__GITRU_COLLABORATION_HARNESS__;
      if (!api) return done(null);
      // The browser driver invokes only this compiled finite entry point.
      // Generated native commands, UI actions and event ownership stay inside
      // the application module; no arbitrary native invocation/eval is sent.
      void api.runScenario(scenarioName).then(done, () => done(null));
    },
    name,
  );
  if (!result || result.scenario !== name || !result.status)
    throw new Error(`Retained scenario ${name} omitted its bounded receipt`);
  HarnessScenarioSchema.parse(result.scenario);
  const receipt = HarnessScenarioResultSchema.parse(result);
  const artifactRoot = process.env.GITRU_E2E_ARTIFACTS;
  if (!artifactRoot)
    throw new Error("Retained harness artifact root is missing");
  mkdirSync(artifactRoot, { recursive: true });
  writeFileSync(
    resolve(artifactRoot, `scenario-${receipt.scenario}.json`),
    `${JSON.stringify(receipt, null, 2)}\n`,
    { encoding: "utf8", mode: 0o600 },
  );
  if (receipt.outcome === "failed")
    throw new Error(
      `Retained scenario ${name} failed during ${receipt.stage} (${receipt.failure?.kind ?? "unclassified"}${receipt.failure?.kind === "native_error" ? `:${receipt.failure.code}` : ""})`,
    );
  // The executor/native wrappers have already validated this status. Keep a
  // non-null narrow type for the phase assertions after the strict receipt.
  if (!receipt.status) throw new Error("Retained scenario status is missing");
  return { ...receipt, status: receipt.status };
}

function requirePullCommitFixture(
  result: HarnessScenarioResult,
  cacheOnly: boolean,
) {
  const evidence = result.pull_commits;
  if (!evidence) throw new Error("Retained pull-commit evidence is missing");
  if (
    evidence.account_id !== pullCommitFixture.accountId ||
    evidence.subject_id !== pullCommitFixture.subjectId ||
    evidence.context.base_oid !== pullCommitFixture.baseOid ||
    evidence.context.head_oid !== pullCommitFixture.headOid ||
    evidence.context.source_repository_provider_id !==
      pullCommitFixture.sourceRepositoryProviderId ||
    evidence.context.metadata_facet_revision !==
      result.status?.checkpoint?.committed_facet_revision ||
    !/^[1-9]\d*$/.test(evidence.facet_revision) ||
    evidence.oids.length !== pullCommitFixture.oids.length ||
    evidence.oids.some((oid, index) => oid !== pullCommitFixture.oids[index]) ||
    evidence.completeness.state !== "complete" ||
    evidence.completeness.reason !== null ||
    evidence.cache_only !== cacheOnly
  )
    throw new Error("Retained pull-commit evidence changed exact context");
  if (cacheOnly) {
    if (
      evidence.provider_call_count_before !== "0" ||
      evidence.provider_call_count_after !== "0" ||
      evidence.vault_load_count_before !== "0" ||
      evidence.vault_load_count_after !== "0"
    )
      throw new Error(
        "Restarted pull-commit presentation accessed provider credentials",
      );
  } else if (
    BigInt(evidence.provider_call_count_after) <=
      BigInt(evidence.provider_call_count_before) ||
    BigInt(evidence.vault_load_count_after) <=
      BigInt(evidence.vault_load_count_before)
  ) {
    throw new Error("Phase-one pull commits were not provider hydrated");
  }
}

describe("retained native collaboration synchronization", () => {
  before(async () => {
    await browser.setTimeout({ script: 190_000 });
    await browser.waitUntil(
      () =>
        browser.execute(() => Boolean(window.__GITRU_COLLABORATION_HARNESS__)),
      {
        timeout: 30_000,
        timeoutMsg: "Compiled retained scenario executor did not initialize",
      },
    );
  });

  if (phase === "main") {
    // The main document persists across cases. Each executor finally restores
    // embedded main and removes its exact child surfaces before the next WDIO
    // script, preserving the real embedded-driver addressing boundary.
    for (const name of [
      "concurrent-demand",
      "hints-and-catchup",
      "reload-and-expiry",
      "normal-tab-lifecycle",
      "authority",
      "disconnect",
    ] satisfies HarnessScenario[]) {
      it(`qualifies ${name} through the compiled native scenario`, async () => {
        const result = await scenario(name);
        if (result.outcome !== "passed")
          throw new Error(
            `Retained scenario ${name} returned a crash checkpoint`,
          );
        if (result.status.authorized_hydrate_requests !== "0")
          throw new Error(
            "Automatic visible interest became a durable hydration request",
          );
        if (name === "authority") {
          const evidence = result.authority;
          if (
            !evidence ||
            Object.values(evidence.checks).some(
              (outcome) => outcome !== "permission_denied",
            ) ||
            !evidence.main_renewed ||
            !evidence.main_released ||
            !evidence.lease_count_restored ||
            !evidence.account_snapshot_unchanged ||
            !evidence.native_revision_unchanged ||
            !evidence.native_owner_unchanged ||
            !evidence.provider_calls_unchanged ||
            !evidence.vault_access_unchanged
          )
            throw new Error(
              "Native peer authority omitted exact denials or owner cleanup",
            );
        }
        if (name === "hints-and-catchup" || name === "disconnect") {
          const outcome =
            name === "hints-and-catchup"
              ? result.obsolete_reads?.retention_reset
              : result.obsolete_reads?.disconnect;
          if (outcome !== "stale_view" && outcome !== "cancelled")
            throw new Error(
              "Obsolete native SDK read did not carry an actual fence receipt",
            );
        }
      });
    }
  } else if (phase === "restart") {
    it("reads the retained SQLite/vault state in a fresh app and driver session", async () => {
      const result = await scenario("restart");
      if (result.outcome !== "passed" || !result.status.checkpoint)
        throw new Error(
          "Native restart did not preserve its prior process checkpoint",
        );
      if (result.status.core.session_id === result.status.checkpoint.session_id)
        throw new Error(
          "Crash qualification reused the original native session",
        );
      if (result.status.process_id === result.status.checkpoint.process_id)
        throw new Error(
          "Crash qualification did not launch a fresh native process",
        );
      if (result.status.checkpoint.kind === "committed_before_hint")
        requirePullCommitFixture(result, true);
      else if (result.pull_commits !== null)
        throw new Error(
          "Pre-commit restart unexpectedly exposed pull-commit evidence",
        );
    });
  } else {
    it("reaches a native issued checkpoint for the launch-owned hard crash", async () => {
      const name = HarnessScenarioSchema.parse(phase);
      const result = await scenario(name);
      if (result.outcome !== "checkpoint" || !result.status.checkpoint)
        throw new Error(
          "Native crash phase did not issue its run-bound checkpoint",
        );
      if (result.status.checkpoint.run_nonce !== result.status.core.run_nonce)
        throw new Error(
          "Native crash checkpoint belongs to a different fixture run",
        );
      if (result.status.checkpoint.session_id !== result.status.core.session_id)
        throw new Error("Native crash checkpoint belongs to a retired session");
      if (name === "crash-after-commit")
        requirePullCommitFixture(result, false);
      else if (result.pull_commits !== null)
        throw new Error(
          "Pre-commit crash unexpectedly exposed pull-commit evidence",
        );
      // The outer runner performs and verifies the exact owned process kill.
      // A test receipt is only checkpoint evidence, never a simulated crash.
    });
  }
});
