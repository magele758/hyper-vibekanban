import { createFileRoute } from '@tanstack/react-router';
import { GlobalAgentsPage } from '@/pages/agents/GlobalAgentsPage';

export const Route = createFileRoute('/_app/agents')({
  component: GlobalAgentsPage,
});
