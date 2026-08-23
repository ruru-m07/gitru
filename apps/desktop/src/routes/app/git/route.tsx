import { createFileRoute } from "@tanstack/react-router";
import { GitPageLayout } from "@/features/git/components/git-page-layout";

export const Route = createFileRoute("/app/git")({
  component: GitPageLayout,
});
