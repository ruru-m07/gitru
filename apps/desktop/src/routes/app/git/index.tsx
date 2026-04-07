import type { FileStatusKind } from "@gitru/commands";
import { Button } from "@gitru/ui/components/button";
import { CopyButton } from "@gitru/ui/components/copy-button";
import { Group, GroupSeparator } from "@gitru/ui/components/group";
import { Kbd, KbdGroup } from "@gitru/ui/components/kbd";
import { Label } from "@gitru/ui/components/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@gitru/ui/components/popover";
import { Separator } from "@gitru/ui/components/separator";
import { Switch } from "@gitru/ui/components/switch";
import { cn } from "@gitru/ui/lib/utils";
import { parseDiffFromFile } from "@pierre/diffs";
import {
  MultiFileDiff,
  Virtualizer,
  WorkerPoolContextProvider,
} from "@pierre/diffs/react";
import { createFileRoute, useRouterState } from "@tanstack/react-router";
import {
  ArrowUpFromLine,
  ChevronDown,
  ChevronsUp,
  CircleAlertIcon,
  Diff,
  GitBranch,
  Loader2,
  Minus,
  MoveHorizontal,
  Plus,
  Settings,
  TextWrap,
  Undo,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
// import { useTheme } from "next-themes";
import { ImageDiffViewer } from "@/components/diff/image/ImageDiffViewer";
import { useDiffViewerSettings } from "@/components/diff/useDiffViewSettingStore";
import { getStatusIcon } from "@/components/getStatusIcon";
import HistoryGraph from "@/components/historyGraph";
import LoaderIndicator from "@/components/loaderIndicator";
import { GitruBorderedSVG } from "@/components/svgs/gitru-borderd";
import {
  useGetCommitById,
  useGetCurrentBranch,
  useGetCurrentBranchStash,
  useGetDiff,
  useGetStatus,
  useGetStatusAheadBehind,
  useGitApplyPatchBlock,
  useGitPush,
  useStashList,
  useStashShow,
} from "@/hooks";
import {
  selectActiveRepository,
  selectActiveSessionRepoKey,
  useAppStore,
} from "@/store/useAppStore";
import { SplitSVG } from "../../../components/svgs/splitSVG";
import { UnifiedSVG } from "../../../components/svgs/unifiedSVG";
import { diffWorkerFactory } from "../../../lib/diffWorkerFactory";
import {
  type ResolvedFileSelection,
  resolveFileSelection,
} from "../../../lib/gitSelectionResolver";

export const Route = createFileRoute("/app/git/")({
  component: App,
});

const TAB_STRESS_QUERY_PARAM = "tabStress";
const MEDIAN_SWITCH_TARGET_MS = 180;

type StressScenario = "same-repo" | "mixed-repo";

type TabSwitchStressReport = {
  scenario: StressScenario;
  tabCount: number;
  sampleCount: number;
  minMs: number;
  medianMs: number;
  p95Ms: number;
  maxMs: number;
  isolationMismatchCount: number;
  snapshotCoverage: number;
  latencyPass: boolean;
  isolationPass: boolean;
};

const waitForNextFrame = () =>
  new Promise<void>((resolve) => {
    requestAnimationFrame(() => resolve());
  });

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

const percentile = (values: number[], ratio: number) => {
  if (values.length === 0) {
    return 0;
  }

  const sorted = [...values].sort((left, right) => left - right);
  const index = clamp(
    Math.ceil(sorted.length * ratio) - 1,
    0,
    sorted.length - 1,
  );

  return sorted[index] ?? 0;
};

function App() {
  const routerState = useRouterState();
  const mainWindowView = useAppStore((state) => state.mainWindowView);
  const isStressMode = useMemo(() => {
    const params = new URLSearchParams(routerState.location.search);
    const value = params.get(TAB_STRESS_QUERY_PARAM);

    return value === "1" || value === "true";
  }, [routerState.location.search]);

  if (isStressMode) {
    return <TabSwitchStressPanel />;
  }

  return (
    <>
      <MainActionBar />
      {mainWindowView === null && <EmptyStateScreen />}
      {mainWindowView === "FileDiff" && <DiffBoxBody />}
      {mainWindowView === "HistoryGraph" && <HistoryGraph />}
    </>
  );
}

const TabSwitchStressPanel = () => {
  const repositories = useAppStore((state) => state.repositories);
  const activeRepository = useAppStore(selectActiveRepository);
  const createTab = useAppStore((state) => state.createTab);
  const activateTab = useAppStore((state) => state.activateTab);
  const closeTab = useAppStore((state) => state.closeTab);
  const [tabCountInput, setTabCountInput] = useState("60");
  const [sampleCountInput, setSampleCountInput] = useState("200");
  const [runSameRepoScenario, setRunSameRepoScenario] = useState(true);
  const [runMixedRepoScenario, setRunMixedRepoScenario] = useState(true);
  const [isRunning, setIsRunning] = useState(false);
  const [reports, setReports] = useState<TabSwitchStressReport[]>([]);
  const [message, setMessage] = useState("");

  const runScenario = useCallback(
    async (
      scenario: StressScenario,
      targetTabCount: number,
      sampleCount: number,
    ): Promise<TabSwitchStressReport> => {
      const startingState = useAppStore.getState();
      const anchorTabId =
        startingState.activeTabId ?? startingState.tabs[0]?.id ?? null;

      if (!anchorTabId) {
        throw new Error("No tab available for stress scenario");
      }

      const anchorTab =
        startingState.tabs.find((tab) => tab.id === anchorTabId) ?? null;
      const fallbackRepoId =
        activeRepository?.id ?? repositories[0]?.id ?? null;
      const anchorRepoId = anchorTab?.repositoryId ?? fallbackRepoId;

      const createdTabIds: string[] = [];
      const expectedRepositoryByTabId = new Map<string, string | null>();

      expectedRepositoryByTabId.set(
        anchorTabId,
        scenario === "same-repo"
          ? anchorRepoId
          : (anchorTab?.repositoryId ?? null),
      );

      const scenarioRoutePath = `/app/git?${TAB_STRESS_QUERY_PARAM}=1`;

      for (let index = 1; index < targetTabCount; index += 1) {
        const repositoryId =
          scenario === "same-repo"
            ? anchorRepoId
            : (repositories[index % Math.max(repositories.length, 1)]?.id ??
              anchorRepoId);

        const tab = createTab({
          routePath: scenarioRoutePath,
          repositoryId,
          title:
            scenario === "same-repo"
              ? `Stress Same ${index + 1}`
              : `Stress Mixed ${index + 1}`,
        });

        createdTabIds.push(tab.id);
        expectedRepositoryByTabId.set(tab.id, repositoryId);
      }

      const tabIds = [anchorTabId, ...createdTabIds].filter(
        (tabId, index, all) => all.indexOf(tabId) === index,
      );

      for (const tabId of tabIds) {
        activateTab(tabId);
        await waitForNextFrame();
      }

      const samples: number[] = [];
      let pointer = 0;

      while (samples.length < sampleCount) {
        const tabId = tabIds[pointer % tabIds.length];
        const start = performance.now();

        activateTab(tabId);
        await waitForNextFrame();

        samples.push(performance.now() - start);
        pointer += 1;
      }

      const postSwitchState = useAppStore.getState();
      const isolationMismatchCount = tabIds.filter((tabId) => {
        const expectedRepositoryId =
          expectedRepositoryByTabId.get(tabId) ?? null;
        const nextTab = postSwitchState.tabs.find((tab) => tab.id === tabId);

        return (nextTab?.repositoryId ?? null) !== expectedRepositoryId;
      }).length;

      const snapshotCoverage =
        tabIds.length === 0
          ? 0
          : tabIds.filter(
              (tabId) => postSwitchState.sessionsById[tabId]?.snapshot,
            ).length / tabIds.length;

      for (const tabId of [...createdTabIds].reverse()) {
        const currentState = useAppStore.getState();
        if (currentState.tabs.length <= 1) {
          break;
        }

        closeTab(tabId);
      }

      const finalState = useAppStore.getState();
      if (finalState.tabs.some((tab) => tab.id === anchorTabId)) {
        activateTab(anchorTabId);
        await waitForNextFrame();
      }

      const minMs = Math.min(...samples);
      const medianMs = percentile(samples, 0.5);
      const p95Ms = percentile(samples, 0.95);
      const maxMs = Math.max(...samples);
      const latencyPass = medianMs <= MEDIAN_SWITCH_TARGET_MS;
      const isolationPass = isolationMismatchCount === 0;

      return {
        scenario,
        tabCount: tabIds.length,
        sampleCount: samples.length,
        minMs,
        medianMs,
        p95Ms,
        maxMs,
        isolationMismatchCount,
        snapshotCoverage,
        latencyPass,
        isolationPass,
      };
    },
    [activateTab, activeRepository?.id, closeTab, createTab, repositories],
  );

  const runStressTest = useCallback(async () => {
    const targetTabCount = clamp(Number(tabCountInput) || 60, 10, 120);
    const sampleCount = clamp(Number(sampleCountInput) || 200, 50, 2000);

    if (!runSameRepoScenario && !runMixedRepoScenario) {
      setMessage("Enable at least one scenario before running stress tests.");
      return;
    }

    if (!repositories.length) {
      setMessage("Add at least one repository before running stress tests.");
      return;
    }

    if (runMixedRepoScenario && repositories.length < 2) {
      setMessage("Mixed scenario needs at least two repositories.");
      return;
    }

    setIsRunning(true);
    setReports([]);
    setMessage("Running tab switching stress scenarios...");

    const scenarios: StressScenario[] = [];
    if (runSameRepoScenario) scenarios.push("same-repo");
    if (runMixedRepoScenario) scenarios.push("mixed-repo");

    try {
      const nextReports: TabSwitchStressReport[] = [];

      for (const scenario of scenarios) {
        const report = await runScenario(scenario, targetTabCount, sampleCount);
        nextReports.push(report);
        setReports([...nextReports]);
      }

      const allPass = nextReports.every(
        (report) => report.latencyPass && report.isolationPass,
      );

      setMessage(
        allPass
          ? `Stress tests passed. Median switch target <= ${MEDIAN_SWITCH_TARGET_MS}ms met.`
          : "Stress tests completed with regressions. See report details below.",
      );
    } catch (error) {
      setMessage(
        error instanceof Error
          ? error.message
          : "Stress tests failed unexpectedly.",
      );
    } finally {
      setIsRunning(false);
    }
  }, [
    repositories,
    runMixedRepoScenario,
    runSameRepoScenario,
    runScenario,
    sampleCountInput,
    tabCountInput,
  ]);

  return (
    <div className="h-full overflow-y-auto p-4">
      <div className="mx-auto max-w-4xl space-y-4">
        <div className="rounded-lg border p-4">
          <h2 className="text-lg font-medium">Tab Switch Stress Runner</h2>
          <p className="text-sm text-muted-foreground mt-1">
            Path: /app/git?tabStress=1
          </p>
          <p className="text-sm text-muted-foreground">
            Runs same-repo and mixed-repo tab switching tests and reports
            median, p95, and isolation mismatches.
          </p>
        </div>

        <div className="rounded-lg border p-4 space-y-4">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <div className="space-y-2">
              <Label htmlFor="stress-tab-count">
                Target tab count (10-120)
              </Label>
              <input
                id="stress-tab-count"
                className="w-full rounded-md border bg-background px-3 py-2 text-sm"
                inputMode="numeric"
                value={tabCountInput}
                onChange={(event) => setTabCountInput(event.target.value)}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="stress-sample-count">
                Samples per scenario (50-2000)
              </Label>
              <input
                id="stress-sample-count"
                className="w-full rounded-md border bg-background px-3 py-2 text-sm"
                inputMode="numeric"
                value={sampleCountInput}
                onChange={(event) => setSampleCountInput(event.target.value)}
              />
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-4">
            <div className="flex items-center gap-2">
              <Switch
                checked={runSameRepoScenario}
                onCheckedChange={setRunSameRepoScenario}
              />
              <span className="text-sm">Same repo scenario</span>
            </div>

            <div className="flex items-center gap-2">
              <Switch
                checked={runMixedRepoScenario}
                onCheckedChange={setRunMixedRepoScenario}
              />
              <span className="text-sm">Mixed repo scenario</span>
            </div>
          </div>

          <div className="flex items-center gap-3">
            <Button
              onClick={() => {
                void runStressTest();
              }}
              disabled={isRunning}
            >
              {isRunning ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  Running...
                </>
              ) : (
                "Run Stress Test"
              )}
            </Button>

            <span className="text-sm text-muted-foreground">
              Repositories detected: {repositories.length}
            </span>
          </div>

          {message ? (
            <p className="text-sm text-muted-foreground">{message}</p>
          ) : null}
        </div>

        {reports.length > 0 ? (
          <div className="rounded-lg border overflow-hidden">
            <table className="w-full text-sm">
              <thead className="bg-muted/50 text-muted-foreground">
                <tr>
                  <th className="px-3 py-2 text-left">Scenario</th>
                  <th className="px-3 py-2 text-right">Tabs</th>
                  <th className="px-3 py-2 text-right">Samples</th>
                  <th className="px-3 py-2 text-right">Median (ms)</th>
                  <th className="px-3 py-2 text-right">P95 (ms)</th>
                  <th className="px-3 py-2 text-right">Max (ms)</th>
                  <th className="px-3 py-2 text-right">Isolation</th>
                  <th className="px-3 py-2 text-right">Snapshot</th>
                </tr>
              </thead>
              <tbody>
                {reports.map((report) => (
                  <tr key={report.scenario} className="border-t">
                    <td className="px-3 py-2">{report.scenario}</td>
                    <td className="px-3 py-2 text-right">{report.tabCount}</td>
                    <td className="px-3 py-2 text-right">
                      {report.sampleCount}
                    </td>
                    <td
                      className={cn(
                        "px-3 py-2 text-right",
                        report.latencyPass ? "text-green-600" : "text-red-600",
                      )}
                    >
                      {report.medianMs.toFixed(2)}
                    </td>
                    <td className="px-3 py-2 text-right">
                      {report.p95Ms.toFixed(2)}
                    </td>
                    <td className="px-3 py-2 text-right">
                      {report.maxMs.toFixed(2)}
                    </td>
                    <td
                      className={cn(
                        "px-3 py-2 text-right",
                        report.isolationPass
                          ? "text-green-600"
                          : "text-red-600",
                      )}
                    >
                      {report.isolationMismatchCount === 0
                        ? "ok"
                        : `${report.isolationMismatchCount} mismatches`}
                    </td>
                    <td className="px-3 py-2 text-right">
                      {(report.snapshotCoverage * 100).toFixed(0)}%
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : null}
      </div>
    </div>
  );
};

const DiffBoxBody = () => {
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const repoSelectionState = useAppStore((state) =>
    repoStateKey ? state.selectionByRepo[repoStateKey] : undefined,
  );
  const gitViewState = useAppStore((state) =>
    repoStateKey ? state.gitViewByRepo[repoStateKey] : undefined,
  );
  const { data: status } = useGetStatus();
  const { data: currentBranchStash } = useGetCurrentBranchStash();
  const { data: stashes } = useStashList();

  const activeSource =
    gitViewState?.leftPanelView === "stash"
      ? "stash"
      : gitViewState?.leftPanelView === "history"
        ? "history"
        : "worktree";
  const activeStashReference =
    activeSource === "stash"
      ? gitViewState?.stashViewMode === "branch"
        ? (currentBranchStash?.reference ?? null)
        : (gitViewState?.selectedStashReference ?? null)
      : null;
  const activeHistoryCommitHash =
    activeSource === "history"
      ? (gitViewState?.selectedHistoryCommitHash ?? null)
      : null;

  const { data: stashShow } = useStashShow(activeStashReference);
  const { data: historyCommit } = useGetCommitById(
    activeHistoryCommitHash ?? "",
  );

  const activeSelection =
    activeSource === "stash"
      ? activeStashReference
        ? (repoSelectionState?.stashByReference[activeStashReference] ?? null)
        : null
      : activeSource === "history"
        ? activeHistoryCommitHash
          ? (repoSelectionState?.historyByCommit?.[activeHistoryCommitHash] ??
            null)
          : null
        : (repoSelectionState?.worktree ?? null);

  const resolvedSelection = resolveFileSelection({
    selection: activeSelection,
    files:
      activeSource === "stash"
        ? (stashShow?.files ?? [])
        : activeSource === "history"
          ? (historyCommit?.files ?? [])
          : (status?.files ?? []),
    context: {
      source: activeSource,
      stashReference: activeStashReference,
      availableStashReferences: (stashes ?? []).map((stash) => stash.reference),
      historyCommitHash: activeHistoryCommitHash,
    },
  });

  return (
    <>
      {resolvedSelection.state === "valid" ? (
        <>
          <FileLevelStatusBar resolvedSelection={resolvedSelection} />
          <DiffArea
            filePath={resolvedSelection.file.path}
            fileNewPath={resolvedSelection.file.new_path ?? null}
            status={resolvedSelection.file.status}
            stashReference={
              resolvedSelection.identity.source === "stash"
                ? (resolvedSelection.identity.stashReference ?? null)
                : null
            }
            commitHash={
              resolvedSelection.identity.source === "history"
                ? (resolvedSelection.identity.historyCommitHash ?? null)
                : null
            }
            worktreeScope={
              resolvedSelection.identity.source === "worktree"
                ? resolvedSelection.identity.worktreeScope
                : undefined
            }
          />
        </>
      ) : (
        <EmptyStateScreen />
      )}
    </>
  );
};

const MainActionBar = () => {
  const { data: currentBranch } = useGetCurrentBranch();
  const { data: statusAheadBehind } = useGetStatusAheadBehind();

  const { mutateAsync: push, isPending } = useGitPush();

  return (
    <div className="w-full justify-between min-h-14 max-h-14 h-14 border-b flex">
      <div className="min-h-14 max-h-14 h-14 flex w-full">
        <Button
          className="flex justify-between items-center min-h-full rounded-none border-x-0 max-w-72 w-full"
          variant="ghost"
        >
          <div className="flex items-center gap-4 min-w-0 flex-1">
            <GitBranch className="size-7.5" strokeWidth={1.5} />

            <div className="flex flex-col items-start min-w-0 flex-1">
              <span className="text-xs text-muted-foreground font-[450]">
                Current Branch
              </span>
              <span className="truncate block w-full text-left">
                {currentBranch?.display_name}
              </span>
            </div>
          </div>

          <ChevronDown size={18} />
        </Button>
        <Separator orientation="vertical" className={"border-0"} />
        {statusAheadBehind && statusAheadBehind.is_published ? (
          (statusAheadBehind && statusAheadBehind.ahead > 0) ||
          (statusAheadBehind && statusAheadBehind.behind > 0) ? (
            <>
              <Button
                className="flex justify-between items-center min-h-full rounded-none border-x-0 w-72"
                variant={"ghost"}
                onClick={async () => {
                  await push();
                }}
              >
                <div className="flex items-center justify-center gap-4">
                  {isPending ? (
                    <Loader2
                      className="animate-spin size-7.5"
                      strokeWidth={1.5}
                    />
                  ) : (
                    <ChevronsUp className="size-8 rotate-180" />
                  )}
                  <div className="flex-col flex items-start">
                    <span className="text-xs text-muted-foreground font-[450]">
                      {statusAheadBehind.ahead > 0
                        ? "Push to Origin"
                        : "Pull from Origin"}
                    </span>
                    <span>
                      {statusAheadBehind
                        ? `${statusAheadBehind.ahead} / ${statusAheadBehind.behind}`
                        : "0 / 0"}
                    </span>
                  </div>
                </div>
                <ChevronDown size={18} />
              </Button>
              <Separator orientation="vertical" className={"border-0"} />
            </>
          ) : null
        ) : (
          <>
            <Button
              className="flex border-x-0 justify-between items-center min-h-full rounded-none max-w-60 w-60"
              variant={"ghost"}
              onClick={async () => {
                await push();
              }}
            >
              <div className="flex items-center gap-4 min-w-0 flex-1">
                {isPending ? (
                  <Loader2
                    className="animate-spin size-7.5"
                    strokeWidth={1.5}
                  />
                ) : (
                  <ArrowUpFromLine className="size-7.5" strokeWidth={1.5} />
                )}
                <div className="flex flex-col flex-1 items-start min-w-0">
                  <span className="text-xs text-muted-foreground font-normal">
                    Publish Branch
                  </span>
                  <span className="truncate block w-full text-left">
                    Published as {currentBranch?.name}
                  </span>
                </div>
              </div>
            </Button>
            <Separator orientation="vertical" className={"border-0"} />
            <Button
              className="flex border-x-0 justify-between items-center min-h-full rounded-none"
              variant={"ghost"}
              onClick={async () => {}}
            >
              <ChevronDown size={18} />
            </Button>
            <Separator orientation="vertical" className={"border-0"} />
          </>
        )}
      </div>
      <div></div>
    </div>
  );
};

const FileLevelStatusBar = ({
  resolvedSelection,
}: {
  resolvedSelection: ResolvedFileSelection;
}) => {
  const clearWorktreeSelectionForRepo = useAppStore(
    (state) => state.clearWorktreeSelectionForRepo,
  );
  const clearStashSelectionForRepo = useAppStore(
    (state) => state.clearStashSelectionForRepo,
  );
  const clearHistorySelectionForRepo = useAppStore(
    (state) => state.clearHistorySelectionForRepo,
  );
  const setMainWindowView = useAppStore((state) => state.setMainWindowView);

  return (
    <div className="w-full h-9.25 border-b flex justify-between items-center">
      <FileLevelStatusBarLeft resolvedSelection={resolvedSelection} />
      <div className="flex items-center gap-2 pr-2">
        <Button
          size="icon-xs"
          variant="outline"
          className="relative"
          aria-label="Open notifications"
          onClick={() => {
            setMainWindowView(null);
            if (resolvedSelection.state === "none") {
              return;
            }

            const selection = resolvedSelection.identity;
            if (selection.source === "stash" && selection.stashReference) {
              clearStashSelectionForRepo(selection.stashReference);
              return;
            }

            if (selection.source === "history" && selection.historyCommitHash) {
              clearHistorySelectionForRepo(selection.historyCommitHash);
              return;
            }

            clearWorktreeSelectionForRepo();
          }}
        >
          <X />
        </Button>

        <SettingsPopover />
      </div>
    </div>
  );
};

const DiffArea = ({
  filePath,
  fileNewPath,
  status,
  stashReference,
  commitHash,
  worktreeScope,
}: {
  filePath: string;
  fileNewPath: string | null;
  status: FileStatusKind[];
  stashReference: string | null;
  commitHash: string | null;
  worktreeScope?: "staged" | "unstaged" | "conflicted";
}) => {
  const { diffStyle, overflow } = useDiffViewerSettings();
  const [effectiveDiffStyle, setEffectiveDiffStyle] =
    useState<typeof diffStyle>(diffStyle);

  const derivedScope =
    worktreeScope ??
    (status?.some((s) => String(s).startsWith("Index")) &&
    !status?.some((s) => String(s).startsWith("Worktree"))
      ? "staged"
      : status?.some((s) => String(s).startsWith("Worktree"))
        ? "unstaged"
        : undefined);

  const { data: diffData, isLoading } = useGetDiff(filePath, {
    fileNewPath,
    status,
    stashReference,
    commitHash,
    parentIndex: commitHash ? 1 : undefined,
    diffScope:
      derivedScope === "staged"
        ? "Staged"
        : derivedScope === "unstaged" || derivedScope === "conflicted"
          ? "Unstaged"
          : "Worktree",
  });

  useEffect(() => {
    let mounted = true;

    const compute = () => {
      try {
        const layoutEl = document.querySelector(
          '[data-layout-id="local-git-layout"]',
        );
        if (!layoutEl) {
          return diffStyle;
        }

        const raw =
          getComputedStyle(layoutEl).getPropertyValue("--right-width") || "";
        const num = parseFloat(raw.trim().replace("px", ""));

        if (!isNaN(num) && num < 750) {
          return "unified" as typeof diffStyle;
        }
      } catch (e) {
        console.error("[DiffArea] Error reading --right-width:", e);
      }

      return diffStyle;
    };

    const recompute = () => {
      if (!mounted) return;
      setEffectiveDiffStyle(compute());
    };

    recompute();

    window.addEventListener("resize", recompute);

    const layoutEl = document.querySelector(
      '[data-layout-id="local-git-layout"]',
    );
    let mo: MutationObserver | undefined;

    if (layoutEl) {
      mo = new MutationObserver(() => {
        console.log("[DiffArea] Layout element changed, recomputing...");
        recompute();
      });
      mo.observe(layoutEl, {
        attributes: true,
        attributeFilter: ["style"],
      });
    }

    return () => {
      mounted = false;
      window.removeEventListener("resize", recompute);
      mo?.disconnect();
    };
  }, [diffStyle]);

  const { mutateAsync: applyPatchBlock } = useGitApplyPatchBlock();

  const source = stashReference ? "stash" : commitHash ? "history" : "worktree";

  const canStageOrDiscard =
    source === "worktree" && derivedScope === "unstaged";
  const canUnstage = source === "worktree" && derivedScope === "staged";
  const patchDiffScope =
    derivedScope === "staged"
      ? "Staged"
      : derivedScope === "unstaged" || derivedScope === "conflicted"
        ? "Unstaged"
        : "Worktree";

  const parsedDiff = useMemo(() => {
    if (!diffData?.oldFile || !diffData?.newFile) {
      return null;
    }

    try {
      return parseDiffFromFile(
        {
          name: diffData.oldFile.name,
          contents: diffData.oldFile.contents,
          cacheKey: `${source}:${filePath}:old`,
        },
        {
          name: diffData.newFile.name,
          contents: diffData.newFile.contents,
          cacheKey: `${source}:${filePath}:new`,
        },
      );
    } catch (error) {
      console.warn("Failed to parse diff from file contents", error);
      return null;
    }
  }, [diffData, filePath, source]);

  const blockMetadataLookup = useMemo(() => {
    if (!parsedDiff) {
      return new Map<string, ChunkActionMetadata>();
    }

    const lookup = new Map<string, ChunkActionMetadata>();

    for (let hunkIndex = 0; hunkIndex < parsedDiff.hunks.length; hunkIndex++) {
      const hunk = parsedDiff.hunks[hunkIndex];
      let additionCursor = hunk.additionStart;
      let deletionCursor = hunk.deletionStart;
      let changeIndex = 0;

      for (const content of hunk.hunkContent) {
        if (content.type === "context") {
          const contextLen = content.lines;
          additionCursor += contextLen;
          deletionCursor += contextLen;
          continue;
        }

        const additionsLen = content.additions;
        const deletionsLen = content.deletions;

        // Store metadata for the FIRST line of each block only
        // Smart pairing: if both additions and deletions exist, show only 1 annotation (on additions side)
        // If only one side exists, show annotation on that side

        if (additionsLen > 0 && deletionsLen > 0) {
          // Replacement block (paired): show one annotation on additions side
          const payload: ChunkActionMetadata = {
            source,
            filePath,
            fileNewPath,
            stashReference,
            commitHash,
            hunkIndex,
            changeIndex,
            side: "additions",
            additions: {
              start: additionCursor,
              end: additionCursor + additionsLen - 1,
              count: additionsLen,
            },
            deletions: {
              start: deletionCursor,
              end: deletionCursor + deletionsLen - 1,
              count: deletionsLen,
            },
          };
          lookup.set(`additions:${additionCursor + additionsLen - 1}`, payload);
        } else if (additionsLen > 0) {
          // Pure addition: show annotation on additions side
          const payload: ChunkActionMetadata = {
            source,
            filePath,
            fileNewPath,
            stashReference,
            commitHash,
            hunkIndex,
            changeIndex,
            side: "additions",
            additions: {
              start: additionCursor,
              end: additionCursor + additionsLen - 1,
              count: additionsLen,
            },
            deletions: {
              start: null,
              end: null,
              count: 0,
            },
          };
          lookup.set(`additions:${additionCursor + additionsLen - 1}`, payload);
        } else if (deletionsLen > 0) {
          // Pure deletion: show annotation on deletions side
          const payload: ChunkActionMetadata = {
            source,
            filePath,
            fileNewPath,
            stashReference,
            commitHash,
            hunkIndex,
            changeIndex,
            side: "deletions",
            additions: {
              start: null,
              end: null,
              count: 0,
            },
            deletions: {
              start: deletionCursor,
              end: deletionCursor + deletionsLen - 1,
              count: deletionsLen,
            },
          };
          lookup.set(`deletions:${deletionCursor + deletionsLen - 1}`, payload);
        }

        additionCursor += additionsLen;
        deletionCursor += deletionsLen;
        changeIndex += 1;
      }
    }

    return lookup;
  }, [parsedDiff, source, filePath, fileNewPath, stashReference, commitHash]);

  const blockAnnotations = useMemo(() => {
    const annotations: Array<{
      side: "additions" | "deletions";
      lineNumber: number;
      metadata: ChunkActionMetadata;
    }> = [];

    // Convert blockMetadataLookup Map entries to lineAnnotations array
    blockMetadataLookup.forEach((metadata, key) => {
      const [side, lineNumberStr] = key.split(":");
      const lineNumber = parseInt(lineNumberStr, 10);

      if (!isNaN(lineNumber)) {
        annotations.push({
          side: side as "additions" | "deletions",
          lineNumber,
          metadata,
        });
      }
    });

    return annotations;
  }, [blockMetadataLookup]);

  const assetKind = String(diffData?.asset_diff?.kind ?? "").toLowerCase();
  const isImageAssetDiff = assetKind === "image";
  const imageAssetDiff = isImageAssetDiff
    ? (diffData?.asset_diff ?? null)
    : null;

  return (
    <div
      className={cn(
        "bg-secondary h-full max-h-[calc(var(--layout-height)---spacing(23.25))] w-full relative overflow-y-auto _bg-[color-mix(in_oklab,var(--color-secondary)_70%,var(--color-background))]",
      )}
    >
      {isLoading ? (
        <div className="p-2.5">
          <LoaderIndicator />
        </div>
      ) : (
        <>
          {imageAssetDiff ? <ImageDiffViewer diff={imageAssetDiff} /> : null}
          {!isImageAssetDiff && (
            <div className="max-h-[calc(var(--layout-height)---spacing(23.25))] h-full w-full flex overflow-auto select-auto">
              <WorkerPoolContextProvider
                poolOptions={{
                  workerFactory: diffWorkerFactory,
                  poolSize: 4,
                }}
                highlighterOptions={{
                  theme: {
                    dark: "github-dark",
                    light: "github-light",
                  },
                  langs: [
                    "typescript",
                    "tsx",
                    "javascript",
                    "jsx",
                    "rust",
                    "json",
                    "css",
                    "html",
                    "markdown",
                    "toml",
                    "yaml",
                  ],
                }}
              >
                <Virtualizer
                  className="max-h-[calc(var(--layout-height)---spacing(23.25))] overflow-auto w-full"
                  contentClassName="space-y-4 w-full!"
                >
                  <MultiFileDiff
                    key={`${diffData?.oldFile?.name}-${diffData?.newFile?.name}-${source}-${patchDiffScope}-${diffData?.patch}`}
                    className="w-full"
                    oldFile={{
                      contents: diffData?.oldFile?.contents || "",
                      name: diffData?.oldFile?.name || "untitled.txt",
                    }}
                    newFile={{
                      contents: diffData?.newFile?.contents || "",
                      name: diffData?.newFile?.name || "untitled.txt",
                    }}
                    options={{
                      diffStyle: effectiveDiffStyle,
                      overflow,
                      disableFileHeader: true,
                      collapsedContextThreshold: 0,
                      lineHoverHighlight: "both",
                      unsafeCSS: `
                      [data-background] {
                        --diffs-light-bg: var(--secondary) !important;
                        --diffs-dark-bg: var(--secondary) !important;
                      }
                      `,
                    }}
                    lineAnnotations={blockAnnotations}
                    renderAnnotation={(annotation) => {
                      if (!canStageOrDiscard && !canUnstage) {
                        return null;
                      }

                      const payload = {
                        filePath: annotation.metadata.filePath,
                        fileNewPath: annotation.metadata.fileNewPath,
                        diffScope: patchDiffScope,
                        additions: {
                          start:
                            annotation.metadata.additions.start ?? undefined,
                          count: annotation.metadata.additions.count,
                        },
                        deletions: {
                          start:
                            annotation.metadata.deletions.start ?? undefined,
                          count: annotation.metadata.deletions.count,
                        },
                      } as const;

                      return (
                        <div
                          style={{
                            position: "relative",
                            zIndex: 10,
                            width: "100%",
                            overflow: "visible",
                          }}
                        >
                          <div className="absolute -top-2 right-4 flex gap-1">
                            {canStageOrDiscard && (
                              <>
                                <Button
                                  size={"icon-xs"}
                                  variant={"outline"}
                                  aria-label="Stage changes"
                                  onClick={async () => {
                                    try {
                                      await applyPatchBlock({
                                        ...payload,
                                        action: "Stage",
                                      });
                                    } catch {
                                      // handled by mutation toast
                                    }
                                  }}
                                >
                                  <Plus />
                                </Button>
                                <Button
                                  size={"icon-xs"}
                                  variant={"outline"}
                                  aria-label="Discard changes"
                                  onClick={async () => {
                                    try {
                                      await applyPatchBlock({
                                        ...payload,
                                        action: "Discard",
                                      });
                                    } catch {
                                      // handled by mutation toast
                                    }
                                  }}
                                >
                                  <Undo />
                                </Button>
                              </>
                            )}
                            {canUnstage && (
                              <Button
                                size={"icon-xs"}
                                variant={"outline"}
                                aria-label="Unstage changes"
                                onClick={async () => {
                                  try {
                                    await applyPatchBlock({
                                      ...payload,
                                      action: "Unstage",
                                    });
                                  } catch {
                                    // handled by mutation toast
                                  }
                                }}
                              >
                                <Minus />
                              </Button>
                            )}
                          </div>
                        </div>
                      );
                    }}
                  />
                </Virtualizer>
              </WorkerPoolContextProvider>
            </div>
          )}
        </>
      )}
    </div>
  );
};

type ChunkActionMetadata = {
  source: "worktree" | "stash" | "history";
  filePath: string;
  fileNewPath: string | null;
  stashReference: string | null;
  commitHash: string | null;
  hunkIndex: number;
  changeIndex: number;
  side: "additions" | "deletions";
  additions: {
    start: number | null;
    end: number | null;
    count: number;
  };
  deletions: {
    start: number | null;
    end: number | null;
    count: number;
  };
};

const FileLevelStatusBarLeft = ({
  resolvedSelection,
}: {
  resolvedSelection: ResolvedFileSelection;
}) => {
  if (resolvedSelection.state === "none") {
    return null;
  }

  const selectedFile =
    resolvedSelection.state === "valid"
      ? {
          filePath: resolvedSelection.file.path,
          fileNewPath: resolvedSelection.file.new_path,
          status: resolvedSelection.file.status,
        }
      : {
          filePath: resolvedSelection.identity.filePath,
          fileNewPath: resolvedSelection.identity.fileNewPath,
          status: undefined,
        };

  return (
    <div className="items-center h-full px-2 flex gap-2">
      {selectedFile?.status && selectedFile?.filePath ? (
        <>
          {getStatusIcon(selectedFile?.status)}
          <span className="group flex items-center">
            {renderPath(selectedFile?.filePath)}
            <div className="ml-1 opacity-0 group-hover:opacity-100 transition-opacity text-xs text-muted-foreground">
              <CopyButton
                size={"xs"}
                variant="ghost"
                text={selectedFile?.filePath || ""}
              />
            </div>
          </span>
        </>
      ) : null}
      {resolvedSelection.state === "stale" ? (
        <span className="text-xs text-amber-600 flex items-center gap-1">
          <CircleAlertIcon size={14} />
          Unavailable
        </span>
      ) : null}
      {selectedFile?.fileNewPath ? (
        <div>
          <MoveHorizontal
            className="text-muted-foreground opacity-70"
            size={16}
          />
        </div>
      ) : null}
      {selectedFile?.fileNewPath ? (
        <span>{renderPath(selectedFile.fileNewPath)}</span>
      ) : null}
    </div>
  );
};

const renderPath = (path: string) => {
  const parts = path.split("/");
  const fileName = parts.pop();
  const dir = parts.join("/");

  return (
    <span>
      {dir && (
        <>
          <span className="text-muted-foreground/75">{dir}/</span>
        </>
      )}
      {fileName}
    </span>
  );
};

const SettingsPopover = () => {
  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            size="icon-xs"
            variant="outline"
            className="relative"
            aria-label="Open notifications"
          />
        }
      >
        <Settings size={16} aria-hidden="true" />
      </PopoverTrigger>
      <SettingsPopoverContent />
    </Popover>
  );
};

const SettingsPopoverContent = () => {
  const { setDiffStyle, diffStyle, overflow, setOverflow } =
    useDiffViewerSettings();

  return (
    <PopoverContent className="fit mr-4 px-0 mt-0.5 w-96">
      <Label className="text-muted-foreground">Settings</Label>
      <Separator className={"mt-2 mb-3"} />
      <div className="flex flex-col gap-4 mt-1">
        <div>
          <Label className="flex items-center gap-2 mb-3">
            <Diff size={16} />
            Diff Style
          </Label>
          <div className="flex items-center justify-center ">
            <Group className="flex w-full h-full">
              <Button
                className={cn(
                  "rounded-none w-32 h-32! shadow-none first:rounded-s-md last:rounded-e-md focus-visible:z-10",
                  diffStyle === "unified" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                onClick={() => {
                  setDiffStyle("unified");
                }}
              >
                <UnifiedSVG />
              </Button>
              <GroupSeparator className="bg-primary/40" />
              <Button
                className={cn(
                  "rounded-none w-32 h-32! shadow-none rounded-r-md border-l-0 focus-visible:z-10",
                  diffStyle === "split" &&
                    "border-primary/40 bg-primary/10! hover:bg-primary/13!",
                )}
                variant="outline"
                onClick={() => {
                  setDiffStyle("split");
                }}
              >
                <SplitSVG />
              </Button>
            </Group>
          </div>
        </div>
        <Separator />
        <div className="flex justify-between">
          <Label htmlFor="wrapping" className="flex items-center gap-2">
            <TextWrap size={16} />
            Wrapping
          </Label>
          <Switch
            checked={overflow === "wrap"}
            onCheckedChange={(checked) => {
              setOverflow(checked ? "wrap" : "scroll");
            }}
            id="wrapping"
          />
        </div>
      </div>
    </PopoverContent>
  );
};

const EmptyStateScreen = () => {
  return (
    <div className="w-full flex justify-center max-h-[calc(var(--layout-height)---spacing(14))] h-full bg-background border-r">
      <div className="w-full h-full flex flex-col items-center justify-center -mt-20">
        <GitruBorderedSVG />
        <div className="flex flex-col gap-0.5 w-60 select-none">
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              Command Pannel
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>K</Kbd>
              </KbdGroup>
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              New Branch
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>⇧</Kbd>
                <Kbd>N</Kbd>
              </KbdGroup>
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-muted-foreground text-sm font-light">
              Pull Changes
            </span>
            <span>
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>⇧</Kbd>
                <Kbd>P</Kbd>
              </KbdGroup>
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
