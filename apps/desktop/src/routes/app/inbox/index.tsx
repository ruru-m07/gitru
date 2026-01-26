import { Button, buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { toast } from "sonner";
import { getStatusIcon } from "@/components/getStatusIcon";
import { useGitFetch, useGitPull, useGitPush } from "@/hooks";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  const { mutateAsync: fetch } = useGitFetch();
  const { mutateAsync: push } = useGitPush();
  const { mutateAsync: pull } = useGitPull();

  return (
    <div className="p-4 space-y-4">
      <Link className={buttonVariants()} to="/auth/onboarding">
        Go to Onboarding
      </Link>
      <br />
      <Button
        // onClick={() => {
        //   toast.promise(
        //     (async () => {
        //       await new Promise(requestAnimationFrame);
        //       return fetch();
        //     })(),
        //     {
        //       loading: "Fetching changes...",
        //       success: "Fetch completed",
        //       error: "Fetch error",
        //     },
        //   );
        // }}

        onClick={() => {
          toast.promise(fetch(), {
            loading: "Fetching changes...",
            success: (data) => {
              console.log(data);
              return data.message;
            },
            error: (err) => err ?? "Fetch error",
          });
        }}
      >
        fetch
      </Button>
      <Button
        onClick={async () => {
          const data = await push();
          console.log(data);
          toast.success(data);
        }}
      >
        push
      </Button>
      <Button
        onClick={async () => {
          const data = await pull();
          console.log(data);
          toast.success(data.message);
        }}
      >
        pull
      </Button>
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
