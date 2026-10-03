import {
  collaboration,
  collaborationErrorMessage,
} from "@gitru/collaboration-client";
import { useCollaborationVersion } from "@gitru/collaboration-client/react";
import { useQuery } from "@tanstack/react-query";
import PageLayout from "@/components/page-layout";
import {
  type LocalLinkRouteTarget,
  localLinkRequest,
  sameLocalLinkTarget,
} from "./local-link-navigation";
import { CollaborationWorkspace } from "./workspace";

export function LinkedCollaborationRoute({
  kind,
  target,
  invalid,
}: {
  kind: "pull_request" | "issue";
  target?: LocalLinkRouteTarget;
  invalid?: boolean;
}) {
  if (invalid)
    return (
      <PageLayout>
        <p role="alert" className="p-5">
          This linked repository target is incomplete. Open the link again from
          Local Git.
        </p>
      </PageLayout>
    );
  return target ? (
    <ValidatedLinkedRoute
      key={JSON.stringify(target)}
      kind={kind}
      target={target}
    />
  ) : (
    <CollaborationWorkspace kind={kind} />
  );
}
function ValidatedLinkedRoute({
  kind,
  target,
}: {
  kind: "pull_request" | "issue";
  target: LocalLinkRouteTarget;
}) {
  const version = useCollaborationVersion();
  const query = useQuery({
    queryKey: [
      "collaboration",
      "local-links",
      target.local_repository_id,
      "navigation",
      target,
      version,
    ],
    queryFn: async ({ signal }) => {
      const receipt = await collaboration.validateLocalNavigation(
        localLinkRequest(target),
        signal,
      );
      if (!sameLocalLinkTarget(target, receipt)) throw { code: "stale_view" };
      return receipt;
    },
    networkMode: "always",
    retry: false,
    staleTime: Infinity,
    gcTime: 0,
    refetchInterval: false,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
  });
  if (query.isPending)
    return (
      <PageLayout>
        <p role="status" className="p-5">
          Validating the saved local repository link…
        </p>
      </PageLayout>
    );
  if (query.isError)
    return (
      <PageLayout>
        <p role="alert" className="p-5">
          {collaborationErrorMessage(query.error)} Open the link again from
          Local Git; another account or repository will not be substituted.
        </p>
      </PageLayout>
    );
  return <CollaborationWorkspace kind={kind} target={target} />;
}
