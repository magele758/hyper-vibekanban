import { createFileRoute } from "@tanstack/react-router";
import { requireAuthenticated } from "@remote/shared/lib/route-auth";
import { ProjectAgentsPage } from "@/pages/agents/ProjectAgentsPage";

export const Route = createFileRoute("/projects/$projectId_/agents")({
  beforeLoad: async ({ location }) => {
    await requireAuthenticated(location);
  },
  component: ProjectAgentsPage,
});
