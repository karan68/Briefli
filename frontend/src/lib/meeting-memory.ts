import type { MeetingMemory } from '@/services/memoryService';

export type MemoryView = 'review' | 'weekly' | 'confirmed' | 'open';

const FOLLOW_UP_INTERVAL_MS = 7 * 24 * 60 * 60 * 1000;

export function filterMemories(items: MeetingMemory[], view: MemoryView): MeetingMemory[] {
  switch (view) {
    case 'review':
      return items.filter((item) => item.reviewStatus === 'suggested');
    case 'confirmed':
      return items.filter((item) => item.reviewStatus !== 'suggested');
    case 'weekly':
      return items.filter((item) => isFollowUpDue(item));
    case 'open':
      return items.filter(
        (item) =>
          item.reviewStatus !== 'suggested' &&
          item.resolutionStatus === 'open' &&
          item.kind !== 'decision',
      );
  }
}

export function countMemoryViews(items: MeetingMemory[]): Record<MemoryView, number> {
  return {
    review: filterMemories(items, 'review').length,
    weekly: filterMemories(items, 'weekly').length,
    confirmed: filterMemories(items, 'confirmed').length,
    open: filterMemories(items, 'open').length,
  };
}

export function isFollowUpDue(item: MeetingMemory, now = new Date()): boolean {
  if (
    item.reviewStatus === 'suggested' ||
    item.resolutionStatus !== 'open' ||
    item.kind === 'decision'
  ) {
    return false;
  }
  if (!item.followUpReviewedAt) return true;
  const reviewedAt = Date.parse(item.followUpReviewedAt);
  if (!Number.isFinite(reviewedAt)) return true;
  return now.getTime() - reviewedAt >= FOLLOW_UP_INTERVAL_MS;
}

export function strictDueDate(value: string | null): Date | null {
  if (!value || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
  const [year, month, day] = value.split('-').map(Number);
  const date = new Date(Date.UTC(year, month - 1, day));
  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return null;
  }
  return date;
}

export function isOverdue(item: MeetingMemory, now = new Date()): boolean {
  const dueDate = strictDueDate(item.dueDate);
  if (!dueDate || item.resolutionStatus !== 'open') return false;
  const today = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
  return dueDate.getTime() < today;
}

export function memoryEvidencePath(item: MeetingMemory): string {
  const meetingId = encodeURIComponent(item.meetingId);
  if (!item.sourceTranscriptId) {
    return `/meeting-details?id=${meetingId}`;
  }
  return `/meeting-details?id=${meetingId}&segment=${encodeURIComponent(item.sourceTranscriptId)}`;
}

export function reviewStatusForText(item: MeetingMemory, editedText: string): 'confirmed' | 'corrected' {
  return editedText.trim() === item.suggestedText.trim() ? 'confirmed' : 'corrected';
}
