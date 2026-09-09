import { createFileRoute } from '@tanstack/react-router';
import { ProjectsOverviewPageContainer } from '@/pages/projects/ProjectsOverviewPage';
import { LocalProjectsOverviewPage } from '@/pages/projects/LocalProjectsOverviewPage';
import { useUserSystem } from '@/shared/hooks/useUserSystem';

function OverviewRoute() {
  const { liteMode } = useUserSystem();
  if (liteMode) {
    return <LocalProjectsOverviewPage />;
  }
  return <ProjectsOverviewPageContainer />;
}

export const Route = createFileRoute('/_app/overview')({
  component: OverviewRoute,
});
