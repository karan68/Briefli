'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import {
  ArrowUpRight,
  BriefcaseBusiness,
  Check,
  CheckCircle2,
  CircleHelp,
  Clipboard,
  ClipboardCheck,
  Loader2,
  Pencil,
  Plus,
  Trash2,
  X,
} from 'lucide-react';
import { toast } from 'sonner';
import { formatBriefJson, formatBriefMarkdown, hasBriefContent } from '@/lib/conversation-brief';
import { memoryEvidencePath } from '@/lib/meeting-memory';
import type { MeetingMemory } from '@/services/memoryService';
import {
  memorySpaceService,
  type ConversationBrief,
  type LocalBriefMetrics,
  type MeetingSpaceAssignment,
  type MemorySpace,
} from '@/services/memorySpaceService';

export default function PreparePage() {
  const router = useRouter();
  const [spaces, setSpaces] = useState<MemorySpace[]>([]);
  const [assignments, setAssignments] = useState<MeetingSpaceAssignment[]>([]);
  const [selectedSpaceId, setSelectedSpaceId] = useState<string | null>(null);
  const [brief, setBrief] = useState<ConversationBrief | null>(null);
  const [newSpaceName, setNewSpaceName] = useState('');
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [busyMeetingId, setBusyMeetingId] = useState<string | null>(null);
  const [busySpaceId, setBusySpaceId] = useState<string | null>(null);
  const [editingSpaceId, setEditingSpaceId] = useState<string | null>(null);
  const [editingSpaceName, setEditingSpaceName] = useState('');
  const [pendingDeleteSpaceId, setPendingDeleteSpaceId] = useState<string | null>(null);
  const [metrics, setMetrics] = useState<LocalBriefMetrics | null>(null);
  const [savingMetrics, setSavingMetrics] = useState(false);
  const trackedBriefsRef = useRef<Set<string>>(new Set());

  const loadOverview = useCallback(async () => {
    const [spaceList, assignmentList] = await Promise.all([
      memorySpaceService.list(),
      memorySpaceService.assignments(),
    ]);
    setSpaces(spaceList);
    setAssignments(assignmentList);
    setSelectedSpaceId((current) =>
      current && spaceList.some((space) => space.id === current)
        ? current
        : spaceList[0]?.id ?? null,
    );
  }, []);

  const loadMetrics = useCallback(async () => {
    setMetrics(await memorySpaceService.metrics());
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [spaceList, assignmentList, localMetrics] = await Promise.all([
          memorySpaceService.list(),
          memorySpaceService.assignments(),
          memorySpaceService.metrics(),
        ]);
        if (!cancelled) {
          setSpaces(spaceList);
          setAssignments(assignmentList);
          setMetrics(localMetrics);
          setSelectedSpaceId(spaceList[0]?.id ?? null);
        }
      } catch (error) {
        if (!cancelled) {
          toast.error('Preparation spaces could not be loaded', {
            description: error instanceof Error ? error.message : String(error),
          });
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    if (!selectedSpaceId) {
      setBrief(null);
      return;
    }
    memorySpaceService
      .brief(selectedSpaceId)
      .then((nextBrief) => {
        if (!cancelled) setBrief(nextBrief);
      })
      .catch((error) => {
        if (!cancelled) {
          setBrief(null);
          toast.error('Brief could not be prepared', {
            description: error instanceof Error ? error.message : String(error),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [selectedSpaceId, assignments]);

  useEffect(() => {
    if (
      !metrics?.enabled ||
      !brief ||
      !hasBriefContent(brief) ||
      trackedBriefsRef.current.has(brief.space.id)
    ) {
      return;
    }
    let cancelled = false;
    memorySpaceService
      .recordMetric('brief_opened', brief.space.id, null)
      .then((recorded) => {
        if (!cancelled && recorded) {
          trackedBriefsRef.current.add(brief.space.id);
          return loadMetrics();
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [brief, metrics?.enabled, loadMetrics]);

  const selectedSpace = useMemo(
    () => spaces.find((space) => space.id === selectedSpaceId) ?? null,
    [spaces, selectedSpaceId],
  );

  const createSpace = async () => {
    const name = newSpaceName.trim();
    if (!name) return;
    setCreating(true);
    try {
      const created = await memorySpaceService.create(name);
      setNewSpaceName('');
      await loadOverview();
      setSelectedSpaceId(created.id);
    } catch (error) {
      toast.error('Space was not created', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setCreating(false);
    }
  };

  const assignMeeting = async (meetingId: string, spaceId: string | null) => {
    setBusyMeetingId(meetingId);
    try {
      await memorySpaceService.assign(meetingId, spaceId);
      await loadOverview();
    } catch (error) {
      toast.error('Meeting assignment was not updated', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusyMeetingId(null);
    }
  };

  const beginRename = (space: MemorySpace) => {
    setPendingDeleteSpaceId(null);
    setEditingSpaceId(space.id);
    setEditingSpaceName(space.name);
  };

  const renameSpace = async () => {
    if (!editingSpaceId || !editingSpaceName.trim()) return;
    setBusySpaceId(editingSpaceId);
    try {
      await memorySpaceService.rename(editingSpaceId, editingSpaceName.trim());
      setEditingSpaceId(null);
      setEditingSpaceName('');
      await loadOverview();
    } catch (error) {
      toast.error('Space was not renamed', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusySpaceId(null);
    }
  };

  const deleteSpace = async (space: MemorySpace) => {
    setBusySpaceId(space.id);
    try {
      await memorySpaceService.delete(space.id);
      setPendingDeleteSpaceId(null);
      if (editingSpaceId === space.id) {
        setEditingSpaceId(null);
        setEditingSpaceName('');
      }
      await loadOverview();
      toast.success(`${space.name} deleted`);
    } catch (error) {
      toast.error('Space was not deleted', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setBusySpaceId(null);
    }
  };

  const copyBrief = async (format: 'markdown' | 'json') => {
    if (!brief) return;
    try {
      const content = format === 'markdown' ? formatBriefMarkdown(brief) : formatBriefJson(brief);
      await navigator.clipboard.writeText(content);
      toast.success(`${format === 'markdown' ? 'Markdown' : 'JSON'} brief copied`);
    } catch (error) {
      toast.error('Brief could not be copied', {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  };

  const setMetricsEnabled = async (enabled: boolean) => {
    setSavingMetrics(true);
    try {
      await memorySpaceService.setMetricsEnabled(enabled);
      if (!enabled) trackedBriefsRef.current.clear();
      await loadMetrics();
    } catch (error) {
      toast.error('Local usage counters were not updated', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setSavingMetrics(false);
    }
  };

  const clearMetrics = async () => {
    setSavingMetrics(true);
    try {
      await memorySpaceService.clearMetrics();
      trackedBriefsRef.current.clear();
      if (brief) trackedBriefsRef.current.add(brief.space.id);
      await loadMetrics();
    } catch (error) {
      toast.error('Local usage history was not cleared', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setSavingMetrics(false);
    }
  };

  const openEvidence = async (item: MeetingMemory) => {
    if (metrics?.enabled && selectedSpaceId) {
      try {
        const recorded = await memorySpaceService.recordMetric(
          'source_opened',
          selectedSpaceId,
          item.id,
        );
        if (recorded) await loadMetrics();
      } catch {
        // Local measurement must never block access to meeting evidence.
      }
    }
    router.push(memoryEvidencePath(item));
  };

  return (
    <main className="h-screen overflow-y-auto bg-briefli-paper px-8 py-7 custom-scrollbar">
      <div className="mx-auto max-w-7xl">
        <header className="mb-7 border-b border-briefli-line pb-6">
          <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-briefli-confirmed">Next conversation brief</p>
          <h1 className="font-brand text-3xl font-semibold text-briefli-ink">Prepare</h1>
          <p className="mt-2 text-sm text-briefli-muted">
            Bring confirmed decisions and open loops back before the conversation starts.
          </p>
        </header>

        {loading ? (
          <div className="flex items-center gap-2 py-10 text-sm text-gray-500">
            <Loader2 className="h-4 w-4 animate-spin" /> Loading conversation spaces...
          </div>
        ) : (
          <div className="grid gap-8 lg:grid-cols-[250px_minmax(0,1fr)]">
            <aside className="space-y-6 border-r border-briefli-line pr-6">
              <section>
                <h2 className="mb-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-briefli-muted">Recurring spaces</h2>
                <div className="space-y-1">
                  {spaces.map((space) => {
                    const isSelected = selectedSpaceId === space.id;
                    const isEditing = editingSpaceId === space.id;
                    const isDeleting = pendingDeleteSpaceId === space.id;
                    const isBusy = busySpaceId === space.id;

                    if (isEditing) {
                      return (
                        <div key={space.id} className="flex items-center gap-1 rounded-md border border-gray-200 bg-white p-1">
                          <input
                            value={editingSpaceName}
                            onChange={(event) => setEditingSpaceName(event.target.value)}
                            onKeyDown={(event) => {
                              if (event.key === 'Enter') void renameSpace();
                              if (event.key === 'Escape') setEditingSpaceId(null);
                            }}
                            maxLength={100}
                            autoFocus
                            aria-label={`Rename ${space.name}`}
                            className="min-w-0 flex-1 px-2 py-1 text-sm focus:outline-none"
                          />
                          <button
                            onClick={renameSpace}
                            disabled={isBusy || !editingSpaceName.trim()}
                            aria-label="Save space name"
                            title="Save space name"
                            className="rounded p-1.5 text-emerald-700 hover:bg-emerald-50 disabled:opacity-40"
                          >
                            {isBusy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
                          </button>
                          <button
                            onClick={() => setEditingSpaceId(null)}
                            aria-label="Cancel rename"
                            title="Cancel rename"
                            className="rounded p-1.5 text-gray-500 hover:bg-gray-100"
                          >
                            <X className="h-3.5 w-3.5" />
                          </button>
                        </div>
                      );
                    }

                    return (
                      <div
                        key={space.id}
                        className={`group flex items-center rounded text-sm ${
                          isSelected ? 'bg-[#202621] text-white' : 'text-briefli-muted'
                        }`}
                      >
                        <button
                          onClick={() => {
                            setSelectedSpaceId(space.id);
                            setPendingDeleteSpaceId(null);
                          }}
                          className={`min-w-0 flex-1 truncate px-3 py-2 text-left ${
                            isSelected ? '' : 'hover:bg-briefli-sidebar hover:text-briefli-ink'
                          }`}
                        >
                          {space.name}
                        </button>
                        <span className="px-1 text-xs opacity-60">{space.meetingCount}</span>
                        {isDeleting ? (
                          <div className="flex items-center gap-1 pr-1">
                            <button
                              onClick={() => deleteSpace(space)}
                              disabled={isBusy}
                              className="rounded bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700 disabled:opacity-50"
                            >
                              {isBusy ? 'Deleting' : 'Delete'}
                            </button>
                            <button
                              onClick={() => setPendingDeleteSpaceId(null)}
                              aria-label="Cancel delete"
                              title="Cancel delete"
                              className="rounded p-1 hover:bg-white/10"
                            >
                              <X className="h-3.5 w-3.5" />
                            </button>
                          </div>
                        ) : (
                          <div className="flex items-center pr-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                            <button
                              onClick={() => beginRename(space)}
                              aria-label={`Rename ${space.name}`}
                              title="Rename space"
                              className="rounded p-1.5 hover:bg-white/10"
                            >
                              <Pencil className="h-3.5 w-3.5" />
                            </button>
                            <button
                              onClick={() => {
                                setEditingSpaceId(null);
                                setPendingDeleteSpaceId(space.id);
                              }}
                              aria-label={`Delete ${space.name}`}
                              title="Delete space"
                              className="rounded p-1.5 hover:bg-red-500/20 hover:text-red-300"
                            >
                              <Trash2 className="h-3.5 w-3.5" />
                            </button>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
                <div className="mt-2 flex gap-2">
                  <input
                    value={newSpaceName}
                    onChange={(event) => setNewSpaceName(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') void createSpace();
                    }}
                    maxLength={100}
                    placeholder="Client or project"
                    aria-label="New recurring space name"
                    className="min-w-0 flex-1 rounded border border-briefli-line bg-briefli-surface px-3 py-2 text-sm focus:border-briefli-ink focus:outline-none"
                  />
                  <button
                    onClick={createSpace}
                    disabled={creating || !newSpaceName.trim()}
                    aria-label="Create recurring space"
                    title="Create recurring space"
                    className="rounded bg-[#202621] p-2 text-white hover:bg-black disabled:opacity-40"
                  >
                    {creating ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
                  </button>
                </div>
              </section>

              {metrics && (
                <section className="border-t border-briefli-line pt-4">
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <h2 className="text-[10px] font-semibold uppercase tracking-[0.12em] text-briefli-muted">Local usage counters</h2>
                      <p className="mt-1 text-xs text-briefli-muted">Stored only on this device.</p>
                    </div>
                    <input
                      type="checkbox"
                      checked={metrics.enabled}
                      disabled={savingMetrics}
                      onChange={(event) => setMetricsEnabled(event.target.checked)}
                      aria-label="Enable local Prepare usage counters"
                      className="h-4 w-4"
                    />
                  </div>
                  {metrics.enabled && (
                    <div className="mt-3 flex items-end justify-between gap-3 border-t border-gray-100 pt-3">
                      <dl className="grid grid-cols-2 gap-4 text-xs">
                        <div>
                          <dt className="text-gray-500">Briefs opened</dt>
                          <dd className="mt-0.5 font-semibold text-gray-900">{metrics.briefOpenCount}</dd>
                        </div>
                        <div>
                          <dt className="text-gray-500">Sources opened</dt>
                          <dd className="mt-0.5 font-semibold text-gray-900">{metrics.sourceOpenCount}</dd>
                        </div>
                      </dl>
                      <button
                        onClick={clearMetrics}
                        disabled={savingMetrics || (metrics.briefOpenCount === 0 && metrics.sourceOpenCount === 0)}
                        className="text-xs text-gray-500 hover:text-red-700 disabled:opacity-40"
                      >
                        Clear
                      </button>
                    </div>
                  )}
                </section>
              )}

              <section>
                <h2 className="mb-2 text-[10px] font-semibold uppercase tracking-[0.12em] text-briefli-muted">Meeting assignments</h2>
                {assignments.length === 0 ? (
                  <p className="text-sm text-gray-500">Record a meeting to assign it here.</p>
                ) : (
                  <div className="max-h-[52vh] space-y-2 overflow-y-auto pr-1">
                    {assignments.map((assignment) => (
                      <label key={assignment.meetingId} className="block border-b border-briefli-line py-2 last:border-b-0">
                        <span className="mb-1 block truncate text-xs font-medium text-gray-700">
                          {assignment.meetingTitle}
                        </span>
                        <select
                          value={assignment.spaceId ?? ''}
                          disabled={busyMeetingId === assignment.meetingId}
                          onChange={(event) => assignMeeting(assignment.meetingId, event.target.value || null)}
                          aria-label={`Space for ${assignment.meetingTitle}`}
                          className="w-full rounded border border-briefli-line bg-briefli-surface px-2 py-1.5 text-xs text-briefli-muted"
                        >
                          <option value="">Unassigned</option>
                          {spaces.map((space) => (
                            <option key={space.id} value={space.id}>{space.name}</option>
                          ))}
                        </select>
                      </label>
                    ))}
                  </div>
                )}
              </section>
            </aside>

            <section className="min-w-0 pb-10">
              {!selectedSpace ? (
                <EmptyBrief
                  title="Create a recurring space"
                  description="Group prior meetings by a client or project to prepare from confirmed memory."
                />
              ) : !brief ? (
                <div className="flex items-center gap-2 py-8 text-sm text-gray-500">
                  <Loader2 className="h-4 w-4 animate-spin" /> Preparing {selectedSpace.name}...
                </div>
              ) : !hasBriefContent(brief) ? (
                <EmptyBrief
                  title={`No confirmed memory for ${selectedSpace.name}`}
                  description="Assign meetings to this space, then confirm useful decisions, commitments, and questions in Memory."
                />
              ) : (
                <div>
                  <div className="mb-7 flex flex-wrap items-center gap-3 border-b border-briefli-line pb-5">
                    <div className="flex h-10 w-10 items-center justify-center rounded bg-[#202621] text-white">
                      <BriefcaseBusiness className="h-5 w-5" />
                    </div>
                    <div>
                      <h2 className="font-brand text-2xl font-semibold text-briefli-ink">{brief.space.name}</h2>
                      <p className="text-xs text-briefli-muted">Prepared from {brief.space.meetingCount} assigned meetings</p>
                    </div>
                    <div className="ml-auto flex gap-2">
                      <button
                        onClick={() => copyBrief('markdown')}
                        className="inline-flex items-center gap-1 rounded border border-briefli-line bg-briefli-surface px-2.5 py-1.5 text-xs text-briefli-muted hover:text-briefli-ink"
                      >
                        <Clipboard className="h-3.5 w-3.5" /> Markdown
                      </button>
                      <button
                        onClick={() => copyBrief('json')}
                        className="inline-flex items-center gap-1 rounded border border-briefli-line bg-briefli-surface px-2.5 py-1.5 text-xs text-briefli-muted hover:text-briefli-ink"
                      >
                        <Clipboard className="h-3.5 w-3.5" /> JSON
                      </button>
                    </div>
                  </div>
                  <div className="space-y-8">
                    <BriefSection
                      title="Decisions"
                      items={brief.decisions}
                      icon={<ClipboardCheck className="h-4 w-4" />}
                      onOpen={openEvidence}
                    />
                    <BriefSection
                      title="Open commitments"
                      items={brief.commitments}
                      icon={<CheckCircle2 className="h-4 w-4" />}
                      onOpen={openEvidence}
                    />
                    <BriefSection
                      title="Unresolved questions"
                      items={brief.openQuestions}
                      icon={<CircleHelp className="h-4 w-4" />}
                      onOpen={openEvidence}
                    />
                  </div>
                </div>
              )}
            </section>
          </div>
        )}
      </div>
    </main>
  );
}

function BriefSection({
  title,
  items,
  icon,
  onOpen,
}: {
  title: string;
  items: MeetingMemory[];
  icon: React.ReactNode;
  onOpen: (item: MeetingMemory) => void;
}) {
  if (items.length === 0) return null;
  return (
    <section>
      <h3 className="mb-3 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.1em] text-briefli-muted">
        {icon} {title} <span className="text-xs font-normal text-gray-400">{items.length}</span>
      </h3>
      <div className="border-t border-briefli-line">
        {items.map((item) => (
          <article key={item.id} className="border-b border-briefli-line py-5">
            <p className="text-[15px] font-medium leading-6 text-briefli-ink">{item.text}</p>
            <div className="mt-2 flex flex-wrap gap-2 text-xs text-gray-600">
              {item.owner && <span className="rounded bg-briefli-sidebar px-2 py-1">Owner: {item.owner}</span>}
              {item.dueDate && <span className="rounded bg-[#f4ead2] px-2 py-1 text-[#7e5a12]">Due: {item.dueDate}</span>}
            </div>
            {item.sourceExcerpt && (
              <blockquote className="mt-3 border-l-2 border-briefli-line pl-3 text-xs leading-5 text-briefli-muted">
                &ldquo;{item.sourceExcerpt}&rdquo;
              </blockquote>
            )}
            <button
              onClick={() => onOpen(item)}
              className="mt-3 inline-flex items-center gap-1 text-xs font-medium text-briefli-confirmed hover:underline"
            >
              {item.meetingTitle}{item.sourceTimestamp && ` · ${item.sourceTimestamp}`}
              <ArrowUpRight className="h-3.5 w-3.5" />
            </button>
          </article>
        ))}
      </div>
    </section>
  );
}

function EmptyBrief({ title, description }: { title: string; description: string }) {
  return (
    <div className="border-y border-dashed border-briefli-line px-8 py-14 text-center">
      <BriefcaseBusiness className="mx-auto h-7 w-7 text-gray-400" />
      <h2 className="mt-3 text-sm font-semibold text-gray-900">{title}</h2>
      <p className="mx-auto mt-1 max-w-md text-sm text-gray-500">{description}</p>
    </div>
  );
}
