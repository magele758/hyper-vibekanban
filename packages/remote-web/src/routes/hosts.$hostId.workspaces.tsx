import { createFileRoute } from "@tanstack/react-router";
import { requireAuthenticated } from "@remote/shared/lib/route-auth";
import { WorkspacesLanding } from "@/pages/workspaces/WorkspacesLanding";
import { RemoteWorkspacesPageShell } from "@remote/pages/RemoteWorkspacesPageShell";

export const Route = createFileRoute("/hosts/$hostId/workspaces")({
  beforeLoad: async ({ location }) => {
    await requireAuthenticated(location);
  },
  component: WorkspacesRouteComponent,
});

function WorkspacesRouteComponent() {
  return (
    <RemoteWorkspacesPageShell>
      <WorkspacesLanding />
    </RemoteWorkspacesPageShell>
  );
}
