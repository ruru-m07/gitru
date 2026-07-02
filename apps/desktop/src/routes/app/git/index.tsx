import { createFileRoute } from "@tanstack/react-router";
import { FileLevelStatusBarLeft } from "./components/file-level-status-bar-left";
import { GitMainView } from "./components/git-main-view";

export const Route = createFileRoute("/app/git/")({
  component: GitMainView,
});

export { FileLevelStatusBarLeft };
