import { createFileRoute } from "@tanstack/react-router";
import { requireAuthenticated } from "@remote/shared/lib/route-auth";
import { ProjectInboxPage } from "@/pages/agents/ProjectInboxPage";

export const Route = createFileRoute("/projects/$projectId_/inbox")({
  beforeLoad: async ({ location }) => {
    await requireAuthenticated(location);
  },
  component: ProjectInboxPage,
});
