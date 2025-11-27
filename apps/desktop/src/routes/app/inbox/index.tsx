import { Button } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { history } from "@/tauri";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <div>
      <Link to="/auth/onboarding">Go to Onboarding</Link>

      <Button
        onClick={async () => {
          const data = await history({
            repoPath: "/Users/ruru/Projects/next.js",
            limit: 100,
            skip: 0,
          });
          console.log({
            data,
          });
        }}
      >
        load history
      </Button>
    </div>
  );
}
