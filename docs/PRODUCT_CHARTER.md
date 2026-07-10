# Briefli Product Charter

**Status:** Active product and engineering direction
**Last validated:** 2026-07-10
**Scope:** Briefli desktop application on `devtest`

## Product Thesis

Briefli is the private, trustworthy record of what was decided, what was
promised, and what remains unresolved. It brings that record back before the
next conversation and links every machine-created item to the original meeting
evidence.

Briefli is not competing on transcription, summaries, bot-free capture, or
generic meeting chat alone. Those capabilities are market table stakes. Local
processing is the architecture that makes a trustworthy memory product
possible; it is not the entire user promise.

## Initial User

The first user is an independent consultant, fractional operator, founder, or
other client-facing professional who:

- has recurring conversations with the same people;
- mixes online and in-person meetings;
- personally owns follow-through;
- handles information they do not want stored by another meeting vendor; and
- faces reputational cost when an agreement or obligation is forgotten.

This is an entry point, not a permanent exclusion of other users.

## Job To Be Done

> Before my next conversation, remind me what we agreed, what I owe, what they
> owe, and what remains unresolved, with a direct path to where it was said.

## Product Contract

Briefli maintains three durable memory types:

1. **Decision:** an agreement or change in direction.
2. **Commitment:** an obligation with an optional owner and due date.
3. **Open question:** an issue that still needs resolution.

Every machine-created memory starts as **suggested**, never as confirmed fact.
Each memory can carry the source meeting, transcript segment, excerpt, meeting
timestamp, audio range, and extraction confidence. A user can confirm, correct,
or reject it. Confirmed or corrected memories must never be silently deleted or
overwritten by a later summary regeneration.

## Signature Workflow

1. Briefli captures an online or in-person conversation.
2. The summary process proposes decisions, commitments, and open questions.
3. Briefli deterministically links each proposal to the strongest available
   transcript segment. Missing or weak evidence remains visibly unverified.
4. A short review inbox asks about suggested items only.
5. Confirmed records become eligible for a Next Conversation Brief.
6. Before a repeat conversation, Briefli shows prior decisions, open
   commitments, unresolved questions, and direct evidence links.

Review is exception-based. Briefli must not require users to approve generic
summary prose or perform routine cleanup after every meeting.

## Trust Invariants

- AI output is a suggestion until a user confirms or corrects it.
- A confidence score must not be presented as certainty.
- Evidence text must come from a stored transcript segment, not be invented by
  the language model or copied from summary prose.
- Confirmed and corrected records survive summary regeneration.
- Rejected suggestions do not silently reappear from the same meeting text.
- Source jumps degrade honestly: transcript-only evidence remains usable when
  an audio file or audio timing is unavailable.
- Local and cloud operations are distinguishable in the interface.
- The user can export durable records in an open format without an account.

## System Design

### Ownership boundaries

- SQLite is the source of truth for meetings, transcripts, and durable memory.
- Rust repositories own extraction reconciliation, state validation, evidence
  matching, and transactions.
- Tauri commands expose typed operations; the frontend does not mutate memory
  state optimistically without rollback.
- React owns presentation, review interaction, filters, and navigation.
- Summary templates provide candidates only. Markdown parsing is treated as an
  untrusted boundary and must fail without deleting durable user state.

### Memory ledger

The `meeting_memories` table is additive during migration from the existing
commitments implementation. Its important fields are:

- identity: `id`, `meeting_id`, `kind`, `text`;
- workflow: `review_status`, `resolution_status`;
- metadata: `owner`, `due_date`, `evidence_confidence`;
- evidence: `source_transcript_id`, `source_excerpt`, `source_timestamp`,
  `source_audio_start_time`, `source_audio_end_time`;
- reconciliation: immutable `source_fingerprint` and `suggested_text` keep a
  corrected or rejected suggestion from reappearing after regeneration;
- audit: `created_at`, `updated_at`, `reviewed_at`.

`review_status` is one of `suggested`, `confirmed`, `corrected`, or `rejected`.
`resolution_status` is `open` or `done` for commitments and open questions; a
decision normally remains `open` until supersession is implemented.

### Evidence matching

Evidence matching is deterministic and local:

1. Prefer an exact normalized excerpt supplied in a structured summary row.
2. Otherwise score stored transcript segments by token overlap with the memory
   text and any reference excerpt.
3. Persist only excerpts copied from the selected transcript segment.
4. Leave evidence fields empty when the score does not clear a documented
   threshold.

This is provenance linking, not proof that the AI interpretation is correct.
User confirmation remains the authority.

### Recurring spaces and briefs

- `memory_spaces` are user-created recurring contexts; Briefli does not infer a
  person, client, or project graph.
- A meeting can belong to at most one space and can be reassigned or unassigned.
- Space rename and deletion are explicit operations. Deletion also removes its
  assignments because SQLite foreign-key enforcement is currently disabled.
- Conversation briefs are deterministic database views. They include only
  confirmed or corrected decisions, open commitments, and open questions.
- Markdown and JSON exports preserve meeting and transcript-segment identifiers
  so the record remains traceable outside the application.

### Local product measurement

Prepare usage counters are disabled by default and stored only in the local
SQLite database. When explicitly enabled, Briefli counts brief opens and source
opens. The user can see the counters, disable future collection, or clear the
history. These counters do not use the analytics module or a network service.

## Delivery Plan

### Phase 1: Trusted Evidence Ledger

- [x] Add the additive memory schema and indexes.
- [x] Parse decisions, commitments, and open questions without destructive
  behavior on unknown summary formats.
- [x] Link suggestions to stored transcript evidence.
- [x] Preserve reviewed records during reconciliation.
- [x] Expose list, review, edit, and status commands through Tauri.
- [x] Replace the commitments-only page with a review-oriented Memory page.
- [x] Support transcript/audio navigation from every evidenced item.
- [x] Add repository, parser, state-transition, and frontend helper tests.

**Exit criteria:** reviewed records survive regeneration; invalid states are
rejected; evidence excerpts always originate in stored transcripts; the focused
test suite and TypeScript check pass.

### Phase 2: Repeat-Conversation Brief

- [x] Add manually managed spaces for a client, project, or recurring context.
- [x] Associate meetings with one space without automatic entity graphs.
- [x] Generate a deterministic brief from reviewed memories first.
- [x] Add an on-demand **Prepare** action with source links.
- [x] Measure brief opens and source use locally and only with user consent.

Calendar matching and notifications follow only after manual briefs demonstrate
repeated use.

### Phase 3: Follow-Through

- [x] Add overdue and weekly loose-end review.
- [ ] Suggest completion, contradiction, and supersession for confirmation.
- [x] Learn exact owner aliases from explicit corrections, keep exact
  correction/rejection preferences across regeneration, and let users inspect
  or forget learned names.
- [x] Export Markdown, JSON, and clipboard-friendly records.
- [ ] Add semantic retrieval only where observed full-text failures justify it.

### Evidence-gated follow-up

Completion, contradiction, and supersession suggestions are intentionally not
implemented as lexical heuristics. Work starts only after the app has multiple
confirmed records in the same recurring space and a structured candidate can
include both source memory IDs, newer transcript evidence, and an explicit user
confirmation step. Until then, Briefli must not imply that two similar or
different sentences contradict or replace one another.

Semantic retrieval remains gated on documented full-text search failures from
real use. The first implementation must be evaluated against those examples and
must preserve source citations. Embeddings are not added merely to complete a
roadmap checkbox.

## Explicit Non-Goals

Briefli will not currently build meeting bots, video recording, sales coaching,
sentiment or talk-time scoring, autonomous email sending, a CRM replacement,
team administration, a generalized knowledge graph, a digital twin, a template
marketplace, or a transcription-accuracy arms race.

The memory ledger is not a general task manager. It tracks obligations that
have meeting provenance.

## Quality Gates

Each implementation step requires the narrowest executable validation before
the next step. The complete slice requires:

- migration execution against a fresh SQLite database;
- Rust parser and repository tests, including regeneration and malformed input;
- Tauri command validation for all state transitions;
- frontend tests for review, filtering, error rollback, and evidence navigation;
- TypeScript type checking;
- Rust formatting and focused `cargo test`;
- a final production frontend build when the environment permits it.

No phase is considered complete because a screen renders. Persistence,
regeneration safety, failure behavior, and source navigation are part of the
feature.

## Product Measures

The north-star measure is **weekly repeat conversations entered with a
source-backed Briefli brief per activated user**.

Supporting measures are capture success, time to first successful capture,
median review time, suggestion acceptance rate, high-confidence false-positive
rate, brief usage before eligible meetings, time to recover a prior decision,
and four-week retained use. Transcript minutes and generated-summary counts are
inventory metrics, not primary measures of value.
