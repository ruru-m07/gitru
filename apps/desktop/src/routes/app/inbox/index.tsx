import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import PageLayout from "@/components/pageLayout";
import { useGitHistoryGraph } from "@/hooks";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  const startRef = useRef<number | null>(null);
  const fetchCountRef = useRef(0);

  const [logs, setLogs] = useState<string[]>([]);

  const { data, isFetching, isFetchingNextPage, fetchNextPage } =
    useGitHistoryGraph({
      limit: 10000,
      include_local: true,
      include_remotes: true,
      include_tags: true,
      include_stash: true,
    });

  // START timer
  useEffect(() => {
    if (isFetching && startRef.current === null) {
      startRef.current = performance.now();
    }
  }, [isFetching]);

  // END timer
  useEffect(() => {
    if (!isFetching && startRef.current !== null) {
      const duration = performance.now() - startRef.current;

      fetchCountRef.current += 1;

      const label =
        fetchCountRef.current === 1
          ? "Initial load"
          : `Page ${fetchCountRef.current}`;

      const log = `${label}: ${duration.toFixed(2)} ms`;

      console.log(log);

      setLogs((prev) => [...prev, log]);

      // reset for next cycle
      startRef.current = null;
    }
  }, [isFetching]);

  const totalRows = data?.pages.flatMap((p) => p.rows).length ?? 0;

  return (
    <PageLayout className="overflow-y-auto p-4 space-y-4">
      {/* Data */}
      <div className="text-lg font-semibold">Rows: {totalRows}</div>

      {/* Button */}
      <button
        onClick={() => fetchNextPage()}
        disabled={isFetchingNextPage}
        className="px-4 py-2 bg-blue-500 text-white rounded disabled:opacity-50"
      >
        {isFetchingNextPage ? "Loading..." : "Fetch Next Page"}
      </button>

      {/* Logs */}
      <div className="mt-4">
        <div className="font-medium mb-2">Timing Logs:</div>
        <div className="bg-neutral-900/10 text-sm p-3 rounded space-y-1">
          {logs.map((log, i) => (
            <div key={i}>{log}</div>
          ))}
        </div>
      </div>
    </PageLayout>
  );
}
