import { Button } from "@gitru/ui/components/button";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
	component: App,
});

function App() {
	return (
		<div>
			<Button
				onClick={() => {
					window.location.href = "/app";
				}}
			>
				go to /app
			</Button>
			<Button
				onClick={() => {
					window.location.href = "/auth/onboarding";
				}}
			>
				go to /auth/onboarding
			</Button>
		</div>
	);
}
