/**
 * Board Copilot tools — natural-language issue edits / assign suggestions.
 * Invoked when the assistant emits a fenced ```vk-tool JSON block.
 */

export type ToolContext = {
  projectId: string;
  auth: string;
  remoteApi: string;
  send: (payload: unknown) => void;
};

type ToolResult = { ok: boolean; summary: string; data?: unknown };

async function remoteFetch<T>(
  remoteApi: string,
  path: string,
  auth: string,
  init?: RequestInit
): Promise<T> {
  const res = await fetch(`${remoteApi}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      Authorization: auth,
      ...(init?.headers ?? {}),
    },
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Remote ${path} → ${res.status}: ${body}`);
  }
  return (await res.json()) as T;
}

export const TOOL_SYSTEM_PROMPT = `You are a board Copilot for Vibe Kanban (参谋，不是 coding agent).
You help clarify requirements, prioritize, edit issues, and suggest assigning Agents.

When you need to mutate the board, emit ONE fenced block exactly like:

\`\`\`vk-tool
{"tool":"update_issue","issue_id":"<uuid>","title":"...","description":"...","status_id":"..."}
\`\`\`

or

\`\`\`vk-tool
{"tool":"assign_agent","issue_id":"<uuid>","agent_id":"<uuid>"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"list_agents"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"list_issues","limit":20}
\`\`\`

or

\`\`\`vk-tool
{"tool":"create_issue","title":"...","description":"...","status_id":"<optional>"}
\`\`\`

Rules:
- Prefer asking clarifying questions before mutating.
- After a tool runs you will receive a tool result; then continue in natural language.
- Do NOT pretend to write code or open workspaces yourself — suggest assigning an Agent instead.
- Only use vk-tool blocks for real mutations; otherwise reply in Chinese or English as the user prefers.
`;

const VK_TOOL_RE = /```vk-tool\s*([\s\S]*?)```/i;

export function extractToolCall(
  text: string
): { tool: string; args: Record<string, unknown> } | null {
  const match = text.match(VK_TOOL_RE);
  if (!match) return null;
  try {
    const parsed = JSON.parse(match[1].trim()) as Record<string, unknown>;
    const tool = String(parsed.tool ?? '');
    if (!tool) return null;
    return { tool, args: parsed };
  } catch {
    return null;
  }
}

export async function executeTool(
  tool: string,
  args: Record<string, unknown>,
  ctx: ToolContext
): Promise<ToolResult> {
  ctx.send({ type: 'tool_start', tool, args });
  try {
    let result: ToolResult;
    switch (tool) {
      case 'list_agents':
        result = await listAgents(ctx);
        break;
      case 'list_issues':
        result = await listIssues(ctx, Number(args.limit ?? 20));
        break;
      case 'update_issue':
        result = await updateIssue(ctx, args);
        break;
      case 'assign_agent':
        result = await assignAgent(ctx, args);
        break;
      case 'create_issue':
        result = await createIssue(ctx, args);
        break;
      default:
        result = { ok: false, summary: `Unknown tool: ${tool}` };
    }
    ctx.send({ type: 'tool_result', tool, ...result });
    return result;
  } catch (err) {
    const summary = err instanceof Error ? err.message : String(err);
    ctx.send({ type: 'tool_result', tool, ok: false, summary });
    return { ok: false, summary };
  }
}

async function listAgents(ctx: ToolContext): Promise<ToolResult> {
  const data = await remoteFetch<{ agents: Array<{ id: string; name: string }> }>(
    ctx.remoteApi,
    `/v1/agents?project_id=${ctx.projectId}`,
    ctx.auth
  );
  const agents = data.agents ?? [];
  return {
    ok: true,
    summary: `Found ${agents.length} agents`,
    data: agents.map((a) => ({ id: a.id, name: a.name })),
  };
}

async function listIssues(ctx: ToolContext, limit: number): Promise<ToolResult> {
  const data = await remoteFetch<{
    issues: Array<{
      id: string;
      simple_id: string;
      title: string;
      status_id: string;
    }>;
  }>(ctx.remoteApi, `/v1/issues?project_id=${ctx.projectId}`, ctx.auth);
  const issues = (data.issues ?? []).slice(0, Math.max(1, Math.min(limit, 50)));
  return {
    ok: true,
    summary: `Found ${issues.length} issues`,
    data: issues,
  };
}

async function updateIssue(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const issueId = String(args.issue_id ?? '');
  if (!issueId) return { ok: false, summary: 'issue_id required' };
  const body: Record<string, unknown> = {};
  if (typeof args.title === 'string') body.title = args.title;
  if (typeof args.description === 'string') body.description = args.description;
  if (typeof args.status_id === 'string') body.status_id = args.status_id;
  if (Object.keys(body).length === 0) {
    return { ok: false, summary: 'no fields to update' };
  }
  const data = await remoteFetch<{ data?: { id: string; title?: string } }>(
    ctx.remoteApi,
    `/v1/issues/${issueId}`,
    ctx.auth,
    { method: 'PATCH', body: JSON.stringify(body) }
  );
  return {
    ok: true,
    summary: `Updated issue ${issueId}`,
    data: data.data ?? data,
  };
}

async function assignAgent(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const issueId = String(args.issue_id ?? '');
  const agentId = String(args.agent_id ?? '');
  if (!issueId || !agentId) {
    return { ok: false, summary: 'issue_id and agent_id required' };
  }
  const data = await remoteFetch<{ data?: { id: string } }>(
    ctx.remoteApi,
    `/v1/issue_assignees`,
    ctx.auth,
    {
      method: 'POST',
      body: JSON.stringify({
        issue_id: issueId,
        agent_id: agentId,
      }),
    }
  );
  return {
    ok: true,
    summary: `Assigned agent ${agentId} to issue ${issueId} (task enqueued)`,
    data: data.data ?? data,
  };
}

async function createIssue(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const title = String(args.title ?? '').trim();
  if (!title) return { ok: false, summary: 'title required' };

  let statusId = typeof args.status_id === 'string' ? args.status_id : '';
  if (!statusId) {
    const statuses = await remoteFetch<{
      project_statuses: Array<{ id: string; sort_order: number }>;
    }>(
      ctx.remoteApi,
      `/v1/project_statuses?project_id=${ctx.projectId}`,
      ctx.auth
    );
    const sorted = [...(statuses.project_statuses ?? [])].sort(
      (a, b) => a.sort_order - b.sort_order
    );
    statusId = sorted[0]?.id ?? '';
  }
  if (!statusId) return { ok: false, summary: 'no project status available' };

  const data = await remoteFetch<{ data?: { id: string; simple_id?: string } }>(
    ctx.remoteApi,
    `/v1/issues`,
    ctx.auth,
    {
      method: 'POST',
      body: JSON.stringify({
        project_id: ctx.projectId,
        status_id: statusId,
        title,
        description: typeof args.description === 'string' ? args.description : null,
        priority: null,
        start_date: null,
        target_date: null,
        completed_at: null,
        sort_order: 0,
        parent_issue_id: null,
        parent_issue_sort_order: null,
        extension_metadata: {},
      }),
    }
  );
  return {
    ok: true,
    summary: `Created issue ${data.data?.simple_id ?? data.data?.id ?? ''}`,
    data: data.data ?? data,
  };
}
