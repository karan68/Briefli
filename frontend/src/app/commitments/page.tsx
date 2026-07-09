'use client';

import { useCallback, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ArrowRight, Check, Loader2, RefreshCw } from 'lucide-react';
import { commitmentsService, CommitmentItem } from '@/services/commitmentsService';

export default function CommitmentsPage() {
  const router = useRouter();
  const [items, setItems] = useState<CommitmentItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [syncing, setSyncing] = useState(false);

  const load = useCallback(async () => {
    const list = await commitmentsService.list();
    setItems(list);
  }, []);

  const sync = useCallback(async () => {
    setSyncing(true);
    try {
      await commitmentsService.sync();
      await load();
    } finally {
      setSyncing(false);
    }
  }, [load]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setLoading(true);
      try {
        await commitmentsService.sync();
        if (!cancelled) await load();
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [load]);

  const toggle = async (item: CommitmentItem) => {
    const next: 'open' | 'done' = item.status === 'done' ? 'open' : 'done';
    setItems((prev) => prev.map((i) => (i.id === item.id ? { ...i, status: next } : i)));
    try {
      await commitmentsService.setStatus(item.id, next);
    } catch {
      await load();
    }
  };

  const openItems = items.filter((i) => i.status !== 'done');
  const doneItems = items.filter((i) => i.status === 'done');
  const goToMeeting = (id: string) => router.push(`/meeting-details?id=${id}`);

  return (
    <div className="min-h-screen bg-gray-50 py-8 pr-8">
      <div className="mx-auto max-w-3xl">        <div className="mb-6 flex items-start justify-between">
          <div>
            <h1 className="text-2xl font-bold text-gray-900">Commitments</h1>
            <p className="mt-1 text-sm text-gray-500">
              Action items from every meeting, tracked in one place.
            </p>
          </div>
          <button
            onClick={sync}
            disabled={syncing}
            className="inline-flex items-center gap-2 rounded-md border border-gray-200 bg-white px-3 py-1.5 text-sm text-gray-700 hover:bg-gray-50 disabled:opacity-50"
          >
            <RefreshCw className={`h-4 w-4 ${syncing ? 'animate-spin' : ''}`} />
            Refresh
          </button>
        </div>

        {loading && items.length === 0 ? (
          <div className="flex items-center gap-2 text-sm text-gray-500">
            <Loader2 className="h-4 w-4 animate-spin" /> Gathering your commitments…
          </div>
        ) : items.length === 0 ? (
          <div className="rounded-lg border border-dashed border-gray-300 bg-white p-8 text-center text-sm text-gray-500">
            No action items yet. Generate a meeting summary and its action items
            will show up here.
          </div>
        ) : (
          <div className="space-y-6">
            <Section title={`Open (${openItems.length})`} items={openItems} onToggle={toggle} onOpenMeeting={goToMeeting} />
            {doneItems.length > 0 && (
              <Section title={`Done (${doneItems.length})`} items={doneItems} onToggle={toggle} onOpenMeeting={goToMeeting} muted />
            )}
          </div>
        )}
      </div>
    </div>
  );
}

interface SectionProps {
  title: string;
  items: CommitmentItem[];
  onToggle: (item: CommitmentItem) => void;
  onOpenMeeting: (meetingId: string) => void;
  muted?: boolean;
}

function Section({ title, items, onToggle, onOpenMeeting, muted }: SectionProps) {
  return (
    <div>
      <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-400">{title}</h2>
      <div className="space-y-2">
        {items.map((item) => (
          <div
            key={item.id}
            className={`flex items-start gap-3 rounded-lg border border-gray-200 bg-white p-3 ${muted ? 'opacity-60' : ''}`}
          >
            <button
              onClick={() => onToggle(item)}
              className={`mt-0.5 flex h-5 w-5 flex-shrink-0 items-center justify-center rounded border ${
                item.status === 'done'
                  ? 'border-gray-900 bg-gray-900 text-white'
                  : 'border-gray-300 hover:border-gray-400'
              }`}
              aria-label={item.status === 'done' ? 'Mark as open' : 'Mark as done'}
            >
              {item.status === 'done' && <Check className="h-3.5 w-3.5" />}
            </button>
            <div className="min-w-0 flex-1">
              <p className={`text-sm ${item.status === 'done' ? 'text-gray-500 line-through' : 'text-gray-900'}`}>
                {item.text}
              </p>
              {item.owner && !/^(unspecified|n\/?a|none|tbd)\.?$/i.test(item.owner.trim()) && (
                <span className="mt-1 mr-2 inline-block rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600">
                  {item.owner}
                </span>
              )}
              {item.dueDate && (
                <span className="mt-1 mr-2 inline-block rounded bg-blue-50 px-1.5 py-0.5 text-xs text-blue-700">
                  Due {item.dueDate}
                </span>
              )}
              <button
                onClick={() => onOpenMeeting(item.meetingId)}
                className="mt-1 inline-flex items-center gap-1 text-xs text-blue-600 hover:underline"
              >
                {item.meetingTitle}
                <ArrowRight className="h-3 w-3" />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
