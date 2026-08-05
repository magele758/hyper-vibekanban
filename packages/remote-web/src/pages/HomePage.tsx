import { useCallback, useEffect, useMemo, type ReactNode } from "react";
import { useNavigate, useSearch } from "@tanstack/react-router";
import { SettingsDialog } from "@/shared/dialogs/settings/SettingsDialog";
import { useOrganizationStore } from "@/shared/stores/useOrganizationStore";
import { useAuth } from "@/shared/hooks/auth/useAuth";
import { useIsMobile } from "@/shared/hooks/useIsMobile";
import { useRelayAppBarHosts } from "@remote/shared/hooks/useRelayAppBarHosts";
import { ProjectsOverviewPageContainer } from "@/pages/projects/ProjectsOverviewPage";

function getHostInitials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "??";
  const words = trimmed.split(/\s+/);
  if (words.length >= 2) {
    return (words[0][0] + words[1][0]).toUpperCase();
  }
  return trimmed.slice(0, 2).toUpperCase();
}

export default function HomePage() {
  const navigate = useNavigate();
  const search = useSearch({ from: "/" });
  const setSelectedOrgId = useOrganizationStore((s) => s.setSelectedOrgId);
  const { isSignedIn } = useAuth();
  const { hosts } = useRelayAppBarHosts(isSignedIn);
  const isMobile = useIsMobile();

  const openRelaySettings = useCallback((hostId?: string) => {
    void SettingsDialog.show({
      initialSection: "relay",
      ...(hostId ? { initialState: { hostId } } : {}),
    });
  }, []);

  useEffect(() => {
    const legacyOrgId = search.legacyOrgSettingsOrgId;
    if (!legacyOrgId) {
      return;
    }

    setSelectedOrgId(legacyOrgId);
    navigate({
      to: "/",
      search: {},
      replace: true,
    });

    void SettingsDialog.show({
      initialSection: "organizations",
      initialState: { organizationId: legacyOrgId },
    });
  }, [navigate, search.legacyOrgSettingsOrgId, setSelectedOrgId]);

  const mobileHosts = useMemo(() => {
    if (!isMobile || !isSignedIn) {
      return null;
    }

    return (
      <section className="mb-double">
        <h2 className="text-lg font-semibold text-high">Your Hosts</h2>
        {hosts.length === 0 ? (
          <div className="mt-base rounded-sm border border-border bg-secondary p-base text-center">
            <p className="text-sm text-low">No hosts linked yet</p>
            <button
              type="button"
              className="mt-base rounded-sm border border-border bg-primary px-base py-half text-sm font-medium text-normal hover:border-brand/60 hover:text-high"
              onClick={() => {
                openRelaySettings();
              }}
            >
              Link a host
            </button>
          </div>
        ) : (
          <div className="mt-base space-y-half">
            {hosts.map((host) => {
              const isOnline = host.status === "online";
              const isUnpaired = host.status === "unpaired";
              const isClickable = isOnline || isUnpaired;

              return (
                <button
                  key={host.id}
                  type="button"
                  disabled={!isClickable}
                  className={`flex w-full items-center gap-base rounded-sm border border-border bg-primary px-base py-base text-left transition-colors ${
                    isClickable
                      ? "hover:border-high/20 hover:bg-panel"
                      : "opacity-50"
                  }`}
                  onClick={() => {
                    if (isOnline) {
                      navigate({
                        to: "/hosts/$hostId/workspaces",
                        params: { hostId: host.id },
                      });
                    } else if (isUnpaired) {
                      openRelaySettings(host.id);
                    }
                  }}
                >
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-brand/15 text-xs font-semibold text-brand">
                    {getHostInitials(host.name)}
                  </div>
                  <span className="min-w-0 flex-1 truncate text-sm font-medium text-high">
                    {host.name}
                  </span>
                  <span
                    className={`h-2.5 w-2.5 shrink-0 rounded-full ${
                      isOnline
                        ? "bg-success"
                        : isUnpaired
                          ? "border border-warning bg-white"
                          : "bg-low"
                    }`}
                  />
                </button>
              );
            })}
            <button
              type="button"
              className="flex w-full items-center justify-center rounded-sm border border-dashed border-border px-base py-half text-sm text-low hover:border-brand/60 hover:text-normal"
              onClick={() => {
                openRelaySettings();
              }}
            >
              Link a host
            </button>
          </div>
        )}
      </section>
    );
  }, [hosts, isMobile, isSignedIn, navigate, openRelaySettings]);

  return (
    <ProjectsOverviewPageContainer topContent={mobileHosts as ReactNode} />
  );
}
