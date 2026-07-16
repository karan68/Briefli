package com.briefli.companion.capture

import java.io.File

class CaptureRecovery(private val repository: CaptureRepository) {
    fun recoverInterruptedCaptures(nowEpochMs: Long = System.currentTimeMillis()) {
        repository.listInterrupted().forEach { capture ->
            runCatching {
                repository.setStatus(capture.id, CaptureStatus.FINALIZING)
                val cataloged = repository.listSegments(capture.id).map { File(it.path) }
                val discovered = repository.captureDirectory(capture.id)
                    .resolve("segments")
                    .listFiles { file -> file.isFile && file.extension.equals("aac", ignoreCase = true) }
                    ?.toList()
                    .orEmpty()
                val segments = (cataloged + discovered)
                    .distinctBy { it.absolutePath }
                    .filter { it.length() > MIN_SEGMENT_BYTES }
                val finalized = SegmentFinalizer.finalize(
                    repository.captureDirectory(capture.id),
                    segments,
                )
                val catalogedDuration = repository.listSegments(capture.id).sumOf { it.durationMs }
                val wallDuration = (nowEpochMs - capture.startedAtEpochMs).coerceAtLeast(1)
                repository.completeCapture(
                    capture.id,
                    finalized.durationMs.takeIf { it > 0 }
                        ?: maxOf(catalogedDuration, wallDuration).coerceAtMost(MAX_CAPTURE_DURATION_MS),
                    finalized,
                )
                repository.cleanupSegments(capture.id)
            }.onFailure { error ->
                repository.setStatus(
                    capture.id,
                    CaptureStatus.FAILED,
                    "Interrupted recording could not be recovered: ${error.message}",
                )
            }
        }

        repository.captures.value
            .filter { capture ->
                capture.status in setOf(CaptureStatus.READY, CaptureStatus.SYNCING, CaptureStatus.SYNCED) &&
                    capture.finalPath?.let(::File)?.isFile == true
            }
            .forEach { repository.cleanupSegments(it.id) }
    }

    private companion object {
        const val MIN_SEGMENT_BYTES = 7L
        const val MAX_CAPTURE_DURATION_MS = 86_400_000L
    }
}
