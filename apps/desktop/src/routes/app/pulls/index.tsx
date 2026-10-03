import { createFileRoute } from "@tanstack/react-router";
import { CollaborationWorkspace } from "@/features/collaboration/workspace";

export const Route = createFileRoute("/app/pulls/")({
  component: RouteComponent,
});

function RouteComponent() {
  return <CollaborationWorkspace kind="pull_request" />;
}
