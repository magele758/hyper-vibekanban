import { useMemo, useState } from 'react';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { boardAgentsApi } from '@/shared/lib/boardAgentsApi';
import type { OrgAgentEntry } from 'shared/remote-types';
import type { AvailableCodingAgent } from 'shared/types';

type ChatRuntime = 'cursor' | 'pi' | 'opencode';

/**
 * Create an agent from the org-wide roster.
 *
 * Unlike the project-scoped form, the project must be chosen explicitly here:
 * agents are still owned by a project, this view just spans several.
 */
export function WorkforceCreatePanel({
  projects,
  installed,
  existingAgents,
  onCancel,
  onCreated,
}: {
  projects: { id: string; name: string }[];
  installed: AvailableCodingAgent[];
  existingAgents: OrgAgentEntry[];
  onCancel: () => void;
  onCreated: () => void;
}) {
  const [projectId, setProjectId] = useState(projects[0]?.id ?? '');
  const [name, setName] = useState('');
  const [instructions, setInstructions] = useState('');
  const [defaultExecutor, setDefaultExecutor] = useState('');
  const [chatRuntime, setChatRuntime] = useState<ChatRuntime>('cursor');
  const [reviewerAgentId, setReviewerAgentId] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // A reviewer must live in the same project, so the options track the picker.
  const reviewerOptions = useMemo(
    () => existingAgents.filter((agent) => agent.project_id === projectId),
    [existingAgents, projectId]
  );

  const handleCreate = async () => {
    if (!projectId || !name.trim()) {
      setError('请选择项目并填写名称');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await boardAgentsApi.createAgent({
        project_id: projectId,
        name: name.trim(),
        instructions: instructions.trim(),
        default_executor: defaultExecutor || null,
        max_concurrent_tasks: 1,
        chat_runtime: chatRuntime,
        reviewer_agent_id: reviewerAgentId || undefined,
      });
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mb-6 space-y-3 rounded-lg border border-border bg-secondary p-4">
      <h2 className="font-medium text-normal">新增员工</h2>

      <label className="block text-xs text-low">
        所属项目
        <select
          className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
          value={projectId}
          onChange={(e) => {
            setProjectId(e.target.value);
            // The previous reviewer may belong to another project.
            setReviewerAgentId('');
          }}
        >
          {projects.map((project) => (
            <option key={project.id} value={project.id}>
              {project.name}
            </option>
          ))}
        </select>
      </label>

      <input
        className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
        placeholder="名称"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <textarea
        className="w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
        placeholder="Instructions / 系统提示"
        rows={3}
        value={instructions}
        onChange={(e) => setInstructions(e.target.value)}
      />

      <label className="block text-xs text-low">
        干活用的 Coding Agent
        <select
          className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
          value={defaultExecutor}
          onChange={(e) => setDefaultExecutor(e.target.value)}
        >
          <option value="">使用本机默认</option>
          {installed.map((item) => (
            <option key={item.executor} value={item.executor}>
              {item.executor}
              {item.available ? '' : '（本机未检测到）'}
            </option>
          ))}
        </select>
      </label>

      <label className="block text-xs text-low">
        对话 Runtime
        <select
          className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
          value={chatRuntime}
          onChange={(e) => setChatRuntime(e.target.value as ChatRuntime)}
        >
          <option value="cursor">Cursor SDK（默认，已接入）</option>
          <option value="pi">Pi（规划中）</option>
          <option value="opencode">OpenCode（规划中）</option>
        </select>
      </label>

      <label className="block text-xs text-low">
        审查者（可选）
        <select
          className="mt-1 w-full rounded-md border border-border bg-primary px-3 py-2 text-sm"
          value={reviewerAgentId}
          onChange={(e) => setReviewerAgentId(e.target.value)}
        >
          <option value="">不自动审查</option>
          {reviewerOptions.map((agent) => (
            <option key={agent.id} value={agent.id}>
              {agent.name}
            </option>
          ))}
        </select>
        <p className="mt-1 text-[11px] text-low">
          该 Agent 任务成功完成后，自动派一个审查任务给这里选的 Agent。
        </p>
      </label>

      {error && <p className="text-sm text-destructive">{error}</p>}

      <div className="flex gap-2">
        <PrimaryButton disabled={busy} onClick={() => void handleCreate()}>
          {busy ? '创建中…' : '创建'}
        </PrimaryButton>
        <button
          type="button"
          className="rounded-md px-3 py-1.5 text-sm text-low"
          onClick={onCancel}
        >
          取消
        </button>
      </div>
    </div>
  );
}
