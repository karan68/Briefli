import { describe, expect, test } from 'bun:test';
import {
  countMemoryViews,
  filterMemories,
  isFollowUpDue,
  isOverdue,
  memoryEvidencePath,
  reviewStatusForText,
  strictDueDate,
} from '../../src/lib/meeting-memory';
import type { MeetingMemory } from '../../src/services/memoryService';

function memory(overrides: Partial<MeetingMemory> = {}): MeetingMemory {
  return {
    id: 'memory-1',
    meetingId: 'meeting-1',
    meetingTitle: 'Client review',
    kind: 'commitment',
    text: 'Send the proposal',
    suggestedText: 'Send the proposal',
    owner: 'Sam',
    dueDate: 'Friday',
    reviewStatus: 'suggested',
    resolutionStatus: 'open',
    evidenceConfidence: 1,
    sourceTranscriptId: 'transcript-1',
    sourceExcerpt: 'Sam will send the proposal by Friday.',
    sourceTimestamp: '00:04:12',
    sourceAudioStartTime: 252,
    sourceAudioEndTime: 258,
    createdAt: '2026-07-10T00:00:00Z',
    updatedAt: '2026-07-10T00:00:00Z',
    reviewedAt: null,
    followUpReviewedAt: null,
    ...overrides,
  };
}

describe('filterMemories', () => {
  const items = [
    memory(),
    memory({ id: 'memory-2', reviewStatus: 'confirmed' }),
    memory({ id: 'memory-3', kind: 'decision', reviewStatus: 'corrected' }),
    memory({ id: 'memory-4', kind: 'open_question', reviewStatus: 'confirmed', resolutionStatus: 'done' }),
  ];

  test('review contains suggestions only', () => {
    expect(filterMemories(items, 'review').map((item) => item.id)).toEqual(['memory-1']);
  });

  test('confirmed excludes suggestions', () => {
    expect(filterMemories(items, 'confirmed').map((item) => item.id)).toEqual([
      'memory-2',
      'memory-3',
      'memory-4',
    ]);
  });

  test('open contains unresolved commitments and questions, not decisions', () => {
    expect(filterMemories(items, 'open').map((item) => item.id)).toEqual(['memory-2']);
  });

  test('counts each view independently', () => {
    expect(countMemoryViews(items)).toEqual({ review: 1, weekly: 1, confirmed: 3, open: 1 });
  });
});

describe('weekly follow-up review', () => {
  const now = new Date('2026-07-10T12:00:00Z');

  test('includes never-reviewed and seven-day-old confirmed open loops', () => {
    expect(isFollowUpDue(memory({ reviewStatus: 'confirmed' }), now)).toBe(true);
    expect(
      isFollowUpDue(
        memory({ reviewStatus: 'confirmed', followUpReviewedAt: '2026-07-03T12:00:00Z' }),
        now,
      ),
    ).toBe(true);
  });

  test('excludes recent, completed, suggested, and decision records', () => {
    expect(
      isFollowUpDue(
        memory({ reviewStatus: 'confirmed', followUpReviewedAt: '2026-07-04T12:00:01Z' }),
        now,
      ),
    ).toBe(false);
    expect(isFollowUpDue(memory({ reviewStatus: 'confirmed', resolutionStatus: 'done' }), now)).toBe(false);
    expect(isFollowUpDue(memory(), now)).toBe(false);
    expect(isFollowUpDue(memory({ reviewStatus: 'confirmed', kind: 'decision' }), now)).toBe(false);
  });
});

describe('strict due dates', () => {
  const now = new Date('2026-07-10T12:00:00Z');

  test('accepts only real YYYY-MM-DD calendar dates', () => {
    expect(strictDueDate('2026-07-09')?.toISOString()).toBe('2026-07-09T00:00:00.000Z');
    expect(strictDueDate('Friday')).toBeNull();
    expect(strictDueDate('2026-02-30')).toBeNull();
    expect(strictDueDate('2026-7-9')).toBeNull();
  });

  test('marks only prior dates on open loops overdue', () => {
    expect(isOverdue(memory({ reviewStatus: 'confirmed', dueDate: '2026-07-09' }), now)).toBe(true);
    expect(isOverdue(memory({ reviewStatus: 'confirmed', dueDate: '2026-07-10' }), now)).toBe(false);
    expect(isOverdue(memory({ reviewStatus: 'confirmed', dueDate: 'Friday' }), now)).toBe(false);
    expect(
      isOverdue(memory({ reviewStatus: 'confirmed', dueDate: '2026-07-09', resolutionStatus: 'done' }), now),
    ).toBe(false);
  });
});

describe('memoryEvidencePath', () => {
  test('links directly to the source segment when evidence exists', () => {
    expect(memoryEvidencePath(memory())).toBe(
      '/meeting-details?id=meeting-1&segment=transcript-1',
    );
  });

  test('falls back to the meeting when no source segment was matched', () => {
    expect(memoryEvidencePath(memory({ sourceTranscriptId: null }))).toBe(
      '/meeting-details?id=meeting-1',
    );
  });

  test('encodes identifiers safely', () => {
    expect(
      memoryEvidencePath(memory({ meetingId: 'meeting & one', sourceTranscriptId: 'segment/2' })),
    ).toBe('/meeting-details?id=meeting%20%26%20one&segment=segment%2F2');
  });
});

describe('reviewStatusForText', () => {
  test('confirms unchanged wording and marks edited wording corrected', () => {
    const item = memory();
    expect(reviewStatusForText(item, ' Send the proposal ')).toBe('confirmed');
    expect(reviewStatusForText(item, 'Send the revised proposal')).toBe('corrected');
  });
});
