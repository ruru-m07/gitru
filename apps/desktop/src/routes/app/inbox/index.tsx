import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <div className="p-4 space-y-4">
      <Link to="/auth/onboarding">Go to Onboarding</Link>
    </div>
  );
}
