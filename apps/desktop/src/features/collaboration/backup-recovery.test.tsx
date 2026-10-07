import type { CollaborationRecoveryPreview } from "@gitru/commands";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it } from "vitest";
import {
  mockTauriCommand,
  mockTauriCommandResult,
} from "../../../tests/mocks/tauri";
import { BackupRecoveryButton } from "./backup-recovery";

const preview: CollaborationRecoveryPreview = {
  session_id: "native-session",
  restore: {
    confirmation_id: "native-confirmation",
    incoming: {
      sha256: "a".repeat(64),
      revision: "12",
      schema_version: 15,
      accounts: 2,
      drafts: 7,
      commands: 3,
    },
    current_revision: "20",
    current_drafts: 9,
    current_accounts: 2,
    newer_current_drafts_remain_in_original_bundle: true,
    reauthentication_required: true,
    cached_provider_data_will_be_removed: true,
    recovery_generation: "1",
    quarantined_commands: 3,
    incoming_evidence_retained: true,
  },
  interrupted: null,
};
const done = { runtime_ready: true, originals_preserved: true };
async function open() {
  const user = userEvent.setup();
  const view = render(<BackupRecoveryButton />);
  await user.click(screen.getByRole("button", { name: "Backups" }));
  return { user, ...view };
}

it("is usable without loading accounts and sends no filesystem path from the renderer", async () => {
  const backup = mockTauriCommandResult(
    "collaboration_backup",
    preview.restore?.incoming,
  );
  const { user } = await open();
  expect(backup).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Save a backup" }));
  expect(
    await screen.findByText(/Backup verified and saved: 7 drafts/),
  ).toBeVisible();
  expect(backup).toHaveBeenCalledWith({});
});

it("treats a cancelled native picker as no prepared recovery", async () => {
  mockTauriCommandResult("collaboration_prepare_restore", null);
  const confirm = mockTauriCommandResult(
    "collaboration_confirm_recovery",
    done,
  );
  const { user } = await open();
  await user.click(
    screen.getByRole("button", { name: "Choose backup to restore" }),
  );
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(confirm).not.toHaveBeenCalled();
});

it("requires the explicit inspected replacement choice and uses native session identifiers", async () => {
  mockTauriCommandResult("collaboration_prepare_restore", preview);
  const confirm = mockTauriCommandResult(
    "collaboration_confirm_recovery",
    done,
  );
  const { user } = await open();
  await user.click(
    screen.getByRole("button", { name: "Choose backup to restore" }),
  );
  expect(await screen.findByRole("alertdialog")).toBeVisible();
  expect(screen.getByText("Commands held for review")).toBeVisible();
  expect(screen.getByText(/Drafts are not merged automatically/)).toBeVisible();
  expect(confirm).not.toHaveBeenCalled();
  await user.click(
    screen.getByRole("button", { name: "Replace collaboration data" }),
  );
  expect(confirm).toHaveBeenCalledWith({
    request: {
      session_id: "native-session",
      confirmation_id: "native-confirmation",
      action: "replace_current_data",
    },
  });
  expect(await screen.findByText(/Recovery finished/)).toBeVisible();
});

it("cancels a prepared preview instead of replacing data", async () => {
  mockTauriCommandResult("collaboration_prepare_restore", preview);
  const cancel = mockTauriCommandResult("collaboration_cancel_recovery", done);
  const confirm = mockTauriCommandResult(
    "collaboration_confirm_recovery",
    done,
  );
  const { user } = await open();
  await user.click(
    screen.getByRole("button", { name: "Choose backup to restore" }),
  );
  await user.click(
    await screen.findByRole("button", { name: "Cancel recovery" }),
  );
  expect(cancel).toHaveBeenCalledWith({ sessionId: "native-session" });
  expect(confirm).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(),
  );
});

it("keeps an expired preview dismissible after native retirement", async () => {
  mockTauriCommandResult("collaboration_prepare_restore", preview);
  mockTauriCommand("collaboration_cancel_recovery", () => {
    throw { code: "stale_view", message: "Expired", retry_after_seconds: null };
  });
  const { user } = await open();
  await user.click(
    screen.getByRole("button", { name: "Choose backup to restore" }),
  );
  await user.click(
    await screen.findByRole("button", { name: "Cancel recovery" }),
  );
  await waitFor(() =>
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(),
  );
});

it("offers keep-original confirmation for an interrupted recovery", async () => {
  mockTauriCommandResult("collaboration_inspect_interrupted_recovery", {
    session_id: "interrupted-session",
    restore: null,
    interrupted: {
      confirmation_id: "original-confirmation",
      original_files: 3,
      current_data_will_remain_in_recovery_bundle: true,
    },
  });
  const confirm = mockTauriCommandResult(
    "collaboration_confirm_recovery",
    done,
  );
  const { user } = await open();
  await user.click(
    screen.getByRole("button", { name: "Inspect interrupted recovery" }),
  );
  await user.click(
    await screen.findByRole("button", { name: "Keep original data" }),
  );
  expect(confirm).toHaveBeenCalledWith({
    request: {
      session_id: "interrupted-session",
      confirmation_id: "original-confirmation",
      action: "keep_original_data",
    },
  });
});

it("releases a late prepared session after the initiating view unmounts", async () => {
  let resolve!: (value: CollaborationRecoveryPreview) => void;
  mockTauriCommand(
    "collaboration_prepare_restore",
    () =>
      new Promise<CollaborationRecoveryPreview>((done) => {
        resolve = done;
      }),
  );
  const cancel = mockTauriCommandResult("collaboration_cancel_recovery", done);
  const { user, unmount } = await open();
  await user.click(
    screen.getByRole("button", { name: "Choose backup to restore" }),
  );
  unmount();
  await act(async () => resolve(preview));
  await waitFor(() =>
    expect(cancel).toHaveBeenCalledWith({ sessionId: "native-session" }),
  );
});

it("keeps a failed preparation visible without offering an unverified confirmation", async () => {
  mockTauriCommand("collaboration_prepare_restore", () => {
    throw {
      code: "storage",
      message: "The backup could not be verified",
      retry_after_seconds: null,
    };
  });
  const { user } = await open();
  await user.click(
    screen.getByRole("button", { name: "Choose backup to restore" }),
  );
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Saved collaboration data could not be read. Try again.",
  );
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
});
