import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/app/pulls/")({
  component: RouteComponent,
});

function RouteComponent() {
  return <div>adsl</div>;
}
