'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import {
  ArrowUpRight,
  Check,
  CheckCircle2,
  CircleHelp,
  ClipboardCheck,
  FileQuestion,
  Loader2,
  Pencil,
  RefreshCw,
  RotateCcw,
  X,
} from 'lucide-react';
import { toast } from 'sonner';
import {
  countMemoryViews,
  filterMemories,
  isOverdue,
  memoryEvidencePath,
  reviewStatusForText,
  type MemoryView,
} from '@/lib/meeting-memory';
import {
  memoryService,
  type MeetingMemory,
  type MemoryOwnerAlias,
  type ResolutionStatus,
} from '@/services/memoryService';

interface EditDraft {
  text: string;
  owner: string;
  dueDate: string;
}

const VIEW_LABELS: Record<MemoryView, string> = {
  review: 'Review',
  weekly: 'Weekly',
  confirmed: 'Confirmed',
  open: 'Open loops',
};

export default function MemoryPage() {
  const router = useRouter();
  const [items, setItems] = useState<MeetingMemory[]>([]);
  const [ownerAliases, setOwnerAliases] = useState<MemoryOwnerAlias[]>([]);
  const [view, setView] = useState<MemoryView>('review');
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<EditDraft | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const [memories, aliases] = await Promise.all([
      memoryService.list(),
      memoryService.listOwnerAliases(),
    ]);
    setItems(memories);
    setOwnerAliases(aliases);
  }, []);

  const sync = useCallback(async () => {
    setSyncing(true);
    setError(null);
    try {
      await memoryService.sync();
      await load();
    } catch (syncError) {
      const message = syncError instanceof Error ? syncError.message : String(syncError);
      setError(message);
      toast.error('Memory could not be refreshed', { description: message });
    } finally {
      setSyncing(false);
    }
  }, [load]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setLoading(true);
      try {
        await memoryService.sync();
        const [memories, aliases] = await Promise.all([
          memoryService.list(),
          memoryService.listOwnerAliases(),
        ]);
        if (!cancelled) {
          setItems(memories);
          setOwnerAliases(aliases);
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(loadError instanceof Error ? loadError.message : String(loadError));
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const counts = useMemo(() => countMemoryViews(items), [items]);
  const visibleItems = useMemo(() => filterMemories(items, view), [items, view]);

  const beginEdit = (item: MeetingMemory) => {
    setEditingId(item.id);
    setDraft({ text: item.text, owner: item.owner ?? '', dueDate: item.dueDate ?? '' });
  };

  const review = async (
    item: MeetingMemory,
    status: 'confirmed' | 'corrected' | 'rejected',
    values: EditDraft,
  ) => {
    if (!values.text.trim()) {
      toast.error('Memory text cannot be empty');
      return;
    }
    setBusyId(item.id);
    try {
      await memoryService.review({
        id: item.id,
        status,
        text: values.text.trim(),
        owner: values.owner.trim() || null,
        dueDate: values.dueDate.trim() || null,
      });
      await load();
      setEditingId(null);
      setDraft(null);
    } catch (reviewError) {
      toast.error('Memory was not updated', {
        description: reviewError instanceof Error ? reviewError.message : String(reviewError),
      });
    } finally {
      setBusyId(null);
    }
  };

  const setResolution = async (item: MeetingMemory, status: ResolutionStatus) => {
    setBusyId(item.id);
    try {
      await memoryService.setResolution(item.id, status);
      setItems((current) =>
        current.map((memory) =>
          memory.id === item.id ? { ...memory, resolutionStatus: status } : memory,
        ),
      );
    } catch (resolutionError) {
      toast.error('Status was not updated', {
        description:
          resolutionError instanceof Error ? resolutionError.message : String(resolutionError),
      });
    } finally {
      setBusyId(null);
    }
  };

  const markFollowUpReviewed = async (item: MeetingMemory) => {
    setBusyId(item.id);
    try {
      await memoryService.markFollowUpReviewed(item.id);
      await load();
    } catch (reviewError) {
      toast.error('Follow-up review was not saved', {
        description: reviewError instanceof Error ? reviewError.message : String(reviewError),
      });
    } finally {
      setBusyId(null);
    }
  };

  const forgetOwnerAlias = async (alias: MemoryOwnerAlias) => {
    try {
      await memoryService.deleteOwnerAlias(alias.alias);
      setOwnerAliases((current) => current.filter((item) => item.alias !== alias.alias));
    } catch (deleteError) {
      toast.error('Learned name was not removed', {
        description: deleteError instanceof Error ? deleteError.message : String(deleteError),
      });
    }
  };

  return (
    <main className="h-screen overflow-y-auto bg-briefli-paper px-8 py-7 custom-scrollbar">
      <div className="mx-auto max-w-5xl">
        <header className="mb-7 flex items-end justify-between gap-4 border-b border-briefli-line pb-6">
          <div>
            <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-briefli-confirmed">Evidence ledger</p>
            <h1 className="font-brand text-3xl font-semibold text-briefli-ink">Memory</h1>
            <p className="mt-2 max-w-2xl text-sm text-briefli-muted">
              Review suggestions before they become part of your trusted record.
            </p>
          </div>
          <button
            onClick={sync}
            disabled={syncing}
            className="inline-flex items-center gap-2 rounded border border-briefli-line bg-briefli-surface px-3 py-2 text-sm text-briefli-muted hover:text-briefli-ink disabled:opacity-50"
          >
            <RefreshCw className={`h-4 w-4 ${syncing ? 'animate-spin' : ''}`} />
            Refresh
          </button>
        </header>

        <div className="mb-5 inline-flex border-b border-briefli-line" role="tablist">
          {(Object.keys(VIEW_LABELS) as MemoryView[]).map((candidate) => (
            <button
              key={candidate}
              role="tab"
              aria-selected={view === candidate}
              onClick={() => setView(candidate)}
              className={`border-b-2 px-4 py-2 text-sm ${
                view === candidate
                  ? 'border-briefli-capture font-semibold text-briefli-ink'
                  : 'border-transparent text-briefli-muted hover:text-briefli-ink'
              }`}
            >
              {VIEW_LABELS[candidate]} <span className="ml-1 opacity-70">{counts[candidate]}</span>
            </button>
          ))}
        </div>

        {error && (
          <div className="mb-4 rounded-md border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
            Briefli could not load meeting memory. {error}
          </div>
        )}

        {loading ? (
          <div className="flex items-center gap-2 py-8 text-sm text-gray-500">
            <Loader2 className="h-4 w-4 animate-spin" /> Gathering meeting memory...
          </div>
        ) : visibleItems.length === 0 ? (
          <EmptyView view={view} />
        ) : (
          <div className="overflow-hidden rounded border border-briefli-line bg-briefli-surface">
            {visibleItems.map((item) => {
              const isEditing = editingId === item.id;
              const isBusy = busyId === item.id;
              return (
                <article key={item.id} className="border-b border-briefli-line p-5 last:border-b-0">
                  <div className="flex items-start gap-3">
                    <KindIcon kind={item.kind} />
                    <div className="min-w-0 flex-1">
                      <div className="mb-1 flex flex-wrap items-center gap-2">
                        <span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-briefli-muted">
                          {kindLabel(item.kind)}
                        </span>
                        <span className={`rounded px-1.5 py-0.5 text-xs ${statusClass(item.reviewStatus)}`}>
                          {item.reviewStatus}
                        </span>
                      </div>

                      {isEditing && draft ? (
                        <EditMemory draft={draft} setDraft={setDraft} kind={item.kind} />
                      ) : (
                        <>
                          <p className="text-[15px] font-medium leading-6 text-briefli-ink">{item.text}</p>
                          <div className="mt-2 flex flex-wrap gap-2 text-xs text-gray-600">
                            {item.owner && <span className="rounded bg-[#efeee6] px-2 py-1">Owner: {item.owner}</span>}
                            {item.dueDate && <span className="rounded bg-[#f4ead2] px-2 py-1 text-[#7e5a12]">Due: {item.dueDate}</span>}
                            {isOverdue(item) && (
                              <span className="rounded bg-red-50 px-2 py-1 font-medium text-red-700">Overdue</span>
                            )}
                            {item.resolutionStatus === 'done' && (
                              <span className="rounded bg-emerald-50 px-2 py-1 text-emerald-700">Done</span>
                            )}
                          </div>
                        </>
                      )}

                      {item.sourceExcerpt ? (
                        <blockquote className="mt-3 border-l-2 border-briefli-line pl-3 text-xs leading-5 text-briefli-muted">
                          &ldquo;{item.sourceExcerpt}&rdquo;
                        </blockquote>
                      ) : (
                        <p className="mt-3 text-xs text-amber-700">
                          No transcript segment matched strongly enough. Verify in the meeting.
                        </p>
                      )}

                      <div className="mt-3 flex flex-wrap items-center gap-2">
                        <button
                          onClick={() => router.push(memoryEvidencePath(item))}
                          className="inline-flex items-center gap-1 text-xs font-medium text-briefli-confirmed hover:underline"
                        >
                          {item.meetingTitle}
                          {item.sourceTimestamp && ` · ${item.sourceTimestamp}`}
                          <ArrowUpRight className="h-3.5 w-3.5" />
                        </button>

                        <div className="ml-auto flex flex-wrap gap-2">
                          {item.reviewStatus === 'suggested' && !isEditing && (
                            <SuggestedActions
                              item={item}
                              busy={isBusy}
                              onConfirm={() => review(item, 'confirmed', draftFrom(item))}
                              onCorrect={() => beginEdit(item)}
                              onReject={() => review(item, 'rejected', draftFrom(item))}
                            />
                          )}

                          {isEditing && draft && (
                            <>
                              <button
                                onClick={() => review(item, reviewStatusForText(item, draft.text), draft)}
                                disabled={isBusy}
                                className="inline-flex items-center gap-1 rounded bg-briefli-ink px-2.5 py-1.5 text-xs text-white hover:bg-black disabled:opacity-50"
                              >
                                {isBusy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
                                Save
                              </button>
                              <button
                                onClick={() => { setEditingId(null); setDraft(null); }}
                                className="rounded-md border border-gray-200 px-2.5 py-1.5 text-xs text-gray-700 hover:bg-gray-50"
                              >
                                Cancel
                              </button>
                            </>
                          )}

                          {item.reviewStatus !== 'suggested' && item.kind !== 'decision' && (
                            <>
                              {view === 'weekly' && item.resolutionStatus === 'open' && (
                                <button
                                  onClick={() => markFollowUpReviewed(item)}
                                  disabled={isBusy}
                                  className="inline-flex items-center gap-1 rounded bg-briefli-ink px-2.5 py-1.5 text-xs text-white hover:bg-black disabled:opacity-50"
                                >
                                  {isBusy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
                                  Reviewed for now
                                </button>
                              )}
                              <button
                                onClick={() => setResolution(item, item.resolutionStatus === 'done' ? 'open' : 'done')}
                                disabled={isBusy}
                                className="inline-flex items-center gap-1 rounded-md border border-gray-200 px-2.5 py-1.5 text-xs text-gray-700 hover:bg-gray-50 disabled:opacity-50"
                              >
                                {item.resolutionStatus === 'done' ? <RotateCcw className="h-3.5 w-3.5" /> : <CheckCircle2 className="h-3.5 w-3.5" />}
                                {item.resolutionStatus === 'done' ? 'Reopen' : 'Mark done'}
                              </button>
                            </>
                          )}
                        </div>
                      </div>
                    </div>
                  </div>
                </article>
              );
            })}
          </div>
        )}

        {ownerAliases.length > 0 && (
          <section className="mt-8 border-t border-gray-200 pt-5">
            <h2 className="text-sm font-semibold text-gray-900">Learned names</h2>
            <p className="mt-1 text-xs text-gray-500">
              Learned only when you correct a commitment owner. Forgetting a name does not change saved records.
            </p>
            <div className="mt-3 flex flex-wrap gap-2">
              {ownerAliases.map((alias) => (
                <div
                  key={alias.alias}
                  className="inline-flex items-center gap-2 rounded-md border border-gray-200 bg-white px-2.5 py-1.5 text-xs text-gray-700"
                >
                  <span>{alias.alias} → {alias.canonicalName}</span>
                  <button
                    onClick={() => forgetOwnerAlias(alias)}
                    aria-label={`Forget learned name ${alias.alias}`}
                    title="Forget learned name"
                    className="text-gray-400 hover:text-red-700"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}
      </div>
    </main>
  );
}

function SuggestedActions({
  item,
  busy,
  onConfirm,
  onCorrect,
  onReject,
}: {
  item: MeetingMemory;
  busy: boolean;
  onConfirm: () => void;
  onCorrect: () => void;
  onReject: () => void;
}) {
  return (
    <>
      <button
        onClick={onConfirm}
        disabled={busy}
        className="inline-flex items-center gap-1 rounded bg-[#2f6e5d] px-2.5 py-1.5 text-xs text-white hover:bg-[#245648] disabled:opacity-50"
      >
        <Check className="h-3.5 w-3.5" /> Confirm
      </button>
      <button
        onClick={onCorrect}
        disabled={busy}
        className="inline-flex items-center gap-1 rounded border border-briefli-line px-2.5 py-1.5 text-xs text-briefli-muted hover:bg-briefli-sidebar"
      >
        <Pencil className="h-3.5 w-3.5" /> Correct
      </button>
      <button
        onClick={onReject}
        disabled={busy}
        aria-label={`Reject ${kindLabel(item.kind)}`}
        title="Reject suggestion"
        className="rounded-md border border-gray-200 p-1.5 text-gray-500 hover:bg-red-50 hover:text-red-700 disabled:opacity-50"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </>
  );
}

function EditMemory({
  draft,
  setDraft,
  kind,
}: {
  draft: EditDraft;
  setDraft: (draft: EditDraft) => void;
  kind: MeetingMemory['kind'];
}) {
  return (
    <div className="space-y-2">
      <textarea
        value={draft.text}
        onChange={(event) => setDraft({ ...draft, text: event.target.value })}
        rows={3}
        aria-label="Memory text"
        className="w-full resize-y rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-gray-500 focus:outline-none"
      />
      {kind === 'commitment' && (
        <div className="grid gap-2 sm:grid-cols-2">
          <input
            value={draft.owner}
            onChange={(event) => setDraft({ ...draft, owner: event.target.value })}
            placeholder="Owner"
            aria-label="Owner"
            className="rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-gray-500 focus:outline-none"
          />
          <input
            value={draft.dueDate}
            onChange={(event) => setDraft({ ...draft, dueDate: event.target.value })}
            placeholder="Due date"
            aria-label="Due date"
            className="rounded-md border border-gray-300 px-3 py-2 text-sm focus:border-gray-500 focus:outline-none"
          />
        </div>
      )}
    </div>
  );
}

function KindIcon({ kind }: { kind: MeetingMemory['kind'] }) {
  const icon = kind === 'decision'
    ? <ClipboardCheck className="h-5 w-5" />
    : kind === 'open_question'
      ? <CircleHelp className="h-5 w-5" />
      : <CheckCircle2 className="h-5 w-5" />;
  return (
    <div className="flex h-9 w-9 flex-none items-center justify-center rounded bg-briefli-sidebar text-briefli-muted">
      {icon}
    </div>
  );
}

function EmptyView({ view }: { view: MemoryView }) {
  const copy: Record<MemoryView, [string, string]> = {
    review: ['Nothing to review', 'Generate a meeting summary and new suggestions will appear here.'],
    weekly: ['Weekly review is clear', 'Confirmed open loops will return here seven days after you review them.'],
    confirmed: ['No confirmed memory yet', 'Confirm a suggested decision, commitment, or open question first.'],
    open: ['No open loops', 'Confirmed commitments and unresolved questions will appear here.'],
  };
  return (
    <div className="border-y border-dashed border-briefli-line px-6 py-12 text-center">
      <FileQuestion className="mx-auto h-6 w-6 text-gray-400" />
      <h2 className="mt-3 text-sm font-semibold text-gray-900">{copy[view][0]}</h2>
      <p className="mt-1 text-sm text-gray-500">{copy[view][1]}</p>
    </div>
  );
}

function draftFrom(item: MeetingMemory): EditDraft {
  return { text: item.text, owner: item.owner ?? '', dueDate: item.dueDate ?? '' };
}

function kindLabel(kind: MeetingMemory['kind']): string {
  return kind === 'open_question' ? 'Open question' : kind;
}

function statusClass(status: MeetingMemory['reviewStatus']): string {
  return status === 'suggested'
    ? 'bg-[#f4ead2] text-[#7e5a12]'
    : 'bg-[#e2eee9] text-briefli-confirmed';
}
