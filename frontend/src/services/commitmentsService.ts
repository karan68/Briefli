import { invoke } from '@tauri-apps/api/core';

export interface CommitmentItem {
  id: string;
  meetingId: string;
  meetingTitle: string;
  text: string;
  owner: string | null;
  dueDate: string | null;
  status: string;
}

/**
 * Cross-meeting commitments. Action items are extracted per meeting by the AI
 * summary; the backend reconciles them into a single tracked list (adding new
 * items, dropping removed ones, and preserving open/done status). This client
 * is a thin wrapper over the Tauri commands.
 */
export const commitmentsService = {
  async list(): Promise<CommitmentItem[]> {
    return invoke<CommitmentItem[]>('api_commitments_list');
  },

  async setStatus(id: string, status: 'open' | 'done'): Promise<void> {
    await invoke('api_commitments_set_status', { id, status });
  },

  /**
   * Reconcile commitments with every meeting's current summary in one backend
   * pass. Returns the total commitment count afterwards.
   */
  async sync(): Promise<number> {
    return invoke<number>('api_commitments_sync');
  },
};
