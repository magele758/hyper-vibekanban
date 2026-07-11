import { createFileRoute } from '@tanstack/react-router';
import { ProjectAgentDetailPage } from '@/pages/agents/ProjectAgentChatPage';

export const Route = createFileRoute(
  '/_app/projects/$projectId_/agents_/$agentId'
)({
  component: ProjectAgentDetailPage,
});
