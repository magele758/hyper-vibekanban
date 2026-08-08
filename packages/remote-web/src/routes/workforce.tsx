import { createFileRoute } from "@tanstack/react-router";
import { requireAuthenticated } from "@remote/shared/lib/route-auth";
import { WorkforcePage } from "@/pages/agents/WorkforcePage";

export const Route = createFileRoute("/workforce")({
  beforeLoad: async ({ location }) => {
    await requireAuthenticated(location);
  },
  component: WorkforcePage,
});
