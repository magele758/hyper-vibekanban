import type { PatchTypeWithKey } from './types';

/** Initial load: newest completed coding-agent turn only (scroll loads older). */
export const INITIAL_HISTORIC_CODING_AGENT_PROCESSES = 1;

/** Older history loaded per scroll-to-top trigger (coding-agent turns). */
export const LOAD_MORE_CODING_AGENT_PROCESSES = 2;

/** @deprecated Prefer process-based initial load; kept for any legacy callers. */
export const MIN_INITIAL_ENTRIES = 10;
export const REMAINING_BATCH_SIZE = 50;

/** Load older history only when scrollTop is within this many px of the top. */
export const LOAD_MORE_TOP_THRESHOLD_PX = 96;

export const makeLoadingPatch = (
  executionProcessId: string
): PatchTypeWithKey => ({
  type: 'NORMALIZED_ENTRY',
  content: {
    entry_type: {
      type: 'loading',
    },
    content: '',
    timestamp: null,
  },
  patchKey: `${executionProcessId}:loading`,
  executionProcessId,
});

export const nextActionPatch: (
  failed: boolean,
  execution_processes: number,
  needs_setup: boolean,
  setup_help_text?: string
) => PatchTypeWithKey = (
  failed,
  execution_processes,
  needs_setup,
  setup_help_text
) => ({
  type: 'NORMALIZED_ENTRY',
  content: {
    entry_type: {
      type: 'next_action',
      failed: failed,
      execution_processes: execution_processes,
      needs_setup: needs_setup,
      setup_help_text: setup_help_text ?? null,
    },
    content: '',
    timestamp: null,
  },
  patchKey: 'next_action',
  executionProcessId: '',
});
