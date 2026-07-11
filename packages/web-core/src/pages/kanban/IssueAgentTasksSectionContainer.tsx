import { useMemo } from 'react';
import { RobotIcon } from '@phosphor-icons/react';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { CollapsibleSectionHeader } from '@vibe/ui/components/CollapsibleSectionHeader';
import type { AgentTask, AgentTaskStatus } from 'shared/remote-types';
import { cn } from '@/shared/lib/utils';

interface IssueAgentTasksSectionContainerProps {
  issueId: string;
}

const STATUS_LABEL: Record<AgentTaskStatus, string> = {
  queued: '排队中',
  dispatched: '已派发',
  running: '执行中',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
};

function statusClass(status: AgentTaskStatus): string {
  switch (status) {
    case 'completed':
      return 'text-normal';
    case 'failed':
    case 'cancelled':
      return 'text-destructive';
    case 'running':
    case 'dispatched':
      return 'text-brand';
    default:
      return 'text-low';
  }
}

function sortTasks(a: AgentTask, b: AgentTask): number {
  return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
}

export function IssueAgentTasksSectionContainer({
  issueId,
}: IssueAgentTasksSectionContainerProps) {
  const { agentTasks, agents } = useProjectContext();

  const tasks = useMemo(
    () => agentTasks.filter((t) => t.issue_id === issueId).sort(sortTasks),
    [agentTasks, issueId]
  );

  const agentsById = useMemo(() => {
    const map = new Map(agents.map((a) => [a.id, a]));
    return map;
  }, [agents]);

  if (tasks.length === 0) {
    return null;
  }

  return (
    <CollapsibleSectionHeader
      title={`Agent 任务 (${tasks.length})`}
      persistKey="kanban-issue-agent-tasks"
      defaultExpanded
    >
      <ul className="space-y-2 border-t px-4 py-3">
        {tasks.map((task) => {
          const agent = agentsById.get(task.agent_id);
          return (
            <li
              key={task.id}
              className="rounded-md border border-border bg-secondary px-3 py-2 text-sm"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="flex min-w-0 items-center gap-1.5 font-medium text-normal">
                  <RobotIcon className="size-4 shrink-0 text-brand" />
                  <span className="truncate">
                    {agent?.name ?? task.agent_id.slice(0, 8)}
                  </span>
                </span>
                <span className={cn('shrink-0 text-xs', statusClass(task.status))}>
                  {STATUS_LABEL[task.status] ?? task.status}
                </span>
              </div>
              <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-low">
                <span>触发: {task.trigger}</span>
                <span>
                  尝试 {task.attempt}/{task.max_attempts}
                </span>
                {task.local_workspace_id && (
                  <span className="truncate">
                    workspace: {task.local_workspace_id.slice(0, 8)}…
                  </span>
                )}
              </div>
              {task.failure_reason && (
                <p className="mt-1 line-clamp-3 text-xs text-destructive">
                  {task.failure_reason}
                </p>
              )}
            </li>
          );
        })}
      </ul>
    </CollapsibleSectionHeader>
  );
}
