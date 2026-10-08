import { type ChildProcess, spawn, spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import { once } from "node:events";
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, sep } from "node:path";
import OriginalWorkerService from "@wdio/tauri-service";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SevereServiceError } from "webdriverio";
import { canonicalHarnessPath, nativeCanonicalSpelling } from "./harness-paths";
import {
  type CrashDriverAck,
  captureHarnessProcess,
  HARNESS_APPLICATION_ID,
  type HarnessProcessInput,
  type HarnessRunnerPhase,
  monitorHarnessCrash,
  monitorHarnessStop,
} from "./harness-process";
import HarnessWorkerService, {
  inspectPinnedHarnessLauncher,
  launcher,
} from "./harness-service";

const delegated = vi.hoisted(() => ({
  prepare: vi.fn(),
  workerStart: vi.fn(),
  workerEnd: vi.fn(),
  complete: vi.fn(),
}));
vi.mock("@wdio/tauri-service", () => ({
  default: class OriginalWorker {},
  launcher: class OriginalLauncher {
    constructor(shape: object) {
      Object.assign(this, shape);
    }
    async onPrepare(...args: unknown[]) {
      await delegated.prepare(...args);
    }
    async onWorkerStart(...args: unknown[]) {
      await delegated.workerStart(...args);
    }
    async onWorkerEnd(...args: unknown[]) {
      await delegated.workerEnd(...args);
    }
    async onComplete(...args: unknown[]) {
      await delegated.complete(...args);
    }
  },
}));

// These are real, test-owned Node children. They qualify the ownership and
// force-exit helper, not the native Tauri/process-restart scenario itself.
type Fixture = {
  createdRoot: string;
  root: string;
  artifacts: string;
  nonce: string;
  proc: ChildProcess;
  input: HarnessProcessInput;
  checkpoint: Record<string, unknown>;
  ack: CrashDriverAck;
  current: { value: boolean };
};
const fixtures: Fixture[] = [];
async function fixture(
  phase: HarnessRunnerPhase = "crash-before-commit",
  ignoreTerminate = false,
): Promise<Fixture> {
  const createdRoot = mkdtempSync(join(tmpdir(), "gitru-process-test-"));
  const root = canonicalHarnessPath(createdRoot);
  chmodSync(root, 0o700);
  const artifacts = join(root, "artifacts");
  mkdirSync(artifacts, { mode: 0o700 });
  const nonce = randomUUID();
  writeFileSync(
    join(root, "run.json"),
    JSON.stringify({
      version: 1,
      application_id: HARNESS_APPLICATION_ID,
      run_nonce: nonce,
    }),
    { mode: 0o600 },
  );
  const binary = canonicalHarnessPath(process.execPath);
  const proc = spawn(
    binary,
    [
      "-e",
      `${ignoreTerminate ? "process.on('SIGTERM', () => {});" : ""}process.stdout.write('ready');setInterval(() => {}, 1000);`,
    ],
    {
      stdio: ["ignore", "pipe", "ignore"],
    },
  );
  const current = { value: true };
  const value: Fixture = {
    createdRoot,
    root,
    artifacts,
    nonce,
    proc,
    current,
    input: {
      root,
      runNonce: nonce,
      artifactsDirectory: artifacts,
      binaryPath: binary,
      phase,
      proc,
      environment: {
        GITRU_COLLABORATION_HARNESS_ROOT: root,
        GITRU_COLLABORATION_HARNESS_RUN_NONCE: nonce,
        GITRU_COLLABORATION_HARNESS_PHASE: phase,
        GITRU_E2E_ARTIFACTS: artifacts,
        NEVER_PRINT_THIS_FIXTURE_VALUE: "synthetic-private-environment",
      },
      stillOwned: () => current.value,
    },
    checkpoint: {},
    ack: {} as CrashDriverAck,
  };
  fixtures.push(value);
  await Promise.race([
    once(proc.stdout!, "data"),
    once(proc, "error").then(([error]) => Promise.reject(error)),
    once(proc, "exit").then(() =>
      Promise.reject(new Error("Fixture child exited before ready")),
    ),
  ]);
  if (!proc.pid) throw new Error("Test-owned child omitted its PID");
  value.ack = {
    run_nonce: nonce,
    session_id: randomUUID(),
    scenario_generation: "2",
    kind:
      phase === "crash-after-commit"
        ? "committed_before_hint"
        : "before_commit",
    process_id: proc.pid,
  };
  value.checkpoint = {
    ...value.ack,
    gate_id: phase === "crash-after-commit" ? null : randomUUID(),
    committed_phase: phase === "crash-after-commit" ? "one" : null,
    committed_facet_revision:
      phase === "crash-after-commit" ? "9007199254740993" : null,
  };
  return value;
}
function checkpoint(value: Fixture) {
  writeFileSync(
    join(value.root, "crash-checkpoint.json"),
    JSON.stringify(value.checkpoint),
    { mode: 0o600 },
  );
}
function acknowledgment(value: Fixture, payload: unknown = value.ack) {
  writeFileSync(
    join(value.artifacts, "crash-driver-ack.json"),
    JSON.stringify(payload),
    { mode: 0o600 },
  );
}
async function rejectsWithoutKill(value: Fixture, reason: string) {
  const owned = captureHarnessProcess(value.input);
  const kill = vi.spyOn(value.proc, "kill");
  await expect(
    monitorHarnessCrash(owned, new AbortController().signal),
  ).rejects.toMatchObject({ code: reason });
  expect(kill).not.toHaveBeenCalled();
  expect(existsSync(join(value.artifacts, "process-exit.json"))).toBe(false);
}
afterEach(async () => {
  vi.restoreAllMocks();
  for (const value of fixtures.splice(0)) {
    if (value.proc.exitCode === null && value.proc.signalCode === null) {
      const exited = once(value.proc, "exit");
      value.proc.kill("SIGKILL");
      await exited;
    }
    rmSync(value.root, { force: true, recursive: true });
  }
  delegated.prepare.mockReset();
  delegated.workerStart.mockReset();
  delegated.workerEnd.mockReset();
  delegated.complete.mockReset();
});

describe("retained crash process ownership", () => {
  for (const phase of ["crash-before-commit", "crash-after-commit"] as const) {
    it(`signals only its actual test-owned child after a matching ${phase} ack`, async () => {
      const value = await fixture(phase);
      checkpoint(value);
      acknowledgment(value);
      const owned = captureHarnessProcess(value.input);
      const kill = vi.spyOn(value.proc, "kill");
      const proof = await monitorHarnessCrash(
        owned,
        new AbortController().signal,
      );
      expect(kill).toHaveBeenCalledExactlyOnceWith("SIGKILL");
      expect(proof).toMatchObject({
        ...value.ack,
        phase,
        binary: value.input.binaryPath,
        observed_signal: "SIGKILL",
        exit_code: null,
      });
      expect(value.proc.signalCode).toBe("SIGKILL");
      expect(
        JSON.parse(
          readFileSync(join(value.artifacts, "process-exit.json"), "utf8"),
        ),
      ).toEqual(proof);
      expect(JSON.stringify(proof)).not.toContain(
        "synthetic-private-environment",
      );
    });
  }
  for (const offset of [-16_000, 10_000]) {
    it("rejects an expired or future crash acknowledgment without signalling", async () => {
      const value = await fixture();
      checkpoint(value);
      acknowledgment(value);
      const time = new Date(Date.now() + offset);
      utimesSync(join(value.artifacts, "crash-driver-ack.json"), time, time);
      await rejectsWithoutKill(value, "checkpoint_ack_expired");
    });
  }
  it("refuses an expired monotonic before-commit window without signalling", async () => {
    const value = await fixture();
    checkpoint(value);
    acknowledgment(value);
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      monitorHarnessCrash(
        captureHarnessProcess(value.input),
        new AbortController().signal,
        performance.now() - 1,
      ),
    ).rejects.toMatchObject({ code: "checkpoint_window_expired" });
    expect(kill).not.toHaveBeenCalled();
    expect(existsSync(join(value.artifacts, "process-exit.json"))).toBe(false);
  });
  it("never treats a checkpoint without a passed driver acknowledgment as crash authorization", async () => {
    const value = await fixture();
    checkpoint(value);
    const kill = vi.spyOn(value.proc, "kill");
    const stop = new AbortController();
    const monitor = monitorHarnessCrash(
      captureHarnessProcess(value.input),
      stop.signal,
    );
    const rejection = expect(monitor).rejects.toMatchObject({
      code: "monitor_stopped",
    });
    await new Promise((done) => setTimeout(done, 120));
    expect(kill).not.toHaveBeenCalled();
    stop.abort();
    await rejection;
    expect(kill).not.toHaveBeenCalled();
  });
  for (const changed of [
    "run_nonce",
    "session_id",
    "scenario_generation",
    "process_id",
  ] as const) {
    it(`rejects an ack with another ${changed} without signaling any child`, async () => {
      const value = await fixture();
      checkpoint(value);
      acknowledgment(value, {
        ...value.ack,
        [changed]:
          changed === "process_id"
            ? value.ack.process_id + 1
            : changed === "scenario_generation"
              ? "3"
              : randomUUID(),
      });
      await rejectsWithoutKill(value, "checkpoint_ack_mismatch");
    });
  }
  it("rejects a wrong native crash phase rather than interpreting a commit as an uncommitted receipt", async () => {
    const value = await fixture();
    value.checkpoint = {
      ...value.checkpoint,
      kind: "committed_before_hint",
      gate_id: null,
      committed_phase: "one",
      committed_facet_revision: "4",
    };
    checkpoint(value);
    acknowledgment(value);
    await rejectsWithoutKill(value, "checkpoint_phase_mismatch");
  });
  for (const payload of [
    { extra: "path" },
    { scenario_generation: "01" },
    { scenario_generation: "18446744073709551616" },
    { process_id: "42" },
    { kind: ["before_commit"] },
  ]) {
    it(`rejects malformed ack schema ${JSON.stringify(payload)}`, async () => {
      const value = await fixture();
      checkpoint(value);
      acknowledgment(value, { ...value.ack, ...payload });
      await rejectsWithoutKill(value, "invalid_schema");
    });
  }
  it("rejects unmodeled native checkpoint fields", async () => {
    const value = await fixture();
    value.checkpoint.path = "/tmp/unowned";
    checkpoint(value);
    acknowledgment(value);
    await rejectsWithoutKill(value, "invalid_schema");
  });
  it("keeps retained checkpoint reads bounded", async () => {
    const value = await fixture();
    writeFileSync(
      join(value.root, "crash-checkpoint.json"),
      Buffer.alloc(4097),
    );
    acknowledgment(value);
    await rejectsWithoutKill(value, "oversized_json");
  });
  it("fails closed after the original launcher retires its captured handle", async () => {
    const value = await fixture();
    checkpoint(value);
    acknowledgment(value);
    const owned = captureHarnessProcess(value.input);
    value.current.value = false;
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      monitorHarnessCrash(owned, new AbortController().signal),
    ).rejects.toMatchObject({ code: "retired_process_handle" });
    expect(kill).not.toHaveBeenCalled();
  });
  it("never signals an exited handle whose old PID might later be reused", async () => {
    const value = await fixture();
    checkpoint(value);
    acknowledgment(value);
    const owned = captureHarnessProcess(value.input);
    const exited = once(value.proc, "exit");
    value.proc.kill("SIGTERM");
    await exited;
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      monitorHarnessCrash(owned, new AbortController().signal),
    ).rejects.toMatchObject({ code: "retired_process_handle" });
    expect(kill).not.toHaveBeenCalled();
  });
  it("rechecks the root marker before authorizing a kill", async () => {
    const value = await fixture();
    checkpoint(value);
    acknowledgment(value);
    const owned = captureHarnessProcess(value.input);
    writeFileSync(
      join(value.root, "run.json"),
      JSON.stringify({
        version: 1,
        application_id: HARNESS_APPLICATION_ID,
        run_nonce: randomUUID(),
      }),
    );
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      monitorHarnessCrash(owned, new AbortController().signal),
    ).rejects.toMatchObject({ code: "root_marker_mismatch" });
    expect(kill).not.toHaveBeenCalled();
  });
  it("freezes the launch-owned keyed storage selection", async () => {
    const value = await fixture();
    writeFileSync(
      join(value.root, "run.json"),
      JSON.stringify({
        version: 1,
        application_id: HARNESS_APPLICATION_ID,
        run_nonce: value.input.runNonce,
        storage_mode: "keyed",
      }),
    );
    checkpoint(value);
    acknowledgment(value);
    const owned = captureHarnessProcess(value.input);
    expect(owned.storageMode).toBe("keyed");
    writeFileSync(
      join(value.root, "run.json"),
      JSON.stringify({
        version: 1,
        application_id: HARNESS_APPLICATION_ID,
        run_nonce: value.input.runNonce,
        storage_mode: "plaintext",
      }),
    );
    await expect(
      monitorHarnessCrash(owned, new AbortController().signal),
    ).rejects.toMatchObject({ code: "root_marker_mismatch" });
  });
  it("requires the exact launcher's binary and task environment at capture", async () => {
    const value = await fixture();
    const kill = vi.spyOn(value.proc, "kill");
    const wrongBinary = join(value.root, "other-binary");
    writeFileSync(wrongBinary, "task-owned placeholder");
    expect(() =>
      captureHarnessProcess({ ...value.input, binaryPath: wrongBinary }),
    ).toThrow("owned_binary_mismatch");
    expect(() =>
      captureHarnessProcess({
        ...value.input,
        environment: {
          ...value.input.environment,
          GITRU_COLLABORATION_HARNESS_RUN_NONCE: randomUUID(),
        },
      }),
    ).toThrow("launcher_environment_mismatch");
    expect(kill).not.toHaveBeenCalled();
  });
  it("does not allow artifact output outside its validated run root", async () => {
    const value = await fixture();
    const kill = vi.spyOn(value.proc, "kill");
    expect(() =>
      captureHarnessProcess({ ...value.input, artifactsDirectory: value.root }),
    ).toThrow("artifacts_outside_root");
    expect(() =>
      captureHarnessProcess({
        ...value.input,
        artifactsDirectory: canonicalHarnessPath(tmpdir()),
      }),
    ).toThrow("artifacts_outside_root");
    expect(kill).not.toHaveBeenCalled();
  });
  it.skipIf(process.platform === "win32")(
    "rejects root, ack, and checkpoint symlinks",
    async () => {
      const value = await fixture();
      const kill = vi.spyOn(value.proc, "kill");
      const linkedRoot = join(value.root, "linked-root");
      symlinkSync(value.root, linkedRoot);
      expect(() =>
        captureHarnessProcess({ ...value.input, root: linkedRoot }),
      ).toThrow();
      checkpoint(value);
      symlinkSync(
        join(value.root, "crash-checkpoint.json"),
        join(value.artifacts, "crash-driver-ack.json"),
      );
      await rejectsWithoutKill(value, "unsafe_file");
      rmSync(join(value.artifacts, "crash-driver-ack.json"));
      rmSync(join(value.root, "crash-checkpoint.json"));
      const other = join(value.root, "checkpoint-source.json");
      writeFileSync(other, JSON.stringify(value.checkpoint));
      symlinkSync(other, join(value.root, "crash-checkpoint.json"));
      acknowledgment(value);
      await rejectsWithoutKill(value, "unsafe_file");
      expect(kill).not.toHaveBeenCalled();
    },
  );
  it("rejects a lexical root alias before authorizing an actual owned child", async () => {
    const value = await fixture();
    const kill = vi.spyOn(value.proc, "kill");
    const alias = `${value.root}${sep}.${sep}`;
    expect(() =>
      captureHarnessProcess({
        ...value.input,
        root: alias,
        environment: {
          ...value.input.environment,
          GITRU_COLLABORATION_HARNESS_ROOT: alias,
        },
      }),
    ).toThrow("noncanonical_path");
    expect(kill).not.toHaveBeenCalled();
  });
  it.skipIf(process.platform !== "win32")(
    "requires OS canonical identity and namespace for real Windows root and evidence paths",
    async () => {
      const value = await fixture();
      expect(value.root.startsWith("\\\\?\\")).toBe(true);
      expect(canonicalHarnessPath(value.root)).toBe(value.root);
      const owned = captureHarnessProcess(value.input);
      expect(owned.pid).toBe(value.proc.pid);
      const ordinaryRoot = realpathSync.native(value.root);
      const ordinaryArtifacts = realpathSync.native(value.artifacts);
      expect(ordinaryRoot).not.toBe(value.root);
      const kill = vi.spyOn(value.proc, "kill");
      const created = lstatSync(value.createdRoot);
      const canonical = lstatSync(value.root);
      expect({ dev: created.dev, ino: created.ino }).toEqual({
        dev: canonical.dev,
        ino: canonical.ino,
      });
      expect(canonicalHarnessPath(value.createdRoot)).toBe(value.root);
      const alias = nativeCanonicalSpelling(value.createdRoot);
      // The real CI temp base contains RUNNER~1. Exercise that actual alias
      // whenever present; no guessed DOS name or fabricated filesystem result.
      if (/[^\\]*~\d+(?:\\|$)/.test(value.createdRoot)) {
        expect(alias).not.toBe(value.root);
        expect(value.root).not.toMatch(/[^\\]*~\d+(?:\\|$)/);
      }
      if (alias !== value.root) {
        expect(() =>
          captureHarnessProcess({
            ...value.input,
            root: alias,
            artifactsDirectory: join(alias, "artifacts"),
            environment: {
              ...value.input.environment,
              GITRU_COLLABORATION_HARNESS_ROOT: alias,
              GITRU_E2E_ARTIFACTS: join(alias, "artifacts"),
            },
          }),
        ).toThrow("unsafe_directory");
      }
      expect(() =>
        captureHarnessProcess({
          ...value.input,
          root: ordinaryRoot,
          artifactsDirectory: ordinaryArtifacts,
          environment: {
            ...value.input.environment,
            GITRU_COLLABORATION_HARNESS_ROOT: ordinaryRoot,
            GITRU_E2E_ARTIFACTS: ordinaryArtifacts,
          },
        }),
      ).toThrow("unsafe_directory");
      expect(kill).not.toHaveBeenCalled();
    },
  );
});

describe("run-owned timeout cleanup", () => {
  it("stops its exact main-phase child and produces separate cleanup evidence", async () => {
    const value = await fixture("main");
    const owned = captureHarnessProcess(value.input);
    writeFileSync(
      join(value.artifacts, "stop-driver.json"),
      JSON.stringify({ run_nonce: value.nonce }),
    );
    const kill = vi.spyOn(value.proc, "kill");
    const proof = await monitorHarnessStop(
      owned,
      () => [owned],
      new AbortController().signal,
    );
    expect(kill).toHaveBeenCalledExactlyOnceWith("SIGTERM");
    expect(proof?.processes).toEqual([
      {
        process_id: owned.pid,
        binary: value.input.binaryPath,
        requested_signals: ["SIGTERM"],
        observed_signal: "SIGTERM",
        exit_code: null,
      },
    ]);
    expect(existsSync(join(value.artifacts, "driver-stopped.json"))).toBe(true);
    expect(existsSync(join(value.artifacts, "process-exit.json"))).toBe(false);
  });
  // Windows ChildProcess SIGTERM terminates abruptly; a JS handler cannot retain
  // that child. The positive owned SIGKILL cases still run on every platform.
  it.skipIf(process.platform === "win32")(
    "forces only a still-live owned child that ignored graceful termination",
    async () => {
      const value = await fixture("restart", true);
      const owned = captureHarnessProcess(value.input);
      writeFileSync(
        join(value.artifacts, "stop-driver.json"),
        JSON.stringify({ run_nonce: value.nonce }),
      );
      const kill = vi.spyOn(value.proc, "kill");
      const proof = await monitorHarnessStop(
        owned,
        () => [owned],
        new AbortController().signal,
      );
      expect(kill.mock.calls).toEqual([["SIGTERM"], ["SIGKILL"]]);
      expect(proof?.processes[0]).toMatchObject({
        process_id: owned.pid,
        requested_signals: ["SIGTERM", "SIGKILL"],
        observed_signal: "SIGKILL",
      });
      expect(existsSync(join(value.artifacts, "process-exit.json"))).toBe(
        false,
      );
    },
  );
  it("rejects an unrelated run nonce without signaling its retained handles", async () => {
    const value = await fixture("main");
    const owned = captureHarnessProcess(value.input);
    writeFileSync(
      join(value.artifacts, "stop-driver.json"),
      JSON.stringify({ run_nonce: randomUUID() }),
    );
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      monitorHarnessStop(owned, () => [owned], new AbortController().signal),
    ).rejects.toMatchObject({ code: "stop_run_mismatch" });
    expect(kill).not.toHaveBeenCalled();
  });
  it("rejects PID/path programs in the stop request", async () => {
    const value = await fixture("main");
    const owned = captureHarnessProcess(value.input);
    writeFileSync(
      join(value.artifacts, "stop-driver.json"),
      JSON.stringify({
        run_nonce: value.nonce,
        process_id: 1,
        path: "/tmp/other",
      }),
    );
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      monitorHarnessStop(owned, () => [owned], new AbortController().signal),
    ).rejects.toMatchObject({ code: "invalid_schema" });
    expect(kill).not.toHaveBeenCalled();
  });
  it("refuses mixed run captures before signaling either child", async () => {
    const first = await fixture("main");
    const second = await fixture("main");
    const firstOwned = captureHarnessProcess(first.input);
    const secondOwned = captureHarnessProcess(second.input);
    writeFileSync(
      join(first.artifacts, "stop-driver.json"),
      JSON.stringify({ run_nonce: first.nonce }),
    );
    const firstKill = vi.spyOn(first.proc, "kill");
    const secondKill = vi.spyOn(second.proc, "kill");
    await expect(
      monitorHarnessStop(
        firstOwned,
        () => [firstOwned, secondOwned],
        new AbortController().signal,
      ),
    ).rejects.toMatchObject({ code: "stop_handle_scope_mismatch" });
    expect(firstKill).not.toHaveBeenCalled();
    expect(secondKill).not.toHaveBeenCalled();
  });
  it("cancels an idle cleanup monitor without claiming a process stopped", async () => {
    const value = await fixture("main");
    const owned = captureHarnessProcess(value.input);
    const signal = new AbortController();
    signal.abort();
    const kill = vi.spyOn(value.proc, "kill");
    expect(
      await monitorHarnessStop(owned, () => [owned], signal.signal),
    ).toBeUndefined();
    expect(kill).not.toHaveBeenCalled();
    expect(existsSync(join(value.artifacts, "driver-stopped.json"))).toBe(
      false,
    );
  });
});

describe("pinned launcher integration", () => {
  function service(value: Fixture, overrides: Record<string, unknown> = {}) {
    const shape = {
      isEmbeddedMode: true,
      embeddedProcesses: new Map([["0", { proc: value.proc }]]),
      embeddedConfigs: new Map([
        [
          "0",
          {
            appBinaryPath: value.input.binaryPath,
            options: { env: value.input.environment },
          },
        ],
      ]),
      ...overrides,
    };
    return {
      shape,
      instance: Reflect.construct(launcher, [shape]) as launcher,
    };
  }
  it("exports the original worker unchanged and delegates normal completion", async () => {
    const value = await fixture("main");
    const { instance } = service(value);
    const kill = vi.spyOn(value.proc, "kill");
    expect(HarnessWorkerService).toBe(OriginalWorkerService);
    await instance.onPrepare({}, []);
    await instance.onComplete(0, {}, []);
    expect(delegated.prepare).toHaveBeenCalledOnce();
    expect(delegated.complete).toHaveBeenCalledExactlyOnceWith(0, {}, []);
    expect(kill).not.toHaveBeenCalled();
    expect(existsSync(join(value.artifacts, "process-exit.json"))).toBe(false);
  });
  it("refuses changed or missing dependency versions before granting process authority", async () => {
    const value = await fixture("main");
    const { shape } = service(value);
    const kill = vi.spyOn(value.proc, "kill");
    for (const version of ["1.5.0", undefined, null])
      expect(() => inspectPinnedHarnessLauncher(shape, version)).toThrowError(
        "unsupported_launcher_version",
      );
    expect(kill).not.toHaveBeenCalled();
  });
  for (const overrides of [
    { embeddedProcesses: [] },
    { embeddedProcesses: new Map() },
    {
      embeddedProcesses: new Map([
        ["0", { proc: { pid: 1, kill: () => true } }],
      ]),
    },
    { isEmbeddedMode: false },
  ]) {
    it("fails closed on an unsupported launcher/process map shape", async () => {
      const value = await fixture("main");
      const { instance } = service(value, overrides);
      const kill = vi.spyOn(value.proc, "kill");
      await expect(instance.onPrepare({}, [])).rejects.toMatchObject({
        code: "unsupported_launcher_shape",
      });
      expect(delegated.complete).toHaveBeenCalledOnce();
      expect(kill).not.toHaveBeenCalled();
    });
  }
  it("binds the crash monitor to the actual post-health-check handle while preserving older cleanup ownership", async () => {
    const first = await fixture();
    const replacement = await fixture();
    const { instance, shape } = service(first);
    const firstKill = vi.spyOn(first.proc, "kill");
    const replacementKill = vi.spyOn(replacement.proc, "kill");
    await instance.onPrepare({}, []);
    delegated.workerStart.mockImplementationOnce(() => {
      shape.embeddedProcesses.set("0", { proc: replacement.proc });
    });
    await instance.onWorkerStart("0-0", undefined);
    first.ack.process_id = replacement.proc.pid!;
    first.checkpoint.process_id = replacement.proc.pid!;
    checkpoint(first);
    acknowledgment(first);
    // Ack alone cannot kill while WDIO still needs its native driver for
    // session teardown. Only the successful worker-end boundary authorizes it.
    await new Promise((done) => setTimeout(done, 120));
    expect(replacementKill).not.toHaveBeenCalled();
    await instance.onWorkerEnd("0-0", 0, [], 0);
    expect(delegated.workerEnd).toHaveBeenCalledExactlyOnceWith("0-0");
    await vi.waitFor(() =>
      expect(existsSync(join(first.artifacts, "process-exit.json"))).toBe(true),
    );
    expect(firstKill).not.toHaveBeenCalled();
    expect(replacementKill).toHaveBeenCalledExactlyOnceWith("SIGKILL");
    await instance.onComplete(0, {}, []);
    expect(delegated.complete).toHaveBeenCalledOnce();
  });
  it("refuses a failed worker despite its passed checkpoint ack and preserves cleanup", async () => {
    const value = await fixture();
    const { instance } = service(value);
    const kill = vi.spyOn(value.proc, "kill");
    await instance.onPrepare({}, []);
    await instance.onWorkerStart("0-0", undefined);
    checkpoint(value);
    acknowledgment(value);
    await expect(instance.onWorkerEnd("0-0", 1, [], 0)).rejects.toMatchObject({
      code: "crash_worker_failed",
    });
    await expect(instance.onComplete(1, {}, [])).rejects.toBeInstanceOf(
      SevereServiceError,
    );
    expect(kill).not.toHaveBeenCalled();
    expect(delegated.complete).toHaveBeenCalledOnce();
    expect(existsSync(join(value.artifacts, "process-exit.json"))).toBe(false);
  });
  it("keeps the original monotonic worker-start bound when teardown finishes late", async () => {
    const value = await fixture();
    const { instance } = service(value);
    const kill = vi.spyOn(value.proc, "kill");
    await instance.onPrepare({}, []);
    const clock = vi.spyOn(performance, "now").mockReturnValue(100);
    await instance.onWorkerStart("0-0", undefined);
    checkpoint(value);
    acknowledgment(value);
    // Starting a new deadline inside worker-end would incorrectly authorize
    // this still-live exact process after its native hold could expire.
    clock.mockReturnValue(45_101);
    await expect(instance.onWorkerEnd("0-0", 0, [], 0)).rejects.toMatchObject({
      code: "checkpoint_window_expired",
    });
    await expect(instance.onComplete(1, {}, [])).rejects.toBeInstanceOf(
      SevereServiceError,
    );
    expect(kill).not.toHaveBeenCalled();
    expect(delegated.complete).toHaveBeenCalledOnce();
    expect(existsSync(join(value.artifacts, "process-exit.json"))).toBe(false);
  });
  it("requires crash proof at completion even if worker-end was never delivered", async () => {
    const value = await fixture();
    const { instance } = service(value);
    const kill = vi.spyOn(value.proc, "kill");
    await instance.onPrepare({}, []);
    await expect(instance.onComplete(0, {}, [])).rejects.toBeInstanceOf(
      SevereServiceError,
    );
    expect(kill).not.toHaveBeenCalled();
    expect(delegated.complete).toHaveBeenCalledOnce();
  });
  it("makes original worker-end failures fatal through the installed dispatcher", async () => {
    const { runHook } = installedHookDispatcher();
    const value = await fixture("main");
    const { instance } = service(value);
    await runHook([instance], "onPrepare", {}, []);
    delegated.workerEnd.mockRejectedValueOnce(new Error("Own teardown failed"));
    await expect(
      runHook([instance], "onWorkerEnd", "0-0", 0, [], 0),
    ).rejects.toBeInstanceOf(SevereServiceError);
    await expect(instance.onComplete(1, {}, [])).rejects.toBeInstanceOf(
      SevereServiceError,
    );
    expect(delegated.complete).toHaveBeenCalledOnce();
  });
  it("preserves a preparation failure together with a delegated cleanup failure", async () => {
    const value = await fixture("main");
    const { instance } = service(value, { embeddedProcesses: [] });
    delegated.complete.mockRejectedValueOnce(
      new Error("Own launcher cleanup failed"),
    );
    const failure = await instance
      .onPrepare({}, [])
      .catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(SevereServiceError);
    expect((failure as Error).cause).toBeInstanceOf(AggregateError);
    expect(((failure as Error).cause as AggregateError).errors).toHaveLength(2);
    expect(delegated.complete).toHaveBeenCalledOnce();
  });
  it("delegates cleanup when the original launcher rejects during preparation", async () => {
    const value = await fixture("main");
    const { instance } = service(value);
    const kill = vi.spyOn(value.proc, "kill");
    const cause = new Error("Own startup failed");
    delegated.prepare.mockRejectedValueOnce(cause);
    await expect(instance.onPrepare({}, [])).rejects.toMatchObject({
      name: "SevereServiceError",
      cause,
    });
    expect(delegated.complete).toHaveBeenCalledExactlyOnceWith(1, {}, []);
    expect(kill).not.toHaveBeenCalled();
  });

  function installedHookDispatcher() {
    // Execute the exact installed CLI dispatcher, including its private
    // HookError class. This starts no WDIO workers/services or native apps and
    // does not mirror the dependency's swallow-versus-reject logic in our code.
    const require = createRequire(import.meta.url);
    const cliDirectory = dirname(require.resolve("@wdio/cli"));
    const metadata = JSON.parse(
      readFileSync(join(cliDirectory, "../package.json"), "utf8"),
    );
    expect(metadata.version).toBe("9.31.7");
    const source = readFileSync(join(cliDirectory, "index.js"), "utf8");
    const start = source.indexOf(
      "var HookError = class extends SevereServiceError {",
    );
    const end = source.indexOf("async function runLauncherHook(", start);
    expect(start).toBeGreaterThanOrEqual(0);
    expect(end).toBeGreaterThan(start);
    const log = { error: vi.fn(), debug: vi.fn() };
    const runHook = new Function(
      "SevereServiceError",
      "log",
      `${source.slice(start, end)}\nreturn runServiceHook;`,
    )(SevereServiceError, log) as (
      services: object[],
      hook: string,
      ...args: unknown[]
    ) => Promise<void>;
    return { runHook, log };
  }
  it("makes pinned ownership failure fatal through the actual installed WDIO dispatcher", async () => {
    const { runHook, log } = installedHookDispatcher();
    // The old ordinary-error behavior is an independent installed control.
    await expect(
      runHook(
        [
          {
            onPrepare: async () => {
              throw new Error("Ordinary fixture failure");
            },
          },
        ],
        "onPrepare",
      ),
    ).resolves.toBeUndefined();
    expect(log.error).toHaveBeenCalledOnce();
    const value = await fixture("main");
    const { instance } = service(value, { embeddedProcesses: [] });
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      runHook([instance], "onPrepare", {}, []),
    ).rejects.toBeInstanceOf(SevereServiceError);
    expect(delegated.complete).toHaveBeenCalledOnce();
    expect(kill).not.toHaveBeenCalled();
  });
  it("makes actual stop-monitor failure fatal at installed onComplete while preserving cleanup", async () => {
    const { runHook } = installedHookDispatcher();
    const value = await fixture("main");
    const { instance } = service(value);
    await runHook([instance], "onPrepare", {}, []);
    writeFileSync(
      join(value.artifacts, "stop-driver.json"),
      JSON.stringify({ run_nonce: randomUUID() }),
    );
    await vi.waitFor(() =>
      expect(existsSync(join(value.artifacts, "driver-stop-error.json"))).toBe(
        true,
      ),
    );
    const kill = vi.spyOn(value.proc, "kill");
    await expect(
      runHook([instance], "onComplete", 0, {}, []),
    ).rejects.toBeInstanceOf(SevereServiceError);
    expect(delegated.complete).toHaveBeenCalledExactlyOnceWith(0, {}, []);
    expect(kill).not.toHaveBeenCalled();
    expect(existsSync(join(value.artifacts, "driver-stopped.json"))).toBe(
      false,
    );
  });
  it("makes original worker-start failure fatal through the installed dispatcher", async () => {
    const { runHook } = installedHookDispatcher();
    const value = await fixture("main");
    const { instance } = service(value);
    await runHook([instance], "onPrepare", {}, []);
    delegated.workerStart.mockRejectedValueOnce(
      new Error("Own health check failed"),
    );
    await expect(
      runHook([instance], "onWorkerStart", "0-0", undefined),
    ).rejects.toBeInstanceOf(SevereServiceError);
    await runHook([instance], "onComplete", 1, {}, []);
    expect(delegated.complete).toHaveBeenCalledExactlyOnceWith(1, {}, []);
  });
});

describe("installed driver dotenv isolation", () => {
  it("loads only the owned empty configured file instead of a fixture cwd .env", () => {
    const root = canonicalHarnessPath(
      mkdtempSync(join(tmpdir(), "gitru-dotenv-test-")),
    );
    chmodSync(root, 0o700);
    try {
      // Both inputs are synthetic, task-owned files. The child receives no
      // inherited environment and never starts WDIO or inspects a personal .env.
      writeFileSync(
        join(root, ".env"),
        "GITRU_TEST_DOTENV_SENTINEL=task-only-sentinel\nGH_TOKEN=task-only-sentinel\n",
        { flag: "wx", mode: 0o600 },
      );
      const configured = join(root, "driver.env");
      writeFileSync(configured, "", { flag: "wx", mode: 0o600 });
      const require = createRequire(import.meta.url);
      const cliRequire = createRequire(require.resolve("@wdio/cli"));
      const dotenvConfig = cliRequire.resolve("dotenv/config");
      const invoke = (path?: string) => {
        const child = spawnSync(
          canonicalHarnessPath(process.execPath),
          [
            "-e",
            "require(process.argv[1]);process.stdout.write(JSON.stringify({sentinel:process.env.GITRU_TEST_DOTENV_SENTINEL==='task-only-sentinel',credential_variable_present:process.env.GH_TOKEN!==undefined}));",
            dotenvConfig,
          ],
          {
            cwd: root,
            env: {
              DOTENV_CONFIG_QUIET: "true",
              ...(path ? { DOTENV_CONFIG_PATH: path } : {}),
            },
            encoding: "utf8",
            timeout: 5000,
            maxBuffer: 4096,
          },
        );
        expect(child.error).toBeUndefined();
        expect(child.status).toBe(0);
        expect(child.signal).toBeNull();
        return JSON.parse(child.stdout);
      };
      // The real installed module's unconfigured control reproduces the risk.
      expect(invoke()).toEqual({
        sentinel: true,
        credential_variable_present: true,
      });
      expect(invoke(configured)).toEqual({
        sentinel: false,
        credential_variable_present: false,
      });
    } finally {
      rmSync(root, { recursive: true });
    }
  });
});
