import {
  AccountSnapshotSchema,
  ChangePageSchema,
  GithubCliDiscoverySchema,
} from "@gitru/commands";
import { browser } from "@wdio/globals";

type ChildSnapshot = {
  label: string;
  route: string;
  instanceNonce: string;
  hasAccountsButton: boolean;
  hasPatForm: boolean;
};

describe("packaged collaboration storage", () => {
  it("reads local snapshots without a provider or Gitru cloud account", async () => {
    const result = await browser.executeAsync((done) => {
      const native = (
        window as Window & {
          __TAURI__: {
            core: { invoke: (name: string, args?: object) => Promise<unknown> };
          };
        }
      ).__TAURI__;
      Promise.all([
        native.core.invoke("collaboration_accounts"),
        native.core.invoke("collaboration_changes_since", {
          afterRevision: "0",
        }),
        native.core.invoke("collaboration_discover_github_cli"),
      ]).then(
        ([accounts, changes, cli]) => done({ accounts, changes, cli }),
        (error) => done({ error }),
      );
    });
    if (!result || typeof result !== "object") {
      throw new Error("Native collaboration did not return a snapshot");
    }
    if ("error" in result) {
      throw new Error(
        `Native collaboration read failed: ${JSON.stringify(result.error)}`,
      );
    }
    if (
      !("accounts" in result) ||
      !("changes" in result) ||
      !("cli" in result)
    ) {
      throw new Error("Native collaboration snapshots were incomplete");
    }
    const accounts = AccountSnapshotSchema.parse(result.accounts);
    const changes = ChangePageSchema.parse(result.changes);
    const cli = GithubCliDiscoverySchema.parse(result.cli);
    if (cli.status !== "not_installed" || cli.accounts.length !== 0) {
      throw new Error("E2E unexpectedly enabled personal GitHub CLI discovery");
    }
    if (accounts.accounts.length !== 0 || changes.changes.length !== 0) {
      throw new Error("The isolated E2E collaboration database was not empty");
    }
    if (changes.has_more || changes.reset_required) {
      throw new Error("A fresh local database unexpectedly required catch-up");
    }
    if (accounts.revision !== changes.revision) {
      throw new Error("Read-only snapshots changed the durable revision");
    }
  });

  it("opens host account settings from the real Inbox child and restores that same tab", async () => {
    // The embedded driver looks up WebviewWindows and cannot resolve main once
    // it contains child Webviews. Start one script before children exist, keep
    // its document alive through SPA navigation, then restore embedded mode
    // and wait for child cleanup before any subsequent driver command.
    await browser.setTimeout({ script: 150_000 });
    const result = await browser.executeAsync((done) => {
      const native = (
        window as unknown as {
          __TAURI__: {
            core: {
              invoke: (name: string) => Promise<Array<{ label: string }>>;
            };
            event: {
              listen: (
                name: string,
                handler: (event: { payload: ChildSnapshot }) => void,
              ) => Promise<() => void>;
              emitTo: (
                target: { kind: "Webview"; label: string },
                name: string,
                payload: { label: string; action: string },
              ) => Promise<void>;
            };
          };
        }
      ).__TAURI__;

      const scenarioDeadline = Date.now() + 100_000;
      const delay = () =>
        new Promise((resolve) => window.setTimeout(resolve, 100));
      const labels = async () =>
        (await native.core.invoke("plugin:webview|get_all_webviews"))
          .map((view) => view.label)
          .filter((label) => label.startsWith("tab-webview:"));
      const request = (label: string, action = "inspect") =>
        new Promise<ChildSnapshot>((resolve, reject) => {
          let unlisten = () => {};
          let finished = false;
          const timer = window.setTimeout(() => finish(null), 5_000);
          function finish(snapshot: ChildSnapshot | null) {
            if (finished) return;
            finished = true;
            window.clearTimeout(timer);
            unlisten();
            if (snapshot) resolve(snapshot);
            else reject(new Error("The fixed E2E probe did not answer"));
          }
          void native.event
            .listen("gitru:e2e-collaboration-result", (event) => {
              if (event.payload.label === label) finish(event.payload);
            })
            .then((remove) => {
              unlisten = remove;
              if (finished) return remove();
              return native.event.emitTo(
                { kind: "Webview", label },
                "gitru:e2e-collaboration-request",
                { label, action },
              );
            })
            .catch(() => finish(null));
        });
      async function waitFor<T>(
        read: () => Promise<T | null> | T | null,
        cleanup = false,
      ) {
        const deadline = cleanup
          ? Date.now() + 25_000
          : Math.min(Date.now() + 25_000, scenarioDeadline);
        while (Date.now() < deadline) {
          const value = await read();
          if (value !== null) return value;
          await delay();
        }
        throw new Error("The fixed E2E state did not settle");
      }
      function closeDialog() {
        const popup = document.querySelector(
          '[data-slot="dialog-popup"][data-open]:not([data-closed])',
        );
        const close = Array.from(
          popup?.querySelectorAll<HTMLButtonElement>("button") ?? [],
        ).find(
          (button) => button.querySelector(".sr-only")?.textContent === "Close",
        );
        close?.click();
      }

      void (async () => {
        let stage = "open host";
        let failure: string | null = null;
        let before: ChildSnapshot | null = null;
        let after: ChildSnapshot | null = null;
        let host: ChildSnapshot | null = null;
        let restored: ChildSnapshot | null = null;
        try {
          await request("main", "open-host");
          stage = "mount native child";
          const childLabel = await waitFor(async () => {
            const children = await labels();
            return children.length === 1 ? (children[0] ?? null) : null;
          });
          stage = "mount child Accounts button";
          await waitFor(async () => {
            // Native creation precedes the child's JS listener registration.
            // An inspect emitted during startup can be lost; retry this
            // read-only probe within the existing readiness deadline. Click
            // actions below still run once and fail if they do not answer.
            const child = await request(childLabel).catch(() => null);
            if (!child) return null;
            return child.hasAccountsButton ? child : null;
          });
          stage = "navigate native child to Inbox";
          await request(childLabel, "click-inbox");
          before = await waitFor(async () => {
            const child = await request(childLabel);
            return child.route.startsWith("/app/inbox") &&
              child.hasAccountsButton
              ? child
              : null;
          });
          if (before.hasPatForm)
            throw new Error("Child mounted credential controls");

          stage = "open host PAT dialog from native child";
          await request(childLabel, "click-accounts");
          await waitFor(() => {
            const input = document.querySelector<HTMLInputElement>(
              '[data-slot="dialog-popup"][data-open]:not([data-closed]) input[name="github-token"]',
            );
            return input &&
              !input.disabled &&
              input.getBoundingClientRect().width > 0
              ? input
              : null;
          });
          host = await request("main");
          if (!host.hasPatForm || (await request(childLabel)).hasPatForm) {
            throw new Error("Credential controls were not confined to main");
          }

          stage = "close host PAT dialog";
          closeDialog();
          await waitFor(() =>
            document.querySelector('input[name="github-token"]') === null
              ? true
              : null,
          );
          stage = "restore same Inbox child";
          const original = before;
          after = await waitFor(async () => {
            const child = await request(childLabel);
            return child.instanceNonce === original.instanceNonce &&
              child.route === original.route &&
              child.hasAccountsButton &&
              !child.hasPatForm &&
              (await labels()).includes(childLabel)
              ? child
              : null;
          });
        } catch {
          // Fixed stage names diagnose the regression without returning raw
          // provider errors, DOM text, credential values, or arbitrary payloads.
          failure = stage;
        } finally {
          try {
            closeDialog();
            restored = await request("main", "restore-embedded");
            await waitFor(
              async () =>
                (await labels()).length === 0 &&
                window.location.pathname === "/app/git" &&
                new URLSearchParams(window.location.search).get("embedded") ===
                  "1" &&
                document.querySelector('input[name="github-token"]') === null
                  ? true
                  : null,
              true,
            );
          } catch {
            failure = failure
              ? `${failure}; restore embedded cleanup`
              : "restore embedded cleanup";
          }
        }
        done({ before, after, host, restored, failure });
      })();
    });
    if (!result || typeof result !== "object" || !("failure" in result)) {
      throw new Error("The native child scenario did not return a result");
    }
    if (result.failure) {
      throw new Error(
        `The native child account scenario failed during: ${result.failure}`,
      );
    }
    if (
      !("before" in result) ||
      !("after" in result) ||
      !("host" in result) ||
      !("restored" in result)
    ) {
      throw new Error("The native child scenario omitted its fixed metadata");
    }
    if (!result.before || !result.after || !result.host || !result.restored) {
      throw new Error("The native child account scenario did not complete");
    }
    await browser.setTimeout({ script: 30_000 });
  });
});

it("backs up, cancels preview and restores through the packaged recovery UI", async () => {
  await browser.setTimeout({ script: 150_000 });
  const result = await browser.executeAsync((done) => {
    const native = (
      window as unknown as {
        __TAURI__: {
          core: {
            invoke: (command: string, args?: object) => Promise<unknown>;
          };
          event: {
            emitTo: (
              target: { kind: "Webview"; label: string },
              event: string,
              payload: object,
            ) => Promise<void>;
          };
        };
      }
    ).__TAURI__;
    const end = Date.now() + 120_000;
    let stage = "open recovery workspace";
    const button = (text: string) =>
      Array.from(document.querySelectorAll<HTMLButtonElement>("button")).find(
        (value) => value.textContent?.trim() === text && !value.disabled,
      );
    const wait = async (test: () => boolean) => {
      while (Date.now() < end) {
        if (test()) return;
        await new Promise((resolve) => setTimeout(resolve, 50));
      }
      throw new Error("Fixed recovery state did not settle");
    };
    const click = async (text: string) => {
      await wait(() => Boolean(button(text)));
      button(text)?.click();
    };
    const text = (value: string) =>
      document.body.textContent?.includes(value) === true;
    const accounts = async () =>
      native.core.invoke("collaboration_accounts") as Promise<{
        revision: string;
        authorization_view: string;
        accounts: unknown[];
      }>;
    void (async () => {
      try {
        await native.event.emitTo(
          { kind: "Webview", label: "main" },
          "gitru:e2e-collaboration-request",
          { label: "main", action: "open-recovery" },
        );
        await click("Backups");
        const before = await accounts();
        if (before.accounts.length !== 0)
          throw new Error("Unexpected E2E accounts");
        stage = "save verified native backup";
        await native.core.invoke("collaboration_e2e_recovery_picker", {
          restore: false,
        });
        await click("Save a backup");
        await wait(() => text("Backup verified and saved"));
        stage = "prepare and cancel real restore preview";
        await native.core.invoke("collaboration_e2e_recovery_picker", {
          restore: true,
        });
        await click("Choose backup to restore");
        await wait(() => text("Replace collaboration data with this backup?"));
        let paused = false;
        try {
          await accounts();
        } catch (failure) {
          paused =
            typeof failure === "object" &&
            failure !== null &&
            "code" in failure &&
            failure.code === "not_ready";
        }
        if (!paused) throw new Error("Preview did not fence native reads");
        await click("Cancel recovery");
        await wait(() => !text("Replace collaboration data with this backup?"));
        const cancelled = await accounts();
        if (cancelled.revision !== before.revision)
          throw new Error("Cancel replaced data");
        stage = "confirm native restore and restart";
        await native.core.invoke("collaboration_e2e_recovery_picker", {
          restore: true,
        });
        await click("Choose backup to restore");
        await click("Replace collaboration data");
        await wait(() => text("Recovery finished."));
        const after = await accounts();
        if (
          after.accounts.length !== 0 ||
          BigInt(after.revision) <= BigInt(before.revision) ||
          BigInt(after.authorization_view) <= BigInt(before.authorization_view)
        )
          throw new Error("Restore did not fence the old generation");
        const cli = (await native.core.invoke(
          "collaboration_discover_github_cli",
        )) as { status: string; accounts: unknown[] };
        if (cli.status !== "not_installed" || cli.accounts.length !== 0)
          throw new Error("Recovery changed isolated CLI policy");
        done({
          passed: true,
          before: before.revision,
          after: after.revision,
          cancelled: cancelled.revision,
        });
      } catch {
        done({ passed: false, stage });
      }
    })();
  });
  if (
    !result ||
    typeof result !== "object" ||
    !("passed" in result) ||
    result.passed !== true
  )
    throw new Error(
      `Packaged recovery failed at ${result && typeof result === "object" && "stage" in result ? result.stage : "unknown stage"}`,
    );
  await browser.setTimeout({ script: 30_000 });
});
