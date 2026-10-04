//! Node-only ownership proof for the retained fixture runner, never product code.
import { ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  fstatSync,
  fsyncSync,
  lstatSync,
  openSync,
  readSync,
  renameSync,
  unlinkSync,
  writeSync,
} from "node:fs";
import { isAbsolute, join, relative, resolve, sep } from "node:path";
import { canonicalHarnessPath } from "./harness-paths";

export const HARNESS_APPLICATION_ID = "com.ruru.gitru.e2e.collaboration";
export const HARNESS_CRASH_TIMEOUT_MS = 240_000;
export const HARNESS_PROCESS_EXIT_TIMEOUT_MS = 10_000;
const MAX_JSON_BYTES = 4096;
const POLL_MS = 100;
const UUID = /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const U64_MAX = 18_446_744_073_709_551_615n;

export type HarnessCrashPhase = "crash-before-commit" | "crash-after-commit";
export type HarnessRunnerPhase = "main" | "restart" | HarnessCrashPhase;
export const HARNESS_STOP_MONITOR_TIMEOUT_MS = 21 * 60_000;
const STOP_GRACE_MS = 2000;
export type CrashDriverAck = {
  run_nonce: string;
  session_id: string;
  scenario_generation: string;
  kind: "before_commit" | "committed_before_hint";
  process_id: number;
};
type Checkpoint = CrashDriverAck & {
  gate_id: string | null;
  committed_phase: "one" | null;
  committed_facet_revision: string | null;
};
export type HarnessProcessExitProof = CrashDriverAck & {
  version: 1;
  application_id: typeof HARNESS_APPLICATION_ID;
  phase: HarnessCrashPhase;
  binary: string;
  requested_signal: "SIGKILL";
  observed_signal: NodeJS.Signals | null;
  exit_code: number | null;
};

export class HarnessProcessError extends Error {
  constructor(public readonly code: string) {
    super(`Retained harness process qualification failed: ${code}`);
    this.name = "HarnessProcessError";
  }
}
function fail(code: string): never {
  throw new HarnessProcessError(code);
}
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value))
    fail("invalid_schema");
  return value as Record<string, unknown>;
}
function keys(value: Record<string, unknown>, expected: string[]) {
  if (
    Object.keys(value).length !== expected.length ||
    expected.some((key) => !Object.hasOwn(value, key))
  )
    fail("invalid_schema");
}
function decimal(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^[1-9]\d{0,19}$/.test(value) &&
    BigInt(value) <= U64_MAX
  );
}
function ack(value: unknown): CrashDriverAck {
  const source = record(value);
  keys(source, [
    "run_nonce",
    "session_id",
    "scenario_generation",
    "kind",
    "process_id",
  ]);
  if (
    typeof source.run_nonce !== "string" ||
    !UUID.test(source.run_nonce) ||
    typeof source.session_id !== "string" ||
    !UUID.test(source.session_id) ||
    !decimal(source.scenario_generation) ||
    typeof source.kind !== "string" ||
    !["before_commit", "committed_before_hint"].includes(String(source.kind)) ||
    typeof source.process_id !== "number" ||
    !Number.isInteger(source.process_id) ||
    source.process_id <= 0 ||
    source.process_id > 0xffff_ffff
  )
    fail("invalid_schema");
  return source as CrashDriverAck;
}
function checkpoint(value: unknown, phase: HarnessCrashPhase): Checkpoint {
  const source = record(value);
  keys(source, [
    "run_nonce",
    "session_id",
    "scenario_generation",
    "kind",
    "process_id",
    "gate_id",
    "committed_phase",
    "committed_facet_revision",
  ]);
  const common = ack({
    run_nonce: source.run_nonce,
    session_id: source.session_id,
    scenario_generation: source.scenario_generation,
    kind: source.kind,
    process_id: source.process_id,
  });
  if (phase === "crash-before-commit") {
    if (
      common.kind !== "before_commit" ||
      typeof source.gate_id !== "string" ||
      !UUID.test(source.gate_id) ||
      source.committed_phase !== null ||
      source.committed_facet_revision !== null
    )
      fail("checkpoint_phase_mismatch");
  } else if (
    common.kind !== "committed_before_hint" ||
    source.gate_id !== null ||
    source.committed_phase !== "one" ||
    !decimal(source.committed_facet_revision)
  ) {
    fail("checkpoint_phase_mismatch");
  }
  return source as Checkpoint;
}

type Identity = { dev: number; ino: number };
function directory(path: string): Identity {
  if (!isAbsolute(path) || resolve(path) !== path) fail("noncanonical_path");
  const stat = lstatSync(path);
  if (
    !stat.isDirectory() ||
    stat.isSymbolicLink() ||
    canonicalHarnessPath(path) !== path
  )
    fail("unsafe_directory");
  if (process.platform !== "win32" && (stat.mode & 0o077) !== 0)
    fail("public_directory");
  return { dev: stat.dev, ino: stat.ino };
}
function sameIdentity(path: string, expected: Identity) {
  const actual = directory(path);
  if (actual.dev !== expected.dev || actual.ino !== expected.ino)
    fail("directory_replaced");
}
function regular(path: string): Identity {
  if (!isAbsolute(path) || resolve(path) !== path) fail("noncanonical_path");
  const stat = lstatSync(path);
  if (
    !stat.isFile() ||
    stat.isSymbolicLink() ||
    canonicalHarnessPath(path) !== path
  )
    fail("unsafe_file");
  return { dev: stat.dev, ino: stat.ino };
}
function boundedJson(path: string, optional = false): unknown | undefined {
  let descriptor: number;
  let identity: Identity;
  try {
    identity = regular(path);
    descriptor = openSync(
      path,
      constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0),
    );
  } catch (error) {
    if (optional && (error as NodeJS.ErrnoException).code === "ENOENT")
      return undefined;
    throw error;
  }
  try {
    const stat = fstatSync(descriptor);
    if (
      !stat.isFile() ||
      stat.dev !== identity.dev ||
      stat.ino !== identity.ino
    )
      fail("file_replaced");
    if (stat.size > MAX_JSON_BYTES) fail("oversized_json");
    // Keep the read bounded even if a private task file grows after its stat.
    const buffer = Buffer.alloc(MAX_JSON_BYTES + 1);
    let count = 0;
    while (count < buffer.length) {
      const read = readSync(
        descriptor,
        buffer,
        count,
        buffer.length - count,
        null,
      );
      if (!read) break;
      count += read;
    }
    if (count > MAX_JSON_BYTES) fail("oversized_json");
    try {
      return JSON.parse(buffer.subarray(0, count).toString("utf8"));
    } catch {
      return fail("invalid_json");
    }
  } finally {
    closeSync(descriptor);
  }
}

export type HarnessProcessInput = {
  root: string;
  runNonce: string;
  artifactsDirectory: string;
  binaryPath: string;
  phase: HarnessRunnerPhase;
  proc: ChildProcess;
  environment: Readonly<Record<string, string | undefined>>;
  /** Read from the original launcher's current map; never an OS PID lookup. */
  stillOwned: () => boolean;
};
export type OwnedHarnessProcess = Readonly<{
  input: HarnessProcessInput;
  rootIdentity: Identity;
  artifactsIdentity: Identity;
  binaryIdentity: Identity;
  pid: number;
}>;
const captures = new WeakSet<OwnedHarnessProcess>();

function validateRoot(owned: OwnedHarnessProcess) {
  const { input } = owned;
  sameIdentity(input.root, owned.rootIdentity);
  sameIdentity(input.artifactsDirectory, owned.artifactsIdentity);
  const marker = record(boundedJson(join(input.root, "run.json")));
  keys(marker, ["version", "application_id", "run_nonce"]);
  if (
    marker.version !== 1 ||
    marker.application_id !== HARNESS_APPLICATION_ID ||
    marker.run_nonce !== input.runNonce
  )
    fail("root_marker_mismatch");
}
function validateCapture(owned: OwnedHarnessProcess) {
  if (!captures.has(owned)) fail("unknown_process_capture");
  validateRoot(owned);
  const { input } = owned;
  const env = input.environment;
  if (!(input.proc instanceof ChildProcess) || input.proc.pid !== owned.pid)
    fail("retired_process_handle");
  if (
    env.GITRU_COLLABORATION_HARNESS_ROOT !== input.root ||
    env.GITRU_COLLABORATION_HARNESS_RUN_NONCE !== input.runNonce ||
    env.GITRU_COLLABORATION_HARNESS_PHASE !== input.phase ||
    env.GITRU_E2E_ARTIFACTS !== input.artifactsDirectory
  )
    fail("launcher_environment_mismatch");
  const identity = regular(input.binaryPath);
  if (
    typeof input.proc.spawnfile !== "string" ||
    input.proc.spawnfile !== input.binaryPath ||
    canonicalHarnessPath(input.proc.spawnfile) !== input.binaryPath ||
    identity.dev !== owned.binaryIdentity.dev ||
    identity.ino !== owned.binaryIdentity.ino
  )
    fail("owned_binary_mismatch");
}
function validateOwnership(owned: OwnedHarnessProcess) {
  validateCapture(owned);
  if (
    !owned.input.stillOwned() ||
    owned.input.proc.exitCode !== null ||
    owned.input.proc.signalCode !== null ||
    owned.input.proc.killed
  )
    fail("retired_process_handle");
}

export function captureHarnessProcess(
  input: HarnessProcessInput,
): OwnedHarnessProcess {
  if (
    !UUID.test(input.runNonce) ||
    !["main", "restart", "crash-before-commit", "crash-after-commit"].includes(
      input.phase,
    ) ||
    typeof input.stillOwned !== "function" ||
    !(input.proc instanceof ChildProcess) ||
    typeof input.proc.pid !== "number" ||
    !Number.isInteger(input.proc.pid) ||
    input.proc.pid <= 0
  )
    fail("invalid_process_capture");
  const nested = relative(input.root, input.artifactsDirectory);
  if (
    !nested ||
    nested === ".." ||
    nested.startsWith(`..${sep}`) ||
    isAbsolute(nested)
  )
    fail("artifacts_outside_root");
  const owned: OwnedHarnessProcess = Object.freeze({
    input: Object.freeze({ ...input }),
    rootIdentity: directory(input.root),
    artifactsIdentity: directory(input.artifactsDirectory),
    binaryIdentity: regular(input.binaryPath),
    pid: input.proc.pid,
  });
  captures.add(owned);
  validateOwnership(owned);
  return owned;
}

type ArtifactName =
  | "process-exit.json"
  | "process-exit-error.json"
  | "driver-stopped.json"
  | "driver-stop-error.json";
function writeArtifact(
  owned: OwnedHarnessProcess,
  name: ArtifactName,
  value: unknown,
) {
  validateRoot(owned);
  const bytes = Buffer.from(JSON.stringify(value));
  if (bytes.length > MAX_JSON_BYTES) fail("oversized_artifact");
  const destination = join(owned.input.artifactsDirectory, name);
  try {
    regular(destination);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
  const temporary = join(
    owned.input.artifactsDirectory,
    `.process-${randomUUID()}.tmp`,
  );
  const descriptor = openSync(
    temporary,
    constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL,
    0o600,
  );
  try {
    writeSync(descriptor, bytes);
    fsyncSync(descriptor);
  } catch (error) {
    closeSync(descriptor);
    unlinkSync(temporary);
    throw error;
  }
  closeSync(descriptor);
  try {
    validateRoot(owned);
    renameSync(temporary, destination);
    if (process.platform !== "win32") {
      const directoryHandle = openSync(
        owned.input.artifactsDirectory,
        constants.O_RDONLY,
      );
      try {
        fsyncSync(directoryHandle);
      } finally {
        closeSync(directoryHandle);
      }
    }
  } catch (error) {
    try {
      unlinkSync(temporary);
    } catch {}
    throw error;
  }
}

function waitPoll(signal: AbortSignal): Promise<void> {
  if (signal.aborted)
    return Promise.reject(new HarnessProcessError("monitor_stopped"));
  return new Promise((resolveWait, reject) => {
    const stop = () => {
      clearTimeout(timer);
      signal.removeEventListener("abort", stop);
      reject(new HarnessProcessError("monitor_stopped"));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", stop);
      resolveWait();
    }, POLL_MS);
    signal.addEventListener("abort", stop, { once: true });
  });
}

function killAndObserve(
  owned: OwnedHarnessProcess,
): Promise<{ code: number | null; signal: NodeJS.Signals | null }> {
  // Every authority check is synchronous and repeated immediately before the
  // sole kill call. The actual ChildProcess handle is the only signal target.
  validateOwnership(owned);
  return new Promise((resolveExit, reject) => {
    const proc = owned.input.proc;
    const cleanup = () => {
      clearTimeout(timer);
      proc.removeListener("exit", exited);
      proc.removeListener("error", errored);
    };
    const exited = (code: number | null, signal: NodeJS.Signals | null) => {
      cleanup();
      if (signal !== "SIGKILL")
        return reject(new HarnessProcessError("unexpected_process_exit"));
      resolveExit({ code, signal });
    };
    const errored = () => {
      cleanup();
      reject(new HarnessProcessError("owned_kill_error"));
    };
    const timer = setTimeout(() => {
      cleanup();
      reject(new HarnessProcessError("owned_exit_timeout"));
    }, HARNESS_PROCESS_EXIT_TIMEOUT_MS);
    proc.once("exit", exited);
    proc.once("error", errored);
    try {
      if (!proc.kill("SIGKILL")) {
        cleanup();
        reject(new HarnessProcessError("owned_kill_refused"));
      }
    } catch {
      errored();
    }
  });
}

/** A checkpoint alone never authorizes a kill; a successful WDIO spec must ack it. */
export async function monitorHarnessCrash(
  owned: OwnedHarnessProcess,
  signal: AbortSignal,
  killDeadline?: number,
): Promise<HarnessProcessExitProof> {
  try {
    if (
      owned.input.phase !== "crash-before-commit" &&
      owned.input.phase !== "crash-after-commit"
    )
      fail("invalid_crash_phase");
    if (killDeadline !== undefined && !Number.isFinite(killDeadline))
      fail("invalid_crash_deadline");
    const deadline = Math.min(
      performance.now() + HARNESS_CRASH_TIMEOUT_MS,
      killDeadline ?? Number.POSITIVE_INFINITY,
    );
    while (performance.now() < deadline) {
      if (signal.aborted) fail("monitor_stopped");
      validateOwnership(owned);
      const acknowledgment = boundedJson(
        join(owned.input.artifactsDirectory, "crash-driver-ack.json"),
        true,
      );
      if (acknowledgment !== undefined) {
        // A delayed successful teardown must not outlive the held native gate.
        // This private file is checked after bounded non-symlink JSON reading;
        // no old/future acknowledgment can authorize a current forced crash.
        const acknowledged = lstatSync(
          join(owned.input.artifactsDirectory, "crash-driver-ack.json"),
        );
        const age = Date.now() - acknowledged.mtimeMs;
        if (age < -1 || age > 15_000) fail("checkpoint_ack_expired");
        const verifiedAck = ack(acknowledgment);
        const observed = checkpoint(
          boundedJson(join(owned.input.root, "crash-checkpoint.json")),
          owned.input.phase,
        );
        if (
          verifiedAck.run_nonce !== owned.input.runNonce ||
          Object.entries(verifiedAck).some(
            ([field, value]) =>
              observed[field as keyof CrashDriverAck] !== value,
          ) ||
          verifiedAck.process_id !== owned.pid
        )
          fail("checkpoint_ack_mismatch");
        if (signal.aborted) fail("monitor_stopped");
        if (performance.now() >= deadline) fail("checkpoint_window_expired");
        const exit = await killAndObserve(owned);
        const proof: HarnessProcessExitProof = {
          ...verifiedAck,
          version: 1,
          application_id: HARNESS_APPLICATION_ID,
          phase: owned.input.phase,
          binary: owned.input.binaryPath,
          requested_signal: "SIGKILL",
          observed_signal: exit.signal,
          exit_code: exit.code,
        };
        writeArtifact(owned, "process-exit.json", proof);
        return proof;
      }
      await waitPoll(signal);
    }
    return fail(
      killDeadline === undefined
        ? "checkpoint_ack_timeout"
        : "checkpoint_window_expired",
    );
  } catch (error) {
    const failure =
      error instanceof HarnessProcessError
        ? error
        : new HarnessProcessError("fixture_filesystem_failure");
    try {
      writeArtifact(owned, "process-exit-error.json", {
        version: 1,
        run_nonce: owned.input.runNonce,
        phase: owned.input.phase,
        reason: failure.code,
      });
    } catch {
      // A replaced/untrusted artifact path must never be followed to report an error.
    }
    throw failure;
  }
}

export type HarnessStoppedProcess = {
  process_id: number;
  binary: string;
  requested_signals: Array<"SIGTERM" | "SIGKILL">;
  observed_signal: NodeJS.Signals | null;
  exit_code: number | null;
};
export type HarnessDriverStopProof = {
  version: 1;
  application_id: typeof HARNESS_APPLICATION_ID;
  run_nonce: string;
  phase: HarnessRunnerPhase;
  processes: HarnessStoppedProcess[];
};

function alive(proc: ChildProcess) {
  return proc.exitCode === null && proc.signalCode === null;
}

/** Cleanup may retire an originally captured handle after launcher replacement.
 * It never uses a lookup PID, fabricates crash evidence, or signals an exited handle. */
function stopCapturedProcess(
  owned: OwnedHarnessProcess,
): Promise<HarnessStoppedProcess> {
  validateCapture(owned);
  const proc = owned.input.proc;
  const requested: Array<"SIGTERM" | "SIGKILL"> = [];
  const proof = (
    code: number | null,
    signal: NodeJS.Signals | null,
  ): HarnessStoppedProcess => ({
    process_id: owned.pid,
    binary: owned.input.binaryPath,
    requested_signals: requested,
    observed_signal: signal,
    exit_code: code,
  });
  if (!alive(proc))
    return Promise.resolve(proof(proc.exitCode, proc.signalCode));
  return new Promise((resolveExit, reject) => {
    let grace: ReturnType<typeof setTimeout> | undefined;
    const cleanup = () => {
      clearTimeout(grace);
      clearTimeout(deadline);
      proc.removeListener("exit", exited);
      proc.removeListener("error", errored);
    };
    const exited = (code: number | null, signal: NodeJS.Signals | null) => {
      cleanup();
      resolveExit(proof(code, signal));
    };
    const errored = () => {
      cleanup();
      reject(new HarnessProcessError("owned_stop_error"));
    };
    const deadline = setTimeout(() => {
      cleanup();
      reject(new HarnessProcessError("owned_stop_timeout"));
    }, HARNESS_PROCESS_EXIT_TIMEOUT_MS);
    proc.once("exit", exited);
    proc.once("error", errored);
    try {
      validateCapture(owned);
      requested.push("SIGTERM");
      if (!proc.kill("SIGTERM")) {
        errored();
        return;
      }
      grace = setTimeout(() => {
        try {
          if (!alive(proc)) return;
          validateCapture(owned);
          requested.push("SIGKILL");
          if (!proc.kill("SIGKILL")) errored();
        } catch {
          errored();
        }
      }, STOP_GRACE_MS);
    } catch {
      errored();
    }
  });
}

/** Runs in the owning WDIO launcher while its worker is alive. Timeout cleanup
 * has an independent receipt; it cannot satisfy a passed hard-crash scenario. */
export async function monitorHarnessStop(
  initial: OwnedHarnessProcess,
  retained: () => ReadonlyArray<OwnedHarnessProcess>,
  signal: AbortSignal,
): Promise<HarnessDriverStopProof | undefined> {
  try {
    const deadline = performance.now() + HARNESS_STOP_MONITOR_TIMEOUT_MS;
    while (performance.now() < deadline) {
      if (signal.aborted) return undefined;
      validateRoot(initial);
      const handles = retained();
      if (!handles.length || handles.length > 4)
        fail("invalid_retained_handles");
      const request = boundedJson(
        join(initial.input.artifactsDirectory, "stop-driver.json"),
        true,
      );
      if (request !== undefined) {
        const value = record(request);
        keys(value, ["run_nonce"]);
        if (value.run_nonce !== initial.input.runNonce)
          fail("stop_run_mismatch");
        for (const handle of handles) {
          if (
            handle.input.root !== initial.input.root ||
            handle.input.runNonce !== initial.input.runNonce ||
            handle.input.phase !== initial.input.phase ||
            handle.input.artifactsDirectory !==
              initial.input.artifactsDirectory ||
            handle.input.binaryPath !== initial.input.binaryPath
          )
            fail("stop_handle_scope_mismatch");
          validateCapture(handle);
        }
        const processes = await Promise.all(handles.map(stopCapturedProcess));
        const proof: HarnessDriverStopProof = {
          version: 1,
          application_id: HARNESS_APPLICATION_ID,
          run_nonce: initial.input.runNonce,
          phase: initial.input.phase,
          processes,
        };
        writeArtifact(initial, "driver-stopped.json", proof);
        return proof;
      }
      await waitPoll(signal);
    }
    return fail("stop_monitor_timeout");
  } catch (error) {
    if (
      signal.aborted &&
      error instanceof HarnessProcessError &&
      error.code === "monitor_stopped"
    )
      return undefined;
    const failure =
      error instanceof HarnessProcessError
        ? error
        : new HarnessProcessError("stop_filesystem_failure");
    try {
      writeArtifact(initial, "driver-stop-error.json", {
        version: 1,
        run_nonce: initial.input.runNonce,
        phase: initial.input.phase,
        reason: failure.code,
      });
    } catch {}
    throw failure;
  }
}
