import { WorkerPoolContextProvider } from "@pierre/diffs/react";
import React from "react";
import { workerFactory } from "@/worker/workerFactory";
import { useDiffViewerSettings } from "./useDiffViewSettingStore";

const WorkerPoolProvider = ({ children }: { children: React.ReactElement }) => {
  const { lineDiffType } = useDiffViewerSettings();

  return (
    <WorkerPoolContextProvider
      poolOptions={{
        workerFactory,
      }}
      highlighterOptions={{
        // theme: { dark: "github-dark-high-contrast", light: "github-light" },
        theme: { dark: "vesper", light: "vesper-light" },
        lineDiffType,
      }}
    >
      {children as any}
    </WorkerPoolContextProvider>
  );
};

export default WorkerPoolProvider;
