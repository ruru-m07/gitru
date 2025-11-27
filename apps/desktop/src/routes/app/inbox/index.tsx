import { Button } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { appState } from "@/state";
import {
  useBranches,
  useCurrentBranch,
  useRepository,
  useRepositoryActions,
  useStatus,
} from "@/state/hooks";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  // ✅ Clean! Just one line per query
  const repo = useRepository();
  const {
    data: statusData,
    isLoading: statusLoading,
    isFetching: statusFetching,
    dataUpdatedAt,
  } = useStatus();
  const { data: currentBranch, isLoading: branchLoading } = useCurrentBranch();
  const { data: branches } = useBranches();

  // ✅ Actions are also encapsulated - no need to check repo everywhere
  const actions = useRepositoryActions();

  return (
    <div className="p-4 space-y-4">
      <Link to="/auth/onboarding">Go to Onboarding</Link>

      {/* Repository Info */}
      <div className="p-4 border rounded-lg bg-gray-50 dark:bg-gray-800">
        <h2 className="text-lg font-semibold mb-2">Current Repository</h2>
        <p className="text-sm text-gray-600 dark:text-gray-400">
          {repo?.path ?? "No repository selected"}
        </p>
        {currentBranch && (
          <p className="text-sm mt-1">
            Branch:{" "}
            <span className="font-mono">{currentBranch.display_name}</span>
          </p>
        )}
      </div>

      {/* Status Section */}
      <div className="p-4 border rounded-lg">
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-lg font-semibold">Git Status</h2>
          <div className="flex items-center gap-2">
            {statusFetching && (
              <span className="text-xs text-blue-500">Fetching...</span>
            )}
            <span className="text-xs text-gray-400">
              Updated:{" "}
              {dataUpdatedAt
                ? new Date(dataUpdatedAt).toLocaleTimeString()
                : "Never"}
            </span>
          </div>
        </div>

        {statusLoading ? (
          <p>Loading status...</p>
        ) : statusData?.files?.length ? (
          <ul className="space-y-1 max-h-48 overflow-auto">
            {statusData.files.map((file) => {
              const statusStyle = file.status.some((s) =>
                s.includes("Modified"),
              )
                ? "bg-yellow-100 text-yellow-800"
                : file.status.some((s) => s.includes("New"))
                  ? "bg-green-100 text-green-800"
                  : file.status.some((s) => s.includes("Deleted"))
                    ? "bg-red-100 text-red-800"
                    : "bg-gray-100 text-gray-800";
              return (
                <li
                  key={file.path}
                  className="text-sm font-mono flex items-center gap-2"
                >
                  <span className={`px-1 rounded text-xs ${statusStyle}`}>
                    {file.status.join(", ")}
                  </span>
                  {file.path}
                </li>
              );
            })}
          </ul>
        ) : (
          <p className="text-gray-500">No changes detected</p>
        )}
      </div>

      {/* Action Buttons */}
      <div className="flex flex-wrap gap-2">
        {/* ✅ Using actions hook - cleaner API */}
        <Button
          onClick={async () => {
            console.log("Invalidating status...");
            await actions.invalidateStatus();
            console.log("Status invalidated! UI will refetch automatically.");
          }}
          disabled={!repo}
        >
          Invalidate Status
        </Button>

        {/* ✅ Fetch from remote */}
        <Button
          variant="outline"
          onClick={async () => {
            console.log("Fetching from remote...");
            await actions.fetch();
            console.log("Fetch complete!");
          }}
          disabled={!repo}
        >
          Git Fetch
        </Button>

        {/* ✅ Pull changes */}
        <Button
          variant="outline"
          onClick={async () => {
            console.log("Pulling changes...");
            const result = await actions.pull();
            console.log("Pull result:", result);
          }}
          disabled={!repo}
        >
          Git Pull
        </Button>

        {/* ✅ Push changes */}
        <Button
          variant="outline"
          onClick={async () => {
            console.log("Pushing changes...");
            const result = await actions.push();
            console.log("Push result:", result);
          }}
          disabled={!repo}
        >
          Git Push
        </Button>

        {/* ✅ Invalidate ALL repository state */}
        <Button
          variant="destructive"
          onClick={async () => {
            console.log("Invalidating all repository state...");
            await actions.invalidateAll();
            console.log("All repository state invalidated!");
          }}
          disabled={!repo}
        >
          Invalidate All
        </Button>

        {/* ✅ Global state reset */}
        <Button
          variant="destructive"
          onClick={async () => {
            console.log("Resetting entire app state...");
            await appState.invalidateAll();
            console.log("App state reset complete!");
          }}
        >
          Reset App State
        </Button>
      </div>

      {/* Branches Section */}
      <div className="p-4 border rounded-lg">
        <h2 className="text-lg font-semibold mb-2">Branches</h2>
        {branchLoading ? (
          <p>Loading branches...</p>
        ) : branches?.length ? (
          <div className="flex flex-wrap gap-2">
            {branches.map((branch) => (
              <Button
                key={branch.name}
                variant={
                  branch.name === currentBranch?.name ? "default" : "outline"
                }
                size="sm"
                onClick={async () => {
                  console.log("Checking out branch:", branch.name);
                  await actions.checkout(branch.name);
                }}
              >
                {branch.display_name}
                {branch.is_remote && " (remote)"}
              </Button>
            ))}
          </div>
        ) : (
          <p className="text-gray-500">No branches found</p>
        )}
      </div>

      {/* Debug: Show how the state object works */}
      <div className="p-4 border rounded-lg bg-gray-50 dark:bg-gray-800">
        <h2 className="text-lg font-semibold mb-2">
          State API - New Hooks (Recommended)
        </h2>
        <pre className="text-xs font-mono overflow-auto">
          {`// ✅ NEW: Clean hooks-based API (recommended)
const { data, isLoading } = useStatus();
const { data: branch } = useCurrentBranch();
const { data: branches } = useBranches();
const { data: diff } = useDiff("path/to/file.ts");
const { status, currentBranch, branches } = useRepositoryData();

// Actions hook - encapsulates all operations
const actions = useRepositoryActions();
await actions.pull();
await actions.push();
await actions.fetch();
await actions.checkout("feature-branch");
await actions.invalidateStatus();
await actions.invalidateAll();

// ─────────────────────────────────────────────
// Legacy: Direct state access (still works)
state.repository?.status.get()
state.repository?.branches.checkout("feature")
state.repositories.for("/path")
state.invalidateAll()
state.ui.sidebar.toggle()`}
        </pre>
      </div>
    </div>
  );
}
