import { useCallback, useState } from 'react';
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import {
  CaretDownIcon,
  CaretRightIcon,
  DotsSixVerticalIcon,
  PencilSimpleIcon,
  TrashIcon,
  CheckIcon,
  XIcon,
} from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import type { ExecutorConfig, QueuedMessage } from 'shared/types';

interface SessionQueuePanelProps {
  messages: QueuedMessage[];
  onRemove: (itemId: string) => Promise<void>;
  onUpdate: (
    itemId: string,
    message: string,
    executorConfig: ExecutorConfig
  ) => Promise<void>;
  onReorder: (itemIds: string[]) => Promise<void>;
  onClear: () => Promise<void>;
  disabled?: boolean;
}

function previewText(message: string, max = 80): string {
  const compact = message.replace(/\s+/g, ' ').trim();
  if (compact.length <= max) return compact;
  return `${compact.slice(0, max)}…`;
}

function SortableQueueItem({
  message,
  disabled,
  onRemove,
  onSave,
}: {
  message: QueuedMessage;
  disabled?: boolean;
  onRemove: () => void;
  onSave: (text: string) => void;
}) {
  const { t } = useTranslation('tasks');
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(message.data.message);
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: message.id, disabled });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.6 : 1,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex items-start gap-base px-double py-base border-b last:border-b-0 bg-secondary/40"
    >
      <button
        type="button"
        className="mt-0.5 text-low hover:text-normal cursor-grab active:cursor-grabbing disabled:opacity-40"
        disabled={disabled || editing}
        aria-label={t('followUp.queue.reorder')}
        {...attributes}
        {...listeners}
      >
        <DotsSixVerticalIcon className="h-4 w-4" />
      </button>

      <div className="flex-1 min-w-0">
        {editing ? (
          <textarea
            className="w-full text-sm bg-background border rounded-md px-base py-base min-h-[72px] resize-y"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            disabled={disabled}
            autoFocus
          />
        ) : (
          <p className="text-sm text-normal whitespace-pre-wrap break-words">
            {previewText(message.data.message, 160)}
          </p>
        )}
      </div>

      <div className="flex items-center gap-1 shrink-0">
        {editing ? (
          <>
            <button
              type="button"
              className="p-1 text-low hover:text-normal disabled:opacity-40"
              disabled={disabled || !draft.trim()}
              onClick={() => {
                onSave(draft.trim());
                setEditing(false);
              }}
              title={t('followUp.queue.save')}
            >
              <CheckIcon className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="p-1 text-low hover:text-normal"
              disabled={disabled}
              onClick={() => {
                setDraft(message.data.message);
                setEditing(false);
              }}
              title={t('common:cancel', { defaultValue: 'Cancel' })}
            >
              <XIcon className="h-4 w-4" />
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className="p-1 text-low hover:text-normal disabled:opacity-40"
              disabled={disabled}
              onClick={() => {
                setDraft(message.data.message);
                setEditing(true);
              }}
              title={t('followUp.queue.edit')}
            >
              <PencilSimpleIcon className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="p-1 text-low hover:text-normal disabled:opacity-40"
              disabled={disabled}
              onClick={onRemove}
              title={t('followUp.queue.remove')}
            >
              <TrashIcon className="h-4 w-4" />
            </button>
          </>
        )}
      </div>
    </div>
  );
}

export function SessionQueuePanel({
  messages,
  onRemove,
  onUpdate,
  onReorder,
  onClear,
  disabled,
}: SessionQueuePanelProps) {
  const { t } = useTranslation('tasks');
  const [expanded, setExpanded] = useState(true);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;
      const ids = messages.map((m) => m.id);
      const oldIndex = ids.indexOf(String(active.id));
      const newIndex = ids.indexOf(String(over.id));
      if (oldIndex < 0 || newIndex < 0) return;
      const next = arrayMove(ids, oldIndex, newIndex);
      await onReorder(next);
    },
    [messages, onReorder]
  );

  if (messages.length === 0) return null;

  return (
    <div className="border-b bg-secondary/20">
      <div className="flex items-center gap-base px-double py-base">
        <button
          type="button"
          className="flex items-center gap-base flex-1 min-w-0 text-left"
          onClick={() => setExpanded((v) => !v)}
        >
          {expanded ? (
            <CaretDownIcon className="h-4 w-4 text-low shrink-0" />
          ) : (
            <CaretRightIcon className="h-4 w-4 text-low shrink-0" />
          )}
          <span className="text-sm text-normal font-medium">
            {t('followUp.queue.title', { count: messages.length })}
          </span>
        </button>
        <button
          type="button"
          className="text-xs text-low hover:text-normal disabled:opacity-40"
          disabled={disabled}
          onClick={() => onClear()}
        >
          {t('followUp.queue.clear')}
        </button>
      </div>

      {expanded && (
        <DndContext
          sensors={sensors}
          collisionDetection={closestCenter}
          onDragEnd={handleDragEnd}
        >
          <SortableContext
            items={messages.map((m) => m.id)}
            strategy={verticalListSortingStrategy}
          >
            {messages.map((message) => (
              <SortableQueueItem
                key={message.id}
                message={message}
                disabled={disabled}
                onRemove={() => onRemove(message.id)}
                onSave={(text) =>
                  onUpdate(message.id, text, message.data.executor_config)
                }
              />
            ))}
          </SortableContext>
        </DndContext>
      )}
    </div>
  );
}
