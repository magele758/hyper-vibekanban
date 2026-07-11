import { createFileRoute } from '@tanstack/react-router';
import { ProjectInboxPage } from '@/pages/agents/ProjectInboxPage';

export const Route = createFileRoute('/_app/projects/$projectId_/inbox')({
  component: ProjectInboxPage,
});
