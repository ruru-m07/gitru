import { Button, buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useGitFetch, useGitPush } from "@/hooks";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  const { mutateAsync: fetch } = useGitFetch();
  const { mutateAsync: push } = useGitPush();

  return (
    <div className="p-4 space-y-4">
      <Link className={buttonVariants()} to="/auth/onboarding">
        Go to Onboarding
      </Link>
      <br />
      <Button
        onClick={async () => {
          const data = await fetch();
          console.log(data);
        }}
      >
        fetch
      </Button>
      <Button
        onClick={async () => {
          const data = await push();
          console.log(data);
        }}
      >
        push
      </Button>
    </div>
  );
}
