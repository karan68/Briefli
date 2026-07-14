/**
 * Fast-path transcription engine selection ("Instant First Transcript").
 *
 * Cold-start problem: live recording is gated on a transcription engine being
 * ready. The high-quality Parakeet model is ~670 MB, which forces users to wait
 * minutes before they can record for the first time.
 *
 * Fix: ship a tiny Whisper model (`tiny-q5_1`, ~31 MB) that downloads in seconds
 * and use it as the instant default. Parakeet keeps downloading in the
 * background and, once ready, the engine auto-upgrades to it for better accuracy.
 *
 * The backend picks the live provider from the saved transcript config
 * (`provider` + `model`), where `provider` is either `"parakeet"` or
 * `"localWhisper"`. This module keeps that config pointed at whichever engine is
 * actually ready.
 */

import { invoke } from '@tauri-apps/api/core';
import { WhisperAPI } from './whisper';

/** Tiny Whisper model used as the instant fast-path engine (~31 MB). */
export const FAST_WHISPER_MODEL = 'tiny-q5_1';

/** Default high-quality Parakeet model (~670 MB). */
export const PARAKEET_MODEL = 'parakeet-tdt-0.6b-v3-int8';

type TranscriptionProvider = 'parakeet' | 'localWhisper';

/**
 * localStorage flag set when we fall back to the tiny Whisper model because
 * Parakeet was not yet ready. It lets us auto-upgrade to Parakeet once its
 * download completes, without clobbering an engine the user explicitly chose.
 */
const FASTPATH_FLAG = 'briefli_fastpath_active';

async function setTranscriptEngine(
  provider: TranscriptionProvider,
  model: string,
): Promise<void> {
  await invoke('api_save_transcript_config', { provider, model, apiKey: null });
}

async function getCurrentTranscriptConfig(): Promise<{ provider: string; model: string } | null> {
  try {
    return await invoke<{ provider: string; model: string }>('api_get_transcript_config');
  } catch {
    return null;
  }
}

/** Is the Parakeet transcription model downloaded and usable? */
export async function isParakeetReady(): Promise<boolean> {
  try {
    await invoke('parakeet_init');
    return await invoke<boolean>('parakeet_has_available_models');
  } catch {
    return false;
  }
}

/**
 * Read-only check: is ANY local transcription engine ready to record with
 * (Parakeet or any downloaded Whisper model)? Does not modify the config.
 */
export async function isAnyTranscriptionReady(): Promise<boolean> {
  if (await isParakeetReady()) return true;
  return (await getReadyWhisperModel()) !== null;
}

/** Is a specific Whisper model downloaded and usable? */
export async function isWhisperModelReady(modelName: string): Promise<boolean> {
  try {
    await WhisperAPI.init();
    const models = await WhisperAPI.getAvailableModels();
    return models.some((m: any) => m?.name === modelName && m?.status === 'Available');
  } catch {
    return false;
  }
}

/** Is the tiny ready-to-record Whisper model downloaded and usable? */
export async function isFastWhisperReady(): Promise<boolean> {
  return isWhisperModelReady(FAST_WHISPER_MODEL);
}

/**
 * Return the name of a ready Whisper model, preferring the tiny fast-path model,
 * otherwise any available model. Returns null if none are ready.
 */
export async function getReadyWhisperModel(): Promise<string | null> {
  try {
    await WhisperAPI.init();
    if (!(await WhisperAPI.hasAvailableModels())) return null;
    const models = await WhisperAPI.getAvailableModels();
    const isAvailable = (m: any) => m?.status === 'Available';
    if (models.some((m: any) => m?.name === FAST_WHISPER_MODEL && isAvailable(m))) {
      return FAST_WHISPER_MODEL;
    }
    const anyReady = models.find(isAvailable) as any;
    return anyReady ? anyReady.name : null;
  } catch {
    return null;
  }
}

function setFastpathFlag(active: boolean): void {
  if (typeof localStorage === 'undefined') return;
  if (active) localStorage.setItem(FASTPATH_FLAG, 'true');
  else localStorage.removeItem(FASTPATH_FLAG);
}

function isFastpathActive(): boolean {
  return typeof localStorage !== 'undefined' && localStorage.getItem(FASTPATH_FLAG) === 'true';
}

/**
 * Ensure the live transcript config points at a transcription engine that is
 * actually ready, and return the provider that will be used (or null if nothing
 * is ready yet).
 *
 * Selection rules:
 * 1. Auto-upgrade: if we previously fell back to Whisper and Parakeet is now
 *    ready, switch to Parakeet for better accuracy.
 * 2. Respect the configured engine if it is already usable.
 * 3. Otherwise fall back to whatever is ready, preferring Parakeet.
 */
export async function ensureBestTranscriptionEngine(): Promise<TranscriptionProvider | null> {
  const parakeetReady = await isParakeetReady();

  // 1. Auto-upgrade from the temporary fast-path Whisper model to Parakeet.
  if (parakeetReady && isFastpathActive()) {
    await setTranscriptEngine('parakeet', PARAKEET_MODEL);
    setFastpathFlag(false);
    return 'parakeet';
  }

  // 2. Respect an already-configured engine that is usable.
  const current = await getCurrentTranscriptConfig();
  if (current?.provider === 'parakeet' && parakeetReady) return 'parakeet';
  if (current?.provider === 'localWhisper' && (await isWhisperModelReady(current.model))) {
    return 'localWhisper';
  }

  // 3. Fall back to whatever is ready, preferring Parakeet.
  if (parakeetReady) {
    await setTranscriptEngine('parakeet', PARAKEET_MODEL);
    setFastpathFlag(false);
    return 'parakeet';
  }
  const whisperModel = await getReadyWhisperModel();
  if (whisperModel) {
    await setTranscriptEngine('localWhisper', whisperModel);
    setFastpathFlag(true);
    return 'localWhisper';
  }

  return null;
}

/**
 * Download the tiny Whisper fast-path model. Progress is reported through the
 * Whisper model download events; failures are returned to the caller.
 */
export async function startFastWhisperDownload(): Promise<void> {
  try {
    await WhisperAPI.init();
    await WhisperAPI.downloadModel(FAST_WHISPER_MODEL);
  } catch (error) {
    console.error('[fastTranscription] Failed to start tiny Whisper download:', error);
    throw error;
  }
}
