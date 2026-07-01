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
import { WarningIcon, FolderIcon } from '@phosphor-icons/react';
import { defineModal } from '../lib/modals';

export interface DeletableWorkspaceSummary {
  id: string;
  name: string | null;
  branch: string;
}

export interface DeleteIssueDialogProps {
  issueCount: number;
  /** Non-console workspaces linked to the issue(s) that can be deleted */
  deletableWorkspaces: DeletableWorkspaceSummary[];
  /** Count of linked console-mode workspaces that will always be kept */
  exemptedConsoleWorkspaceCount?: number;
}

export type DeleteIssueDialogResult = {
  action: 'confirmed' | 'canceled';
  deleteWorkspaces?: boolean;
};

const DeleteIssueDialogImpl = NiceModal.create<DeleteIssueDialogProps>(
  ({ issueCount, deletableWorkspaces, exemptedConsoleWorkspaceCount = 0 }) => {
    const modal = useModal();
    const { t } = useTranslation();
    const [deleteWorkspaces, setDeleteWorkspaces] = useState(true);

    const hasDeletableWorkspaces = deletableWorkspaces.length > 0;

    const handleConfirm = () => {
      modal.resolve({
        action: 'confirmed',
        deleteWorkspaces: hasDeletableWorkspaces && deleteWorkspaces,
      } as DeleteIssueDialogResult);
      modal.hide();
    };

    const handleCancel = () => {
      modal.resolve({ action: 'canceled' } as DeleteIssueDialogResult);
      modal.hide();
    };

    return (
      <Dialog open={modal.visible} onOpenChange={handleCancel}>
        <DialogContent className="sm:max-w-[425px]">
          <DialogHeader>
            <div className="flex items-center gap-3">
              <WarningIcon className="h-6 w-6 text-destructive" />
              <DialogTitle>
                {issueCount === 1
                  ? t('issues.deleteDialog.title', 'Delete Issue')
                  : t(
                      'issues.deleteDialog.titlePlural',
                      'Delete {{count}} Issues',
                      { count: issueCount }
                    )}
              </DialogTitle>
            </div>
            <DialogDescription className="text-left pt-2">
              {issueCount === 1
                ? t(
                    'issues.deleteDialog.description',
                    'Are you sure you want to delete this issue? This action cannot be undone.'
                  )
                : t(
                    'issues.deleteDialog.descriptionPlural',
                    'Are you sure you want to delete these {{count}} issues? This action cannot be undone.',
                    { count: issueCount }
                  )}
            </DialogDescription>
          </DialogHeader>

          <div className="py-4 space-y-3">
            {hasDeletableWorkspaces && (
              <div className="flex flex-col gap-2">
                <div
                  className="flex items-center gap-3 text-sm font-medium cursor-pointer select-none"
                  onClick={() => setDeleteWorkspaces((v) => !v)}
                >
                  <Checkbox checked={deleteWorkspaces} />
                  <span className="flex items-center gap-2">
                    <FolderIcon className="h-4 w-4" />
                    {deletableWorkspaces.length === 1
                      ? t(
                          'issues.deleteDialog.deleteWorkspaceLabel',
                          'Also delete linked workspace and its local directory'
                        )
                      : t(
                          'issues.deleteDialog.deleteWorkspacesLabel',
                          'Also delete {{count}} linked workspaces and their local directories',
                          { count: deletableWorkspaces.length }
                        )}
                  </span>
                </div>
                <ul className="pl-7 flex flex-col gap-0.5">
                  {deletableWorkspaces.map((workspace) => (
                    <li
                      key={workspace.id}
                      className="text-xs text-muted-foreground"
                    >
                      <code className="rounded bg-muted px-1 py-0.5 font-mono">
                        {workspace.name || workspace.branch}
                      </code>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {exemptedConsoleWorkspaceCount > 0 && (
              <p className="text-xs text-muted-foreground pl-7">
                {exemptedConsoleWorkspaceCount === 1
                  ? t(
                      'issues.deleteDialog.consoleExemptSingular',
                      '1 console workspace is kept and will not be deleted (console mode).'
                    )
                  : t(
                      'issues.deleteDialog.consoleExemptPlural',
                      '{{count}} console workspaces are kept and will not be deleted (console mode).',
                      { count: exemptedConsoleWorkspaceCount }
                    )}
              </p>
            )}
          </div>

          <DialogFooter className="gap-2">
            <Button variant="outline" onClick={handleCancel}>
              {t('buttons.cancel')}
            </Button>
            <Button variant="destructive" onClick={handleConfirm}>
              {t('buttons.delete')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const DeleteIssueDialog = defineModal<
  DeleteIssueDialogProps,
  DeleteIssueDialogResult
>(DeleteIssueDialogImpl);
