import { Database } from 'bun:sqlite';
import { copyFileSync, existsSync } from 'node:fs';

const databasePath = process.env.BRIEFLI_DB_PATH
  ?? `${process.env.APPDATA}/com.briefli.app/meeting_minutes.sqlite`;

if (!existsSync(databasePath)) {
  throw new Error(`Briefli database not found: ${databasePath}`);
}

const backupPath = `${databasePath}.before-demo-seed.bak`;
const db = new Database(databasePath);
db.run('PRAGMA busy_timeout = 10000');
db.run('PRAGMA wal_checkpoint(FULL)');
copyFileSync(databasePath, backupPath);

const now = '2026-07-10T12:00:00Z';
const sevenDaysAgo = '2026-07-03T12:00:00Z';

type TranscriptSeed = {
  id: string;
  text: string;
  timestamp: string;
  start?: number;
  end?: number;
};

type MemorySeed = {
  id: string;
  kind: 'decision' | 'commitment' | 'open_question';
  text: string;
  owner?: string;
  dueDate?: string;
  reviewStatus: 'suggested' | 'confirmed' | 'corrected' | 'rejected';
  resolutionStatus?: 'open' | 'done';
  transcriptId?: string;
  evidenceConfidence?: number;
  reviewedAt?: string;
  followUpReviewedAt?: string;
};

type MeetingSeed = {
  id: string;
  title: string;
  createdAt: string;
  transcripts: TranscriptSeed[];
  memories: MemorySeed[];
  spaceId?: string;
};

function fingerprint(value: string): string {
  return value
    .toLowerCase()
    .split(/\s+/)
    .map((word) => word.replace(/[^\p{L}\p{N}]/gu, ''))
    .filter(Boolean)
    .join(' ');
}

function at(minutesAgo: number): string {
  return new Date(Date.parse(now) - minutesAgo * 60_000).toISOString();
}

function longTranscript(prefix: string, count: number): TranscriptSeed[] {
  const utterances = [
    'Yeah, the first thing I noticed was that people get stuck during onboarding, right after they connect their account.',
    'Sorry, can I jump in? The billing retries are working, but the error message still makes it look like the payment failed twice.',
    'I tried it with the keyboard only yesterday. The dialog opens, but focus disappears when you close it.',
    'Before we call this release-ready, I want to walk through the rollback one more time. I am not comfortable guessing under pressure.',
    'Support volume was lower this week, although the password-reset tickets are taking us longer than they should.',
    'For retention, I think thirty days is reasonable, but we need a separate answer for confirmed decisions.',
    'Mm-hm. That matches what I saw too. The confusing part is the second screen, not the first one.',
    'Wait, are we talking about the current build or the one from Tuesday? They behave differently on Windows.',
    'I do not have the number in front of me, but it was roughly one in every eight sessions.',
    'Could we park that for five minutes? I want to finish the customer-impact part before we move to implementation.',
    'There is one more edge case: if the laptop wakes from sleep, the status looks connected even though it is not.',
    'Okay, so what I am hearing is that we need a smaller test before we change the default for everyone.',
  ];
  return Array.from({ length: count }, (_, index) => {
    const start = index * 18;
    return {
      id: `${prefix}-t-${String(index + 1).padStart(3, '0')}`,
      text: utterances[index % utterances.length],
      timestamp: new Date(start * 1000).toISOString().slice(11, 19),
      start,
      end: start + 12,
    };
  });
}

const spaces = [
  ['demo-space-acme', 'Demo · Acme Client'],
  ['demo-space-product', 'Demo · Product Alpha'],
  ['demo-space-leadership', 'Demo · Leadership Weekly'],
  ['demo-space-edge', 'Demo · Edge Cases'],
  ['demo-space-inperson', 'Demo · In-person Workshop'],
  ['demo-space-hiring', 'Demo · Hiring Loop'],
  ['demo-space-operations', 'Demo · Operations'],
  ['demo-space-research', 'Demo · User Research'],
] as const;

const meetings: MeetingSeed[] = [
  {
    id: 'demo-meeting-short',
    title: 'Demo · Two-minute decision',
    createdAt: at(60),
    spaceId: 'demo-space-acme',
    transcripts: [
      {
        id: 'demo-short-t-1',
        text: 'Okay, let’s keep this small. Two weeks, just the support team, and then we’ll look at what actually happened.',
        timestamp: '00:00:08',
        start: 8,
        end: 14,
      },
      {
        id: 'demo-short-t-2',
        text: 'Yeah, I can take that. I’ll clean up the proposal and send the revised version by Friday.',
        timestamp: '00:00:18',
        start: 18,
        end: 23,
      },
      {
        id: 'demo-short-t-3',
        text: 'The only thing we haven’t answered is the region. Is Canada going first, or do we start with Ireland?',
        timestamp: '00:00:29',
        start: 29,
        end: 35,
      },
    ],
    memories: [
      {
        id: 'demo-memory-short-decision',
        kind: 'decision',
        text: 'Run a two-week pilot with the smaller support team',
        reviewStatus: 'suggested',
        transcriptId: 'demo-short-t-1',
        evidenceConfidence: 1,
      },
      {
        id: 'demo-memory-short-commitment',
        kind: 'commitment',
        text: 'Send the revised proposal',
        owner: 'Sam',
        dueDate: 'Friday',
        reviewStatus: 'suggested',
        transcriptId: 'demo-short-t-2',
        evidenceConfidence: 1,
      },
      {
        id: 'demo-memory-short-question',
        kind: 'open_question',
        text: 'Which region should join the pilot first?',
        reviewStatus: 'suggested',
        transcriptId: 'demo-short-t-3',
        evidenceConfidence: 0.86,
      },
    ],
  },
  {
    id: 'demo-meeting-medium',
    title: 'Demo · Product planning (medium)',
    createdAt: at(1_440),
    spaceId: 'demo-space-product',
    transcripts: [
      {
        id: 'demo-medium-t-1',
        text: 'For me, offline export has to be in the first public beta. If it is not there, the privacy story does not really hold together. Yeah? Okay, let’s treat it as required.',
        timestamp: '00:03:14', start: 194, end: 201,
      },
      {
        id: 'demo-medium-t-2',
        text: 'I’ve got the Windows installer checklist. I should have said this earlier, but I’ll finish it by July ninth.',
        timestamp: '00:11:42', start: 702, end: 710,
      },
      {
        id: 'demo-medium-t-3',
        text: 'Do imported recordings count toward the weekly number? I am hearing two different answers, so let’s not pretend we settled that.',
        timestamp: '00:21:07', start: 1267, end: 1275,
      },
      ...longTranscript('demo-medium', 24),
    ],
    memories: [
      {
        id: 'demo-memory-medium-decision',
        kind: 'decision',
        text: 'Offline export is required for the first public beta',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-medium-t-1',
        evidenceConfidence: 0.97,
        reviewedAt: at(1_400),
      },
      {
        id: 'demo-memory-medium-overdue',
        kind: 'commitment',
        text: 'Finish the Windows installer checklist',
        owner: 'Alexandra Chen',
        dueDate: '2026-07-09',
        reviewStatus: 'corrected',
        transcriptId: 'demo-medium-t-2',
        evidenceConfidence: 0.91,
        reviewedAt: at(1_390),
      },
      {
        id: 'demo-memory-medium-question',
        kind: 'open_question',
        text: 'Should imported meetings count toward weekly usage?',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-medium-t-3',
        evidenceConfidence: 0.82,
        reviewedAt: at(1_380),
        followUpReviewedAt: sevenDaysAgo,
      },
    ],
  },
  {
    id: 'demo-meeting-long',
    title: 'Demo · Quarterly client review (long)',
    createdAt: at(4_320),
    spaceId: 'demo-space-acme',
    transcripts: [
      ...longTranscript('demo-long', 120),
      {
        id: 'demo-long-t-decision',
        text: 'All right, I think we have enough to choose. Canada first, Ireland after that, and we pause between them to check support volume. Any objections? No? Let’s do the phased rollout.',
        timestamp: '00:38:40', start: 2320, end: 2329,
      },
      {
        id: 'demo-long-t-done',
        text: 'Just to close the loop, I sent the security questionnaire yesterday afternoon. They replied this morning, so that one is done.',
        timestamp: '00:42:15', start: 2535, end: 2542,
      },
    ],
    memories: [
      {
        id: 'demo-memory-long-decision',
        kind: 'decision',
        text: 'Use a phased rollout for Canada and Ireland',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-long-t-decision',
        evidenceConfidence: 0.93,
        reviewedAt: at(4_000),
      },
      {
        id: 'demo-memory-long-done',
        kind: 'commitment',
        text: 'Deliver the security questionnaire',
        owner: 'Priya Shah',
        dueDate: '2026-07-08',
        reviewStatus: 'confirmed',
        resolutionStatus: 'done',
        transcriptId: 'demo-long-t-done',
        evidenceConfidence: 0.95,
        reviewedAt: at(4_000),
      },
    ],
  },
  {
    id: 'demo-meeting-large',
    title: 'Demo · All-hands transcript stress test (500 segments)',
    createdAt: at(10_080),
    spaceId: 'demo-space-leadership',
    transcripts: longTranscript('demo-large', 500),
    memories: [
      {
        id: 'demo-memory-large-no-evidence',
        kind: 'decision',
        text: 'This intentionally has no matched transcript evidence',
        reviewStatus: 'suggested',
      },
      {
        id: 'demo-memory-large-open',
        kind: 'open_question',
        text: 'How should the next all-hands be structured?',
        reviewStatus: 'confirmed',
        reviewedAt: at(9_000),
      },
    ],
  },
  {
    id: 'demo-meeting-unicode',
    title: 'Demo · Unicode, punctuation & missing timing',
    createdAt: at(20_160),
    spaceId: 'demo-space-edge',
    transcripts: [
      {
        id: 'demo-unicode-t-1',
        text: 'José: Um, for café-mode, can we wait until the 日本語 review is finished? Also, the budget I have is €2,500, not twenty-five thousand.',
        timestamp: '00:00:01',
      },
      {
        id: 'demo-unicode-t-2',
        text: 'It is Zoë O’Neil-Smith, with the accent and the curly apostrophe. The export should not break her name again.',
        timestamp: '00:00:11',
      },
    ],
    memories: [
      {
        id: 'demo-memory-unicode',
        kind: 'commitment',
        text: 'Review café-mode in Japanese',
        owner: 'Zoë O’Neil-Smith',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-unicode-t-1',
        evidenceConfidence: 0.78,
        reviewedAt: at(20_000),
      },
      {
        id: 'demo-memory-relative-date',
        kind: 'commitment',
        text: 'Confirm the localized budget',
        owner: 'José',
        dueDate: 'next Friday',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-unicode-t-1',
        evidenceConfidence: 0.74,
        reviewedAt: at(20_000),
      },
    ],
  },
  {
    id: 'demo-meeting-empty',
    title: 'Demo · Empty transcript edge case',
    createdAt: at(30_240),
    spaceId: 'demo-space-edge',
    transcripts: [],
    memories: [],
  },
  {
    id: 'demo-meeting-rejected',
    title: 'Demo · Rejected suggestion (hidden)',
    createdAt: at(40_320),
    transcripts: [
      {
        id: 'demo-rejected-t-1',
        text: 'No, hang on, “launch tomorrow” was just me thinking out loud. Please do not write that down as something we promised.',
        timestamp: '00:02:00', start: 120, end: 127,
      },
    ],
    memories: [
      {
        id: 'demo-memory-rejected',
        kind: 'commitment',
        text: 'Launch tomorrow',
        owner: 'Team',
        reviewStatus: 'rejected',
        transcriptId: 'demo-rejected-t-1',
        evidenceConfidence: 0.42,
        reviewedAt: at(40_000),
      },
    ],
  },
  {
    id: 'demo-meeting-inperson-workshop',
    title: 'Demo · In-person whiteboard workshop (mic-only)',
    createdAt: at(180),
    spaceId: 'demo-space-inperson',
    transcripts: [
      {
        id: 'demo-inperson-t-1',
        text: 'Okay, I think we’re all here. Quick heads-up: this is recording from my laptop mic, so it’ll pick up the room, not any system audio. Can everyone hear me from the back?',
        timestamp: '00:00:12', start: 12, end: 22,
      },
      {
        id: 'demo-inperson-t-2',
        text: 'Looking at the sticky notes, guest checkout has most of the votes. Loyalty points matter, sure, but they should not block somebody from buying. Are we agreed? Great, guest checkout first.',
        timestamp: '00:16:40', start: 1000, end: 1010,
      },
      {
        id: 'demo-inperson-t-3',
        text: 'I’ll grab a photo of the whiteboard before we leave and put it in the project notes. Tomorrow is July eleventh, right? Yeah, I’ll have it there by then.',
        timestamp: '00:18:05', start: 1085, end: 1095,
      },
      {
        id: 'demo-inperson-t-4',
        text: 'Wait, for a guest, are we saving the shipping address? I thought no. I thought maybe. Okay, we are not deciding that in the room today; let’s leave it open.',
        timestamp: '00:23:17', start: 1397, end: 1405,
      },
    ],
    memories: [
      {
        id: 'demo-memory-inperson-decision',
        kind: 'decision',
        text: 'Prioritize guest checkout before loyalty points',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-inperson-t-2',
        evidenceConfidence: 0.96,
        reviewedAt: at(170),
      },
      {
        id: 'demo-memory-inperson-commitment',
        kind: 'commitment',
        text: 'Photograph the whiteboard and attach it to the project notes',
        owner: 'Maya',
        dueDate: '2026-07-11',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-inperson-t-3',
        evidenceConfidence: 0.94,
        reviewedAt: at(170),
      },
      {
        id: 'demo-memory-inperson-question',
        kind: 'open_question',
        text: 'Should guest checkout remember the shipping address?',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-inperson-t-4',
        evidenceConfidence: 0.89,
        reviewedAt: at(170),
      },
    ],
  },
  {
    id: 'demo-meeting-one-on-one',
    title: 'Demo · Manager 1:1',
    createdAt: at(2_880),
    spaceId: 'demo-space-leadership',
    transcripts: [
      {
        id: 'demo-one-on-one-t-1',
        text: 'I’d like to own more of the customer interviews next cycle. Right now I join late, after the questions are already set, and I think I could help earlier.',
        timestamp: '00:04:30', start: 270, end: 279,
      },
      {
        id: 'demo-one-on-one-t-2',
        text: 'That makes sense. I’ll set up two interviews for you to shadow before the end of July, and then we can talk about you leading one.',
        timestamp: '00:12:10', start: 730, end: 738,
      },
      {
        id: 'demo-one-on-one-t-3',
        text: 'On the promotion timing, I don’t want to give you a date today and then move it. Let’s review the scope document first and come back to the timeline.',
        timestamp: '00:19:45', start: 1185, end: 1193,
      },
    ],
    memories: [
      {
        id: 'demo-memory-one-on-one-commitment',
        kind: 'commitment',
        text: 'Arrange two customer-interview shadowing sessions',
        owner: 'Morgan',
        dueDate: '2026-07-31',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-one-on-one-t-2',
        evidenceConfidence: 0.9,
        reviewedAt: at(2_800),
      },
      {
        id: 'demo-memory-one-on-one-question',
        kind: 'open_question',
        text: 'What is the promotion timeline after the scope review?',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-one-on-one-t-3',
        evidenceConfidence: 0.76,
        reviewedAt: at(2_800),
      },
    ],
  },
  {
    id: 'demo-meeting-interview',
    title: 'Demo · Structured engineering interview',
    createdAt: at(5_760),
    spaceId: 'demo-space-hiring',
    transcripts: [
      {
        id: 'demo-interview-t-1',
        text: 'I would put an idempotency key on the payment request. If the client retries after a timeout, we return the first result instead of charging the card again.',
        timestamp: '00:09:22', start: 562, end: 571,
      },
      {
        id: 'demo-interview-t-2',
        text: 'Can we all submit the scorecard before we discuss the candidate? I do not want the first opinion to anchor everybody else. Yeah, independently first.',
        timestamp: '00:43:05', start: 2585, end: 2594,
      },
      {
        id: 'demo-interview-t-3',
        text: 'Mine is mostly done. I’ll submit the scorecard by July tenth, before the debrief starts.',
        timestamp: '00:44:10', start: 2650, end: 2656,
      },
    ],
    memories: [
      {
        id: 'demo-memory-interview-decision',
        kind: 'decision',
        text: 'Complete scorecards independently before panel discussion',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-interview-t-2',
        evidenceConfidence: 0.98,
        reviewedAt: at(5_700),
      },
      {
        id: 'demo-memory-interview-commitment',
        kind: 'commitment',
        text: 'Submit the interview scorecard',
        owner: 'Rina',
        dueDate: '2026-07-10',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-interview-t-3',
        evidenceConfidence: 0.99,
        reviewedAt: at(5_700),
      },
    ],
  },
  {
    id: 'demo-meeting-incident',
    title: 'Demo · Production incident postmortem',
    createdAt: at(8_640),
    spaceId: 'demo-space-operations',
    transcripts: [
      {
        id: 'demo-incident-t-1',
        text: 'At 2:14 the certificate expired, and the workers kept retrying the webhooks. We did not notice until 2:54, so the queue grew for about forty minutes.',
        timestamp: '00:06:18', start: 378, end: 388,
      },
      {
        id: 'demo-incident-t-2',
        text: 'One alert is not enough. Let’s page the owner at thirty days and again at seven days. Actually, not page at thirty—send a warning—then page at seven. Everyone okay with that?',
        timestamp: '00:28:00', start: 1680, end: 1688,
      },
      {
        id: 'demo-incident-t-3',
        text: 'I just pushed the retry-queue drain runbook while we were talking. The link is in the incident channel, so we can mark that done.',
        timestamp: '00:31:42', start: 1902, end: 1910,
      },
      {
        id: 'demo-incident-t-4',
        text: 'Who is taking the quarterly recovery drill? I cannot own another rotation. Same here. Okay, no owner yet; keep that open.',
        timestamp: '00:35:11', start: 2111, end: 2118,
      },
    ],
    memories: [
      {
        id: 'demo-memory-incident-decision',
        kind: 'decision',
        text: 'Alert on certificate expiry at thirty and seven days',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-incident-t-2',
        evidenceConfidence: 0.96,
        reviewedAt: at(8_500),
      },
      {
        id: 'demo-memory-incident-done',
        kind: 'commitment',
        text: 'Write the retry-queue drain runbook',
        owner: 'Lee',
        reviewStatus: 'confirmed',
        resolutionStatus: 'done',
        transcriptId: 'demo-incident-t-3',
        evidenceConfidence: 0.93,
        reviewedAt: at(8_500),
      },
      {
        id: 'demo-memory-incident-question',
        kind: 'open_question',
        text: 'Who owns the quarterly recovery drill?',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-incident-t-4',
        evidenceConfidence: 0.88,
        reviewedAt: at(8_500),
      },
    ],
  },
  {
    id: 'demo-meeting-sales-discovery',
    title: 'Demo · Sales discovery call',
    createdAt: at(11_520),
    spaceId: 'demo-space-acme',
    transcripts: [
      {
        id: 'demo-sales-t-1',
        text: 'For us, raw recordings cannot sit around forever. Thirty days would work, but I still need the decisions we confirmed to stay in the account.',
        timestamp: '00:07:33', start: 453, end: 463,
      },
      {
        id: 'demo-sales-t-2',
        text: 'I can send over the security architecture document. Next Tuesday—does that work? Great, I’ll get it to you then.',
        timestamp: '00:19:04', start: 1144, end: 1152,
      },
      {
        id: 'demo-sales-t-3',
        text: 'How would offline team sync be priced? Per user, per device, or something else? We do not have that answer today; I need to take it back to the team.',
        timestamp: '00:26:29', start: 1589, end: 1595,
      },
    ],
    memories: [
      {
        id: 'demo-memory-sales-decision',
        kind: 'decision',
        text: 'Use a thirty-day audio retention policy for this account',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-sales-t-1',
        evidenceConfidence: 0.81,
        reviewedAt: at(11_400),
      },
      {
        id: 'demo-memory-sales-commitment',
        kind: 'commitment',
        text: 'Send the security architecture document',
        owner: 'Nora',
        dueDate: 'next Tuesday',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-sales-t-2',
        evidenceConfidence: 0.95,
        reviewedAt: at(11_400),
      },
      {
        id: 'demo-memory-sales-question',
        kind: 'open_question',
        text: 'How should offline team sync be priced?',
        reviewStatus: 'suggested',
        transcriptId: 'demo-sales-t-3',
        evidenceConfidence: 0.73,
      },
    ],
  },
  {
    id: 'demo-meeting-standup',
    title: 'Demo · Daily engineering stand-up',
    createdAt: at(12_960),
    spaceId: 'demo-space-product',
    transcripts: [
      {
        id: 'demo-standup-t-1',
        text: 'Yesterday I finished the transcript pagination fix. It is in review now, and no blockers from me.',
        timestamp: '00:00:34', start: 34, end: 41,
      },
      {
        id: 'demo-standup-t-2',
        text: 'I’m picking up the Windows notification failure today. I can reproduce it after sleep, so I’ll start there.',
        timestamp: '00:01:09', start: 69, end: 76,
      },
      {
        id: 'demo-standup-t-3',
        text: 'Anything here that changes the release date? No? Okay, same date, let’s move on.',
        timestamp: '00:02:20', start: 140, end: 146,
      },
    ],
    memories: [
      {
        id: 'demo-memory-standup-commitment',
        kind: 'commitment',
        text: 'Investigate the Windows notification failure',
        owner: 'Ben',
        dueDate: 'today',
        reviewStatus: 'suggested',
        transcriptId: 'demo-standup-t-2',
        evidenceConfidence: 0.92,
      },
    ],
  },
  {
    id: 'demo-meeting-research',
    title: 'Demo · User research interview',
    createdAt: at(17_280),
    spaceId: 'demo-space-research',
    transcripts: [
      {
        id: 'demo-research-t-1',
        text: 'Before a client call I usually search Slack, then my notebook, then sometimes email. It is messy, and I am never sure I found the latest decision.',
        timestamp: '00:05:42', start: 342, end: 352,
      },
      {
        id: 'demo-research-t-2',
        text: 'If the summary says we decided something, I want to click it and hear or read the exact moment. Otherwise I still go back and check manually.',
        timestamp: '00:18:16', start: 1096, end: 1104,
      },
      {
        id: 'demo-research-t-3',
        text: 'Would you use a brief if you had to open it yourself, without the calendar reminding you? Maybe. I think we need to test that instead of assuming.',
        timestamp: '00:32:08', start: 1928, end: 1937,
      },
    ],
    memories: [
      {
        id: 'demo-memory-research-question',
        kind: 'open_question',
        text: 'Is a manual pre-meeting brief useful without calendar integration?',
        reviewStatus: 'confirmed',
        transcriptId: 'demo-research-t-3',
        evidenceConfidence: 0.87,
        reviewedAt: at(17_100),
      },
    ],
  },
];

const deleteDemo = db.transaction(() => {
  db.run("DELETE FROM brief_usage_events WHERE id LIKE 'demo-%'");
  db.run("DELETE FROM meeting_space_assignments WHERE meeting_id LIKE 'demo-%' OR space_id LIKE 'demo-%'");
  db.run("DELETE FROM memory_owner_aliases WHERE alias_key LIKE 'demo %'");
  db.run("DELETE FROM meeting_memories WHERE id LIKE 'demo-%'");
  db.run("DELETE FROM summary_processes WHERE meeting_id LIKE 'demo-%'");
  db.run("DELETE FROM transcripts WHERE id LIKE 'demo-%' OR meeting_id LIKE 'demo-%'");
  db.run("DELETE FROM meetings WHERE id LIKE 'demo-%'");
  db.run("DELETE FROM memory_spaces WHERE id LIKE 'demo-%'");
});

const insertDemo = db.transaction(() => {
  deleteDemo();

  const insertSpace = db.prepare(
    'INSERT INTO memory_spaces (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)',
  );
  for (const [id, name] of spaces) insertSpace.run(id, name, now, now);

  const insertMeeting = db.prepare(
    'INSERT INTO meetings (id, title, created_at, updated_at, folder_path) VALUES (?, ?, ?, ?, NULL)',
  );
  const insertTranscript = db.prepare(
    `INSERT INTO transcripts
     (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, duration)
     VALUES (?, ?, ?, ?, ?, ?, ?)`,
  );
  const insertMemory = db.prepare(
    `INSERT INTO meeting_memories
     (id, meeting_id, kind, text, suggested_text, source_fingerprint,
      owner, due_date, review_status, resolution_status, evidence_confidence,
      source_transcript_id, source_excerpt, source_timestamp,
      source_audio_start_time, source_audio_end_time,
      created_at, updated_at, reviewed_at, follow_up_reviewed_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  );
  const assignSpace = db.prepare(
    'INSERT INTO meeting_space_assignments (meeting_id, space_id, assigned_at) VALUES (?, ?, ?)',
  );

  for (const meeting of meetings) {
    insertMeeting.run(meeting.id, meeting.title, meeting.createdAt, meeting.createdAt);
    const transcriptById = new Map(meeting.transcripts.map((item) => [item.id, item]));
    for (const transcript of meeting.transcripts) {
      insertTranscript.run(
        transcript.id,
        meeting.id,
        transcript.text,
        transcript.timestamp,
        transcript.start ?? null,
        transcript.end ?? null,
        transcript.start !== undefined && transcript.end !== undefined
          ? transcript.end - transcript.start
          : null,
      );
    }
    for (const memory of meeting.memories) {
      const transcript = memory.transcriptId ? transcriptById.get(memory.transcriptId) : undefined;
      insertMemory.run(
        memory.id,
        meeting.id,
        memory.kind,
        memory.text,
        memory.text,
        fingerprint(memory.text),
        memory.owner ?? null,
        memory.dueDate ?? null,
        memory.reviewStatus,
        memory.resolutionStatus ?? 'open',
        memory.evidenceConfidence ?? null,
        transcript?.id ?? null,
        transcript?.text ?? null,
        transcript?.timestamp ?? null,
        transcript?.start ?? null,
        transcript?.end ?? null,
        meeting.createdAt,
        now,
        memory.reviewedAt ?? null,
        memory.followUpReviewedAt ?? null,
      );
    }
    if (meeting.spaceId) assignSpace.run(meeting.id, meeting.spaceId, now);
  }

  db.run(
    `INSERT INTO memory_owner_aliases
     (alias_key, alias, canonical_key, canonical_name, created_at, updated_at)
    VALUES ('demo alex', 'Demo Alex', 'alexandra chen', 'Alexandra Chen', ?, ?)`,
    [now, now],
  );

  db.run(
    `INSERT INTO brief_usage_events (id, event_type, space_id, memory_id, occurred_at)
     VALUES ('demo-metric-brief', 'brief_opened', 'demo-space-acme', NULL, ?),
            ('demo-metric-source', 'source_opened', 'demo-space-acme',
             'demo-memory-medium-decision', ?)`,
    [now, now],
  );
});

insertDemo();

const count = (sql: string) => (db.query(sql).get() as { count: number }).count;
const counts = {
  meetings: count("SELECT COUNT(*) AS count FROM meetings WHERE id LIKE 'demo-%'"),
  transcripts: count("SELECT COUNT(*) AS count FROM transcripts WHERE id LIKE 'demo-%'"),
  memories: count("SELECT COUNT(*) AS count FROM meeting_memories WHERE id LIKE 'demo-%'"),
  spaces: count("SELECT COUNT(*) AS count FROM memory_spaces WHERE id LIKE 'demo-%'"),
  assignments: count(
    "SELECT COUNT(*) AS count FROM meeting_space_assignments WHERE meeting_id LIKE 'demo-%'",
  ),
};

const cases = db.query(
  `SELECT review_status AS reviewStatus, resolution_status AS resolutionStatus, COUNT(*) AS count
   FROM meeting_memories WHERE id LIKE 'demo-%'
   GROUP BY review_status, resolution_status ORDER BY review_status, resolution_status`,
).all();

const evidenceMismatches = count(
  `SELECT COUNT(*) AS count
   FROM meeting_memories mm
   JOIN transcripts t ON t.id = mm.source_transcript_id
   WHERE mm.id LIKE 'demo-%' AND mm.source_excerpt != t.transcript`,
);
if (evidenceMismatches !== 0) {
  throw new Error(`${evidenceMismatches} demo memories contain non-verbatim evidence`);
}

console.log(JSON.stringify({
  databasePath,
  backupPath,
  counts,
  cases,
  evidenceMismatches,
}, null, 2));
db.close();
