import { createFileRoute } from '@tanstack/react-router';
import { ProjectAgentsPage } from '@/pages/agents/ProjectAgentsPage';

export const Route = createFileRoute('/_app/projects/$projectId_/agents')({
  component: ProjectAgentsPage,
});
