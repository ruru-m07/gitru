import { execFileSync } from "node:child_process";
import { appendFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  CdpClient,
  type CdpTarget,
  classifyGitruTargets,
  type FrontendDiagnostic,
  listCdpTargets,
  waitFor,
} from "../cdp";
import { isolatedGitEnvironment } from "../git-environment";

type SmokeOptions = {
  artifactsDirectory: string;
  cdpPort: number;
  fixtureRepository: string;
  fixtureRemote: string;
  fixtureRoot: string;
  host: CdpClient;
};

const openCommandDialog =
  '[data-slot="command-dialog-popup"][data-open]:not([data-closed])';
const openDialog = '[data-slot="dialog-popup"][data-open]:not([data-closed])';
const commitSubject = "e2e: stage and commit";
const smokeBranch = "e2e-smoke-branch";

const delay = (milliseconds: number) =>
  new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));

const xpathLiteral = (value: string): string => {
  if (!value.includes('"')) return `"${value}"`;
  if (!value.includes("'")) return `'${value}'`;
  return `concat(${value
    .split('"')
    .map((part) => `"${part}"`)
    .join(", '\"', ")})`;
};

export async function runCefSmoke(options: SmokeOptions): Promise<{
  diagnostics: FrontendDiagnostic[];
}> {
  const {
    artifactsDirectory,
    cdpPort,
    fixtureRemote,
    fixtureRepository,
    fixtureRoot,
    host,
  } = options;
  const gitEnvironment = isolatedGitEnvironment(
    resolve(fixtureRoot, "global.gitconfig"),
    { GIT_OPTIONAL_LOCKS: "0" },
  );
  const connectedClients = [host];

  const frontendFailures = (diagnostics: FrontendDiagnostic[]) =>
    diagnostics.filter(
      (diagnostic) =>
        diagnostic.level === "error" || diagnostic.level === "exception",
    );

  const assertNoFrontendFailures = (
    diagnostics: FrontendDiagnostic[],
    context: string,
  ): void => {
    const failures = frontendFailures(diagnostics);
    if (failures.length === 0) return;
    throw new Error(
      `CEF reported frontend errors or uncaught exceptions ${context}:\n${failures
        .map((entry) => `${entry.source}: ${entry.message}`)
        .join("\n")}`,
    );
  };

  const git = (...args: string[]): string =>
    execFileSync("git", args, {
      cwd: fixtureRepository,
      encoding: "utf8",
      env: gitEnvironment,
    }).trim();

  const remoteGit = (...args: string[]): string =>
    execFileSync("git", ["--git-dir", fixtureRemote, ...args], {
      encoding: "utf8",
      env: gitEnvironment,
    }).trim();

  const assertEqual = (
    actual: string,
    expected: string,
    context: string,
  ): void => {
    if (actual !== expected) {
      throw new Error(
        `${context}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
      );
    }
  };

  const waitForGit = async (
    read: () => string,
    predicate: (value: string) => boolean,
    description: string,
  ): Promise<string> =>
    waitFor(
      async () => {
        try {
          return read();
        } catch (error) {
          return error instanceof Error ? error.message : String(error);
        }
      },
      predicate,
      description,
      { intervalMs: 250 },
    );

  const combinedSelector = (root: string, selector: string) =>
    `${root} ${selector}`;

  const portalHasElement = (
    page: CdpClient,
    popupSelector: string,
    elementSelector: string,
  ): Promise<boolean> => page.hasCssWithin(popupSelector, elementSelector);

  const waitForPortalElement = async (
    page: CdpClient,
    popupSelector: string,
    elementSelector: string,
  ): Promise<void> => {
    await waitFor(
      () => portalHasElement(page, popupSelector, elementSelector),
      Boolean,
      `${elementSelector} within ${popupSelector}`,
    );
    await page.waitForVisibleCss(
      combinedSelector(popupSelector, elementSelector),
    );
  };

  const setPortalInput = async (
    page: CdpClient,
    popupSelector: string,
    inputSelector: string,
    value: string,
  ): Promise<void> => {
    const selector = combinedSelector(popupSelector, inputSelector);
    await waitForPortalElement(page, popupSelector, inputSelector);
    await page.setInputCss(selector, value);
    await waitFor(
      () =>
        page.evaluate<string | null>(`(() => {
          const input = document.querySelector(${JSON.stringify(selector)});
          return input instanceof HTMLInputElement ? input.value : null;
        })()`),
      (actual) => actual === value,
      `${inputSelector} to retain ${JSON.stringify(value)}`,
      { timeoutMs: 10_000 },
    );
  };

  const clickPortalText = async (
    page: CdpClient,
    popupSelector: string,
    xpathElement: string,
    text: string,
    exact = false,
  ): Promise<void> => {
    const literal = xpathLiteral(text);
    const predicate = exact
      ? `normalize-space(.)=${literal}`
      : `contains(normalize-space(.), ${literal})`;
    const xpath = `.//${xpathElement}[${predicate}]`;
    await waitFor(
      () =>
        page.evaluate<boolean>(`(() => {
          const root = document.querySelector(${JSON.stringify(popupSelector)});
          if (!root) return false;
          const element = document.evaluate(${JSON.stringify(xpath)}, root, null, XPathResult.FIRST_ORDERED_NODE_TYPE).singleNodeValue;
          return element instanceof HTMLElement && element.getClientRects().length > 0;
        })()`),
      Boolean,
      `${text} within ${popupSelector}`,
    );
    await page.clickXpathWithin(popupSelector, xpath);
  };

  const waitForPortalText = async (
    page: CdpClient,
    popupSelector: string,
    xpathElement: string,
    text: string,
  ): Promise<void> => {
    const xpath = `.//${xpathElement}[normalize-space(.)=${xpathLiteral(text)}]`;
    await waitFor(
      () =>
        page.evaluate<boolean>(`(() => {
          const root = document.querySelector(${JSON.stringify(popupSelector)});
          if (!root) return false;
          const element = document.evaluate(${JSON.stringify(xpath)}, root, null, XPathResult.FIRST_ORDERED_NODE_TYPE).singleNodeValue;
          return element instanceof HTMLElement && element.getClientRects().length > 0;
        })()`),
      Boolean,
      `${text} within ${popupSelector}`,
    );
  };

  const openBranchSwitcher = async (page: CdpClient): Promise<void> => {
    const inputSelector = 'input[placeholder="Search branches..."]';
    if (await portalHasElement(page, openCommandDialog, inputSelector)) return;

    await page.waitForVisibleCss('button[aria-label="Switch branch"]');
    await page.clickCss('button[aria-label="Switch branch"]');
    await waitForPortalElement(page, openCommandDialog, inputSelector);
  };

  const openRootAction = async (
    page: CdpClient,
    label: string,
  ): Promise<void> => {
    await openBranchSwitcher(page);
    const remoteImageSources = await page.evaluate<string[]>(`Array.from(
      document.querySelector(${JSON.stringify(openCommandDialog)})?.querySelectorAll('img[src^="https://"]') ?? [],
      (image) => image.src,
    )`);
    if (remoteImageSources.length > 0) {
      throw new Error(
        `Branch list requested remote images: ${remoteImageSources.join(", ")}`,
      );
    }

    const rootInput = 'input[placeholder="Search actions or branches..."]';
    for (let depth = 0; depth < 6; depth += 1) {
      if (await portalHasElement(page, openCommandDialog, rootInput)) break;
      await page.pressKey("Escape");
      await delay(100);
    }
    await setPortalInput(page, openCommandDialog, rootInput, label);
    await clickPortalText(
      page,
      openCommandDialog,
      '*[@data-slot="command-item"]',
      label,
    );
  };

  const capture = (page: CdpClient, name: string): Promise<void> =>
    page.captureScreenshot(resolve(artifactsDirectory, `${name}.png`));

  try {
    await host.waitForVisibleCss('button[aria-label="New tab"]');
    const targetsWithFirstTab = await waitFor(
      () => listCdpTargets(cdpPort),
      (targets) => classifyGitruTargets(targets).children.length === 1,
      "CEF to expose the automatically created child-webview target",
    );
    writeFileSync(
      resolve(artifactsDirectory, "targets-after-first-tab.json"),
      `${JSON.stringify(targetsWithFirstTab, null, 2)}\n`,
    );
    const initialChildTarget =
      classifyGitruTargets(targetsWithFirstTab).children[0];
    if (!initialChildTarget) {
      throw new Error("Could not identify the first child-webview CDP target");
    }
    const initialChild = await CdpClient.connect(initialChildTarget);
    connectedClients.push(initialChild);
    await initialChild.waitForVisibleCss("body");

    await initialChild.evaluate<void>(`(() => {
      window.__GITRU_E2E_DIRECTORY_PICKER_CALLS__ = 0;
      window.__GITRU_E2E_REPOSITORY_PATH__ = ${JSON.stringify(fixtureRepository)};
    })()`);
    const importRepositoryXpath = `//button[contains(normalize-space(.), ${xpathLiteral("Import Local Repository")})]`;
    await initialChild.waitForVisibleXpath(importRepositoryXpath);
    await initialChild.clickXpath(importRepositoryXpath);
    await initialChild.waitForVisibleCss(
      'button[aria-label="Stage stage-me.txt"]',
    );
    const directoryPickerCalls = await initialChild.evaluate<number>(
      "window.__GITRU_E2E_DIRECTORY_PICKER_CALLS__ ?? 0",
    );
    assertEqual(
      String(directoryPickerCalls),
      "1",
      "directory picker seam call count",
    );
    await capture(initialChild, "01-cef-imported-repository");

    await initialChild.clickCss('button[aria-label="Stage stage-me.txt"]');
    await waitForGit(
      () => git("diff", "--cached", "--name-only"),
      (value) => value === "stage-me.txt",
      "stage-me.txt to be staged",
    );
    await capture(initialChild, "02-cef-staged-change");

    const summarySelector = 'input[placeholder="Summary (required)"]';
    await initialChild.setInputCss(summarySelector, commitSubject);
    const commitButtonXpath = `//button[contains(normalize-space(.), ${xpathLiteral("Commit to")})]`;
    await initialChild.waitForVisibleXpath(commitButtonXpath);
    await initialChild.clickXpath(commitButtonXpath);
    await waitForGit(
      () => git("log", "-1", "--format=%s"),
      (value) => value === commitSubject,
      "the UI-created commit",
    );
    assertEqual(
      git("diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"),
      "stage-me.txt",
      "files recorded by the UI-created commit",
    );
    await waitForGit(
      () => git("status", "--porcelain"),
      (value) => value === "",
      "a clean index and worktree after commit",
    );
    await initialChild.waitForVisibleCss('button[aria-label="Push to Origin"]');
    await capture(initialChild, "03-cef-commit-and-sync-decision");

    const originalChildId = initialChild.target.id;
    const initialChildCount =
      classifyGitruTargets(targetsWithFirstTab).children.length;
    await host.clickCss('button[aria-label="New tab"]');
    const targetsWithNewTab = await waitFor(
      () => listCdpTargets(cdpPort),
      (targets) =>
        classifyGitruTargets(targets).children.length === initialChildCount + 1,
      "CEF to expose a CDP target for the new child webview",
    );
    writeFileSync(
      resolve(artifactsDirectory, "targets-after-new-tab.json"),
      `${JSON.stringify(targetsWithNewTab, null, 2)}\n`,
    );
    const newChildTarget = classifyGitruTargets(
      targetsWithNewTab,
    ).children.find((target) => target.id !== originalChildId);
    if (!newChildTarget) {
      throw new Error("Could not identify the new child-webview CDP target");
    }
    const newChild = await CdpClient.connect(newChildTarget);
    connectedClients.push(newChild);
    await newChild.waitForVisibleCss("body");
    await capture(newChild, "04-cef-second-child-webview");
    assertNoFrontendFailures(
      newChild.diagnostics,
      "before closing the temporary child target",
    );

    await host.waitForVisibleCss(
      'button[data-tab-close-button="true"][data-active="true"]',
    );
    await host.clickCss(
      'button[data-tab-close-button="true"][data-active="true"]',
    );
    const targetsAfterClose = await waitFor(
      () => listCdpTargets(cdpPort),
      (targets) =>
        !targets.some((target) => target.id === newChildTarget.id) &&
        classifyGitruTargets(targets).children.length === initialChildCount,
      "CEF to destroy the closed tab's child-webview target",
    );
    writeFileSync(
      resolve(artifactsDirectory, "targets-after-tab-close.json"),
      `${JSON.stringify(targetsAfterClose, null, 2)}\n`,
    );
    writeFileSync(
      resolve(artifactsDirectory, "closed-target-diagnostics.json"),
      `${JSON.stringify(newChild.diagnostics, null, 2)}\n`,
    );
    await newChild.close();
    connectedClients.splice(connectedClients.indexOf(newChild), 1);
    await initialChild.waitForVisibleCss('button[aria-label="Switch branch"]');
    await capture(host, "05-cef-child-lifecycle-host");

    await openRootAction(initialChild, "New Branch");
    await setPortalInput(
      initialChild,
      openCommandDialog,
      'input[placeholder="Enter branch name..."]',
      smokeBranch,
    );
    await clickPortalText(
      initialChild,
      openCommandDialog,
      "button",
      "Create & Checkout",
    );
    await waitForGit(
      () => git("branch", "--show-current"),
      (value) => value === smokeBranch,
      `branch ${smokeBranch}`,
    );
    await initialChild.waitForVisibleCss('button[aria-label="Publish Branch"]');
    await capture(initialChild, "06-cef-created-unpublished-branch");
    await initialChild.clickCss('button[aria-label="Publish Branch"]');
    const publishedHead = git("rev-parse", "HEAD");
    await waitForGit(
      () =>
        remoteGit(
          "for-each-ref",
          "--format=%(objectname)",
          `refs/heads/${smokeBranch}`,
        ),
      (value) => value === publishedHead,
      `published remote branch ${smokeBranch} to match local HEAD`,
    );
    await initialChild.waitForAbsentCss('button[aria-label="Publish Branch"]');
    await initialChild.waitForVisibleCss('button[aria-label="Fetch"]');
    await capture(initialChild, "07-cef-published-branch");

    appendFileSync(
      resolve(fixtureRepository, "stash-note.txt"),
      "dirty change for branch switch\n",
    );
    await delay(750);
    await initialChild.clickCss(
      'button[aria-label="Refresh repository state"]',
    );
    await initialChild.waitForVisibleCss(
      'button[aria-label="Stage stash-note.txt"]',
    );

    await openBranchSwitcher(initialChild);
    await setPortalInput(
      initialChild,
      openCommandDialog,
      'input[placeholder="Search branches..."]',
      "conflict-work",
    );
    await clickPortalText(
      initialChild,
      openCommandDialog,
      '*[@data-slot="command-item"]',
      "conflict-work",
    );
    await clickPortalText(
      initialChild,
      openCommandDialog,
      "button",
      "Stash & Checkout",
    );
    await waitForGit(
      () => git("branch", "--show-current"),
      (value) => value === "conflict-work",
      "conflict-work checkout",
    );
    const stashList = git("stash", "list");
    if (!stashList.includes(`!!Gitru<${smokeBranch}> -> <conflict-work>`)) {
      throw new Error(`Expected Gitru branch stash, got: ${stashList}`);
    }
    if (
      !git("stash", "show", "--name-only", "stash@{0}").includes(
        "stash-note.txt",
      )
    ) {
      throw new Error(
        "The branch-switch stash does not contain stash-note.txt",
      );
    }
    await capture(initialChild, "08-cef-stashed-and-switched");

    await openRootAction(initialChild, "Rebase Onto");
    await setPortalInput(
      initialChild,
      openCommandDialog,
      'input[placeholder="Rebase onto (branch, tag, or commit)…"]',
      "conflict-base",
    );
    await clickPortalText(
      initialChild,
      openCommandDialog,
      "button",
      "Rebase",
      true,
    );
    await waitForGit(
      () => git("ls-files", "--unmerged"),
      (value) => value.includes("\tconflict.txt"),
      "the expected rebase conflict",
    );
    await initialChild.waitForVisibleXpath(
      `//button[normalize-space(.)=${xpathLiteral("Abort")}]`,
    );
    await initialChild.waitForVisibleXpath(
      `//*[normalize-space(.)=${xpathLiteral("Conflicted")}]`,
    );
    await initialChild.clickXpath(
      `//*[normalize-space(.)=${xpathLiteral("Conflicted")}]`,
    );
    await delay(250);
    await initialChild.waitForVisibleXpath(
      `//*[contains(normalize-space(.), ${xpathLiteral("conflict.txt")})]`,
    );
    await capture(initialChild, "09-cef-rebase-conflict");

    await initialChild.clickXpath(
      `//button[normalize-space(.)=${xpathLiteral("Abort")}]`,
    );
    await waitForPortalText(
      initialChild,
      openDialog,
      '*[@data-slot="dialog-title"]',
      "Abort rebase?",
    );
    await clickPortalText(
      initialChild,
      openDialog,
      "button",
      "Abort rebase",
      true,
    );
    await waitForGit(
      () => git("branch", "--show-current"),
      (value) => value === "conflict-work",
      "rebase abort to restore conflict-work",
    );
    await waitForGit(
      () => git("status", "--porcelain"),
      (value) => value === "",
      "a clean worktree after abort",
    );
    const targetsAfterRebaseAbort = await listCdpTargets(cdpPort);
    writeFileSync(
      resolve(artifactsDirectory, "targets-after-rebase-abort.json"),
      `${JSON.stringify(targetsAfterRebaseAbort, null, 2)}\n`,
    );
    await waitFor(
      () => initialChild.textCss('button[aria-label="Switch branch"]'),
      (text) =>
        Boolean(
          text?.includes("Current Branch") && text.includes("conflict-work"),
        ),
      "the UI to leave detached rebase state",
    );
    await waitFor(
      () =>
        initialChild.isVisibleXpath(
          `//*[normalize-space(.)=${xpathLiteral("Conflicted")}]`,
        ),
      (visible) => !visible,
      "the conflict indicator to disappear",
    );
    await capture(initialChild, "10-cef-rebase-aborted");

    const diagnostics = connectedClients.flatMap(
      (client) => client.diagnostics,
    );
    assertNoFrontendFailures(diagnostics, "in a live target");
    return { diagnostics };
  } catch (error) {
    try {
      const failureTargets = await listCdpTargets(cdpPort);
      writeFileSync(
        resolve(artifactsDirectory, "targets-on-failure.json"),
        `${JSON.stringify(failureTargets, null, 2)}\n`,
      );
    } catch (discoveryError) {
      writeFileSync(
        resolve(artifactsDirectory, "targets-on-failure-error.txt"),
        `${
          discoveryError instanceof Error
            ? (discoveryError.stack ?? discoveryError.message)
            : String(discoveryError)
        }\n`,
      );
    }
    writeFileSync(
      resolve(artifactsDirectory, "frontend-diagnostics-failure.json"),
      `${JSON.stringify(
        connectedClients.flatMap((client) => client.diagnostics),
        null,
        2,
      )}\n`,
    );
    await Promise.allSettled(
      connectedClients.map(async (client, index) => {
        await client.captureScreenshot(
          resolve(artifactsDirectory, `failure-${index + 1}.png`),
        );
        const source = await client.evaluate<string>(
          "document.documentElement.outerHTML",
        );
        writeFileSync(
          resolve(artifactsDirectory, `failure-${index + 1}.html`),
          source,
        );
      }),
    );
    throw error;
  } finally {
    await Promise.allSettled(connectedClients.map((client) => client.close()));
  }
}
