package com.briefli.companion.sync

import java.io.File
import java.io.IOException
import java.io.RandomAccessFile

interface CaptureSyncGateway {
    fun registerCapture(manifest: CaptureManifest): RemoteCapture
    fun getCapture(captureId: String): RemoteCapture
    fun uploadChunk(captureId: String, offset: Long, bytes: ByteArray): RemoteCapture
    fun finalizeCapture(captureId: String): RemoteCapture
}

data class UploadResult(
    val uploadedBytes: Long,
    val remoteStatus: String,
)

class CaptureUploader(private val gateway: CaptureSyncGateway) {
    fun upload(
        manifest: CaptureManifest,
        audioFile: File,
        onProgress: (Long) -> Unit = {},
    ): UploadResult {
        manifest.validate()
        require(audioFile.isFile) { "Capture audio file is missing" }
        require(audioFile.length() == manifest.byteLength) { "Capture audio size changed after finalization" }

        var remote = gateway.registerCapture(manifest)
        validateRemote(remote, manifest)
        if (remote.isTransferComplete()) {
            onProgress(manifest.byteLength)
            return UploadResult(manifest.byteLength, remote.status)
        }

        var offset = remote.bytesReceived
        onProgress(offset)
        RandomAccessFile(audioFile, "r").use { input ->
            var noProgressRecoveries = 0
            while (offset < manifest.byteLength) {
                input.seek(offset)
                val chunkSize = minOf(MAX_CHUNK_BYTES.toLong(), manifest.byteLength - offset).toInt()
                val buffer = ByteArray(chunkSize)
                input.readFully(buffer)

                remote = try {
                    gateway.uploadChunk(manifest.captureId, offset, buffer)
                } catch (requestError: IOException) {
                    val recovered = gateway.getCapture(manifest.captureId)
                    validateRemote(recovered, manifest)
                    if (recovered.isTransferComplete()) {
                        onProgress(manifest.byteLength)
                        return UploadResult(manifest.byteLength, recovered.status)
                    }
                    if (recovered.bytesReceived == offset) {
                        noProgressRecoveries += 1
                        if (noProgressRecoveries >= MAX_NO_PROGRESS_RECOVERIES) throw requestError
                    } else {
                        noProgressRecoveries = 0
                    }
                    recovered
                }

                validateRemote(remote, manifest)
                if (remote.isTransferComplete()) {
                    onProgress(manifest.byteLength)
                    return UploadResult(manifest.byteLength, remote.status)
                }
                require(remote.bytesReceived > offset) { "Desktop did not advance the capture offset" }
                offset = remote.bytesReceived
                onProgress(offset)
            }
        }

        remote = gateway.finalizeCapture(manifest.captureId)
        validateRemote(remote, manifest)
        check(remote.isTransferComplete()) { "Desktop did not finalize the capture" }
        return UploadResult(manifest.byteLength, remote.status)
    }

    private fun validateRemote(remote: RemoteCapture, manifest: CaptureManifest) {
        require(remote.id == manifest.captureId) { "Desktop returned a different capture identity" }
        require(remote.byteLength == manifest.byteLength) { "Desktop capture size does not match this recording" }
        require(remote.bytesReceived in 0..manifest.byteLength) { "Desktop returned an invalid capture offset" }
    }

    private fun RemoteCapture.isTransferComplete(): Boolean = status in COMPLETE_STATUSES

    private companion object {
        const val MAX_NO_PROGRESS_RECOVERIES = 3
        val COMPLETE_STATUSES = setOf("received", "importing", "imported", "failed")
    }
}
