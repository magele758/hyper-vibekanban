import { createFileRoute } from '@tanstack/react-router';
import { ProjectsOverviewPageContainer } from '@/pages/projects/ProjectsOverviewPage';

export const Route = createFileRoute('/_app/overview')({
  component: ProjectsOverviewPageContainer,
});
