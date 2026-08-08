import { createFileRoute } from '@tanstack/react-router';
import { WorkforcePage } from '@/pages/agents/WorkforcePage';

export const Route = createFileRoute('/_app/workforce')({
  component: WorkforcePage,
});
