import { useEffect, useState } from 'react';
import { Button } from '@vibe/ui/components/Button';
import { Input } from '@vibe/ui/components/Input';
import { Label } from '@vibe/ui/components/Label';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import { Alert, AlertDescription } from '@vibe/ui/components/Alert';
import { create, useModal } from '@ebay/nice-modal-react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { defineModal } from '@/shared/lib/modals';
import { projectApi } from '@/shared/lib/api';
import { localProjectKeys } from '@/shared/hooks/useLocalProjects';
import type { Project } from 'shared/types';

export type CreateLocalProjectResult = {
  action: 'created' | 'canceled';
  project?: Project;
};

const CreateLocalProjectDialogImpl = create(() => {
  const modal = useModal();
  const { t } = useTranslation('common');
  const queryClient = useQueryClient();
  const [name, setName] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  useEffect(() => {
    if (modal.visible) {
      setName('');
      setError(null);
      setIsCreating(false);
    }
  }, [modal.visible]);

  const handleCreate = async () => {
    const trimmed = name.trim();
    if (trimmed.length < 2) {
      setError(t('lite.projects.nameTooShort'));
      return;
    }

    setError(null);
    setIsCreating(true);
    try {
      const project = await projectApi.create({ name: trimmed });
      await queryClient.invalidateQueries({ queryKey: localProjectKeys.all });
      modal.resolve({ action: 'created', project } as CreateLocalProjectResult);
      modal.hide();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : t('lite.projects.createError')
      );
      setIsCreating(false);
    }
  };

  const handleCancel = () => {
    modal.resolve({ action: 'canceled' } as CreateLocalProjectResult);
    modal.hide();
  };

  return (
    <Dialog
      open={modal.visible}
      onOpenChange={(open) => {
        if (!open && !isCreating) {
          handleCancel();
        }
      }}
    >
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('lite.projects.createTitle')}</DialogTitle>
          <DialogDescription>
            {t('lite.projects.createDescription')}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          <Label htmlFor="local-project-name">
            {t('lite.projects.nameLabel')}
          </Label>
          <Input
            id="local-project-name"
            value={name}
            onChange={(event) => {
              setName(event.target.value);
              setError(null);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && name.trim() && !isCreating) {
                event.preventDefault();
                void handleCreate();
              }
            }}
            placeholder={t('lite.projects.namePlaceholder')}
            autoFocus
          />
        </div>

        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={handleCancel}
            disabled={isCreating}
          >
            {t('lite.projects.cancel')}
          </Button>
          <Button
            type="button"
            onClick={() => void handleCreate()}
            disabled={isCreating || name.trim().length < 2}
          >
            {isCreating ? t('lite.projects.creating') : t('lite.projects.create')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});

export const CreateLocalProjectDialog = defineModal<
  void,
  CreateLocalProjectResult
>(CreateLocalProjectDialogImpl);
