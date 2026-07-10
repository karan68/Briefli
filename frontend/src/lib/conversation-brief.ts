import type { ConversationBrief, MeetingSpaceAssignment } from '@/services/memorySpaceService';

export function briefItemCount(brief: ConversationBrief): number {
  return brief.decisions.length + brief.commitments.length + brief.openQuestions.length;
}

export function assignedMeetingCount(
  assignments: MeetingSpaceAssignment[],
  spaceId: string,
): number {
  return assignments.filter((assignment) => assignment.spaceId === spaceId).length;
}

export function hasBriefContent(brief: ConversationBrief): boolean {
  return briefItemCount(brief) > 0;
}

export function formatBriefMarkdown(brief: ConversationBrief): string {
  const sections = [
    formatSection('Decisions', brief.decisions),
    formatSection('Open commitments', brief.commitments),
    formatSection('Unresolved questions', brief.openQuestions),
  ].filter(Boolean);
  return [`# ${brief.space.name} conversation brief`, ...sections].join('\n\n').trim();
}

export function formatBriefJson(brief: ConversationBrief): string {
  return JSON.stringify(brief, null, 2);
}

function formatSection(title: string, items: ConversationBrief['decisions']): string {
  if (items.length === 0) return '';
  const lines = items.map((item) => {
    const metadata = [
      item.owner ? `owner: ${item.owner}` : null,
      item.dueDate ? `due: ${item.dueDate}` : null,
      `source: ${item.meetingTitle}${item.sourceTimestamp ? ` at ${item.sourceTimestamp}` : ''}`,
      `meeting-id: ${item.meetingId}`,
      item.sourceTranscriptId ? `segment-id: ${item.sourceTranscriptId}` : null,
    ].filter(Boolean);
    const evidence = item.sourceExcerpt
      ? `\n  - evidence: "${inlineText(item.sourceExcerpt)}"`
      : '';
    return `- ${inlineText(item.text)}\n  - ${metadata.join('; ')}${evidence}`;
  });
  return `## ${title}\n${lines.join('\n')}`;
}

function inlineText(value: string): string {
  return value.replace(/\s+/g, ' ').trim().replaceAll('"', '\\"');
}
