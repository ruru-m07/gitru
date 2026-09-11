import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { browser } from "@wdio/globals";
import type {
  TauriCapabilities,
  TauriServiceOptions,
} from "@wdio/tauri-service";
import { isolateCurrentProcessGitEnvironment } from "./git-environment";

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} must be set by the E2E fixture runner`);
  }
  return value;
}

const artifactsDirectory = requiredEnvironment("GITRU_E2E_ARTIFACTS");
const fixtureRepository = requiredEnvironment("GITRU_E2E_REPO");
const fixtureRoot = requiredEnvironment("GITRU_E2E_TEMP_ROOT");
const gitGlobalConfig = resolve(fixtureRoot, "global.gitconfig");
isolateCurrentProcessGitEnvironment(gitGlobalConfig);

const binaryName = process.platform === "win32" ? "gitru.exe" : "gitru";
const targetDirectory = process.env.CARGO_TARGET_DIR
  ? resolve(process.cwd(), "src-tauri", process.env.CARGO_TARGET_DIR)
  : resolve(process.cwd(), "../../target");
const appBinaryPath =
  process.env.GITRU_E2E_BINARY ??
  resolve(targetDirectory, "release", binaryName);

const appEnvironment = {
  GIT_CONFIG_GLOBAL: gitGlobalConfig,
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_TERMINAL_PROMPT: "0",
  GITRU_E2E_ARTIFACTS: artifactsDirectory,
  GITRU_E2E_REPO: fixtureRepository,
  GITRU_E2E_RESET: "1",
  GITRU_E2E_TEMP_ROOT: fixtureRoot,
  UPDATER_BASE_URL: "http://127.0.0.1:9",
};

const safeArtifactName = (value: string) =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/(^-|-$)/g, "")
    .slice(0, 80);

const tauriServiceOptions: TauriServiceOptions = {
  appBinaryPath,
  captureBackendLogs: true,
  captureFrontendLogs: true,
  clearMocks: false,
  driverProvider: "embedded",
  env: appEnvironment,
  logDir: artifactsDirectory,
  resetMocks: false,
  restoreMocks: false,
  startTimeout: 120_000,
  statusPollTimeout: 10_000,
};

const tauriCapabilities: TauriCapabilities = {
  browserName: "tauri",
  "tauri:options": {
    application: appBinaryPath,
  },
};

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  exclude: [],
  maxInstances: 1,
  capabilities: [tauriCapabilities],
  services: [["@wdio/tauri-service", tauriServiceOptions]],
  outputDir: artifactsDirectory,
  logLevel: "info",
  bail: 0,
  waitforTimeout: 20_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 2,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 180_000,
  },
  onPrepare: () => {
    mkdirSync(artifactsDirectory, { recursive: true });
  },
  afterTest: async (test, _context, result) => {
    if (result.passed) return;

    const prefix = `${Date.now()}-${safeArtifactName(test.title) || "failure"}`;
    await browser
      .saveScreenshot(resolve(artifactsDirectory, `${prefix}.png`))
      .catch(() => undefined);
    const source = await browser.getPageSource().catch(() => "");
    if (source) {
      writeFileSync(resolve(artifactsDirectory, `${prefix}.html`), source);
    }
  },
};
