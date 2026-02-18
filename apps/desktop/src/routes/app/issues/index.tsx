import { createFileRoute } from "@tanstack/react-router";
import PageLayout from "@/components/pageLayout";

export const Route = createFileRoute("/app/issues/")({
  component: RouteComponent,
});

function RouteComponent() {
  return <PageLayout className="p-4">Cooking issues</PageLayout>;
}
