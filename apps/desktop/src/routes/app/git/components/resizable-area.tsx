import { cn } from "@gitru/ui/lib/utils";
import { Outlet } from "@tanstack/react-router";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useState } from "react";
import { ResizableLayout } from "@/components/resizable-layout";
import {
  selectActiveRepoSelectIsOpen,
  selectActiveRepository,
  selectActiveSessionRepoKey,
  useAppStore,
} from "@/store/use-app-store";
import {
  DEFAULT_STATUS_FILTERS,
  type FileStatusFilter,
} from "../lib/file-status-filters";
import { HistoryDetailView } from "./history-detail-view";
import { ListFileChanges } from "./list-file-changes";
import { ListRepositories } from "./list-repositories";
import { StashPocView } from "./stash-poc-view";
import { TogglePanelButton } from "./toggle-panel-button";

export const ResizableArea = () => {
  const repoSelectIsOpen = useAppStore(selectActiveRepoSelectIsOpen);
  const setRepoSelectIsOpen = useAppStore((state) => state.setRepoSelectIsOpen);
  const activeRepository = useAppStore(selectActiveRepository);
  const repoStateKey = useAppStore(selectActiveSessionRepoKey);
  const setGitViewStateForRepo = useAppStore(
    (state) => state.setGitViewStateForRepo,
  );

  const shouldReduceMotion = useReducedMotion();
  const [panelDirection, setPanelDirection] = useState<1 | -1>(1);
  const repoPath = activeRepository?.path ?? "";
  const repoGitViewState = useAppStore((state) =>
    repoStateKey ? state.gitViewByRepo[repoStateKey] : undefined,
  );
  const gitViewState: {
    leftPanelView: "changes" | "stash" | "history";
    changesTab: "changes" | "history";
    stashViewMode: "branch" | "all";
    selectedStashReference: string | null;
    selectedHistoryCommitHash: string | null;
    stashStatusFilters: Record<FileStatusFilter, boolean>;
  } = repoPath
    ? (repoGitViewState ?? {
        leftPanelView: "changes",
        changesTab: "changes",
        stashViewMode: "branch",
        selectedStashReference: null,
        selectedHistoryCommitHash: null,
        stashStatusFilters: DEFAULT_STATUS_FILTERS,
      })
    : {
        leftPanelView: "changes",
        changesTab: "changes",
        stashViewMode: "branch",
        selectedStashReference: null,
        selectedHistoryCommitHash: null,
        stashStatusFilters: DEFAULT_STATUS_FILTERS,
      };

  const panelSlideVariants = {
    initial: (direction: 1 | -1) => ({
      x: shouldReduceMotion ? 0 : direction > 0 ? 10 : -10,
      opacity: shouldReduceMotion ? 1 : 0.94,
    }),
    animate: {
      x: 0,
      opacity: 1,
    },
    exit: (direction: 1 | -1) => ({
      x: shouldReduceMotion ? 0 : direction > 0 ? -4 : 4,
      opacity: shouldReduceMotion ? 1 : 0.97,
    }),
  };

  const panelTransition = shouldReduceMotion
    ? { duration: 0.06, ease: "linear" as const }
    : { duration: 0.14, ease: [0.32, 0.72, 0, 1] as const };

  return (
    <div className="flex h-full">
      <ResizableLayout id="local-git-layout" minWidth={350} maxWidth={800}>
        <div className="flex flex-col h-full">
          <TogglePanelButton />
          <div className="h-full border-t max-h-[calc(var(--layout-height)---spacing(13.75))] relative overflow-hidden">
            {repoSelectIsOpen ? (
              <div className="absolute inset-0 bg-background">
                <ListRepositories />
              </div>
            ) : (
              <AnimatePresence
                mode="sync"
                initial={false}
                custom={panelDirection}
              >
                {gitViewState.leftPanelView === "stash" ? (
                  <motion.div
                    key="stash"
                    className="absolute inset-0 bg-background will-change-transform"
                    custom={panelDirection}
                    variants={panelSlideVariants}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    transition={panelTransition}
                  >
                    <StashPocView
                      mode={gitViewState.stashViewMode}
                      onBack={() => {
                        setPanelDirection(-1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "changes",
                            changesTab: "changes",
                          },
                          repoPath,
                        );
                      }}
                    />
                  </motion.div>
                ) : gitViewState.leftPanelView === "history" ? (
                  <motion.div
                    key="history"
                    className="absolute inset-0 bg-background will-change-transform"
                    custom={panelDirection}
                    variants={panelSlideVariants}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    transition={panelTransition}
                  >
                    <HistoryDetailView
                      onBack={() => {
                        setPanelDirection(-1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "changes",
                            changesTab: "history",
                          },
                          repoPath,
                        );
                      }}
                    />
                  </motion.div>
                ) : (
                  <motion.div
                    key="changes"
                    className="absolute inset-0 bg-background will-change-transform"
                    custom={panelDirection}
                    variants={panelSlideVariants}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    transition={panelTransition}
                  >
                    <ListFileChanges
                      activeTab={gitViewState.changesTab}
                      onTabChange={(nextTab) => {
                        setGitViewStateForRepo(
                          { changesTab: nextTab },
                          repoPath,
                        );
                      }}
                      onOpenHistoryView={(commitHash) => {
                        setPanelDirection(1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "history",
                            changesTab: "history",
                            selectedHistoryCommitHash: commitHash,
                          },
                          repoPath,
                        );
                      }}
                      onOpenStashView={(stashReference) => {
                        setPanelDirection(1);
                        setGitViewStateForRepo(
                          {
                            leftPanelView: "stash",
                            changesTab: "changes",
                            stashViewMode: "branch",
                            selectedStashReference: stashReference,
                          },
                          repoPath,
                        );
                      }}
                    />
                  </motion.div>
                )}
              </AnimatePresence>
            )}
          </div>
        </div>
        <div
          className={cn(
            "relative w-(--right-width) h-(--layout-height)",
            repoSelectIsOpen && "cursor-pointer",
          )}
          onClick={() => setRepoSelectIsOpen(false)}
        >
          {repoSelectIsOpen && (
            <div className="absolute inset-0 bg-background/40 z-10 w-full h-full backdrop-blur-[2px]"></div>
          )}
          <Outlet />
        </div>
      </ResizableLayout>
    </div>
  );
};
