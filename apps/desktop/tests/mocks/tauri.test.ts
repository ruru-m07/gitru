import { invoke } from "@tauri-apps/api/core";
import { describe, expect, test } from "vitest";
import { mockTauriCommandResult } from "./tauri";

describe("Tauri command mock boundary", () => {
  test("rejects unregistered native commands", async () => {
    await expect(invoke("unregistered_command")).rejects.toThrow(
      'Unexpected native Tauri command "unregistered_command"',
    );
  });

  test("routes only explicitly registered commands", async () => {
    const command = mockTauriCommandResult("registered_command", {
      ok: true,
    });

    await expect(
      invoke("registered_command", { repositoryId: "repo-1" }),
    ).resolves.toEqual({ ok: true });
    expect(command).toHaveBeenCalledOnce();
    expect(command).toHaveBeenCalledWith({ repositoryId: "repo-1" });
  });
});
