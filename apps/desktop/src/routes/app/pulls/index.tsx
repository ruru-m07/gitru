import { createFileRoute } from "@tanstack/react-router";
import PageLayout from "@/components/page-layout";

export const Route = createFileRoute("/app/pulls/")({
  component: RouteComponent,
});

// TODO: Implement pull request browsing and review UI.
function RouteComponent() {
  return <PageLayout className="p-4">Cooking pulls</PageLayout>;
}
