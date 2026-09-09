import { useQuery } from '@tanstack/react-query';
import { projectApi } from '@/shared/lib/api';

export const localProjectKeys = {
  all: ['local-projects'] as const,
  detail: (id: string) => ['local-projects', id] as const,
};

export function useLocalProjects(enabled = true) {
  return useQuery({
    queryKey: localProjectKeys.all,
    queryFn: () => projectApi.list(),
    enabled,
  });
}

export function useLocalProject(projectId: string | null) {
  return useQuery({
    queryKey: localProjectKeys.detail(projectId ?? ''),
    queryFn: () => projectApi.get(projectId as string),
    enabled: !!projectId,
  });
}
