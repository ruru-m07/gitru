import { createFileRoute } from "@tanstack/react-router";
import { LinkedCollaborationRoute } from "@/features/collaboration/linked-collaboration-route";
import {
  localLinkTarget,
  parseLocalLinkSearch,
} from "@/features/collaboration/local-link-navigation";

export const Route = createFileRoute("/app/issues/")({
  validateSearch: parseLocalLinkSearch,
  component: RouteComponent,
});

function RouteComponent() {
  const search = Route.useSearch();
  return (
    <LinkedCollaborationRoute
      kind="issue"
      target={localLinkTarget(search)}
      invalid={search.invalidLocalLink}
    />
  );
}
