import type { InvokeArgs } from "@tauri-apps/api/core";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { type Mock, vi } from "vitest";

type TauriCommandImplementation<TResult> = (
  payload: InvokeArgs | undefined,
) => TResult | Promise<TResult>;

type AnyTauriCommandImplementation = TauriCommandImplementation<unknown>;

const commandImplementations = new Map<string, AnyTauriCommandImplementation>();

const installFailClosedBoundary = () => {
  clearMocks();
  mockIPC((command, payload) => {
    const implementation = commandImplementations.get(command);

    if (!implementation) {
      throw new Error(
        `Unexpected native Tauri command "${command}". Register it with mockTauriCommand() before exercising this behavior.`,
      );
    }

    return implementation(payload);
  });
  mockWindows("main");
};

export const mockTauriCommand = <TResult>(
  command: string,
  implementation: TauriCommandImplementation<TResult>,
): Mock<TauriCommandImplementation<TResult>> => {
  if (command.trim().length === 0) {
    throw new Error("A Tauri command mock requires a non-empty command name.");
  }

  const commandMock = vi.fn(implementation);
  commandImplementations.set(
    command,
    commandMock as AnyTauriCommandImplementation,
  );
  return commandMock;
};

export const mockTauriCommandResult = <TResult>(
  command: string,
  result: TResult,
) => mockTauriCommand(command, () => result);

export const resetTauriMocks = () => {
  commandImplementations.clear();
  installFailClosedBoundary();
};
