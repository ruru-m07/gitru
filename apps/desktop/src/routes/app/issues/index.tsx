import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/app/issues/")({
	component: RouteComponent,
});

function RouteComponent() {
	return <div>Hello "/app/issues/"!</div>;
}
