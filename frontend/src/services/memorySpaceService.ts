import { invoke } from '@tauri-apps/api/core';
import type { MeetingMemory } from './memoryService';

export interface MemorySpace {
  id: string;
  name: string;
  meetingCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface MeetingSpaceAssignment {
  meetingId: string;
  meetingTitle: string;
  spaceId: string | null;
}

export interface ConversationBrief {
  space: MemorySpace;
  decisions: MeetingMemory[];
  commitments: MeetingMemory[];
  openQuestions: MeetingMemory[];
}

export interface LocalBriefMetrics {
  enabled: boolean;
  briefOpenCount: number;
  sourceOpenCount: number;
}

export const memorySpaceService = {
  create(name: string): Promise<MemorySpace> {
    return invoke<MemorySpace>('api_memory_spaces_create', { name });
  },

  list(): Promise<MemorySpace[]> {
    return invoke<MemorySpace[]>('api_memory_spaces_list');
  },

  rename(id: string, name: string): Promise<MemorySpace> {
    return invoke<MemorySpace>('api_memory_space_rename', { id, name });
  },

  delete(id: string): Promise<void> {
    return invoke('api_memory_space_delete', { id });
  },

  assignments(): Promise<MeetingSpaceAssignment[]> {
    return invoke<MeetingSpaceAssignment[]>('api_memory_space_assignments');
  },

  assign(meetingId: string, spaceId: string | null): Promise<void> {
    return invoke('api_memory_space_assign', { meetingId, spaceId });
  },

  brief(spaceId: string): Promise<ConversationBrief> {
    return invoke<ConversationBrief>('api_memory_space_brief', { spaceId });
  },

  metrics(): Promise<LocalBriefMetrics> {
    return invoke<LocalBriefMetrics>('api_brief_metrics_get');
  },

  setMetricsEnabled(enabled: boolean): Promise<void> {
    return invoke('api_brief_metrics_set_enabled', { enabled });
  },

  recordMetric(
    eventType: 'brief_opened' | 'source_opened',
    spaceId: string,
    memoryId: string | null,
  ): Promise<boolean> {
    return invoke<boolean>('api_brief_metrics_record', { eventType, spaceId, memoryId });
  },

  clearMetrics(): Promise<void> {
    return invoke('api_brief_metrics_clear');
  },
};
