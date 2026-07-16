package com.briefli.companion.sync

import org.json.JSONObject
import java.net.URI

const val PROTOCOL_VERSION = 1
const val MAX_CHUNK_BYTES = 1024 * 1024

private val SHA256_PATTERN = Regex("^[0-9a-fA-F]{64}$")
private val UUID_PATTERN = Regex("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-5][0-9a-fA-F]{3}-[89aAbB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$")

class ProtocolException(message: String) : IllegalArgumentException(message)

data class PairingPayload(
    val protocolVersion: Int,
    val endpoint: String,
    val certificateSha256: String,
    val pairingToken: String,
    val expiresAt: String,
) {
    fun validate(): PairingPayload {
        if (protocolVersion != PROTOCOL_VERSION) {
            throw ProtocolException("Unsupported Briefli sync protocol")
        }
        val uri = runCatching { URI(endpoint) }.getOrNull()
        if (uri?.scheme != "https" || uri.host.isNullOrBlank() || uri.port !in 1..65535) {
            throw ProtocolException("Pairing endpoint must be an HTTPS host and port")
        }
        if (!SHA256_PATTERN.matches(certificateSha256)) {
            throw ProtocolException("Pairing certificate fingerprint is invalid")
        }
        if (pairingToken.isBlank() || !Regex("^[A-Za-z0-9_-]+$").matches(pairingToken)) {
            throw ProtocolException("Pairing token is invalid")
        }
        if (expiresAt.isBlank()) {
            throw ProtocolException("Pairing expiry is missing")
        }
        return copy(
            endpoint = endpoint.trimEnd('/'),
            certificateSha256 = certificateSha256.lowercase(),
        )
    }

    companion object {
        fun parse(raw: String): PairingPayload {
            val json = runCatching { JSONObject(raw) }
                .getOrElse { throw ProtocolException("QR code is not a Briefli pairing code") }
            return runCatching {
                PairingPayload(
                    protocolVersion = json.getInt("protocolVersion"),
                    endpoint = json.getString("endpoint"),
                    certificateSha256 = json.getString("certificateSha256"),
                    pairingToken = json.getString("pairingToken"),
                    expiresAt = json.getString("expiresAt"),
                ).validate()
            }.getOrElse { error ->
                if (error is ProtocolException) throw error
                throw ProtocolException("QR code is missing required Briefli pairing fields")
            }
        }
    }
}

data class CaptureManifest(
    val captureId: String,
    val title: String,
    val startedAt: String,
    val durationMs: Long,
    val byteLength: Long,
    val mediaType: String,
    val fileExtension: String,
    val sha256: String,
) {
    fun validate() {
        if (!UUID_PATTERN.matches(captureId)) throw ProtocolException("Capture ID is invalid")
        if (title.isBlank() || title.length > 200 || '\u0000' in title) {
            throw ProtocolException("Capture title must contain 1 to 200 characters")
        }
        if (durationMs !in 1..86_400_000) throw ProtocolException("Capture duration is invalid")
        if (byteLength !in 1..1_073_741_824) throw ProtocolException("Capture size is invalid")
        if (!SHA256_PATTERN.matches(sha256)) throw ProtocolException("Capture hash is invalid")
    }

    fun toJsonBytes(): ByteArray {
        validate()
        return JSONObject()
            .put("protocolVersion", PROTOCOL_VERSION)
            .put("captureId", captureId)
            .put("title", title)
            .put("startedAt", startedAt)
            .put("durationMs", durationMs)
            .put("byteLength", byteLength)
            .put("mediaType", mediaType)
            .put("fileExtension", fileExtension)
            .put("sha256", sha256.lowercase())
            .toString()
            .toByteArray(Charsets.UTF_8)
    }
}

data class RemoteCapture(
    val id: String,
    val bytesReceived: Long,
    val byteLength: Long,
    val status: String,
    val meetingId: String?,
) {
    companion object {
        fun parse(raw: String): RemoteCapture {
            val json = JSONObject(raw)
            return RemoteCapture(
                id = json.getString("id"),
                bytesReceived = json.getLong("bytesReceived"),
                byteLength = json.getLong("byteLength"),
                status = json.getString("status"),
                meetingId = json.optString("meetingId").takeIf { it.isNotBlank() && it != "null" },
            )
        }
    }
}
