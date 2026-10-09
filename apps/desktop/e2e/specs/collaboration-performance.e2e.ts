import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { browser } from "@wdio/globals";
import { observeProcessMemory } from "../process-observation";
import {
  type HarnessScenario,
  type HarnessScenarioResult,
  HarnessScenarioResultSchema,
} from "../protocol/collaboration-harness";

const mode = process.env.GITRU_COLLABORATION_PERFORMANCE_MODE;
if (mode !== "seed" && mode !== "restart")
  throw new Error("Unknown retained collaboration performance mode");
const scenario: HarnessScenario =
  mode === "seed" ? "performance" : "performance-restart";

async function runPerformance() {
  const result = await browser.executeAsync(
    (
      scenarioName: HarnessScenario,
      done: (receipt: HarnessScenarioResult | null) => void,
    ) => {
      const api = window.__GITRU_COLLABORATION_HARNESS__;
      if (!api) return done(null);
      void api.runScenario(scenarioName).then(done, () => done(null));
    },
    scenario,
  );
  if (!result)
    throw new Error("Packaged performance scenario returned no receipt");
  return HarnessScenarioResultSchema.parse(result);
}

describe("packaged cached collaboration performance", () => {
  before(async () => {
    await browser.setTimeout({ script: 6 * 60_000 });
    await browser.waitUntil(
      () =>
        browser.execute(() => Boolean(window.__GITRU_COLLABORATION_HARNESS__)),
      {
        timeout: 30_000,
        timeoutMsg: "Compiled retained performance executor did not initialize",
      },
    );
  });

  it(`measures the ${mode} SQLite to useful-content path`, async () => {
    const receipt = await runPerformance();
    const artifactRoot = process.env.GITRU_E2E_ARTIFACTS;
    if (!artifactRoot) throw new Error("Performance artifact root is missing");
    mkdirSync(artifactRoot, { recursive: true });
    writeFileSync(
      resolve(artifactRoot, "raw-performance.json"),
      `${JSON.stringify(receipt, null, 2)}\n`,
      { encoding: "utf8", flag: "wx", mode: 0o600 },
    );
    if (receipt.outcome !== "passed")
      throw new Error(
        `Packaged performance scenario failed at ${receipt.stage}: ${receipt.failure?.kind ?? "unknown"}`,
      );
    if (
      !receipt.status ||
      !receipt.performance ||
      receipt.performance.provider_call_count_before !==
        receipt.performance.provider_call_count_after ||
      receipt.performance.vault_load_count_before !==
        receipt.performance.vault_load_count_after ||
      receipt.status.authorized_hydrate_requests !== "0" ||
      receipt.status.core.performance?.item_count !== 10_000 ||
      receipt.status.performance_queries.length === 0
    )
      throw new Error(
        "Packaged performance receipt crossed its cache-only boundary",
      );
    const labels = new Set(
      receipt.performance.views.map((view) => view.webview_label),
    );
    if (
      labels.size !== 2 ||
      !labels.has("main") ||
      !receipt.status.child_label ||
      !labels.has(receipt.status.child_label) ||
      !receipt.status.performance_queries.some(
        (sample) => sample.webview_label === "main" && sample.kind === "items",
      ) ||
      !receipt.status.performance_queries.some(
        (sample) =>
          sample.webview_label === receipt.status?.child_label &&
          sample.kind === "items",
      )
    )
      throw new Error("Both retained native views were not measured");
    writeFileSync(
      resolve(artifactRoot, "process-memory.json"),
      `${JSON.stringify(observeProcessMemory(receipt.status.process_id), null, 2)}\n`,
      { encoding: "utf8", flag: "wx", mode: 0o600 },
    );
  });
});
