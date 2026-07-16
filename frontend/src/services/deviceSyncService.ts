import { invoke } from '@tauri-apps/api/core';

export interface PairingSessionView {
  endpoint: string;
  certificateSha256: string;
  expiresAt: string;
  qrSvg: string;
}

export interface PairedDevice {
  id: string;
  displayName: string;
  publicKeySpkiBase64: string;
  lastSequence: number;
  pairedAt: string;
  lastSeenAt: string | null;
  revokedAt: string | null;
}

export type MobileCaptureStatus =
  | 'pending'
  | 'receiving'
  | 'received'
  | 'importing'
  | 'imported'
  | 'failed';

export interface MobileCapture {
  id: string;
  deviceId: string;
  title: string;
  startedAt: string;
  durationMs: number;
  byteLength: number;
  bytesReceived: number;
  mediaType: string;
  fileExtension: string;
  sha256: string;
  status: MobileCaptureStatus;
  inboxPath: string;
  meetingId: string | null;
  error: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface DeviceSyncStatus {
  running: boolean;
  session: PairingSessionView | null;
  pairedDevices: PairedDevice[];
  captures: MobileCapture[];
}

export const deviceSyncService = {
  getStatus: () => invoke<DeviceSyncStatus>('get_device_sync_status'),
  startSession: () => invoke<PairingSessionView>('start_device_sync_session'),
  stopSession: () => invoke<void>('stop_device_sync_session'),
  unpairDevice: (deviceId: string) => invoke<void>('unpair_device_sync_phone', { deviceId }),
};