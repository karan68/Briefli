'use client';

import { useState, useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { RecordingControls } from '@/components/RecordingControls';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { usePermissionCheck } from '@/hooks/usePermissionCheck';
import { useRecordingState, RecordingStatus } from '@/contexts/RecordingStateContext';
import { useTranscripts } from '@/contexts/TranscriptContext';
import { useConfig } from '@/contexts/ConfigContext';
import { StatusOverlays } from '@/app/_components/StatusOverlays';
import Analytics from '@/lib/analytics';
import { SettingsModals } from './_components/SettingsModal';
import { TranscriptPanel } from './_components/TranscriptPanel';
import { useModalState } from '@/hooks/useModalState';
import { useRecordingStateSync } from '@/hooks/useRecordingStateSync';
import { useRecordingStart } from '@/hooks/useRecordingStart';
import { useRecordingStop } from '@/hooks/useRecordingStop';
import { useTranscriptRecovery } from '@/hooks/useTranscriptRecovery';
import { TranscriptRecovery } from '@/components/TranscriptRecovery';
import { indexedDBService } from '@/services/indexedDBService';
import { toast } from 'sonner';
import { useRouter } from 'next/navigation';
import { LockKeyhole } from 'lucide-react';

// Builds a horizontal sine path across `width` user units; periodic so a -50% shift loops seamlessly.
function sineWavePath(midY: number, amplitude: number, wavelength: number, width = 2000, step = 10) {
  const points: string[] = [];
  for (let x = 0; x <= width; x += step) {
    const y = (midY + amplitude * Math.sin((x / wavelength) * Math.PI * 2)).toFixed(1);
    points.push(`${x === 0 ? 'M' : 'L'} ${x} ${y}`);
  }
  return points.join(' ');
}

// Layered flowing "sound" lines for the idle backdrop (audio/conversation motif).
const WAVE_LINES = [
  { d: sineWavePath(110, 24, 500), colorClass: 'text-briefli-capture', opacity: 0.11, duration: 30, reverse: false },
  { d: sineWavePath(230, 40, 500), colorClass: 'text-briefli-confirmed', opacity: 0.13, duration: 24, reverse: true },
  { d: sineWavePath(350, 30, 500), colorClass: 'text-briefli-caution', opacity: 0.11, duration: 36, reverse: false },
  { d: sineWavePath(460, 44, 500), colorClass: 'text-briefli-capture', opacity: 0.10, duration: 28, reverse: true },
  { d: sineWavePath(555, 22, 500), colorClass: 'text-briefli-confirmed', opacity: 0.11, duration: 40, reverse: false },
];

// Full-screen animated backdrop: drifting warm glows + slow flowing sound-wave lines.
function HeroBackdrop() {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0 overflow-hidden">
      <motion.div
        className="absolute h-[560px] w-[560px] rounded-full bg-briefli-capture opacity-[0.10] blur-[140px]"
        style={{ left: '16%', top: '22%' }}
        animate={{ x: [-40, 50, -40], y: [-30, 40, -30] }}
        transition={{ duration: 26, repeat: Infinity, ease: 'easeInOut' }}
      />
      <motion.div
        className="absolute h-[480px] w-[480px] rounded-full bg-briefli-confirmed opacity-[0.08] blur-[150px]"
        style={{ right: '14%', bottom: '16%' }}
        animate={{ x: [30, -40, 30], y: [20, -30, 20] }}
        transition={{ duration: 34, repeat: Infinity, ease: 'easeInOut' }}
      />
      <motion.div
        className="absolute h-[380px] w-[380px] rounded-full bg-briefli-caution opacity-[0.06] blur-[150px]"
        style={{ left: '54%', top: '56%' }}
        animate={{ x: [0, -40, 20, 0], y: [0, 25, -20, 0] }}
        transition={{ duration: 30, repeat: Infinity, ease: 'easeInOut' }}
      />
      {WAVE_LINES.map((line, i) => (
        <motion.svg
          key={i}
          className="absolute inset-0 h-full w-[200%]"
          viewBox="0 0 2000 600"
          preserveAspectRatio="none"
          fill="none"
          animate={{ x: line.reverse ? ['-50%', '0%'] : ['0%', '-50%'] }}
          transition={{ duration: line.duration, repeat: Infinity, ease: 'linear' }}
        >
          <path
            d={line.d}
            stroke="currentColor"
            strokeWidth={1.5}
            vectorEffect="non-scaling-stroke"
            className={line.colorClass}
            style={{ strokeOpacity: line.opacity }}
          />
        </motion.svg>
      ))}
    </div>
  );
}

// Voice-reactive waveform for the recording state: bar heights follow live mic loudness.
// Falls back to a gentle idle motion if the webview can't open a mic stream.
function VoiceWaveform({ active }: { active: boolean }) {
  const barRefs = useRef<Array<HTMLSpanElement | null>>([]);
  const BAR_COUNT = 48;

  useEffect(() => {
    const bars = barRefs.current;
    const setBar = (i: number, scale: number) => {
      const el = bars[i];
      if (el) el.style.transform = `scaleY(${scale.toFixed(3)})`;
    };

    if (!active) {
      for (let i = 0; i < BAR_COUNT; i++) setBar(i, 0.28);
      return;
    }

    let raf = 0;
    let cancelled = false;
    let stream: MediaStream | null = null;
    let audioCtx: AudioContext | null = null;

    const runIdle = () => {
      const start = performance.now();
      const tick = () => {
        if (cancelled) return;
        const t = (performance.now() - start) / 1000;
        for (let i = 0; i < BAR_COUNT; i++) {
          setBar(i, 0.25 + 0.2 * (Math.sin(t * 2.2 + i * 0.5) * 0.5 + 0.5));
        }
        raf = requestAnimationFrame(tick);
      };
      tick();
    };

    const runReactive = async () => {
      try {
        if (!navigator.mediaDevices?.getUserMedia) throw new Error('no getUserMedia');
        stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        if (cancelled) { stream.getTracks().forEach((t) => t.stop()); return; }
        audioCtx = new AudioContext();
        await audioCtx.resume().catch(() => {});
        const source = audioCtx.createMediaStreamSource(stream);
        const analyser = audioCtx.createAnalyser();
        analyser.fftSize = 128;
        analyser.smoothingTimeConstant = 0.82;
        source.connect(analyser);
        const data = new Uint8Array(analyser.frequencyBinCount);
        const usable = Math.max(1, Math.floor(analyser.frequencyBinCount * 0.66));
        const tick = () => {
          if (cancelled) return;
          analyser.getByteFrequencyData(data);
          for (let i = 0; i < BAR_COUNT; i++) {
            const bin = Math.floor((i / BAR_COUNT) * usable);
            const v = data[bin] / 255;
            setBar(i, 0.12 + Math.pow(v, 0.8) * 1.7);
          }
          raf = requestAnimationFrame(tick);
        };
        tick();
      } catch {
        runIdle();
      }
    };

    runReactive();

    return () => {
      cancelled = true;
      if (raf) cancelAnimationFrame(raf);
      stream?.getTracks().forEach((t) => t.stop());
      audioCtx?.close().catch(() => {});
    };
  }, [active]);

  return (
    <div aria-hidden className="flex h-24 items-center justify-center gap-[3px]">
      {Array.from({ length: BAR_COUNT }).map((_, i) => (
        <span
          key={i}
          ref={(el) => { barRefs.current[i] = el; }}
          className="w-[4px] rounded-full bg-briefli-capture"
          style={{ height: 40, transformOrigin: 'center', transform: 'scaleY(0.12)', transition: 'transform 80ms linear' }}
        />
      ))}
    </div>
  );
}

export default function Home() {
  // Local page state (not moved to contexts)
  const [isRecording, setIsRecordingState] = useState(false);
  const [barHeights, setBarHeights] = useState(['58%', '76%', '58%']);
  const [showRecoveryDialog, setShowRecoveryDialog] = useState(false);

  // Use contexts for state management
  const { meetingTitle, transcripts } = useTranscripts();
  const { transcriptModelConfig, selectedDevices } = useConfig();
  const recordingState = useRecordingState();

  // Extract status from global state
  const { status, isStopping, isProcessing, isSaving } = recordingState;

  // Hooks
  const { hasMicrophone } = usePermissionCheck();
  const { setIsMeetingActive, isCollapsed: sidebarCollapsed, refetchMeetings } = useSidebar();
  const { modals, messages, showModal, hideModal } = useModalState(transcriptModelConfig);
  const { isRecordingDisabled, setIsRecordingDisabled } = useRecordingStateSync(isRecording, setIsRecordingState, setIsMeetingActive);
  const { handleRecordingStart } = useRecordingStart(isRecording, setIsRecordingState, showModal);

  // Get handleRecordingStop function and setIsStopping (state comes from global context)
  const { handleRecordingStop, setIsStopping } = useRecordingStop(
    setIsRecordingState,
    setIsRecordingDisabled
  );

  // Recovery hook
  const {
    recoverableMeetings,
    isLoading: isLoadingRecovery,
    isRecovering,
    checkForRecoverableTranscripts,
    recoverMeeting,
    loadMeetingTranscripts,
    deleteRecoverableMeeting
  } = useTranscriptRecovery();

  const router = useRouter();

  useEffect(() => {
    // Track page view
    Analytics.trackPageView('home');
  }, []);

  // Startup recovery check
  useEffect(() => {
    const performStartupChecks = async () => {
      try {
        // Skip recovery check if currently recording or processing stop
        // This prevents the recovery dialog from showing when:
        if (recordingState.isRecording ||
          status === RecordingStatus.STOPPING ||
          status === RecordingStatus.PROCESSING_TRANSCRIPTS ||
          status === RecordingStatus.SAVING) {
          console.log('Skipping recovery check - recording in progress or processing');
          return;
        }

        // 1. Clean up old meetings (7+ days)
        try {
          await indexedDBService.deleteOldMeetings(7);
        } catch (error) {
          console.warn('⚠️ Failed to clean up old meetings:', error);
        }

        // 2. Clean up saved meetings (24+ hours after save)
        try {
          await indexedDBService.deleteSavedMeetings(24);
        } catch (error) {
          console.warn('⚠️ Failed to clean up saved meetings:', error);
        }

        // 3. Always check for recoverable meetings on startup
        // Don't skip based on sessionStorage - we need to check every time
        await checkForRecoverableTranscripts();
      } catch (error) {
        console.error('Failed to perform startup checks:', error);
      }
    };

    performStartupChecks();
  }, [checkForRecoverableTranscripts, recordingState.isRecording, status]);

  // Watch for recoverable meetings changes and show dialog once per session
  useEffect(() => {
    // Only show dialog if we have meetings and haven't shown it yet this session
    if (recoverableMeetings.length > 0) {
      const shownThisSession = sessionStorage.getItem('recovery_dialog_shown');
      if (!shownThisSession) {
        setShowRecoveryDialog(true);
        sessionStorage.setItem('recovery_dialog_shown', 'true');
      }
    }
  }, [recoverableMeetings]);

  // Handle recovery with toast notifications and navigation
  const handleRecovery = async (meetingId: string) => {
    try {
      const result = await recoverMeeting(meetingId);

      if (result.success) {
        toast.success('Meeting recovered successfully!', {
          description: result.audioRecoveryStatus?.status === 'success'
            ? 'Transcripts and audio recovered'
            : 'Transcripts recovered (no audio available)',
          action: result.meetingId ? {
            label: 'View Meeting',
            onClick: () => {
              router.push(`/meeting-details?id=${result.meetingId}`);
            }
          } : undefined,
          duration: 10000,
        });

        // Refresh sidebar to show the newly recovered meeting
        await refetchMeetings();

        // If no more recoverable meetings, clear session flag so dialog can show again
        if (recoverableMeetings.length === 0) {
          sessionStorage.removeItem('recovery_dialog_shown');
        }

        // Auto-navigate after a short delay
        if (result.meetingId) {
          setTimeout(() => {
            router.push(`/meeting-details?id=${result.meetingId}`);
          }, 2000);
        }
      }
    } catch (error) {
      toast.error('Failed to recover meeting', {
        description: error instanceof Error ? error.message : 'Unknown error occurred',
      });
      throw error;
    }
  };

  // Handle dialog close - clear session flag if no meetings left
  const handleDialogClose = () => {
    setShowRecoveryDialog(false);
    // If user closes dialog and there are no more meetings, clear the flag
    // This allows the dialog to show again next session if new meetings appear
    if (recoverableMeetings.length === 0) {
      sessionStorage.removeItem('recovery_dialog_shown');
    }
  };

  useEffect(() => {
    if (recordingState.isRecording) {
      const interval = setInterval(() => {
        setBarHeights(prev => {
          const newHeights = [...prev];
          newHeights[0] = Math.random() * 20 + 10 + 'px';
          newHeights[1] = Math.random() * 20 + 10 + 'px';
          newHeights[2] = Math.random() * 20 + 10 + 'px';
          return newHeights;
        });
      }, 300);

      return () => clearInterval(interval);
    }
  }, [recordingState.isRecording]);

  // Computed values using global status
  const isProcessingStop = status === RecordingStatus.PROCESSING_TRANSCRIPTS || isProcessing;
  const isIdleEntry =
    transcripts.length === 0 &&
    !recordingState.isRecording &&
    !isProcessingStop &&
    status !== RecordingStatus.SAVING;
  const isRecordingEmpty = recordingState.isRecording && transcripts.length === 0;

  // Single record-control instance: shown inline in the idle hero, or as the bottom bar while active
  const recordingCta = (
    <RecordingControls
      isRecording={recordingState.isRecording}
      onRecordingStop={(callApi = true) => handleRecordingStop(callApi)}
      onRecordingStart={handleRecordingStart}
      onTranscriptReceived={() => { }} // Not actually used by RecordingControls
      onStopInitiated={() => setIsStopping(true)}
      barHeights={barHeights}
      onTranscriptionError={(message) => {
        showModal('errorAlert', message);
      }}
      isRecordingDisabled={isRecordingDisabled}
      isParentProcessing={isProcessingStop}
      selectedDevices={selectedDevices}
      meetingName={meetingTitle}
    />
  );

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      className="flex h-screen flex-col bg-briefli-paper"
    >
      {/* All Modals supported*/}
      <SettingsModals
        modals={modals}
        messages={messages}
        onClose={hideModal}
      />

      {/* Recovery Dialog */}
      <TranscriptRecovery
        isOpen={showRecoveryDialog}
        onClose={handleDialogClose}
        recoverableMeetings={recoverableMeetings}
        onRecover={handleRecovery}
        onDelete={deleteRecoverableMeeting}
        onLoadPreview={loadMeetingTranscripts}
      />
      <div className="relative flex flex-1 overflow-hidden">
        <TranscriptPanel
          isProcessingStop={isProcessingStop}
          isStopping={isStopping}
          showModal={showModal}
        />

        {/* Idle entry hero - large and centered in the visible content area */}
        {isIdleEntry && (
          <div
            className="pointer-events-none absolute inset-0 z-[5] flex flex-col items-center justify-center px-6 text-center transition-[padding] duration-300"
            style={{ paddingLeft: sidebarCollapsed ? '72px' : '240px' }}
          >
            <HeroBackdrop />

            <div
              className="pointer-events-none absolute top-4 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-briefli-confirmed"
              style={{ left: sidebarCollapsed ? '96px' : '264px' }}
            >
              <LockKeyhole className="h-3.5 w-3.5" />
              Stored on this device
            </div>

            <div className="pointer-events-auto relative z-10 flex w-full max-w-2xl flex-col items-center">
              <h1 className="font-brand text-[clamp(44px,5.2vw,68px)] font-semibold leading-[1.05] text-briefli-ink">
                Ready when you are
              </h1>
              <p className="mt-5 max-w-lg text-lg leading-8 text-briefli-muted">
                Start recording and Briefli keeps track of the decisions, promises, and open questions as they happen.
              </p>
              <div className="mt-10">
                {recordingCta}
              </div>
              <p className="mt-6 text-sm text-briefli-muted">
                Works for online calls and in-person conversations
              </p>
            </div>
          </div>
        )}

        {/* Recording state - big and centered, with a voice-reactive waveform */}
        {isRecordingEmpty && (
          <div
            className="pointer-events-none absolute inset-0 z-[5] flex flex-col items-center justify-center px-6 text-center transition-[padding] duration-300"
            style={{ paddingLeft: sidebarCollapsed ? '72px' : '240px' }}
          >
            <HeroBackdrop />

            <div className="pointer-events-auto relative z-10 flex w-full max-w-2xl flex-col items-center">
              <VoiceWaveform active={recordingState.isRecording && !recordingState.isPaused} />
              <h1 className="mt-8 font-brand text-[clamp(40px,4.8vw,60px)] font-semibold leading-[1.05] text-briefli-ink">
                {recordingState.isPaused ? 'Paused' : 'Listening\u2026'}
              </h1>
              <p className="mt-4 max-w-md text-lg leading-8 text-briefli-muted">
                {recordingState.isPaused
                  ? 'Resume when you\u2019re ready.'
                  : 'Speak \u2014 Briefli is capturing every word.'}
              </p>
              <div className="mt-10">
                {recordingCta}
              </div>
            </div>
          </div>
        )}

        {/* Recording controls - bottom bar after transcript content appears */}
        {!isIdleEntry &&
          !isRecordingEmpty &&
          (hasMicrophone || isRecording) &&
          status !== RecordingStatus.PROCESSING_TRANSCRIPTS &&
          status !== RecordingStatus.SAVING && (
            <div className="fixed bottom-8 left-0 right-0 z-10">
              <div
                className="flex justify-center transition-[margin] duration-300"
                style={{
                  marginLeft: sidebarCollapsed ? '72px' : '15rem'
                }}
              >
                <div className="flex w-full max-w-[760px] justify-start px-4">
                  <div className="flex items-center">
                    {recordingCta}
                  </div>
                </div>
              </div>
            </div>
          )}

        {/* Status Overlays - Processing and Saving */}
        <StatusOverlays
          isProcessing={status === RecordingStatus.PROCESSING_TRANSCRIPTS && !recordingState.isRecording}
          isSaving={status === RecordingStatus.SAVING}
          sidebarCollapsed={sidebarCollapsed}
        />
      </div>
    </motion.div>
  );
}
