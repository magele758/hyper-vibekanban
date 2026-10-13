import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useAppRuntime } from '@/shared/hooks/useAppRuntime';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useRemoteCloudHostsAppBarModel } from '@/shared/hooks/useRemoteCloudHosts';
import type { CreateModeHostChoice } from '@/shared/lib/createModeHostPickerModel';
import { listPairedRelayHosts } from '@/shared/lib/relayPairingStorage';
import { listRelayHosts } from '@/shared/lib/remoteApi';

/**
 * Hosts the create picker can run on.
 * Desktop reads the local server's paired machines.
 * The data service page reads hosts paired in this browser, with live status
 * from the remote host list. It does not offer the data service itself.
 */
export function useCreateModeHosts(): {
  hosts: CreateModeHostChoice[];
  isLoading: boolean;
} {
  const runtime = useAppRuntime();
  const { isSignedIn } = useAuth();
  const isRemote = runtime === 'remote';
  const localModel = useRemoteCloudHostsAppBarModel({ enabled: !isRemote });

  const relayHostsQuery = useQuery({
    queryKey: ['create-mode-hosts', 'relay-hosts'],
    queryFn: listRelayHosts,
    enabled: isRemote && isSignedIn,
    staleTime: 30_000,
  });

  const pairedHostsQuery = useQuery({
    queryKey: ['create-mode-hosts', 'paired-hosts'],
    queryFn: async () => {
      try {
        return await listPairedRelayHosts();
      } catch {
        return [];
      }
    },
    enabled: isRemote && isSignedIn,
    staleTime: 5_000,
  });

  const hosts = useMemo<CreateModeHostChoice[]>(() => {
    if (!isRemote) {
      return localModel.hosts.map((host) => ({
        id: host.id,
        name: host.name,
        status: host.status,
      }));
    }

    const pairedHostIds = new Set(
      (pairedHostsQuery.data ?? []).map((host) => host.host_id)
    );

    return (relayHostsQuery.data ?? [])
      .filter((host) => pairedHostIds.has(host.id))
      .map((host) => ({
        id: host.id,
        name: host.name,
        status: host.status === 'online' ? 'online' : 'offline',
      }));
  }, [isRemote, localModel.hosts, pairedHostsQuery.data, relayHostsQuery.data]);

  return {
    hosts,
    isLoading:
      isRemote && (relayHostsQuery.isLoading || pairedHostsQuery.isLoading),
  };
}
