import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { resolve } from "node:path";
import { browser } from "@wdio/globals";
import { isolatedGitEnvironment } from "../git-environment";

const fixtureRepository = requiredEnvironment("GITRU_E2E_REPO");
const artifactsDirectory = requiredEnvironment("GITRU_E2E_ARTIFACTS");
const fixtureRoot = requiredEnvironment("GITRU_E2E_TEMP_ROOT");
const fixtureRemote = resolve(fixtureRoot, "gitru-e2e-remote.git");
const gitEnvironment = isolatedGitEnvironment(
  resolve(fixtureRoot, "global.gitconfig"),
  { GIT_OPTIONAL_LOCKS: "0" },
);
const commitSubject = "e2e: stage and commit";
const smokeBranch = "e2e-smoke-branch";
const openCommandDialog =
  '[data-slot="command-dialog-popup"][data-open]:not([data-closed])';
const openDialog = '[data-slot="dialog-popup"][data-open]:not([data-closed])';

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function git(...args: string[]): string {
  return execFileSync("git", args, {
    cwd: fixtureRepository,
    encoding: "utf8",
    env: gitEnvironment,
  }).trim();
}

function remoteGit(...args: string[]): string {
  return execFileSync("git", ["--git-dir", fixtureRemote, ...args], {
    encoding: "utf8",
    env: gitEnvironment,
  }).trim();
}

function assertEqual(actual: string, expected: string, context: string): void {
  if (actual !== expected) {
    throw new Error(
      `${context}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
    );
  }
}

async function waitForGit(
  read: () => string,
  predicate: (value: string) => boolean,
  description: string,
): Promise<string> {
  let latest = "";
  await browser.waitUntil(
    async () => {
      try {
        latest = read();
        return predicate(latest);
      } catch (error) {
        latest = error instanceof Error ? error.message : String(error);
        return false;
      }
    },
    {
      timeout: 30_000,
      interval: 250,
      timeoutMsg: `Timed out waiting for ${description}; latest Git output: ${latest}`,
    },
  );
  return latest;
}

async function visible(selector: string, timeout = 30_000) {
  await browser.waitUntil(
    async () => (await browser.$(selector)).isDisplayed().catch(() => false),
    {
      timeout,
      interval: 200,
      timeoutMsg: `${selector} was not displayed within ${timeout}ms`,
    },
  );
  return browser.$(selector);
}

async function absent(selector: string, timeout = 30_000): Promise<void> {
  await browser.waitUntil(
    () =>
      browser.execute(
        (candidate) => document.querySelector(candidate) === null,
        selector,
      ),
    {
      timeout,
      interval: 200,
      timeoutMsg: `${selector} was still present after ${timeout}ms`,
    },
  );
}

async function buttonWithText(text: string) {
  return visible(
    `//button[contains(normalize-space(.), ${JSON.stringify(text)})]`,
  );
}

async function portalHasElement(
  popupSelector: string,
  elementSelector: string,
): Promise<boolean> {
  const popup = await browser.$(popupSelector);
  if (!(await popup.isExisting())) return false;
  return (await popup.$(elementSelector)).isExisting();
}

async function waitForPortalElement(
  popupSelector: string,
  elementSelector: string,
  timeout = 30_000,
) {
  await browser.waitUntil(
    () => portalHasElement(popupSelector, elementSelector),
    {
      timeout,
      interval: 100,
      timeoutMsg: `${elementSelector} was not found in the open portal ${popupSelector} within ${timeout}ms`,
    },
  );
  const element = await (await browser.$(popupSelector)).$(elementSelector);
  await element.waitForDisplayed({ timeout });
  return element;
}

async function setPortalInput(
  popupSelector: string,
  inputSelector: string,
  value: string,
): Promise<void> {
  const input = await waitForPortalElement(popupSelector, inputSelector);
  await input.setValue(value);
  await browser.waitUntil(async () => (await input.getValue()) === value, {
    timeout: 10_000,
    interval: 100,
    timeoutMsg: `${inputSelector} did not retain ${JSON.stringify(value)}`,
  });
}

async function clickPortalText(
  popupSelector: string,
  xpathElement: string,
  text: string,
  exact = false,
): Promise<void> {
  const predicate = exact
    ? `normalize-space(.)=${JSON.stringify(text)}`
    : `contains(normalize-space(.), ${JSON.stringify(text)})`;
  const selector = `.//${xpathElement}[${predicate}]`;
  const element = await waitForPortalElement(popupSelector, selector);
  await element.click();
}

async function waitForPortalText(
  popupSelector: string,
  xpathElement: string,
  text: string,
): Promise<void> {
  await waitForPortalElement(
    popupSelector,
    `.//${xpathElement}[normalize-space(.)=${JSON.stringify(text)}]`,
  );
}

async function openBranchSwitcher(): Promise<void> {
  const inputSelector = 'input[placeholder="Search branches..."]';

  if (await portalHasElement(openCommandDialog, inputSelector)) return;

  const opener = await visible('button[aria-label="Switch branch"]');
  await opener.click();
  await waitForPortalElement(openCommandDialog, inputSelector);
}

async function openRootAction(label: string): Promise<void> {
  await openBranchSwitcher();
  const remoteImageSources = await browser.execute(
    (popupSelector) =>
      Array.from(
        document
          .querySelector(popupSelector)
          ?.querySelectorAll<HTMLImageElement>('img[src^="https://"]') ?? [],
        (image) => image.src,
      ),
    openCommandDialog,
  );
  if (remoteImageSources.length > 0) {
    throw new Error(
      `Branch list requested remote images: ${remoteImageSources.join(", ")}`,
    );
  }
  const inputSelector = 'input[placeholder="Search actions or branches..."]';
  for (let depth = 0; depth < 6; depth += 1) {
    if (await portalHasElement(openCommandDialog, inputSelector)) break;
    await browser.keys("Escape");
    await browser.pause(100);
  }
  await setPortalInput(openCommandDialog, inputSelector, label);
  await clickPortalText(
    openCommandDialog,
    '*[@data-slot="command-item"]',
    label,
  );
}

async function capture(name: string): Promise<void> {
  await browser.saveScreenshot(resolve(artifactsDirectory, `${name}.png`));
}

describe("packaged Gitru desktop smoke", () => {
  it("crosses UI, Tauri, Rust, and Git for critical repository flows", async () => {
    await visible("body");

    await browser.execute((repositoryPath) => {
      window.__GITRU_E2E_DIRECTORY_PICKER_CALLS__ = 0;
      window.__GITRU_E2E_REPOSITORY_PATH__ = repositoryPath;
    }, fixtureRepository);
    await (await buttonWithText("Import Local Repository")).click();
    const stageButton = await visible(
      'button[aria-label="Stage stage-me.txt"]',
    );
    const directoryPickerCalls = await browser.execute(
      () => window.__GITRU_E2E_DIRECTORY_PICKER_CALLS__ ?? 0,
    );
    assertEqual(
      String(directoryPickerCalls),
      "1",
      "directory picker seam call count",
    );
    await capture("01-imported-repository");
    await stageButton.click();
    await waitForGit(
      () => git("diff", "--cached", "--name-only"),
      (value) => value === "stage-me.txt",
      "stage-me.txt to be staged",
    );
    await capture("02-staged-change");

    const summary = await visible('input[placeholder="Summary (required)"]');
    await summary.setValue(commitSubject);
    const commitButton = await buttonWithText("Commit to");
    await commitButton.waitForClickable({ timeout: 20_000 });
    await commitButton.click();
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

    await visible('button[aria-label="Push to Origin"]');
    await capture("03-commit-and-sync-decision");

    await openRootAction("New Branch");
    await setPortalInput(
      openCommandDialog,
      'input[placeholder="Enter branch name..."]',
      smokeBranch,
    );
    await clickPortalText(openCommandDialog, "button", "Create & Checkout");
    await waitForGit(
      () => git("branch", "--show-current"),
      (value) => value === smokeBranch,
      `branch ${smokeBranch}`,
    );
    await visible('button[aria-label="Publish Branch"]');
    await capture("04-created-unpublished-branch");
    await (await visible('button[aria-label="Publish Branch"]')).click();
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
    await absent('button[aria-label="Publish Branch"]');
    await visible('button[aria-label="Fetch"]');
    await capture("05-published-branch");

    appendFileSync(
      resolve(fixtureRepository, "stash-note.txt"),
      "dirty change for branch switch\n",
    );
    // The Rust status cache has a 500ms TTL; wait it out before requesting a refresh.
    await browser.pause(750);
    await (
      await visible('button[aria-label="Refresh repository state"]')
    ).click();
    await visible('button[aria-label="Stage stash-note.txt"]');

    await openBranchSwitcher();
    await setPortalInput(
      openCommandDialog,
      'input[placeholder="Search branches..."]',
      "conflict-work",
    );
    await clickPortalText(
      openCommandDialog,
      '*[@data-slot="command-item"]',
      "conflict-work",
    );
    await clickPortalText(openCommandDialog, "button", "Stash & Checkout");
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
    await capture("06-stashed-and-switched");

    await openRootAction("Rebase Onto");
    await setPortalInput(
      openCommandDialog,
      'input[placeholder="Rebase onto (branch, tag, or commit)…"]',
      "conflict-base",
    );
    await clickPortalText(openCommandDialog, "button", "Rebase", true);
    await waitForGit(
      () => git("ls-files", "--unmerged"),
      (value) => value.includes("\tconflict.txt"),
      "the expected rebase conflict",
    );
    await visible("//button[normalize-space(.)='Abort']");
    await (await visible("//*[normalize-space(.)='Conflicted']")).click();
    await browser.pause(250);
    await visible("//*[contains(normalize-space(.), 'conflict.txt')]");
    await capture("07-rebase-conflict");

    await (await visible("//button[normalize-space(.)='Abort']")).click();
    await waitForPortalText(
      openDialog,
      '*[@data-slot="dialog-title"]',
      "Abort rebase?",
    );
    await clickPortalText(openDialog, "button", "Abort rebase", true);
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
    const branchSwitcher = await visible('button[aria-label="Switch branch"]');
    await browser.waitUntil(
      async () => {
        const text = await branchSwitcher.getText();
        return (
          text.includes("Current Branch") && text.includes("conflict-work")
        );
      },
      {
        timeout: 30_000,
        interval: 250,
        timeoutMsg:
          "The UI did not leave detached rebase state after the repository abort completed",
      },
    );
    await (
      await browser.$("//*[normalize-space(.)='Conflicted']")
    ).waitForDisplayed({
      reverse: true,
      timeout: 30_000,
    });
    await capture("08-rebase-aborted");
  });
});
