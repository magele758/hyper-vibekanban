import { makeRequest } from '@/shared/lib/remoteApi';
import type {
  Autopilot,
  AutopilotRun,
  CreateAutopilotRequest,
  UpdateAutopilotRequest,
  ListInboxResponse,
  Squad,
  SquadMember,
  CreateSquadRequest,
  WebhookEndpoint,
  CreateWebhookEndpointRequest,
  FeishuBotBinding,
} from 'shared/remote-types';

export type { FeishuBotBinding };

// Prefer same-origin Vite proxy (`/agent-sidecar`) so the browser does not
// cross-origin fetch 127.0.0.1 (Private Network Access / silent failures).
const SIDECAR_BASE = (
  import.meta.env.VITE_AGENT_SIDECAR_BASE || '/agent-sidecar'
).replace(/\/$/, '');

export type AgentLlmSettings = {
  agent_id: string;
  has_api_key: boolean;
  base_url: string | null;
  model_name: string | null;
  updated_at: string;
};

export type CopilotSession = {
  id: string;
  project_id: string;
  agent_id: string | null;
  issue_id: string | null;
  title: string | null;
  external_agent_id: string | null;
  created_at: string;
  updated_at: string;
};

export type CopilotMessage = {
  id: string;
  session_id: string;
  role: string;
  content: string;
  created_at: string;
};

async function json<T>(res: Response): Promise<T> {
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`${res.status}: ${text}`);
  }
  return res.json() as Promise<T>;
}

export const boardAgentsApi = {
  async createAgent(body: {
    project_id: string;
    name: string;
    instructions?: string;
    default_executor?: string | null;
    max_concurrent_tasks?: number;
    chat_runtime?: 'cursor' | 'pi' | 'opencode';
    api_key?: string;
    base_url?: string;
    model_name?: string;
  }): Promise<{ id: string } & Record<string, unknown>> {
    const data = await json<{ data: { id: string } & Record<string, unknown> }>(
      await makeRequest('/v1/agents', {
        method: 'POST',
        body: JSON.stringify(body),
      })
    );
    return data.data;
  },

  async getLlmSettings(agentId: string): Promise<AgentLlmSettings> {
    return json(await makeRequest(`/v1/agents/${agentId}/llm_settings`));
  },

  async upsertLlmSettings(
    agentId: string,
    body: { api_key?: string; base_url?: string; model_name?: string }
  ): Promise<AgentLlmSettings> {
    return json(
      await makeRequest(`/v1/agents/${agentId}/llm_settings`, {
        method: 'PUT',
        body: JSON.stringify(body),
      })
    );
  },

  async listSessions(params: {
    project_id: string;
    agent_id?: string;
    project_copilot?: boolean;
  }): Promise<CopilotSession[]> {
    const q = new URLSearchParams({ project_id: params.project_id });
    if (params.agent_id) q.set('agent_id', params.agent_id);
    if (params.project_copilot) q.set('project_copilot', 'true');
    const data = await json<{ copilot_sessions: CopilotSession[] }>(
      await makeRequest(`/v1/copilot_sessions?${q}`)
    );
    return data.copilot_sessions;
  },

  async createSession(body: {
    project_id: string;
    agent_id?: string | null;
    title?: string;
  }): Promise<CopilotSession> {
    const data = await json<{ data: CopilotSession }>(
      await makeRequest('/v1/copilot_sessions', {
        method: 'POST',
        body: JSON.stringify(body),
      })
    );
    return data.data;
  },

  async listMessages(sessionId: string): Promise<CopilotMessage[]> {
    const data = await json<{ copilot_messages: CopilotMessage[] }>(
      await makeRequest(`/v1/copilot_messages?session_id=${sessionId}`)
    );
    return data.copilot_messages;
  },

  // ── Autopilots ────────────────────────────────────────────────────────────

  async listAutopilots(projectId: string): Promise<Autopilot[]> {
    const data = await json<{ autopilots: Autopilot[] }>(
      await makeRequest(`/v1/autopilots?project_id=${projectId}`)
    );
    return data.autopilots;
  },

  async createAutopilot(body: CreateAutopilotRequest): Promise<Autopilot> {
    const data = await json<{ data: Autopilot }>(
      await makeRequest('/v1/autopilots', {
        method: 'POST',
        body: JSON.stringify(body),
      })
    );
    return data.data;
  },

  async updateAutopilot(
    id: string,
    body: UpdateAutopilotRequest
  ): Promise<Autopilot> {
    const data = await json<{ data: Autopilot }>(
      await makeRequest(`/v1/autopilots/${id}`, {
        method: 'PUT',
        body: JSON.stringify(body),
      })
    );
    return data.data;
  },

  async deleteAutopilot(id: string): Promise<void> {
    await json<unknown>(
      await makeRequest(`/v1/autopilots/${id}`, { method: 'DELETE' })
    );
  },

  async triggerAutopilot(id: string): Promise<void> {
    await json<unknown>(
      await makeRequest(`/v1/autopilots/${id}/trigger`, { method: 'POST' })
    );
  },

  async listAutopilotRuns(autopilotId: string): Promise<AutopilotRun[]> {
    const data = await json<{ runs: AutopilotRun[] }>(
      await makeRequest(`/v1/autopilots/${autopilotId}/runs`)
    );
    return data.runs;
  },

  // ── Inbox ─────────────────────────────────────────────────────────────────

  async listInbox(params?: {
    include_archived?: boolean;
  }): Promise<ListInboxResponse> {
    const q = new URLSearchParams();
    if (params?.include_archived) q.set('include_archived', 'true');
    const qs = q.toString();
    return json<ListInboxResponse>(
      await makeRequest(`/v1/inbox${qs ? `?${qs}` : ''}`)
    );
  },

  async getInboxUnreadCount(): Promise<number> {
    const data = await json<{ unread_count: number }>(
      await makeRequest('/v1/inbox/unread-count')
    );
    return data.unread_count;
  },

  async markInboxRead(id: string): Promise<void> {
    const res = await makeRequest('/v1/inbox/mark-read', {
      method: 'POST',
      body: JSON.stringify({ ids: [id] }),
    });
    if (!res.ok) {
      throw new Error(`${res.status}: ${await res.text()}`);
    }
  },

  async archiveInboxItem(id: string): Promise<void> {
    const res = await makeRequest('/v1/inbox/archive', {
      method: 'POST',
      body: JSON.stringify({ ids: [id] }),
    });
    if (!res.ok) {
      throw new Error(`${res.status}: ${await res.text()}`);
    }
  },

  // ── Squads ────────────────────────────────────────────────────────────────

  async listSquads(projectId: string): Promise<Squad[]> {
    const data = await json<{ squads: Squad[] }>(
      await makeRequest(`/v1/squads?project_id=${projectId}`)
    );
    return data.squads;
  },

  async createSquad(body: CreateSquadRequest): Promise<Squad> {
    const data = await json<{ data: Squad }>(
      await makeRequest('/v1/squads', {
        method: 'POST',
        body: JSON.stringify(body),
      })
    );
    return data.data;
  },

  async deleteSquad(id: string): Promise<void> {
    await json<unknown>(
      await makeRequest(`/v1/squads/${id}`, { method: 'DELETE' })
    );
  },

  async listSquadMembers(squadId: string): Promise<SquadMember[]> {
    const data = await json<{ members: SquadMember[] }>(
      await makeRequest(`/v1/squads/${squadId}/members`)
    );
    return data.members;
  },

  async addSquadMember(
    squadId: string,
    body: { agent_id?: string; user_id?: string }
  ): Promise<SquadMember> {
    const data = await json<{ data: SquadMember }>(
      await makeRequest(`/v1/squads/${squadId}/members`, {
        method: 'POST',
        body: JSON.stringify(body),
      })
    );
    return data.data;
  },

  async removeSquadMember(squadId: string, memberId: string): Promise<void> {
    await json<unknown>(
      await makeRequest(`/v1/squads/${squadId}/members/${memberId}`, {
        method: 'DELETE',
      })
    );
  },

  // ── Webhooks ──────────────────────────────────────────────────────────────

  async listWebhookEndpoints(projectId: string): Promise<WebhookEndpoint[]> {
    const data = await json<{ endpoints: WebhookEndpoint[] }>(
      await makeRequest(`/v1/webhooks?project_id=${projectId}`)
    );
    return data.endpoints;
  },

  async createWebhookEndpoint(
    body: CreateWebhookEndpointRequest
  ): Promise<WebhookEndpoint> {
    const data = await json<{ data: WebhookEndpoint }>(
      await makeRequest('/v1/webhooks', {
        method: 'POST',
        body: JSON.stringify(body),
      })
    );
    return data.data;
  },

  async deleteWebhookEndpoint(id: string): Promise<void> {
    await json<unknown>(
      await makeRequest(`/v1/webhooks/${id}`, { method: 'DELETE' })
    );
  },

  async rotateWebhookToken(id: string): Promise<WebhookEndpoint> {
    const data = await json<{ data: WebhookEndpoint }>(
      await makeRequest(`/v1/webhooks/${id}/rotate-token`, { method: 'POST' })
    );
    return data.data;
  },

  // ── Feishu (Lark) ─────────────────────────────────────────────────────────

  async listFeishuBindings(projectId: string): Promise<FeishuBotBinding[]> {
    const data = await json<{ bindings: FeishuBotBinding[] }>(
      await makeRequest(`/v1/feishu/bindings?project_id=${projectId}`)
    );
    return data.bindings;
  },

  async createFeishuBinding(body: {
    project_id: string;
    agent_id: string;
    name?: string;
    app_id: string;
    app_secret: string;
    encrypt_key?: string;
    verification_token?: string;
    reply_on_complete?: boolean;
  }): Promise<FeishuBotBinding> {
    return json(
      await makeRequest('/v1/feishu/bindings', {
        method: 'POST',
        body: JSON.stringify(body),
      })
    );
  },

  async updateFeishuBinding(
    id: string,
    body: Record<string, unknown>
  ): Promise<FeishuBotBinding> {
    return json(
      await makeRequest(`/v1/feishu/bindings/${id}`, {
        method: 'PUT',
        body: JSON.stringify(body),
      })
    );
  },

  async deleteFeishuBinding(id: string): Promise<void> {
    await json<unknown>(
      await makeRequest(`/v1/feishu/bindings/${id}`, { method: 'DELETE' })
    );
  },

  async rotateFeishuCallbackToken(id: string): Promise<FeishuBotBinding> {
    return json(
      await makeRequest(`/v1/feishu/bindings/${id}/rotate-token`, {
        method: 'POST',
      })
    );
  },

  async chatStream(params: {
    project_id: string;
    session_id: string;
    agent_id?: string | null;
    message: string;
    onDelta?: (text: string) => void;
    onEvent?: (event: unknown) => void;
    onToolStart?: (toolName: string) => void;
    onToolResult?: (toolName: string, ok: boolean) => void;
    token: string;
  }): Promise<{ reply: string; external_agent_id?: string }> {
    const res = await fetch(`${SIDECAR_BASE}/copilot/chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${params.token}`,
      },
      body: JSON.stringify({
        project_id: params.project_id,
        session_id: params.session_id,
        agent_id: params.agent_id ?? null,
        message: params.message,
      }),
    });
    if (!res.ok || !res.body) {
      throw new Error(`sidecar ${res.status}: ${await res.text()}`);
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    let reply = '';
    let external_agent_id: string | undefined;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const chunks = buffer.split('\n\n');
      buffer = chunks.pop() ?? '';
      for (const chunk of chunks) {
        const line = chunk.split('\n').find((l) => l.startsWith('data: '));
        if (!line) continue;
        const payload = JSON.parse(line.slice(6)) as {
          type: string;
          text?: string;
          reply?: string;
          message?: string;
          external_agent_id?: string;
          event?: unknown;
          tool_name?: string;
          ok?: boolean;
        };
        if (payload.type === 'delta' && payload.text) {
          params.onDelta?.(payload.text);
          reply += payload.text;
        } else if (payload.type === 'event') {
          params.onEvent?.(payload.event);
        } else if (payload.type === 'tool_start') {
          params.onToolStart?.(payload.tool_name ?? '');
        } else if (payload.type === 'tool_result') {
          params.onToolResult?.(payload.tool_name ?? '', payload.ok !== false);
        } else if (payload.type === 'done') {
          reply = payload.reply || reply;
          external_agent_id = payload.external_agent_id;
        } else if (payload.type === 'error') {
          throw new Error(payload.message || 'sidecar error');
        }
      }
    }

    return { reply, external_agent_id };
  },
};
