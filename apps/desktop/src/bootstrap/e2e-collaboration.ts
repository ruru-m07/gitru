/**
 * The embedded WDIO driver addresses WebviewWindows, while workspace tabs are
 * native child Webviews. These fixed E2E actions exercise their real DOM/event
 * wiring without exposing arbitrary evaluation or reading credential values.
 * main.tsx imports this module only in the isolated E2E build.
 */
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { router } from "./create-router";

const REQUEST_EVENT = "gitru:e2e-collaboration-request";
const RESULT_EVENT = "gitru:e2e-collaboration-result";
const view = getCurrentWebview();
const instanceNonce = crypto.randomUUID();

type Request = {
  label: string;
  action:
    | "inspect"
    | "click-inbox"
    | "click-accounts"
    | "open-host"
    | "restore-embedded";
};

function accountsButton() {
  return (
    Array.from(document.querySelectorAll<HTMLButtonElement>("button")).find(
      (button) => button.textContent?.trim() === "Accounts",
    ) ??
    document.querySelector<HTMLButtonElement>(
      'button[aria-label="Accounts"]',
    ) ??
    document.querySelector<HTMLButtonElement>(
      'button[aria-label="Connected accounts"], button[aria-label="Manage connected accounts"]',
    )
  );
}

const stop = await view.listen<Request>(REQUEST_EVENT, async (event) => {
  const request = event.payload;
  if (
    !request ||
    request.label !== view.label ||
    ![
      "inspect",
      "click-inbox",
      "click-accounts",
      "open-host",
      "restore-embedded",
    ].includes(request.action)
  ) {
    return;
  }

  if (request.action === "open-host" || request.action === "restore-embedded") {
    // Fixed routes preserve the pending WDIO script's main document. Child
    // hooks cannot navigate the host or perform either main-only action.
    if (view.label !== "main") return;
    if (request.action === "open-host") {
      await router.navigate({ to: "/app", search: {}, replace: true });
    } else {
      await router.navigate({
        to: "/app/git",
        search: { embedded: 1 },
        replace: true,
      });
    }
  } else if (request.action === "click-inbox") {
    document
      .querySelector<HTMLButtonElement>('button[aria-label="Inbox"]')
      ?.click();
  } else if (request.action === "click-accounts") {
    accountsButton()?.click();
  }

  void view.emitTo({ kind: "Webview", label: "main" }, RESULT_EVENT, {
    label: view.label,
    route: `${window.location.pathname}${window.location.search}${window.location.hash}`,
    instanceNonce,
    hasAccountsButton: Boolean(accountsButton()),
    hasPatForm: document.querySelector('input[name="github-token"]') !== null,
  });
});

if (import.meta.hot) import.meta.hot.dispose(stop);
