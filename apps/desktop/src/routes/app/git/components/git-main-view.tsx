import HistoryGraph from "@/components/history-graph";
import { useAppStore } from "@/store/use-app-store";
import { DiffBoxBody } from "./diff-box-body";
import { EmptyStateScreen } from "./empty-state-screen";
import { MainActionBar } from "./main-action-bar";

export function GitMainView() {
  const mainWindowView = useAppStore((state) => state.mainWindowView);

  return (
    <>
      <MainActionBar />
      {mainWindowView === null && <EmptyStateScreen />}
      {mainWindowView === "FileDiff" && <DiffBoxBody />}
      {mainWindowView === "HistoryGraph" && <HistoryGraph />}
    </>
  );
}
