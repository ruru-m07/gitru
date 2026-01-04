import { Button, buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import { toast } from "sonner";
import { useGitFetch, useGitPull, useGitPush } from "@/hooks";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  const { mutateAsync: fetch } = useGitFetch();
  const { mutateAsync: push } = useGitPush();
  const { mutateAsync: pull } = useGitPull();

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
          toast.success(data.message);
        }}
      >
        fetch
      </Button>
      <Button
        onClick={async () => {
          const data = await push();
          console.log(data);
          toast.success(data.message);
        }}
      >
        push
      </Button>
      <Button
        onClick={async () => {
          const data = await pull();
          console.log(data);
          toast.success(data.message);
        }}
      >
        pull
      </Button>
    </div>
  );
}
