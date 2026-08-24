import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from './KeyboardDialog';
import { Button } from './Button';
import { Checkbox } from './Checkbox';
import NiceModal, { useModal } from '@ebay/nice-modal-react';
import {
  ArchiveIcon,
  GitBranchIcon,
  TrashIcon,
  WarningIcon,
} from '@phosphor-icons/react';
import { defineModal } from '../lib/modals';
import { cn } from '../lib/cn';

export interface DeleteWorkspaceDialogProps {
  branchName: string;
  hasOpenPR?: boolean;
  /** True for InPlace/Console workspaces whose working tree is the user's repo. */
  usesRepoWorkingTree?: boolean;
}

export type DeleteWorkspaceMode = 'archive' | 'delete';

export type DeleteWorkspaceDialogResult = {
  action: 'archived' | 'deleted' | 'canceled';
  deleteBranches?: boolean;
};

const DeleteWorkspaceDialogImpl = NiceModal.create<DeleteWorkspaceDialogProps>(
  ({ branchName, hasOpenPR = false, usesRepoWorkingTree = false }) => {
    const modal = useModal();
    const { t } = useTranslation();
    const [mode, setMode] = useState<DeleteWorkspaceMode>('archive');
    const [deleteBranches, setDeleteBranches] = useState(false);

    const canDeleteBranches = !hasOpenPR;

    const handleConfirm = () => {
      if (mode === 'archive') {
        modal.resolve({ action: 'archived' } as DeleteWorkspaceDialogResult);
      } else {
        modal.resolve({
          action: 'deleted',
          deleteBranches: canDeleteBranches && deleteBranches,
        } as DeleteWorkspaceDialogResult);
      }
      modal.hide();
    };

    const handleCancel = () => {
      modal.resolve({ action: 'canceled' } as DeleteWorkspaceDialogResult);
      modal.hide();
    };

    return (
      <Dialog open={modal.visible} onOpenChange={handleCancel}>
        <DialogContent className="sm:max-w-[480px]">
          <DialogHeader>
            <div className="flex items-center gap-3">
              <WarningIcon className="h-6 w-6 text-destructive" />
              <DialogTitle>
                {t('workspaces.deleteDialog.title', 'Remove workspace')}
              </DialogTitle>
            </div>
            <DialogDescription className="text-left pt-2">
              {t(
                'workspaces.deleteDialog.description',
                'Archive to free disk space while keeping history, or permanently delete this workspace.'
              )}
            </DialogDescription>
          </DialogHeader>

          <div
            role="radiogroup"
            aria-label={t('workspaces.deleteDialog.title', 'Remove workspace')}
            className="py-4 space-y-2"
          >
            <button
              type="button"
              role="radio"
              aria-checked={mode === 'archive'}
              onClick={() => setMode('archive')}
              className={cn(
                'w-full text-left rounded border p-3 space-y-1 transition-colors',
                mode === 'archive'
                  ? 'border-foreground bg-accent/40'
                  : 'border-input hover:bg-accent/20'
              )}
            >
              <div className="flex items-center gap-2 text-sm font-medium">
                <ArchiveIcon className="h-4 w-4 shrink-0" />
                {t('workspaces.deleteDialog.archiveLabel', 'Archive workspace')}
                <span className="text-xs font-normal text-muted-foreground">
                  {t('workspaces.deleteDialog.recommended', 'Recommended')}
                </span>
              </div>
              <p className="text-xs text-muted-foreground pl-6">
                {usesRepoWorkingTree
                  ? t(
                      'workspaces.deleteDialog.archiveDescriptionInPlace',
                      'Hide from the active list and keep conversation history. The repository working tree is not removed.'
                    )
                  : t(
                      'workspaces.deleteDialog.archiveDescription',
                      'Release the local worktree. Conversation history and session logs are kept, and the workspace can be restored from its git branch.'
                    )}
              </p>
            </button>

            <button
              type="button"
              role="radio"
              aria-checked={mode === 'delete'}
              onClick={() => setMode('delete')}
              className={cn(
                'w-full text-left rounded border p-3 space-y-1 transition-colors',
                mode === 'delete'
                  ? 'border-destructive bg-destructive/5'
                  : 'border-input hover:bg-accent/20'
              )}
            >
              <div className="flex items-center gap-2 text-sm font-medium">
                <TrashIcon className="h-4 w-4 shrink-0" />
                {t('workspaces.deleteDialog.deleteLabel', 'Delete permanently')}
              </div>
              <p className="text-xs text-muted-foreground pl-6">
                {t(
                  'workspaces.deleteDialog.deleteDescription',
                  'Erase the workspace, conversation history, session logs, and optionally the git branch. This cannot be undone.'
                )}
              </p>
            </button>
          </div>

          {mode === 'delete' && (
            <div className="flex flex-col gap-1 pb-2">
              <div
                className={`flex items-center gap-3 text-sm font-medium select-none ${
                  canDeleteBranches
                    ? 'cursor-pointer'
                    : 'text-muted-foreground cursor-not-allowed'
                }`}
                onClick={() => {
                  if (canDeleteBranches) setDeleteBranches((v) => !v);
                }}
              >
                <Checkbox
                  checked={deleteBranches}
                  disabled={!canDeleteBranches}
                />
                <span className="flex items-center gap-2">
                  <GitBranchIcon className="h-4 w-4" />
                  {t(
                    'workspaces.deleteDialog.deleteBranchLabel',
                    'Delete branch'
                  )}{' '}
                  <code className="rounded bg-muted px-1 py-0.5 text-xs font-mono">
                    {branchName}
                  </code>
                </span>
              </div>
              {hasOpenPR && (
                <p className="text-xs text-muted-foreground pl-7">
                  {t(
                    'workspaces.deleteDialog.cannotDeleteOpenPr',
                    'Cannot delete branch while PR is open'
                  )}
                </p>
              )}
            </div>
          )}

          <DialogFooter className="gap-2">
            <Button variant="outline" onClick={handleCancel}>
              {t('buttons.cancel')}
            </Button>
            <Button
              variant={mode === 'delete' ? 'destructive' : 'default'}
              onClick={handleConfirm}
            >
              {mode === 'archive'
                ? t('workspaces.deleteDialog.confirmArchive', 'Archive')
                : t(
                    'workspaces.deleteDialog.confirmDelete',
                    'Delete permanently'
                  )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const DeleteWorkspaceDialog = defineModal<
  DeleteWorkspaceDialogProps,
  DeleteWorkspaceDialogResult
>(DeleteWorkspaceDialogImpl);
