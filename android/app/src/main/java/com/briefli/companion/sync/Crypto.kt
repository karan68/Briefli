package com.briefli.companion.sync

import java.security.MessageDigest

object SyncCrypto {
    fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    fun sha256Hex(bytes: ByteArray): String = sha256(bytes).joinToString("") { "%02x".format(it) }

    fun canonicalPayload(
        method: String,
        path: String,
        deviceId: String,
        sequence: Long,
        body: ByteArray,
    ): ByteArray {
        require(sequence > 0) { "Sequence must be positive" }
        return buildString {
            append("BRIEFLI-SYNC-V1\n")
            append(method.uppercase())
            append('\n')
            append(path)
            append('\n')
            append(deviceId)
            append('\n')
            append(sequence)
            append('\n')
            append(sha256Hex(body))
        }.toByteArray(Charsets.UTF_8)
    }
}
