import { describe, expect, test } from 'bun:test';
import {
  assignedMeetingCount,
  briefItemCount,
  formatBriefJson,
  formatBriefMarkdown,
  hasBriefContent,
} from '../../src/lib/conversation-brief';
import type { ConversationBrief, MeetingSpaceAssignment } from '../../src/services/memorySpaceService';
import type { MeetingMemory } from '../../src/services/memoryService';

function memory(id: string): MeetingMemory {
  return {
    id,
    meetingId: 'meeting-1',
    meetingTitle: 'Client review',
    kind: 'decision',
    text: `Memory ${id}`,
    suggestedText: `Memory ${id}`,
    owner: null,
    dueDate: null,
    reviewStatus: 'confirmed',
    resolutionStatus: 'open',
    evidenceConfidence: null,
    sourceTranscriptId: null,
    sourceExcerpt: null,
    sourceTimestamp: null,
    sourceAudioStartTime: null,
    sourceAudioEndTime: null,
    createdAt: '2026-07-10T00:00:00Z',
    updatedAt: '2026-07-10T00:00:00Z',
    reviewedAt: '2026-07-10T00:00:00Z',
    followUpReviewedAt: null,
  };
}

function brief(overrides: Partial<ConversationBrief> = {}): ConversationBrief {
  return {
    space: {
      id: 'space-1',
      name: 'Acme',
      meetingCount: 2,
      createdAt: '2026-07-10T00:00:00Z',
      updatedAt: '2026-07-10T00:00:00Z',
    },
    decisions: [],
    commitments: [],
    openQuestions: [],
    ...overrides,
  };
}

describe('conversation brief helpers', () => {
  test('counts every visible brief item', () => {
    const value = brief({
      decisions: [memory('decision')],
      commitments: [memory('commitment')],
      openQuestions: [memory('question-1'), memory('question-2')],
    });
    expect(briefItemCount(value)).toBe(4);
    expect(hasBriefContent(value)).toBe(true);
  });

  test('reports an empty brief', () => {
    expect(briefItemCount(brief())).toBe(0);
    expect(hasBriefContent(brief())).toBe(false);
  });

  test('counts only meetings assigned to the selected space', () => {
    const assignments: MeetingSpaceAssignment[] = [
      { meetingId: 'meeting-1', meetingTitle: 'One', spaceId: 'space-1' },
      { meetingId: 'meeting-2', meetingTitle: 'Two', spaceId: 'space-2' },
      { meetingId: 'meeting-3', meetingTitle: 'Three', spaceId: 'space-1' },
      { meetingId: 'meeting-4', meetingTitle: 'Four', spaceId: null },
    ];
    expect(assignedMeetingCount(assignments, 'space-1')).toBe(2);
  });

  test('exports deterministic Markdown with source metadata', () => {
    const decision = {
      ...memory('decision'),
      owner: 'Sam',
      dueDate: 'Friday',
      sourceTimestamp: '00:04:12',
      sourceTranscriptId: 'transcript-7',
      sourceExcerpt: 'Sam said, "Start\nwith the pilot."',
    };
    const markdown = formatBriefMarkdown(brief({ decisions: [decision] }));
    expect(markdown).toContain('# Acme conversation brief');
    expect(markdown).toContain('## Decisions');
    expect(markdown).toContain(
      'owner: Sam; due: Friday; source: Client review at 00:04:12; meeting-id: meeting-1; segment-id: transcript-7',
    );
    expect(markdown).toContain('evidence: "Sam said, \\"Start with the pilot.\\""');
    expect(markdown).not.toContain('## Open commitments');
  });

  test('exports valid, lossless JSON', () => {
    const value = brief({ openQuestions: [memory('question')] });
    expect(JSON.parse(formatBriefJson(value))).toEqual(value);
  });
});
