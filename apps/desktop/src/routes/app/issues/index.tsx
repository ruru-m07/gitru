import { createFileRoute } from "@tanstack/react-router";
import { CollaborationWorkspace } from "@/features/collaboration/workspace";

export const Route = createFileRoute("/app/issues/")({
  component: RouteComponent,
});

function RouteComponent() {
  return <CollaborationWorkspace kind="issue" />;
}
