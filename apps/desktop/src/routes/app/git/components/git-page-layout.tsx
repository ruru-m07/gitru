import PageLayout from "@/components/pageLayout";
import StatusBar from "@/components/statusBar";
import { selectActiveRepository, useAppStore } from "@/store/useAppStore";
import { NoRepositoryScreen } from "./no-repository-screen";
import { ResizableArea } from "./resizable-area";

export function GitPageLayout() {
  const activeRepository = useAppStore(selectActiveRepository);

  if (!activeRepository) {
    return <NoRepositoryScreen />;
  }

  return (
    <PageLayout className="flex-col flex justify-between">
      <ResizableArea />
      <StatusBar />
    </PageLayout>
  );
}