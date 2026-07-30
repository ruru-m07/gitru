import { createFileRoute } from "@tanstack/react-router";
import { GitMainView } from "./components/git-main-view";

export const Route = createFileRoute("/app/git/")({
  component: GitMainView,
});
