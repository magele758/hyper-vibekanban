import { createFileRoute } from '@tanstack/react-router';
import { ImportedProjectsPage } from '@/pages/imported/ImportedProjectsPage';

export const Route = createFileRoute('/_app/imported')({
  component: ImportedProjectsPage,
});
