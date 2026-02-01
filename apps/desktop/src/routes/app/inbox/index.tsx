import { buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { getStatusIcon } from "@/components/getStatusIcon";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <div className="p-4 space-y-4">
      <Link className={buttonVariants()} to="/auth/onboarding">
        Go to Onboarding
      </Link>
      <br />
      <ul>
        <li className="flex">
          {getStatusIcon(["IndexModified", "WorktreeModified"])}
          {" : "} modifed
        </li>
        <li className="flex">
          {getStatusIcon(["IndexNew", "WorktreeNew"])}
          {" : "} new
        </li>
        <li className="flex">
          {getStatusIcon(["IndexDeleted", "WorktreeDeleted"])}
          {" : "} deleted
        </li>
        <li className="flex">
          {getStatusIcon([
            "IndexRenamed",
            "WorktreeRenamed",
            "IndexTypechange",
            "WorktreeTypechange",
          ])}
          {" : "} renamed
        </li>
        <li className="flex">
          {getStatusIcon(["WorktreeUnreadable"])}
          {" : "} unreadable
        </li>
        <li className="flex">
          {getStatusIcon(["abc" as any])}
          {" : "} unknown
        </li>
      </ul>

      <div className="size-10 bg-primary dark:bg-popover"></div>
    </div>
  );
}
