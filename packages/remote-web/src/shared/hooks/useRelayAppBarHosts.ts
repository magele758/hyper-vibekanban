import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import type { AppBarHost } from "@vibe/ui/components/AppBar";
import { listPairedRelayHosts } from "@/shared/lib/relayPairingStorage";
import { hostIsLinked, relayListStatus } from "@/shared/lib/relayHostStatus";
import { listRelayHosts, listServerHostLinks } from "@/shared/lib/remoteApi";

const RELAY_APP_BAR_HOSTS_QUERY_KEY = ["relay-app-bar-hosts", "hosts"] as const;
const RELAY_APP_BAR_PAIRED_HOSTS_QUERY_KEY = [
  "relay-app-bar-hosts",
  "paired-hosts",
] as const;
const RELAY_APP_BAR_SERVER_LINKS_QUERY_KEY = [
  "relay-app-bar-hosts",
  "server-links",
] as const;

interface UseRelayAppBarHostsResult {
  hosts: AppBarHost[];
  isLoading: boolean;
}

export interface ResolveRelayNavigationHostOptions {
  routeHostId?: string | null;
}

export function resolveRelayNavigationHostId(
  hosts: AppBarHost[],
  options?: ResolveRelayNavigationHostOptions,
): string | null {
  const routeHostId = options?.routeHostId ?? null;
  if (routeHostId) {
    const routeHost = hosts.find((host) => host.id === routeHostId);
    if (routeHost && routeHost.status !== "unpaired") {
      return routeHostId;
    }
  }

  const onlineHost = hosts.find((host) => host.status === "online");
  if (onlineHost) {
    return onlineHost.id;
  }

  const pairedHost = hosts.find((host) => host.status !== "unpaired");
  if (pairedHost) {
    return pairedHost.id;
  }

  return null;
}

export function useRelayAppBarHosts(
  enabled: boolean,
): UseRelayAppBarHostsResult {
  const hostsQuery = useQuery({
    queryKey: RELAY_APP_BAR_HOSTS_QUERY_KEY,
    queryFn: listRelayHosts,
    enabled,
    staleTime: 30_000,
    refetchInterval: 30_000,
  });

  const pairedHostsQuery = useQuery({
    queryKey: RELAY_APP_BAR_PAIRED_HOSTS_QUERY_KEY,
    queryFn: async () => {
      try {
        return await listPairedRelayHosts();
      } catch (error) {
        console.error("Failed to load paired relay hosts for app bar", error);
        return [];
      }
    },
    enabled,
    staleTime: 5_000,
    refetchInterval: 5_000,
  });

  const serverLinksQuery = useQuery({
    queryKey: RELAY_APP_BAR_SERVER_LINKS_QUERY_KEY,
    queryFn: async () => {
      try {
        return await listServerHostLinks();
      } catch (error) {
        console.error("Failed to load server host links for app bar", error);
        return [];
      }
    },
    enabled,
    staleTime: 5_000,
    refetchInterval: 5_000,
  });

  const hosts = useMemo<AppBarHost[]>(() => {
    if (!enabled) {
      return [];
    }

    const relayHosts = hostsQuery.data ?? [];
    const indexedHostIds = new Set(
      (pairedHostsQuery.data ?? []).map((host) => host.host_id),
    );
    const serverHostIds = new Set(
      (serverLinksQuery.data ?? []).map((host) => host.host_id),
    );

    return relayHosts.map((host) => ({
      id: host.id,
      name: host.name,
      status: relayListStatus(
        host.status,
        hostIsLinked(host.id, indexedHostIds, serverHostIds),
      ),
    }));
  }, [enabled, hostsQuery.data, pairedHostsQuery.data, serverLinksQuery.data]);

  return {
    hosts,
    isLoading: enabled && (hostsQuery.isLoading || pairedHostsQuery.isLoading),
  };
}
