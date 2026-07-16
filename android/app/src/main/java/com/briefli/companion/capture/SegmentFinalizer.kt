package com.briefli.companion.capture

import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream

const val MAX_CAPTURE_BYTES = 1024L * 1024L * 1024L

data class FinalizedAudio(
    val file: File,
    val byteLength: Long,
    val sha256: String,
    val durationMs: Long,
)

object SegmentFinalizer {
    fun finalize(captureDirectory: File, segments: List<File>): FinalizedAudio {
        captureDirectory.mkdirs()
        val destination = captureDirectory.resolve("audio.aac")
        if (destination.isFile && destination.length() > 0) {
            return metadata(destination)
        }

        require(segments.isNotEmpty()) { "No recorded audio segments were found" }
        val ordered = segments.sortedBy { it.name }
        ordered.forEach { segment ->
            require(segment.isFile && segment.length() > 0) {
                "Recorded audio segment is missing or empty: ${segment.name}"
            }
        }

        val temporary = captureDirectory.resolve("audio.aac.tmp")
        if (temporary.exists()) check(temporary.delete()) { "Could not clear partial finalized audio" }
        FileOutputStream(temporary).use { output ->
            ordered.forEach { segment ->
                FileInputStream(segment).use { input -> input.copyTo(output, DEFAULT_BUFFER_SIZE) }
            }
            output.fd.sync()
        }

        require(temporary.length() in 1..MAX_CAPTURE_BYTES) { "Finalized capture size is invalid" }
        if (destination.exists()) check(destination.delete()) { "Could not replace finalized audio" }
        check(temporary.renameTo(destination)) { "Could not atomically finalize recorded audio" }
        return metadata(destination)
    }

    private fun metadata(file: File): FinalizedAudio {
        val digest = java.security.MessageDigest.getInstance("SHA-256")
        FileInputStream(file).use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        return FinalizedAudio(
            file = file,
            byteLength = file.length(),
            sha256 = digest.digest().joinToString("") { "%02x".format(it) },
            durationMs = adtsDurationMs(file),
        )
    }

    private fun adtsDurationMs(file: File): Long {
        var sampleRate = 0
        var totalSamples = 0L
        FileInputStream(file).buffered().use { input ->
            val header = ByteArray(7)
            while (true) {
                var headerBytes = 0
                while (headerBytes < header.size) {
                    val read = input.read(header, headerBytes, header.size - headerBytes)
                    if (read < 0) break
                    headerBytes += read
                }
                if (headerBytes == 0) break
                if (headerBytes != header.size) return 0

                val first = header[0].toInt() and 0xff
                val second = header[1].toInt() and 0xff
                if (first != 0xff || second and 0xf6 != 0xf0) return 0

                val sampleRateIndex = (header[2].toInt() and 0x3c) shr 2
                val frameSampleRate = ADTS_SAMPLE_RATES.getOrNull(sampleRateIndex) ?: return 0
                if (sampleRate == 0) sampleRate = frameSampleRate
                if (sampleRate != frameSampleRate) return 0

                val frameLength = ((header[3].toInt() and 0x03) shl 11) or
                    ((header[4].toInt() and 0xff) shl 3) or
                    ((header[5].toInt() and 0xe0) shr 5)
                if (frameLength < header.size) return 0
                totalSamples += 1024L * ((header[6].toInt() and 0x03) + 1L)

                var remaining = frameLength - header.size
                while (remaining > 0) {
                    val skipped = input.skip(remaining.toLong()).toInt()
                    if (skipped > 0) {
                        remaining -= skipped
                    } else if (input.read() >= 0) {
                        remaining -= 1
                    } else {
                        return 0
                    }
                }
            }
        }
        return if (sampleRate > 0) totalSamples * 1000L / sampleRate else 0
    }

    private val ADTS_SAMPLE_RATES = listOf(
        96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000,
        22_050, 16_000, 12_000, 11_025, 8_000, 7_350,
    )
}
