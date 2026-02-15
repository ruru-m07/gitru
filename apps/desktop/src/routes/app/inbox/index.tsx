import { Button, buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { getStatusIcon } from "@/components/getStatusIcon";
import { useGitHistoryGraph } from "@/hooks";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  const { data, fetchNextPage, hasNextPage, isFetchingNextPage } =
    useGitHistoryGraph({ limit: 10 });

  const rows = data?.pages.flatMap((page) => page.rows) ?? [];

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


      <Button
        onClick={() => fetchNextPage()}
        disabled={!hasNextPage || isFetchingNextPage}
      >
        {isFetchingNextPage
          ? "Loading..."
          : hasNextPage
            ? "Load More"
            : "No More"}
      </Button>

      <Button
        onClick={() => {
          navigator.clipboard.writeText(JSON.stringify(rows, null, 2));
        }}
      >
        copy
      </Button>

      <div className="space-y-2">
        {rows.map((row) => {
          const primaryRef =
            row.heads[0] ||
            row.tags[0] ||
            row.remotes[0] ||
            row.stashes[0] ||
            row.refs[0];

          return (
            <div key={row.oid} className="border p-3 rounded">
              <div className="flex items-center gap-2 mb-1">
                <code className="text-xs text-muted-foreground">
                  {row.commit.id.slice(0, 7)}
                </code>
                <span className="font-medium">{row.commit.summary}</span>
              </div>
              <div className="flex items-center gap-4 text-sm text-muted-foreground">
                {primaryRef ? <span>{primaryRef.name}</span> : null}
                <span>{row.commit.authors.author.name}</span>
                <span>Lane {row.lane}</span>
                <span className="text-green-600">
                  +{row.commit.stats.insertions}
                </span>
                <span className="text-red-600">
                  -{row.commit.stats.deletions}
                </span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
