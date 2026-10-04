//! Only the dedicated retained WDIO config imports this local service.
import { ChildProcess } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import TauriWorkerService, {
  launcher as TauriLauncher,
} from "@wdio/tauri-service";
import { SevereServiceError } from "webdriverio";
import {
  captureHarnessProcess,
  HarnessProcessError,
  type HarnessRunnerPhase,
  monitorHarnessCrash,
  monitorHarnessStop,
  type OwnedHarnessProcess,
} from "./harness-process";
import { assertHarnessCrashProof } from "./harness-qualification";

export default TauriWorkerService;

function fatalLauncherError(
  stage: "preparation" | "worker_start" | "worker_end" | "completion",
  cause: unknown,
): InstanceType<typeof SevereServiceError> {
  // WDIO logs and swallows ordinary service-hook errors. Its actual severe
  // error keeps failures fatal, with a finite public reason and preserved cause.
  const code =
    cause instanceof HarnessProcessError
      ? cause.code
      : `launcher_${stage}_failure`;
  const failure = new SevereServiceError(
    `Retained harness ${stage} failed: ${code}`,
  );
  return Object.assign(failure, { code, cause });
}

type EmbeddedConfig = {
  appBinaryPath: string;
  options: { env: Record<string, string | undefined> };
};
type OwnedLauncherShape = {
  isEmbeddedMode: true;
  embeddedProcesses: Map<string, { proc: ChildProcess }>;
  embeddedConfigs: Map<string, EmbeddedConfig>;
};

function pinnedLauncher(value: unknown): OwnedLauncherShape {
  // Audited @wdio/tauri-service 1.4.0 stores these TypeScript-private members as
  // ordinary JS fields. A changed version/shape never gains crash authority.
  const require = createRequire(import.meta.url);
  const metadata = JSON.parse(
    readFileSync(
      resolve(
        dirname(require.resolve("@wdio/tauri-service")),
        "../../package.json",
      ),
      "utf8",
    ),
  );
  return inspectPinnedHarnessLauncher(value, metadata.version);
}

/** Separate version/shape validation permits deterministic failure tests without
 * modifying the installed dependency or launching an unqualified app. */
export function inspectPinnedHarnessLauncher(
  value: unknown,
  version: unknown,
): OwnedLauncherShape {
  if (version !== "1.4.0" || !value || typeof value !== "object")
    throw new HarnessProcessError("unsupported_launcher_version");
  const shape = value as Partial<OwnedLauncherShape>;
  if (
    shape.isEmbeddedMode !== true ||
    !(shape.embeddedProcesses instanceof Map) ||
    !(shape.embeddedConfigs instanceof Map) ||
    shape.embeddedProcesses.size !== 1 ||
    shape.embeddedConfigs.size !== 1
  )
    throw new HarnessProcessError("unsupported_launcher_shape");
  return shape as OwnedLauncherShape;
}

export class launcher extends TauriLauncher {
  private crashOwned?: OwnedHarnessProcess;
  private crashDeadline?: number;
  private crashStop?: AbortController;
  private crashMonitor?: Promise<void>;
  private crashFailure?: Error;
  private stopSignal?: AbortController;
  private stopMonitor?: Promise<void>;
  private stopFailure?: Error;
  private retained = new Map<ChildProcess, OwnedHarnessProcess>();
  private initial?: OwnedHarnessProcess;

  private captureCurrent(): OwnedHarnessProcess {
    const owner = pinnedLauncher(this);
    const [instance, entry] = [...owner.embeddedProcesses][0];
    const config = owner.embeddedConfigs.get(instance);
    if (
      !(entry.proc instanceof ChildProcess) ||
      !config ||
      typeof config.appBinaryPath !== "string" ||
      !config.options?.env ||
      typeof config.options.env !== "object"
    )
      throw new HarnessProcessError("unsupported_launcher_shape");
    const env = config.options.env;
    const phase = env.GITRU_COLLABORATION_HARNESS_PHASE;
    if (
      ![
        "main",
        "restart",
        "crash-before-commit",
        "crash-after-commit",
      ].includes(phase ?? "")
    )
      throw new HarnessProcessError("invalid_launcher_phase");
    const existing = this.retained.get(entry.proc);
    if (existing) {
      if (!existing.input.stillOwned())
        throw new HarnessProcessError("retired_launcher_capture");
      return existing;
    }
    if (this.retained.size >= 4)
      throw new HarnessProcessError("retained_process_budget");
    const binary = config.appBinaryPath;
    const owned = captureHarnessProcess({
      root: env.GITRU_COLLABORATION_HARNESS_ROOT ?? "",
      runNonce: env.GITRU_COLLABORATION_HARNESS_RUN_NONCE ?? "",
      artifactsDirectory: env.GITRU_E2E_ARTIFACTS ?? "",
      binaryPath: config.appBinaryPath,
      phase: phase as HarnessRunnerPhase,
      proc: entry.proc,
      environment: env,
      stillOwned: () => {
        const current = this as unknown as Partial<OwnedLauncherShape>;
        return (
          current.embeddedProcesses === owner.embeddedProcesses &&
          current.embeddedConfigs === owner.embeddedConfigs &&
          owner.embeddedProcesses.get(instance) === entry &&
          owner.embeddedConfigs.get(instance) === config &&
          config.options.env === env &&
          config.appBinaryPath === binary
        );
      },
    });
    if (
      this.initial &&
      (owned.input.root !== this.initial.input.root ||
        owned.input.runNonce !== this.initial.input.runNonce ||
        owned.input.artifactsDirectory !==
          this.initial.input.artifactsDirectory ||
        owned.input.binaryPath !== this.initial.input.binaryPath ||
        owned.input.phase !== this.initial.input.phase)
    )
      throw new HarnessProcessError("replacement_scope_mismatch");
    this.retained.set(entry.proc, owned);
    return owned;
  }

  override async onPrepare(...args: Parameters<TauriLauncher["onPrepare"]>) {
    try {
      await super.onPrepare(...args);
      const initial = this.captureCurrent();
      this.initial = initial;
      this.stopSignal = new AbortController();
      this.stopMonitor = monitorHarnessStop(
        initial,
        () => {
          this.captureCurrent();
          return [...this.retained.values()];
        },
        this.stopSignal.signal,
      ).then(
        () => undefined,
        (error: unknown) => {
          this.stopFailure =
            error instanceof Error
              ? error
              : new HarnessProcessError("stop_monitor_failure");
        },
      );
    } catch (error) {
      // WDIO does not promise onComplete after an onPrepare error.
      try {
        await super.onComplete(1, args[0], []);
      } catch (cleanupError) {
        throw fatalLauncherError(
          "preparation",
          new AggregateError(
            [error, cleanupError],
            "Retained launcher preparation and cleanup failed",
          ),
        );
      }
      throw fatalLauncherError("preparation", error);
    }
  }

  override async onWorkerStart(
    ...args: Parameters<TauriLauncher["onWorkerStart"]>
  ) {
    try {
      await super.onWorkerStart(...args);
      const owned = this.captureCurrent();
      if (owned.input.phase === "main" || owned.input.phase === "restart")
        return;
      if (this.crashOwned)
        throw new HarnessProcessError("duplicate_crash_worker");
      // The health check can replace a failed embedded server. Capture that
      // exact handle now; only a successful, fully torn-down worker may kill it.
      this.crashOwned = owned;
      // A fresh crash fixture is prepared only by this worker. Starting the
      // bound here guarantees its before-commit gate cannot reach its 60s expiry.
      this.crashDeadline =
        owned.input.phase === "crash-before-commit"
          ? performance.now() + 45_000
          : undefined;
    } catch (error) {
      throw fatalLauncherError("worker_start", error);
    }
  }

  override async onWorkerEnd(
    cid: string,
    exitCode?: number,
    _specs?: string[],
    retries?: number,
  ) {
    try {
      const owned = this.crashOwned;
      if (owned) {
        if (exitCode !== 0 || retries !== 0)
          throw new HarnessProcessError("crash_worker_failed");
        if (this.crashMonitor)
          throw new HarnessProcessError("duplicate_crash_worker_end");
        // Installed WDIO reaches this hook after DELETE session succeeds. The
        // embedded driver's DELETE removes its session map entry only; native
        // views, held provider response and SQLite remain alive. Always kill
        // before delegated launcher cleanup can terminate the owned app.
        this.crashStop = new AbortController();
        this.crashMonitor = monitorHarnessCrash(
          owned,
          this.crashStop.signal,
          this.crashDeadline,
        ).then(
          () => undefined,
          (error: unknown) => {
            this.crashFailure =
              error instanceof Error
                ? error
                : new HarnessProcessError("crash_monitor_failure");
          },
        );
        await this.crashMonitor;
        if (this.crashFailure) throw this.crashFailure;
        assertHarnessCrashProof(
          owned.input.artifactsDirectory,
          owned.input.runNonce,
          owned.input.phase,
          owned.input.binaryPath,
        );
      }
      await super.onWorkerEnd(cid);
    } catch (error) {
      this.crashFailure =
        error instanceof Error
          ? error
          : new HarnessProcessError("worker_end_failure");
      throw fatalLauncherError("worker_end", error);
    }
  }

  override async onComplete(...args: Parameters<TauriLauncher["onComplete"]>) {
    this.crashStop?.abort();
    this.stopSignal?.abort();
    await Promise.all([this.crashMonitor, this.stopMonitor]);
    const failures = [this.crashFailure, this.stopFailure].filter(
      (error): error is Error => error !== undefined,
    );
    if (this.initial && this.initial.input.phase.startsWith("crash-")) {
      try {
        assertHarnessCrashProof(
          this.initial.input.artifactsDirectory,
          this.initial.input.runNonce,
          this.initial.input.phase,
          this.initial.input.binaryPath,
        );
      } catch (error) {
        failures.push(
          error instanceof Error
            ? error
            : new HarnessProcessError("missing_crash_proof"),
        );
      }
    }
    try {
      await super.onComplete(...args);
    } catch (error) {
      failures.push(
        error instanceof Error
          ? error
          : new HarnessProcessError("launcher_cleanup_failure"),
      );
    }
    if (failures.length)
      throw fatalLauncherError(
        "completion",
        new AggregateError(failures, "Retained harness launcher failed"),
      );
  }
}
