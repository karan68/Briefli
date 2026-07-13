"use client";

import { Transcript, TranscriptSegmentData } from '@/types';
import { TranscriptView } from '@/components/TranscriptView';
import { VirtualizedTranscriptView, type TranscriptViewHandle } from '@/components/VirtualizedTranscriptView';
import { TranscriptButtonGroup } from './TranscriptButtonGroup';
import { MeetingTimeline } from './MeetingTimeline';
import { MeetingAudioPlayer } from './MeetingAudioPlayer';
import {
  generateTimeline,
  findActiveMarkerId,
  findActiveSegmentIdAtTime,
  type TimelineMarker,
} from '@/lib/meeting-timeline';
import { useAudioPlayer } from '@/hooks/useAudioPlayer';
import { useSearchParams } from 'next/navigation';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

interface TranscriptPanelProps {
  transcripts: Transcript[];
  customPrompt: string;
  onPromptChange: (value: string) => void;
  onCopyTranscript: () => void;
  onOpenMeetingFolder: () => Promise<void>;
  isRecording: boolean;
  disableAutoScroll?: boolean;

  // Optional pagination props (when using virtualization)
  usePagination?: boolean;
  segments?: TranscriptSegmentData[];
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;

  // Retranscription props
  meetingId?: string;
  meetingFolderPath?: string | null;
  onRefetchTranscripts?: () => Promise<void>;
}

export function TranscriptPanel({
  transcripts,
  customPrompt,
  onPromptChange,
  onCopyTranscript,
  onOpenMeetingFolder,
  isRecording,
  disableAutoScroll = false,
  usePagination = false,
  segments,
  hasMore,
  isLoadingMore,
  totalCount,
  loadedCount,
  onLoadMore,
  meetingId,
  meetingFolderPath,
  onRefetchTranscripts,
}: TranscriptPanelProps) {
  // Segment to jump to when arriving from a cross-meeting search result
  // (e.g. /meeting-details?id=...&segment=...).
  const searchParams = useSearchParams();
  const targetSegmentId = searchParams.get('segment');

  // Convert transcripts to segments if pagination is not used but we want virtualization
  const convertedSegments = useMemo(() => {
    if (usePagination && segments) {
      return segments;
    }
    // Convert transcripts to segments for virtualization
    return transcripts.map(t => ({
      id: t.id,
      timestamp: t.audio_start_time ?? 0,
      endTime: t.audio_end_time,
      text: t.text,
      confidence: t.confidence,
    }));
  }, [transcripts, usePagination, segments]);

  // Imperative handle to the transcript view for timeline-driven navigation.
  const transcriptViewRef = useRef<TranscriptViewHandle>(null);
  const [activeMarkerId, setActiveMarkerId] = useState<string | null>(null);
  const [activeSegmentId, setActiveSegmentId] = useState<string | null>(null);
  const [isTimelineOpen, setIsTimelineOpen] = useState(true);

  // Resolve the finalized meeting recording (saved as `audio.mp4` in the
  // meeting folder) so it can be played back and seeked from the timeline.
  // Note: recordings live outside the app's data dir (e.g. ~/Music), so we do
  // not use the permission-scoped `plugin-fs` here. We build the path and let
  // the audio loader (`read_audio_file`) report whether it can be read; the
  // player is only shown once the audio loads successfully.
  const [audioPath, setAudioPath] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    const resolveAudio = async () => {
      if (isRecording || !meetingFolderPath) {
        setAudioPath(null);
        return;
      }
      try {
        const { join } = await import('@tauri-apps/api/path');
        const candidate = await join(meetingFolderPath, 'audio.mp4');
        if (!cancelled) setAudioPath(candidate);
      } catch {
        if (!cancelled) setAudioPath(null);
      }
    };
    resolveAudio();
    return () => {
      cancelled = true;
    };
  }, [meetingFolderPath, isRecording]);

  const audio = useAudioPlayer(audioPath);

  // Derive timeline key moments from the loaded segments. Only meaningful once
  // recording has stopped and the transcript is available.
  const timelineMarkers = useMemo<TimelineMarker[]>(() => {
    if (isRecording) return [];
    return generateTimeline(
      convertedSegments.map((segment) => ({
        id: segment.id,
        startTime: segment.timestamp,
        endTime: segment.endTime,
        text: segment.text,
      })),
    );
  }, [convertedSegments, isRecording]);

  // Lightweight segment shape reused for resolving the active segment during
  // playback (see the follow-playback effect below).
  const playableSegments = useMemo(
    () => convertedSegments.map((segment) => ({ id: segment.id, startTime: segment.timestamp })),
    [convertedSegments],
  );

  const handleMarkerSelect = useCallback(
    (marker: TimelineMarker) => {
      setActiveMarkerId(marker.id);
      setActiveSegmentId(marker.segmentId);
      transcriptViewRef.current?.scrollToSegment(marker.segmentId);
      // Jump audio playback to the same moment when a recording is available.
      if (audioPath) {
        audio.seek(marker.time);
        audio.play();
      }
    },
    [audioPath, audio],
  );

  // While playing, keep the active marker and transcript segment in sync with
  // the playback position. Setting the same id is a no-op, so this stays cheap.
  useEffect(() => {
    if (!audio.isPlaying) return;
    const segmentId = findActiveSegmentIdAtTime(playableSegments, audio.currentTime);
    if (segmentId) setActiveSegmentId(segmentId);
    const markerId = findActiveMarkerId(timelineMarkers, audio.currentTime);
    if (markerId) setActiveMarkerId(markerId);
  }, [audio.currentTime, audio.isPlaying, playableSegments, timelineMarkers]);

  // Jump to a specific segment when arriving from a search result. Scrolls and
  // highlights it once loaded, and positions the audio playhead at that moment
  // once the recording is ready (seek clamps to duration, so it must wait).
  const jumpedToSegmentRef = useRef<string | null>(null);
  const seekedToSegmentRef = useRef<string | null>(null);
  useEffect(() => {
    if (!targetSegmentId || isRecording) return;
    const segment = convertedSegments.find((item) => item.id === targetSegmentId);
    if (!segment) return;

    // Scroll + highlight (once).
    if (jumpedToSegmentRef.current !== targetSegmentId) {
      jumpedToSegmentRef.current = targetSegmentId;
      setActiveSegmentId(targetSegmentId);
      transcriptViewRef.current?.scrollToSegment(targetSegmentId);
    }

    // Move the audio playhead to the segment once the recording has loaded.
    if (
      seekedToSegmentRef.current !== targetSegmentId &&
      audioPath &&
      audio.duration > 0 &&
      typeof segment.timestamp === 'number'
    ) {
      seekedToSegmentRef.current = targetSegmentId;
      audio.seek(segment.timestamp);
    }
  }, [targetSegmentId, convertedSegments, isRecording, audioPath, audio.duration]);

  const handleToggleTimeline = useCallback(() => {
    setIsTimelineOpen((open) => !open);
  }, []);

  const showTimeline = !isRecording && timelineMarkers.length > 0;
  // Show the player once a recording path is resolved; hide it if the audio
  // could not be loaded (e.g. the meeting has no recording).
  const showAudioPlayer = !isRecording && audioPath !== null && !audio.error;

  return (
    <div className="relative hidden min-w-0 shrink-0 flex-col border-r border-briefli-line bg-briefli-surface md:flex md:w-[38%] lg:w-[40%] lg:max-w-[470px]">
      {/* Title area */}
      <div className="flex items-center justify-between gap-3 border-b border-briefli-line px-4 py-3">
        <div>
          <p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-briefli-muted">Source evidence</p>
          <h2 className="mt-1 text-sm font-semibold text-briefli-ink">Transcript</h2>
        </div>
        <TranscriptButtonGroup
          transcriptCount={usePagination ? (totalCount ?? convertedSegments.length) : (transcripts?.length || 0)}
          onCopyTranscript={onCopyTranscript}
          onOpenMeetingFolder={onOpenMeetingFolder}
          meetingId={meetingId}
          meetingFolderPath={meetingFolderPath}
          onRefetchTranscripts={onRefetchTranscripts}
        />
      </div>

      {/* Recorded audio transport (play / seek), when a recording exists */}
      {showAudioPlayer && (
        <div className="border-b border-briefli-line">
          <MeetingAudioPlayer
            isPlaying={audio.isPlaying}
            currentTime={audio.currentTime}
            duration={audio.duration}
            error={audio.error}
            onPlay={audio.play}
            onPause={audio.pause}
            onSeek={audio.seek}
          />
        </div>
      )}

      {/* Interactive timeline of key moments (collapsible) */}
      {showTimeline && (
        <MeetingTimeline
          markers={timelineMarkers}
          activeMarkerId={activeMarkerId}
          onMarkerSelect={handleMarkerSelect}
          collapsible
          isOpen={isTimelineOpen}
          onToggleOpen={handleToggleTimeline}
          className="max-h-[34vh] border-b border-briefli-line"
        />
      )}

      {/* Transcript content - use virtualized view for better performance */}
      <div className="flex-1 overflow-hidden pb-4">
        <VirtualizedTranscriptView
          ref={transcriptViewRef}
          segments={convertedSegments}
          isRecording={isRecording}
          isPaused={false}
          isProcessing={false}
          isStopping={false}
          enableStreaming={false}
          showConfidence={true}
          disableAutoScroll={disableAutoScroll}
          activeSegmentId={activeSegmentId}
          hasMore={hasMore}
          isLoadingMore={isLoadingMore}
          totalCount={totalCount}
          loadedCount={loadedCount}
          onLoadMore={onLoadMore}
        />
      </div>

      {/* Custom prompt input at bottom of transcript section */}
      {!isRecording && convertedSegments.length > 0 && (
        <div className="border-t border-briefli-line p-3">
          <textarea
            placeholder="Add names or context before generating the record"
            className="min-h-[72px] w-full resize-y rounded border border-briefli-line bg-briefli-paper px-3 py-2 text-sm text-briefli-ink placeholder:text-briefli-muted focus:border-briefli-ink focus:outline-none"
            value={customPrompt}
            onChange={(e) => onPromptChange(e.target.value)}
          />
        </div>
      )}
    </div>
  );
}
