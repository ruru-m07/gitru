import { GraphRow, historyGraph } from "@gitru/commands";
import { Button, buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";
import { getStatusIcon } from "@/components/getStatusIcon";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  const [data, setData] = useState<GraphRow[] | null>(null);

  const [nextCursor, setNextCursor] = useState<string | null>(null);

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
        onClick={async () => {
          const data = await historyGraph({
            repoPath: "/Users/ruru/Projects/gitru",
            // repoPath: "/Users/ruru/Projects/next.js",
            query: {
              limit: 10,
              cursor: nextCursor || undefined,
            },
          });

          console.log("historyGraph", data);
          setData((pre) => [...(pre || []), ...data.rows]);
          setNextCursor(data.cursor || null);
        }}
      >
        {nextCursor ? "Load More" : "Load History"}
      </Button>

      <div className="space-y-2">
        {data?.map((row) => (
          <div key={row.oid} className="border p-3 rounded">
            <div className="flex items-center gap-2 mb-1">
              <code className="text-xs text-muted-foreground">
                {row.short_oid}
              </code>
              <span className="font-medium">{row.message}</span>
            </div>
            <div className="flex items-center gap-4 text-sm text-muted-foreground">
              <span>{row.branches.join("---")}</span>
              <span>{row.author}</span>
              <span>Lane {row.lane}</span>
              <span className="text-green-600">+{row.insertions}</span>
              <span className="text-red-600">-{row.deletions}</span>
            </div>
          </div>
        ))}
      </div>

      {data && data.length > 0 && nextCursor && (
        <div className="text-center text-sm text-muted-foreground">
          Next cursor: {nextCursor.slice(0, 7)}
        </div>
      )}
    </div>
  );
}
