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
You help clarify requirements, prioritize, edit issues, and suggest assigning Agents/Squads/Autopilots.

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

or

\`\`\`vk-tool
{"tool":"list_squads"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"create_squad","name":"...","leader_agent_id":"<uuid>","working_directory":"<optional>","issue_id":"<optional>"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"run_squad","squad_id":"<uuid>","issue_id":"<optional>","working_directory":"<optional>"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"approve_squad_run","run_id":"<uuid>","decision":"approve|reject|comment","comment":"<optional>"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"list_autopilots"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"create_autopilot","name":"...","agent_id":"<optional>","squad_id":"<optional>","cron_expression":"<optional>","issue_title_template":"<optional>"}
\`\`\`

or

\`\`\`vk-tool
{"tool":"trigger_autopilot","autopilot_id":"<uuid>"}
\`\`\`

Rules:
- Prefer asking clarifying questions before mutating.
- After a tool runs you will receive a tool result; then continue in natural language.
- Do NOT pretend to write code or open workspaces yourself — suggest assigning an Agent/Squad instead.
- Only use vk-tool blocks for real mutations; otherwise reply in Chinese or English as the user prefers.
- Squads are multi-agent pipelines; Autopilots are scheduled automations; use them for complex workflows.
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
      case 'list_squads':
        result = await listSquads(ctx);
        break;
      case 'create_squad':
        result = await createSquad(ctx, args);
        break;
      case 'run_squad':
        result = await runSquad(ctx, args);
        break;
      case 'approve_squad_run':
        result = await approveSquadRun(ctx, args);
        break;
      case 'list_autopilots':
        result = await listAutopilots(ctx);
        break;
      case 'create_autopilot':
        result = await createAutopilot(ctx, args);
        break;
      case 'trigger_autopilot':
        result = await triggerAutopilot(ctx, args);
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

async function listSquads(ctx: ToolContext): Promise<ToolResult> {
  const data = await remoteFetch<{ squads: Array<{ id: string; name: string }> }>(
    ctx.remoteApi,
    `/v1/squads?project_id=${ctx.projectId}`,
    ctx.auth
  );
  return {
    ok: true,
    summary: `Found ${data.squads.length} squads`,
    data: data.squads,
  };
}

async function createSquad(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const name = String(args.name ?? '').trim();
  if (!name) return { ok: false, summary: 'name required' };

  const body: Record<string, unknown> = {
    project_id: ctx.projectId,
    name,
  };
  if (typeof args.leader_agent_id === 'string') {
    body.leader_agent_id = args.leader_agent_id;
  }
  if (typeof args.working_directory === 'string') {
    body.working_directory = args.working_directory;
  }
  if (typeof args.issue_id === 'string') {
    body.issue_id = args.issue_id;
  }

  const data = await remoteFetch<{ data: { id: string; name: string } }>(
    ctx.remoteApi,
    `/v1/squads`,
    ctx.auth,
    {
      method: 'POST',
      body: JSON.stringify(body),
    }
  );
  return {
    ok: true,
    summary: `Created squad ${data.data.name} (${data.data.id})`,
    data: data.data,
  };
}

async function runSquad(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const squadId = String(args.squad_id ?? '');
  if (!squadId) return { ok: false, summary: 'squad_id required' };

  const body: Record<string, unknown> = {};
  if (typeof args.issue_id === 'string') {
    body.issue_id = args.issue_id;
  }
  if (typeof args.working_directory === 'string') {
    body.working_directory = args.working_directory;
  }
  if (typeof args.start_from_node_id === 'string') {
    body.start_from_node_id = args.start_from_node_id;
  }
  if (typeof args.resume_run_id === 'string') {
    body.resume_run_id = args.resume_run_id;
  }

  const data = await remoteFetch<{ run_id: string; status?: string }>(
    ctx.remoteApi,
    `/v1/squads/${squadId}/run`,
    ctx.auth,
    {
      method: 'POST',
      body: JSON.stringify(body),
    }
  );
  return {
    ok: true,
    summary: `Squad run started: ${data.run_id}`,
    data,
  };
}

async function approveSquadRun(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const runId = String(args.run_id ?? '');
  const decision = String(args.decision ?? '');
  if (!runId || !decision) {
    return { ok: false, summary: 'run_id and decision required' };
  }

  const body: Record<string, unknown> = { decision };
  if (typeof args.comment === 'string') {
    body.comment = args.comment;
  }

  const data = await remoteFetch<{ run: { id: string; status?: string } }>(
    ctx.remoteApi,
    `/v1/squad-runs/${runId}/approve`,
    ctx.auth,
    {
      method: 'POST',
      body: JSON.stringify(body),
    }
  );
  return {
    ok: true,
    summary: `Squad run ${runId} ${decision}d`,
    data,
  };
}

async function listAutopilots(ctx: ToolContext): Promise<ToolResult> {
  const data = await remoteFetch<{ autopilots: Array<{ id: string; name: string }> }>(
    ctx.remoteApi,
    `/v1/autopilots?project_id=${ctx.projectId}`,
    ctx.auth
  );
  return {
    ok: true,
    summary: `Found ${data.autopilots.length} autopilots`,
    data: data.autopilots,
  };
}

async function createAutopilot(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const name = String(args.name ?? '').trim();
  if (!name) return { ok: false, summary: 'name required' };

  const body: Record<string, unknown> = {
    project_id: ctx.projectId,
    name,
  };
  if (typeof args.agent_id === 'string') {
    body.agent_id = args.agent_id;
  }
  if (typeof args.squad_id === 'string') {
    body.squad_id = args.squad_id;
  }
  if (typeof args.cron_expression === 'string') {
    body.cron_expression = args.cron_expression;
  }
  if (typeof args.issue_title_template === 'string') {
    body.issue_title_template = args.issue_title_template;
  }
  if (typeof args.issue_description_template === 'string') {
    body.issue_description_template = args.issue_description_template;
  }

  const data = await remoteFetch<{ data: { id: string; name: string } }>(
    ctx.remoteApi,
    `/v1/autopilots`,
    ctx.auth,
    {
      method: 'POST',
      body: JSON.stringify(body),
    }
  );
  return {
    ok: true,
    summary: `Created autopilot ${data.data.name} (${data.data.id})`,
    data: data.data,
  };
}

async function triggerAutopilot(
  ctx: ToolContext,
  args: Record<string, unknown>
): Promise<ToolResult> {
  const autopilotId = String(args.autopilot_id ?? '');
  if (!autopilotId) return { ok: false, summary: 'autopilot_id required' };

  await remoteFetch<unknown>(
    ctx.remoteApi,
    `/v1/autopilots/${autopilotId}/trigger`,
    ctx.auth,
    {
      method: 'POST',
      body: JSON.stringify({}),
    }
  );
  return {
    ok: true,
    summary: `Autopilot ${autopilotId} triggered`,
  };
}
