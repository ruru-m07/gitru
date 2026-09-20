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
const remoteFixtureBranchCount = 2_000;
const openCommandDialog =
  '[data-slot="command-dialog-popup"][data-open]:not([data-closed])';
const openDialog = '[data-slot="dialog-popup"][data-open]:not([data-closed])';
const openTooltip = '[data-slot="tooltip-popup"][data-open]:not([data-closed])';
const openBranchPanel =
  "[data-current-branch-panel][data-open]:not([data-closed])";
const openBranchContextMenu = '[data-branch-context-menu][data-state="open"]';

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
  const inputSelector = 'input[placeholder="Filter branches…"]';

  if (await portalHasElement(openBranchPanel, inputSelector)) return;

  const opener = await visible('button[aria-label^="Current branch:"]');
  await opener.click();
  await waitForPortalElement(openBranchPanel, inputSelector);
}

async function assertBranchPanelLayout(): Promise<void> {
  await browser.waitUntil(
    () =>
      browser.execute(
        (triggerSelector, panelSelector) => {
          const trigger = document.querySelector<HTMLElement>(triggerSelector);
          const panel = document.querySelector<HTMLElement>(panelSelector);
          const scrollViewport = panel?.querySelector<HTMLElement>(
            '[data-current-branch-scroll] [data-slot="scroll-area-viewport"]',
          );
          const footer = panel?.querySelector("[data-current-branch-footer]");

          if (!trigger || !panel || !scrollViewport) {
            return false;
          }

          const triggerRect = trigger.getBoundingClientRect();
          const panelRect = panel.getBoundingClientRect();
          const scrollRect = scrollViewport.getBoundingClientRect();

          return (
            Math.abs(panelRect.left - triggerRect.left) <= 2 &&
            Math.abs(panelRect.top - triggerRect.bottom) <= 2 &&
            scrollViewport.scrollHeight > scrollViewport.clientHeight &&
            Math.abs(scrollRect.bottom - panelRect.bottom) <= 2 &&
            footer === null
          );
        },
        "[data-current-branch-trigger]",
        openBranchPanel,
      ),
    {
      timeout: 20_000,
      interval: 100,
      timeoutMsg:
        "Branch panel did not settle below its trigger with a scrollable long list",
    },
  );

  const geometry = await browser.execute(
    (triggerSelector, panelSelector) => {
      const trigger = document.querySelector<HTMLElement>(triggerSelector);
      const panel = document.querySelector<HTMLElement>(panelSelector);
      const statusBar =
        document.querySelector<HTMLElement>("[data-status-bar]");
      const scrollViewport = panel?.querySelector<HTMLElement>(
        '[data-current-branch-scroll] [data-slot="scroll-area-viewport"]',
      );

      if (!trigger || !panel || !statusBar || !scrollViewport) return null;

      const triggerRect = trigger.getBoundingClientRect();
      const panelRect = panel.getBoundingClientRect();
      const statusBarRect = statusBar.getBoundingClientRect();
      const scrollRect = scrollViewport.getBoundingClientRect();
      const panelStyle = getComputedStyle(panel);
      const shadowColors = panelStyle.boxShadow.match(/rgba?\([^)]+\)/g) ?? [];
      return {
        hasFooter: panel.querySelector("[data-current-branch-footer]") !== null,
        innerWidth: window.innerWidth,
        panelBottom: panelRect.bottom,
        panelLeft: panelRect.left,
        panelRight: panelRect.right,
        panelTop: panelRect.top,
        panelWidth: panelRect.width,
        borderRadius: panelStyle.borderRadius,
        boxShadow: panelStyle.boxShadow,
        hasVisibleBoxShadow:
          panelStyle.boxShadow !== "none" &&
          shadowColors.some((color) => !color.endsWith(", 0)")),
        scrollBottom: scrollRect.bottom,
        scrollClientHeight: scrollViewport.clientHeight,
        scrollHeight: scrollViewport.scrollHeight,
        statusBarTop: statusBarRect.top,
        triggerBottom: triggerRect.bottom,
        triggerLeft: triggerRect.left,
        triggerRight: triggerRect.right,
      };
    },
    "[data-current-branch-trigger]",
    openBranchPanel,
  );

  if (!geometry) {
    throw new Error("Branch panel geometry could not be measured");
  }

  const tolerance = 2;
  const expectedBottom = geometry.statusBarTop;
  const failures: string[] = [];

  if (Math.abs(geometry.panelLeft - geometry.triggerLeft) > tolerance) {
    failures.push("panel is not left-aligned to the Current Branch trigger");
  }
  if (Math.abs(geometry.panelTop - geometry.triggerBottom) > tolerance) {
    failures.push(
      "panel does not start directly below the Current Branch trigger",
    );
  }
  if (Math.abs(geometry.panelBottom - expectedBottom) > tolerance) {
    failures.push("panel does not stop at the top of the bottom status bar");
  }
  if (Math.abs(geometry.panelWidth - 365) > tolerance) {
    failures.push("panel width is not 365px");
  }
  if (geometry.panelRight > geometry.innerWidth + tolerance) {
    failures.push("panel overflows the viewport horizontally");
  }
  if (geometry.hasFooter) {
    failures.push("removed branch panel footer is still rendered");
  }
  if (Math.abs(geometry.scrollBottom - geometry.panelBottom) > tolerance) {
    failures.push("branch list does not fill the panel to the bottom edge");
  }
  if (geometry.scrollHeight <= geometry.scrollClientHeight) {
    failures.push("long branch fixture does not scroll inside the panel");
  }
  if (geometry.borderRadius !== "0px") {
    failures.push("panel still has floating-card rounded corners");
  }
  if (geometry.hasVisibleBoxShadow) {
    failures.push("panel still has a floating-card shadow");
  }

  if (failures.length > 0) {
    throw new Error(
      `Branch panel layout failed: ${failures.join("; ")}\n${JSON.stringify(geometry, null, 2)}`,
    );
  }
}

async function assertLocalBranchGrouping(
  currentBranchName: string,
): Promise<void> {
  await waitForPortalText(
    openBranchPanel,
    '*[@role="heading"]',
    "Default Branch",
  );
  await waitForPortalText(
    openBranchPanel,
    '*[@role="heading"]',
    "Other Branches",
  );
  await waitForPortalElement(
    openBranchPanel,
    `button[aria-label^="Current branch ${currentBranchName}"]`,
  );

  const grouping = await browser.execute((panelSelector) => {
    const panel = document.querySelector<HTMLElement>(panelSelector);
    if (!panel) return null;
    const list = panel.querySelector<HTMLElement>(
      '[data-branch-list][aria-label="Local branches"]',
    );
    if (!list) return null;

    return [0, 1, 2, 3].map((index) => {
      const item = list.querySelector<HTMLElement>(`[data-index="${index}"]`);
      const button = item?.querySelector<HTMLElement>("button[aria-label]");
      return {
        ariaCurrent: button?.getAttribute("aria-current") ?? null,
        label:
          item?.getAttribute("role") === "heading"
            ? (item.textContent?.trim() ?? null)
            : (button?.getAttribute("aria-label") ?? null),
        type: item?.getAttribute("role") === "heading" ? "heading" : "branch",
      };
    });
  }, openBranchPanel);

  if (!grouping) {
    throw new Error("Local default/current branch grouping was unavailable");
  }
  const expectedGrouping = [
    { ariaCurrent: null, label: "Default Branch", type: "heading" },
    {
      ariaCurrent: null,
      label: "Checkout main, default or protected, tracks origin/main",
      type: "branch",
    },
    { ariaCurrent: null, label: "Other Branches", type: "heading" },
    {
      ariaCurrent: "true",
      label: `Current branch ${currentBranchName}, tracks origin/${currentBranchName}`,
      type: "branch",
    },
  ];
  if (JSON.stringify(grouping) !== JSON.stringify(expectedGrouping)) {
    throw new Error(
      `Local branch sections are out of order: expected ${JSON.stringify(expectedGrouping)}, got ${JSON.stringify(grouping)}`,
    );
  }
}

async function assertBranchTimestampTooltip(): Promise<void> {
  const timestamp = await waitForPortalElement(
    openBranchPanel,
    "time[data-branch-commit-time]",
  );
  const compactTime = (await timestamp.getText()).trim();
  if (!/^(?:now|\d+[mhdwy]|in \d+[mhdwy])$/.test(compactTime)) {
    throw new Error(
      `Branch age is not compact: ${JSON.stringify(compactTime)}`,
    );
  }

  const dateTime = String(await timestamp.getAttribute("datetime"));
  if (Number.isNaN(Date.parse(dateTime))) {
    throw new Error(`Branch timestamp is invalid: ${JSON.stringify(dateTime)}`);
  }

  // WebKitDriver does not route moveTo hover events into the packaged Tauri
  // webview. Base UI listens for native mouseenter/mouseleave events, so send
  // those directly after establishing a mouse pointer type for React.
  const hoverDispatched = await browser.execute((element) => {
    if (!(element instanceof HTMLElement)) return false;
    const rect = element.getBoundingClientRect();
    const clientX = rect.left + rect.width / 2;
    const clientY = rect.top + rect.height / 2;
    element.dispatchEvent(
      new PointerEvent("pointerover", {
        bubbles: true,
        clientX,
        clientY,
        pointerType: "mouse",
      }),
    );
    element.dispatchEvent(
      new MouseEvent("mouseenter", {
        bubbles: false,
        clientX,
        clientY,
        relatedTarget: document.body,
        view: window,
      }),
    );
    element.dispatchEvent(
      new MouseEvent("mousemove", {
        bubbles: true,
        clientX,
        clientY,
        movementX: 1,
        movementY: 1,
        view: window,
      }),
    );
    return true;
  }, timestamp);
  if (!hoverDispatched) {
    throw new Error("Compact branch timestamp was unavailable for hover");
  }
  const tooltip = await waitForPortalElement("body", openTooltip);
  const expectedExactTime = await browser.execute(
    (value) => new Date(value).toLocaleString(),
    dateTime,
  );
  assertEqual(
    (await tooltip.getText()).trim(),
    expectedExactTime,
    "hovering a compact branch age shows its exact local timestamp",
  );
  await capture("04-branch-timestamp-tooltip");

  await browser.execute((element) => {
    if (!(element instanceof HTMLElement)) return;
    element.dispatchEvent(
      new PointerEvent("pointerout", {
        bubbles: true,
        pointerType: "mouse",
      }),
    );
    element.dispatchEvent(
      new MouseEvent("mouseleave", {
        bubbles: false,
        relatedTarget: document.body,
      }),
    );
  }, timestamp);
  await absent(openTooltip);
}

async function assertBranchContextMenu(): Promise<void> {
  const branchBefore = git("branch", "--show-current");
  await waitForPortalElement(
    openBranchPanel,
    'button[aria-label^="Checkout fixture/branch-01"]',
  );

  const contextMenuDispatched = await browser.execute(
    (panelSelector, branchSelector) => {
      const branchRow = document
        .querySelector(panelSelector)
        ?.querySelector<HTMLElement>(branchSelector);
      if (!branchRow) return false;

      const rect = branchRow.getBoundingClientRect();
      branchRow.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          button: 2,
          buttons: 2,
          cancelable: true,
          clientX: rect.left + Math.min(40, rect.width / 2),
          clientY: rect.top + rect.height / 2,
          view: window,
        }),
      );
      return true;
    },
    openBranchPanel,
    'button[aria-label^="Checkout fixture/branch-01"]',
  );
  if (!contextMenuDispatched) {
    throw new Error("fixture branch row was unavailable for a context menu");
  }

  await waitForPortalText(openBranchContextMenu, "*", "Rename branch");
  const contextMenuVisibility = await browser.execute((selector) => {
    const menu = document.querySelector<HTMLElement>(selector);
    if (!menu) return null;
    const style = getComputedStyle(menu);
    return {
      display: style.display,
      opacity: Number.parseFloat(style.opacity),
      visibility: style.visibility,
    };
  }, openBranchContextMenu);
  if (
    !contextMenuVisibility ||
    contextMenuVisibility.display === "none" ||
    contextMenuVisibility.visibility === "hidden" ||
    contextMenuVisibility.opacity < 0.99
  ) {
    throw new Error(
      `branch context menu is not visibly rendered: ${JSON.stringify(contextMenuVisibility)}`,
    );
  }
  await waitForPortalElement(
    openBranchPanel,
    'input[placeholder="Filter branches…"]',
  );
  assertEqual(
    git("branch", "--show-current"),
    branchBefore,
    "right-clicking a branch row must not check it out",
  );
  await capture("04-branch-context-menu");

  await browser.keys("Escape");
  await absent("[data-branch-context-menu]");
  await waitForPortalElement(
    openBranchPanel,
    'input[placeholder="Filter branches…"]',
  );
}

async function assertLargeRemoteBranchListVirtualized(): Promise<void> {
  const branchBefore = git("branch", "--show-current");
  // The list includes origin/main plus Git's shortened origin/HEAD alias.
  const expectedRemoteBranches = remoteFixtureBranchCount + 2;

  await clickPortalText(openBranchPanel, "button", "Remote");
  await waitForPortalElement(
    openBranchPanel,
    'button[aria-label^="Checkout and track origin/fixture/remote-0001"]',
  );

  const initialState = await browser.execute(
    (panelSelector, deepestBranchNumber) => {
      const panel = document.querySelector<HTMLElement>(panelSelector);
      const viewport = panel?.querySelector<HTMLElement>(
        '[data-current-branch-scroll] [data-slot="scroll-area-viewport"]',
      );
      const remoteTab = Array.from(
        panel?.querySelectorAll<HTMLElement>('[data-slot="tabs-tab"]') ?? [],
      ).find((tab) => tab.textContent?.trim().startsWith("Remote"));
      const list = panel?.querySelector<HTMLElement>("[data-branch-list]");
      const renderedRows = list?.querySelectorAll("[data-branch-row]");
      const firstRow = renderedRows?.item(0);

      if (
        !panel ||
        !viewport ||
        !remoteTab ||
        !list ||
        !renderedRows ||
        !firstRow
      ) {
        return null;
      }

      return {
        deepBranchInitiallyMounted: Boolean(
          panel.querySelector(
            `button[aria-label^="Checkout and track origin/fixture/remote-${String(deepestBranchNumber).padStart(4, "0")}"]`,
          ),
        ),
        firstRowPosition: firstRow.getAttribute("aria-posinset"),
        firstRowSetSize: firstRow.getAttribute("aria-setsize"),
        renderedRowCount: renderedRows.length,
        scrollClientHeight: viewport.clientHeight,
        scrollHeight: viewport.scrollHeight,
        tabText: remoteTab.textContent?.replace(/\s+/g, "").trim(),
        virtualized: list.dataset.virtualized,
      };
    },
    openBranchPanel,
    remoteFixtureBranchCount,
  );

  if (!initialState) {
    throw new Error("Remote branch virtualization state could not be read");
  }
  if (initialState.tabText !== `Remote${expectedRemoteBranches}`) {
    throw new Error(
      `Remote branch count mismatch: ${JSON.stringify(initialState.tabText)}`,
    );
  }
  if (
    initialState.firstRowPosition !== "1" ||
    initialState.firstRowSetSize !== String(expectedRemoteBranches)
  ) {
    throw new Error(
      `Virtualized branch accessibility position is incorrect: ${JSON.stringify(initialState)}`,
    );
  }
  if (initialState.virtualized !== "true") {
    throw new Error(
      `Large remote branch list was not virtualized: ${JSON.stringify(initialState)}`,
    );
  }
  if (
    initialState.renderedRowCount < 1 ||
    initialState.renderedRowCount > 100
  ) {
    throw new Error(
      `Expected a bounded virtualized DOM, got ${initialState.renderedRowCount} mounted remote branch rows`,
    );
  }
  if (initialState.deepBranchInitiallyMounted) {
    throw new Error(
      "A deep remote branch was mounted before scrolling or filtering",
    );
  }
  if (
    initialState.scrollHeight <= initialState.scrollClientHeight ||
    initialState.scrollHeight < remoteFixtureBranchCount * 30
  ) {
    throw new Error(
      `Remote branch virtual scroll extent is too small: ${JSON.stringify(initialState)}`,
    );
  }
  await capture("04-branch-virtualized-remote-list");

  const search = await waitForPortalElement(
    openBranchPanel,
    'input[placeholder="Filter branches…"]',
  );
  await search.click();
  await browser.keys("ArrowDown");
  await browser.waitUntil(
    () =>
      browser.execute(
        () =>
          document.activeElement?.getAttribute("aria-label") ===
          "Checkout and track origin",
      ),
    {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "ArrowDown did not focus the first virtualized remote branch",
    },
  );
  // WebKitDriver does not dispatch its End action to the focused Tauri webview
  // button, so exercise the same native keydown path in the packaged DOM.
  await browser.execute(() => {
    document.activeElement?.dispatchEvent(
      new KeyboardEvent("keydown", {
        bubbles: true,
        cancelable: true,
        key: "End",
      }),
    );
  });
  await browser.waitUntil(
    () =>
      browser.execute(
        () =>
          document.activeElement
            ?.getAttribute("aria-label")
            ?.startsWith("Checkout and track origin/main") ?? false,
      ),
    {
      timeout: 10_000,
      interval: 100,
      timeoutMsg: "End did not focus the final virtualized remote branch",
    },
  );

  await setPortalInput(
    openBranchPanel,
    'input[placeholder="Filter branches…"]',
    "origin/fixture/remote-2000",
  );
  const deepBranchSelector =
    'button[aria-label^="Checkout and track origin/fixture/remote-2000"]';
  await waitForPortalElement(openBranchPanel, deepBranchSelector);
  await browser.waitUntil(
    () =>
      browser.execute((panelSelector) => {
        const viewport = document
          .querySelector(panelSelector)
          ?.querySelector<HTMLElement>(
            '[data-current-branch-scroll] [data-slot="scroll-area-viewport"]',
          );
        if (!viewport) return false;
        return (
          viewport.scrollHeight <= viewport.clientHeight &&
          !viewport.hasAttribute("data-has-overflow-y")
        );
      }, openBranchPanel),
    {
      timeout: 10_000,
      interval: 100,
      timeoutMsg:
        "Filtering the virtual list did not clear the scroll area's overflow state",
    },
  );

  const contextMenuDispatched = await browser.execute(
    (panelSelector, branchSelector) => {
      const branchRow = document
        .querySelector(panelSelector)
        ?.querySelector<HTMLElement>(branchSelector);
      if (!branchRow) return false;

      const rect = branchRow.getBoundingClientRect();
      branchRow.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          button: 2,
          buttons: 2,
          cancelable: true,
          clientX: rect.left + Math.min(40, rect.width / 2),
          clientY: rect.top + rect.height / 2,
          view: window,
        }),
      );
      return true;
    },
    openBranchPanel,
    deepBranchSelector,
  );
  if (!contextMenuDispatched) {
    throw new Error("Filtered deep remote branch was unavailable for actions");
  }
  await waitForPortalText(openBranchContextMenu, "*", "Delete remote branch");
  assertEqual(
    git("branch", "--show-current"),
    branchBefore,
    "filtering and right-clicking a deep remote branch must not check it out",
  );
  await browser.keys("Escape");
  await absent("[data-branch-context-menu]");

  await setPortalInput(
    openBranchPanel,
    'input[placeholder="Filter branches…"]',
    "",
  );
  await clickPortalText(openBranchPanel, "button", "Local");
  const currentBranch = await waitForPortalElement(
    openBranchPanel,
    'button[aria-label^="Current branch main"]',
  );
  assertEqual(
    String(await currentBranch.getAttribute("aria-current")),
    "true",
    "the local current branch remains identified after virtualized remote navigation",
  );
}

async function openRootAction(label: string): Promise<void> {
  await browser.execute(() => {
    document.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "k",
        metaKey: true,
        bubbles: true,
      }),
    );
  });
  const inputSelector = 'input[placeholder="Search actions or branches..."]';
  await waitForPortalElement(openCommandDialog, inputSelector);
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

    await openBranchSwitcher();
    await assertBranchPanelLayout();
    await assertBranchTimestampTooltip();
    await assertBranchContextMenu();
    await assertLargeRemoteBranchListVirtualized();
    await capture("04-branch-panel-full-height");
    await (
      await waitForPortalElement(
        openBranchPanel,
        'button[aria-label="New branch"]',
      )
    ).click();
    await setPortalInput(
      openDialog,
      'input[placeholder="feature/my-branch"]',
      smokeBranch,
    );
    await clickPortalText(openDialog, "button", "Create & Checkout");
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
    await assertLocalBranchGrouping(smokeBranch);
    await capture("05-branch-default-current-grouping");
    await setPortalInput(
      openBranchPanel,
      'input[placeholder="Filter branches…"]',
      "conflict-work",
    );
    await clickPortalText(openBranchPanel, "button", "conflict-work");
    await clickPortalText(openDialog, "button", "Stash & Checkout");
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
    const branchSwitcher = await visible(
      'button[aria-label^="Current branch:"]',
    );
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
