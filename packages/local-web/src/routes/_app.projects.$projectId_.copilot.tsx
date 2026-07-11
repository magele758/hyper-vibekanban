import { createFileRoute } from '@tanstack/react-router';
import { ProjectCopilotPage } from '@/pages/agents/ProjectAgentChatPage';

export const Route = createFileRoute('/_app/projects/$projectId_/copilot')({
  component: ProjectCopilotPage,
});
