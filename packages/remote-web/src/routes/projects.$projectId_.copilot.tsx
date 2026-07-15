import { createFileRoute } from "@tanstack/react-router";
import { requireAuthenticated } from "@remote/shared/lib/route-auth";
import { ProjectCopilotPage } from "@/pages/agents/ProjectAgentChatPage";

export const Route = createFileRoute("/projects/$projectId_/copilot")({
  beforeLoad: async ({ location }) => {
    await requireAuthenticated(location);
  },
  component: ProjectCopilotPage,
});
