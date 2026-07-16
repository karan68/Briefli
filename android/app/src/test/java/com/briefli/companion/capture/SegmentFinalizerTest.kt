package com.briefli.companion.capture

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Test
import java.io.File
import java.nio.file.Files

class SegmentFinalizerTest {
    @Test
    fun `segments are assembled in stable filename order`() {
        val directory = Files.createTempDirectory("briefli-finalizer").toFile()
        val segments = directory.resolve("segments").apply { mkdirs() }
        val second = segments.resolve("segment-00001.aac").apply { writeBytes(byteArrayOf(4, 5)) }
        val first = segments.resolve("segment-00000.aac").apply { writeBytes(byteArrayOf(1, 2, 3)) }

        val finalized = SegmentFinalizer.finalize(directory, listOf(second, first))

        assertArrayEquals(byteArrayOf(1, 2, 3, 4, 5), finalized.file.readBytes())
        assertEquals(5, finalized.byteLength)
        assertEquals("74f81fe167d99b4cb41d6d0ccda82278caee9f3e2f25d5e5a3936ff3dcec60d0", finalized.sha256)
        assertEquals(0, finalized.durationMs)
        assertFalse(directory.resolve("audio.aac.tmp").exists())
    }

    @Test
    fun `finalization is idempotent after atomic rename`() {
        val directory = Files.createTempDirectory("briefli-finalizer").toFile()
        val segment = directory.resolve("segment-00000.aac").apply { writeBytes(byteArrayOf(7, 8, 9)) }
        val first = SegmentFinalizer.finalize(directory, listOf(segment))
        segment.writeBytes(byteArrayOf(0))

        val retry = SegmentFinalizer.finalize(directory, listOf(segment))

        assertEquals(first, retry)
        assertArrayEquals(byteArrayOf(7, 8, 9), retry.file.readBytes())
    }

    @Test
    fun `missing segments cannot produce a ready capture`() {
        val directory = Files.createTempDirectory("briefli-finalizer").toFile()

        assertThrows(IllegalArgumentException::class.java) {
            SegmentFinalizer.finalize(directory, listOf(File(directory, "missing.aac")))
        }
    }

    @Test
    fun `ADTS frame count produces audio duration`() {
        val directory = Files.createTempDirectory("briefli-finalizer").toFile()
        val segment = directory.resolve("segment-00000.aac")
        val frame = byteArrayOf(
            0xff.toByte(), 0xf1.toByte(), 0x50, 0x40, 0x01, 0x60, 0x00,
            0x00, 0x00, 0x00, 0x00,
        )
        segment.outputStream().use { output -> repeat(43) { output.write(frame) } }

        val finalized = SegmentFinalizer.finalize(directory, listOf(segment))

        assertEquals(998, finalized.durationMs)
    }
}
