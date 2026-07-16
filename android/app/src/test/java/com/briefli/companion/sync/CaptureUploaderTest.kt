package com.briefli.companion.sync

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.io.IOException
import java.nio.file.Files

class CaptureUploaderTest {
    @Test
    fun `uploads bounded chunks and finalizes`() {
        val bytes = ByteArray(MAX_CHUNK_BYTES * 2 + 31) { (it % 251).toByte() }
        val file = temporaryAudio(bytes)
        val gateway = FakeGateway(bytes.size.toLong())
        val progress = mutableListOf<Long>()

        val result = CaptureUploader(gateway).upload(manifest(bytes), file, progress::add)

        assertArrayEquals(bytes, gateway.received.toByteArray())
        assertEquals(listOf(0L, MAX_CHUNK_BYTES.toLong(), (MAX_CHUNK_BYTES * 2).toLong(), bytes.size.toLong()), progress)
        assertEquals(listOf(MAX_CHUNK_BYTES, MAX_CHUNK_BYTES, 31), gateway.chunkSizes)
        assertEquals(1, gateway.finalizeCalls)
        assertEquals("received", result.remoteStatus)
    }

    @Test
    fun `resumes at the desktop offset without resending prefix`() {
        val bytes = ByteArray(MAX_CHUNK_BYTES + 17) { (it % 193).toByte() }
        val gateway = FakeGateway(bytes.size.toLong(), initialBytes = bytes.copyOfRange(0, 500_000))

        CaptureUploader(gateway).upload(manifest(bytes), temporaryAudio(bytes))

        assertArrayEquals(bytes, gateway.received.toByteArray())
        assertEquals(500_000L, gateway.requestedOffsets.first())
    }

    @Test
    fun `lost chunk response reconciles through capture status`() {
        val bytes = ByteArray(MAX_CHUNK_BYTES + 9) { (it % 127).toByte() }
        val gateway = FakeGateway(bytes.size.toLong(), loseFirstChunkResponse = true)

        CaptureUploader(gateway).upload(manifest(bytes), temporaryAudio(bytes))

        assertArrayEquals(bytes, gateway.received.toByteArray())
        assertEquals(listOf(0L, MAX_CHUNK_BYTES.toLong()), gateway.requestedOffsets)
        assertTrue(gateway.statusCalls >= 1)
    }

    @Test
    fun `already received capture is an idempotent success`() {
        val bytes = byteArrayOf(1, 2, 3)
        val gateway = FakeGateway(bytes.size.toLong(), initialBytes = bytes, initialStatus = "imported")

        val result = CaptureUploader(gateway).upload(manifest(bytes), temporaryAudio(bytes))

        assertEquals("imported", result.remoteStatus)
        assertTrue(gateway.requestedOffsets.isEmpty())
        assertEquals(0, gateway.finalizeCalls)
    }

    private fun manifest(bytes: ByteArray) = CaptureManifest(
        captureId = "550e8400-e29b-41d4-a716-446655440000",
        title = "Upload test",
        startedAt = "2026-07-15T10:00:00.000Z",
        durationMs = 1_000,
        byteLength = bytes.size.toLong(),
        mediaType = "audio/aac",
        fileExtension = "aac",
        sha256 = SyncCrypto.sha256Hex(bytes),
    )

    private fun temporaryAudio(bytes: ByteArray): File =
        Files.createTempFile("briefli-upload", ".aac").toFile().apply { writeBytes(bytes) }

    private class FakeGateway(
        private val expectedLength: Long,
        initialBytes: ByteArray = ByteArray(0),
        private val initialStatus: String = "receiving",
        private val loseFirstChunkResponse: Boolean = false,
    ) : CaptureSyncGateway {
        val received = ArrayList<Byte>(initialBytes.size).apply { initialBytes.forEach(::add) }
        val requestedOffsets = mutableListOf<Long>()
        val chunkSizes = mutableListOf<Int>()
        var statusCalls = 0
        var finalizeCalls = 0
        private var responseLost = false

        override fun registerCapture(manifest: CaptureManifest): RemoteCapture = remote(initialStatus)

        override fun getCapture(captureId: String): RemoteCapture {
            statusCalls += 1
            return remote()
        }

        override fun uploadChunk(captureId: String, offset: Long, bytes: ByteArray): RemoteCapture {
            requestedOffsets += offset
            chunkSizes += bytes.size
            check(offset == received.size.toLong())
            bytes.forEach(received::add)
            if (loseFirstChunkResponse && !responseLost) {
                responseLost = true
                throw IOException("response lost after server commit")
            }
            return remote()
        }

        override fun finalizeCapture(captureId: String): RemoteCapture {
            finalizeCalls += 1
            check(received.size.toLong() == expectedLength)
            return remote("received")
        }

        private fun remote(status: String = "receiving") = RemoteCapture(
            id = "550e8400-e29b-41d4-a716-446655440000",
            bytesReceived = received.size.toLong(),
            byteLength = expectedLength,
            status = status,
            meetingId = null,
        )
    }
}
