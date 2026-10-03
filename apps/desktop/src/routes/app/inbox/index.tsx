import { createFileRoute } from "@tanstack/react-router";
import { CollaborationWorkspace } from "@/features/collaboration/workspace";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  return <CollaborationWorkspace kind="notification" />;
}
