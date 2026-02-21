import { buttonVariants } from "@gitru/ui/components/button";
import { createFileRoute, Link } from "@tanstack/react-router";
import PageLayout from "@/components/pageLayout";

export const Route = createFileRoute("/app/inbox/")({
  component: RouteComponent,
});

function RouteComponent() {
  return (
    <PageLayout className="p-4">
      Cooking inbox
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
