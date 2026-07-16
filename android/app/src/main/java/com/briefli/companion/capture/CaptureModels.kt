package com.briefli.companion.capture

enum class CaptureStatus {
    RECORDING,
    PAUSED,
    FINALIZING,
    READY,
    SYNCING,
    SYNCED,
    FAILED,
}

data class CaptureRecord(
    val id: String,
    val title: String,
    val startedAt: String,
    val startedAtEpochMs: Long,
    val durationMs: Long,
    val finalPath: String?,
    val byteLength: Long,
    val sha256: String?,
    val status: CaptureStatus,
    val syncedBytes: Long,
    val error: String?,
    val updatedAtEpochMs: Long,
)

data class CaptureSegment(
    val captureId: String,
    val segmentIndex: Int,
    val path: String,
    val byteLength: Long,
    val durationMs: Long,
)
