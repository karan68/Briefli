import { invoke } from '@tauri-apps/api/core';

export type MemoryKind = 'decision' | 'commitment' | 'open_question';
export type ReviewStatus = 'suggested' | 'confirmed' | 'corrected';
export type ResolutionStatus = 'open' | 'done';

export interface MeetingMemory {
  id: string;
  meetingId: string;
  meetingTitle: string;
  kind: MemoryKind;
  text: string;
  suggestedText: string;
  owner: string | null;
  dueDate: string | null;
  reviewStatus: ReviewStatus;
  resolutionStatus: ResolutionStatus;
  evidenceConfidence: number | null;
  sourceTranscriptId: string | null;
  sourceExcerpt: string | null;
  sourceTimestamp: string | null;
  sourceAudioStartTime: number | null;
  sourceAudioEndTime: number | null;
  createdAt: string;
  updatedAt: string;
  reviewedAt: string | null;
  followUpReviewedAt: string | null;
}

export interface ReviewMemoryInput {
  id: string;
  status: 'confirmed' | 'corrected' | 'rejected';
  text: string;
  owner: string | null;
  dueDate: string | null;
}

export interface MemoryOwnerAlias {
  alias: string;
  canonicalName: string;
}

export const memoryService = {
  sync(): Promise<number> {
    return invoke<number>('api_memories_sync');
  },

  list(): Promise<MeetingMemory[]> {
    return invoke<MeetingMemory[]>('api_memories_list');
  },

  review(input: ReviewMemoryInput): Promise<void> {
    return invoke('api_memory_review', {
      id: input.id,
      status: input.status,
      text: input.text,
      owner: input.owner,
      dueDate: input.dueDate,
    });
  },

  setResolution(id: string, status: ResolutionStatus): Promise<void> {
    return invoke('api_memory_set_resolution', { id, status });
  },

  markFollowUpReviewed(id: string): Promise<void> {
    return invoke('api_memory_mark_follow_up_reviewed', { id });
  },

  listOwnerAliases(): Promise<MemoryOwnerAlias[]> {
    return invoke<MemoryOwnerAlias[]>('api_memory_owner_aliases_list');
  },

  deleteOwnerAlias(alias: string): Promise<void> {
    return invoke('api_memory_owner_alias_delete', { alias });
  },
};
