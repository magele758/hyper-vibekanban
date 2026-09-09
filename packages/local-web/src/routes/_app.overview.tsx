import { createFileRoute } from '@tanstack/react-router';
import { ProjectsOverviewPageContainer } from '@/pages/projects/ProjectsOverviewPage';
import { LocalReposOverviewPage } from '@/pages/workspaces/LocalReposOverviewPage';
import { useUserSystem } from '@/shared/hooks/useUserSystem';

function OverviewRoute() {
  const { liteMode } = useUserSystem();
  if (liteMode) {
    return <LocalReposOverviewPage />;
  }
  return <ProjectsOverviewPageContainer />;
}

export const Route = createFileRoute('/_app/overview')({
  component: OverviewRoute,
});
