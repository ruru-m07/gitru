import { createFileRoute } from "@tanstack/react-router";
import PageLayout from "@/components/pageLayout";
import { ResizableLayout } from "@/components/resizableLayout";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <PageLayout className="overflow-y-auto">
      <ResizableLayout
        rightPannelClassName="h-full"
        id="inbox-layout"
        minWidth={350}
        maxWidth={800}
      >
        <div className="p-4">Left</div>
        <div className="p-4">Right</div>
      </ResizableLayout>
    </PageLayout>
  );
}
