import { createFileRoute } from "@tanstack/react-router";
import Roadmap from "@/components/roadmap";

export const Route = createFileRoute("/roadmap/$")({
  component: Roadmap,
});
