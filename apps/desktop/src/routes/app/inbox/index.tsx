import { requestDiff, useTauriEvent } from "@gitru/commands";
import { Button, buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import PageLayout from "@/components/pageLayout";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  useTauriEvent<string>("diff_event", (payload) => {
    console.log(payload);
  });

  return (
    <PageLayout className="p-4">
      Cooking inbox
      <Button
        onClick={async () => {
          const dataa = await requestDiff({
            filePath: "packages/commands/src/types.ts",
          });
          console.log(dataa);
        }}
      >
        requestDiff
      </Button>
      <Link
        className={buttonVariants({
          className: "mt-4 w-fit",
        })}
        to="/auth/onboarding"
      >
        Go to Onboarding
      </Link>
    </PageLayout>
  );
}
