import { createFileRoute } from "@tanstack/react-router";
import { requireAuthenticated } from "@remote/shared/lib/route-auth";
import { GlobalAgentsPage } from "@/pages/agents/GlobalAgentsPage";

export const Route = createFileRoute("/agents")({
  beforeLoad: async ({ location }) => {
    await requireAuthenticated(location);
  },
  component: GlobalAgentsPage,
});
