import { createFileRoute } from "@tanstack/react-router";
import { requireAuthenticated } from "@remote/shared/lib/route-auth";
import { ProjectAgentDetailPage } from "@/pages/agents/ProjectAgentChatPage";

export const Route = createFileRoute("/projects/$projectId_/agents_/$agentId")({
  beforeLoad: async ({ location }) => {
    await requireAuthenticated(location);
  },
  component: ProjectAgentDetailPage,
});
