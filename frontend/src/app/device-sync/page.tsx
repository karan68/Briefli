'use client';

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import {
  CheckCircle2,
  Clock3,
  FileAudio,
  Loader2,
  RefreshCw,
  ShieldCheck,
  Smartphone,
  Square,
  Unlink,
  Wifi,
  XCircle,
} from 'lucide-react';
import { toast } from 'sonner';
import {
  deviceSyncService,
  type DeviceSyncStatus,
  type MobileCapture,
  type MobileCaptureStatus,
  type PairedDevice,
} from '@/services/deviceSyncService';
import { ConfirmationModal } from '@/components/ConfirmationModel/confirmation-modal';

const STATUS_LABELS: Record<MobileCaptureStatus, string> = {
  pending: 'Waiting',
  receiving: 'Transferring',
  received: 'Queued',
  importing: 'Transcribing',
  imported: 'In Briefli',
  failed: 'Needs attention',
};

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDuration(durationMs: number): string {
  const totalMinutes = Math.max(1, Math.round(durationMs / 60_000));
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return hours ? `${hours}h ${minutes}m` : `${minutes}m`;
}

function statusIcon(status: MobileCaptureStatus) {
  if (status === 'imported') return <CheckCircle2 className="h-4 w-4 text-briefli-confirmed" />;
  if (status === 'failed') return <XCircle className="h-4 w-4 text-[#c94c38]" />;
  if (status === 'receiving' || status === 'importing') {
    return <Loader2 className="h-4 w-4 animate-spin text-[#356f73]" />;
  }
  return <Clock3 className="h-4 w-4 text-briefli-muted" />;
}

export default function DeviceSyncPage() {
  const router = useRouter();
  const [status, setStatus] = useState<DeviceSyncStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [sessionBusy, setSessionBusy] = useState(false);
  const [unpairTarget, setUnpairTarget] = useState<PairedDevice | null>(null);
  const [unpairBusy, setUnpairBusy] = useState(false);

  const refresh = useCallback(async (quiet = false) => {
    try {
      const next = await deviceSyncService.getStatus();
      setStatus(next);
    } catch (error) {
      if (!quiet) {
        toast.error('Phone sync status is unavailable', {
          description: error instanceof Error ? error.message : String(error),
        });
      }
    } finally {
      if (!quiet) setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const interval = window.setInterval(() => void refresh(true), 2_000);
    return () => window.clearInterval(interval);
  }, [refresh]);

  const startSession = async () => {
    setSessionBusy(true);
    try {
      await deviceSyncService.startSession();
      await refresh(true);
    } catch (error) {
      toast.error('Could not start phone sync', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setSessionBusy(false);
    }
  };

  const stopSession = async () => {
    setSessionBusy(true);
    try {
      await deviceSyncService.stopSession();
      await refresh(true);
    } catch (error) {
      toast.error('Could not stop phone sync', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setSessionBusy(false);
    }
  };

  const unpairPhone = async () => {
    if (!unpairTarget) return;
    setUnpairBusy(true);
    try {
      await deviceSyncService.unpairDevice(unpairTarget.id);
      toast.success(`${unpairTarget.displayName} unpaired`);
      setUnpairTarget(null);
      await refresh(true);
    } catch (error) {
      toast.error('Could not unpair phone', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setUnpairBusy(false);
    }
  };

  const activeCaptureCount = useMemo(
    () => status?.captures.filter((capture) => capture.status !== 'imported').length ?? 0,
    [status?.captures],
  );

  if (loading) {
    return (
      <div className="flex h-screen items-center justify-center bg-briefli-paper text-briefli-muted">
        <Loader2 className="h-5 w-5 animate-spin" />
      </div>
    );
  }

  return (
    <>
    <main className="h-screen overflow-y-auto bg-briefli-paper custom-scrollbar">
      <header className="border-b border-briefli-line px-8 py-6">
        <div className="mx-auto flex max-w-6xl items-end justify-between gap-6">
          <div>
            <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-[#356f73]">
              Local capture handoff
            </p>
            <h1 className="font-brand text-3xl font-semibold text-briefli-ink">Phone sync</h1>
          </div>
          <button
            onClick={() => void refresh()}
            className="inline-flex h-9 items-center gap-2 rounded border border-briefli-line bg-briefli-surface px-3 text-sm text-briefli-muted hover:text-briefli-ink"
          >
            <RefreshCw className="h-4 w-4" />
            Refresh
          </button>
        </div>
      </header>

      <div className="mx-auto max-w-6xl px-8 py-7">
        <section className="grid min-h-[390px] grid-cols-1 border-b border-briefli-line pb-8 lg:grid-cols-[minmax(0,1fr)_320px] lg:gap-12">
          <div className="flex min-h-[340px] items-center justify-center border-b border-briefli-line py-8 lg:border-b-0 lg:border-r lg:pr-12">
            {status?.running && status.session ? (
              <div className="flex w-full max-w-xl items-center gap-8">
                <div className="flex h-[250px] w-[250px] flex-none items-center justify-center border border-briefli-line bg-white p-3">
                  <img
                    src={`data:image/svg+xml;base64,${status.session.qrSvg}`}
                    alt="Briefli phone pairing code"
                    className="h-full w-full"
                  />
                </div>
                <div className="min-w-0 space-y-5">
                  <div>
                    <div className="mb-2 flex items-center gap-2 text-sm font-medium text-briefli-ink">
                      <Wifi className="h-4 w-4 text-[#356f73]" />
                      Sync session open
                    </div>
                    <p className="break-all text-xs text-briefli-muted">{status.session.endpoint}</p>
                  </div>
                  <div className="flex items-center gap-2 text-xs text-briefli-muted">
                    <Clock3 className="h-4 w-4" />
                    Expires {new Date(status.session.expiresAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                  </div>
                  <button
                    onClick={stopSession}
                    disabled={sessionBusy}
                    className="inline-flex h-9 items-center gap-2 rounded border border-briefli-line bg-briefli-surface px-3 text-sm font-medium text-briefli-ink hover:bg-briefli-sidebar disabled:opacity-50"
                  >
                    {sessionBusy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Square className="h-4 w-4" />}
                    End session
                  </button>
                </div>
              </div>
            ) : (
              <div className="max-w-md text-center">
                <div className="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full border border-briefli-line bg-briefli-surface">
                  <Smartphone className="h-9 w-9 text-[#356f73]" />
                </div>
                <h2 className="font-brand text-2xl font-semibold text-briefli-ink">Ready for Briefli Capture</h2>
                <div className="mt-5 flex justify-center gap-5 text-xs text-briefli-muted">
                  <span className="inline-flex items-center gap-1.5"><Wifi className="h-4 w-4" /> Same network</span>
                  <span className="inline-flex items-center gap-1.5"><ShieldCheck className="h-4 w-4" /> Encrypted</span>
                </div>
                <button
                  onClick={startSession}
                  disabled={sessionBusy}
                  className="mt-7 inline-flex h-10 items-center gap-2 rounded bg-[#202621] px-4 text-sm font-medium text-white hover:bg-[#303832] disabled:opacity-50"
                >
                  {sessionBusy ? <Loader2 className="h-4 w-4 animate-spin" /> : <Smartphone className="h-4 w-4" />}
                  Start sync session
                </button>
              </div>
            )}
          </div>

          <aside className="py-8 lg:py-4">
            <div className="mb-6 flex items-center justify-between">
              <h2 className="text-sm font-semibold text-briefli-ink">Paired phones</h2>
              <span className="text-xs text-briefli-muted">{status?.pairedDevices.length ?? 0}</span>
            </div>
            {status?.pairedDevices.length ? (
              <div className="space-y-3">
                {status.pairedDevices.map((device) => (
                  <div key={device.id} className="flex items-center gap-3 border-b border-briefli-line pb-3">
                    <Smartphone className="h-5 w-5 text-briefli-muted" />
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium text-briefli-ink">{device.displayName}</p>
                      <p className="text-xs text-briefli-muted">
                        {device.lastSeenAt ? `Seen ${new Date(device.lastSeenAt).toLocaleDateString()}` : 'Paired'}
                      </p>
                    </div>
                    <button
                      onClick={() => setUnpairTarget(device)}
                      className="inline-flex h-8 flex-none items-center gap-1.5 rounded border border-briefli-line px-2 text-xs text-briefli-muted hover:border-[#c94c38] hover:text-[#a53e2f]"
                      aria-label={`Unpair ${device.displayName}`}
                      title={`Unpair ${device.displayName}`}
                    >
                      <Unlink className="h-3.5 w-3.5" />
                      Unpair
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-sm text-briefli-muted">No paired phones</p>
            )}
          </aside>
        </section>

        <section className="py-8">
          <div className="mb-5 flex items-center justify-between">
            <div>
              <h2 className="text-lg font-semibold text-briefli-ink">Phone captures</h2>
              <p className="mt-1 text-xs text-briefli-muted">{activeCaptureCount} active</p>
            </div>
          </div>

          {status?.captures.length ? (
            <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
              {status.captures.map((capture) => (
                <CaptureRow key={capture.id} capture={capture} onOpen={() => {
                  if (capture.meetingId) router.push(`/meeting-details?id=${capture.meetingId}`);
                }} />
              ))}
            </div>
          ) : (
            <div className="border-y border-briefli-line py-12 text-center text-sm text-briefli-muted">
              No phone captures yet
            </div>
          )}
        </section>
      </div>
    </main>
    <ConfirmationModal
      isOpen={unpairTarget !== null}
      title="Unpair phone"
      text={`Unpair ${unpairTarget?.displayName ?? 'this phone'}? It will need to scan a new QR code before it can sync again.`}
      confirmLabel="Unpair"
      isConfirming={unpairBusy}
      onConfirm={unpairPhone}
      onCancel={() => setUnpairTarget(null)}
    />
    </>
  );
}

function CaptureRow({ capture, onOpen }: { capture: MobileCapture; onOpen: () => void }) {
  const progress = capture.byteLength > 0
    ? Math.min(100, Math.round((capture.bytesReceived / capture.byteLength) * 100))
    : 0;

  return (
    <button
      onClick={onOpen}
      disabled={!capture.meetingId}
      className="min-h-[104px] border border-briefli-line bg-briefli-surface p-4 text-left disabled:cursor-default"
    >
      <div className="flex items-start gap-3">
        <FileAudio className="mt-0.5 h-5 w-5 flex-none text-[#356f73]" />
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold text-briefli-ink">{capture.title}</p>
              <p className="mt-1 text-xs text-briefli-muted">
                {formatDuration(capture.durationMs)} · {formatBytes(capture.byteLength)} · {new Date(capture.startedAt).toLocaleDateString()}
              </p>
            </div>
            <span className="inline-flex flex-none items-center gap-1.5 text-xs text-briefli-muted">
              {statusIcon(capture.status)}
              {STATUS_LABELS[capture.status]}
            </span>
          </div>
          {capture.status === 'receiving' && (
            <div className="mt-4 h-1 overflow-hidden bg-[#deddd3]">
              <div className="h-full bg-[#356f73]" style={{ width: `${progress}%` }} />
            </div>
          )}
          {capture.error && <p className="mt-3 line-clamp-2 text-xs text-[#a53e2f]">{capture.error}</p>}
        </div>
      </div>
    </button>
  );
}