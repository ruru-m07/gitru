import { createFileRoute } from "@tanstack/react-router";
import Progress from "@/components/progress";

export const Route = createFileRoute("/progress/$")({
  component: Progress,
});
