import { RobotIcon } from '@phosphor-icons/react';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import { UserProvider } from '@/shared/providers/remote/UserProvider';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { LoginRequiredPrompt } from '@/shared/dialogs/shared/LoginRequiredPrompt';

function GlobalAgentsInner() {
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b border-border px-6 py-4">
        <div className="flex items-center gap-3">
          <RobotIcon className="size-5 text-brand" />
          <h1 className="text-lg font-semibold text-normal">全局指挥台</h1>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-4xl space-y-6">
          <div className="rounded-lg border border-border bg-secondary/40 p-6">
            <h2 className="mb-2 text-base font-medium text-normal">
              通过对话管理项目
            </h2>
            <p className="text-sm text-low">
              在此页面，您可以通过对话创建和管理
              Project、Issue、Workspace、Agent、Squad、Autopilot。
            </p>
            <p className="mt-2 text-sm text-low">
              Phase 2 将添加完整的聊天 UI 和工具集成。
            </p>
          </div>

          <div className="space-y-3">
            <h3 className="text-sm font-medium text-normal">快速操作</h3>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <button
                type="button"
                className="rounded-lg border border-border bg-primary p-4 text-left transition-colors hover:bg-secondary"
                disabled
              >
                <div className="text-sm font-medium text-normal">
                  创建 Issue
                </div>
                <div className="mt-1 text-xs text-low">即将推出</div>
              </button>
              <button
                type="button"
                className="rounded-lg border border-border bg-primary p-4 text-left transition-colors hover:bg-secondary"
                disabled
              >
                <div className="text-sm font-medium text-normal">运行 SOP</div>
                <div className="mt-1 text-xs text-low">即将推出</div>
              </button>
              <button
                type="button"
                className="rounded-lg border border-border bg-primary p-4 text-left transition-colors hover:bg-secondary"
                disabled
              >
                <div className="text-sm font-medium text-normal">
                  创建 Squad
                </div>
                <div className="mt-1 text-xs text-low">即将推出</div>
              </button>
              <button
                type="button"
                className="rounded-lg border border-border bg-primary p-4 text-left transition-colors hover:bg-secondary"
                disabled
              >
                <div className="text-sm font-medium text-normal">
                  管理 Autopilot
                </div>
                <div className="mt-1 text-xs text-low">即将推出</div>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export function GlobalAgentsPage() {
  const { isSignedIn } = useAuth();
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);

  if (!isSignedIn) {
    return <LoginRequiredPrompt />;
  }
  if (!selectedOrgId) {
    return null;
  }

  return (
    <OrgProvider organizationId={selectedOrgId}>
      <UserProvider>
        <GlobalAgentsInner />
      </UserProvider>
    </OrgProvider>
  );
}
