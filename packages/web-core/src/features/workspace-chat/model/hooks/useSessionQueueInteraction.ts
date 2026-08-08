import { useCallback } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { queueApi } from '@/shared/lib/api';
import type {
  DraftFollowUpData,
  ExecutorConfig,
  QueuedMessage,
  QueueStatus,
} from 'shared/types';

interface UseSessionQueueInteractionOptions {
  sessionId: string | undefined;
}

interface UseSessionQueueInteractionResult {
  /** Queued messages in order */
  messages: QueuedMessage[];
  /** Whether the queue has at least one message */
  isQueued: boolean;
  /** Whether a queue operation is in progress */
  isQueueLoading: boolean;
  /** Append a message to the queue */
  queueMessage: (
    message: string,
    executorConfig: ExecutorConfig
  ) => Promise<void>;
  /** Clear the entire queue */
  clearQueue: () => Promise<void>;
  /** Remove a single queued message */
  removeMessage: (itemId: string) => Promise<void>;
  /** Update a queued message */
  updateMessage: (
    itemId: string,
    message: string,
    executorConfig: ExecutorConfig
  ) => Promise<void>;
  /** Reorder the queue */
  reorderMessages: (itemIds: string[]) => Promise<void>;
  /** Refresh queue status from server */
  refreshQueueStatus: () => Promise<void>;
}

const QUEUE_STATUS_KEY = 'queue-status';

function messagesFromStatus(status: QueueStatus): QueuedMessage[] {
  return status.status === 'queued' ? status.messages : [];
}

/**
 * Hook to manage multi-item queue interaction for session follow-ups.
 */
export function useSessionQueueInteraction({
  sessionId,
}: UseSessionQueueInteractionOptions): UseSessionQueueInteractionResult {
  const queryClient = useQueryClient();

  const { data: queueStatus = { status: 'empty' as const }, refetch } =
    useQuery<QueueStatus>({
      queryKey: [QUEUE_STATUS_KEY, sessionId],
      queryFn: () => queueApi.getStatus(sessionId!),
      enabled: !!sessionId,
    });

  const messages = messagesFromStatus(queueStatus);
  const isQueued = messages.length > 0;

  const setStatus = useCallback(
    (status: QueueStatus) => {
      queryClient.setQueryData([QUEUE_STATUS_KEY, sessionId], status);
    },
    [queryClient, sessionId]
  );

  const queueMutation = useMutation({
    mutationFn: (data: DraftFollowUpData) => queueApi.queue(sessionId!, data),
    onSuccess: setStatus,
  });

  const clearMutation = useMutation({
    mutationFn: () => queueApi.clear(sessionId!),
    onSuccess: setStatus,
  });

  const removeMutation = useMutation({
    mutationFn: (itemId: string) => queueApi.remove(sessionId!, itemId),
    onSuccess: setStatus,
  });

  const updateMutation = useMutation({
    mutationFn: ({
      itemId,
      data,
    }: {
      itemId: string;
      data: DraftFollowUpData;
    }) => queueApi.update(sessionId!, itemId, data),
    onSuccess: setStatus,
  });

  const reorderMutation = useMutation({
    mutationFn: (itemIds: string[]) => queueApi.reorder(sessionId!, itemIds),
    onSuccess: setStatus,
  });

  const queueMessage = useCallback(
    async (message: string, executorConfig: ExecutorConfig) => {
      if (!sessionId) return;
      await queueMutation.mutateAsync({
        message,
        executor_config: executorConfig,
      });
    },
    [sessionId, queueMutation]
  );

  const clearQueue = useCallback(async () => {
    if (!sessionId) return;
    await clearMutation.mutateAsync();
  }, [sessionId, clearMutation]);

  const removeMessage = useCallback(
    async (itemId: string) => {
      if (!sessionId) return;
      await removeMutation.mutateAsync(itemId);
    },
    [sessionId, removeMutation]
  );

  const updateMessage = useCallback(
    async (itemId: string, message: string, executorConfig: ExecutorConfig) => {
      if (!sessionId) return;
      await updateMutation.mutateAsync({
        itemId,
        data: { message, executor_config: executorConfig },
      });
    },
    [sessionId, updateMutation]
  );

  const reorderMessages = useCallback(
    async (itemIds: string[]) => {
      if (!sessionId) return;
      await reorderMutation.mutateAsync(itemIds);
    },
    [sessionId, reorderMutation]
  );

  const refreshQueueStatus = useCallback(async () => {
    if (!sessionId) return;
    await refetch();
  }, [sessionId, refetch]);

  return {
    messages,
    isQueued,
    isQueueLoading:
      queueMutation.isPending ||
      clearMutation.isPending ||
      removeMutation.isPending ||
      updateMutation.isPending ||
      reorderMutation.isPending,
    queueMessage,
    clearQueue,
    removeMessage,
    updateMessage,
    reorderMessages,
    refreshQueueStatus,
  };
}
