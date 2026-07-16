package com.briefli.companion.sync

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class ProtocolTest {
    @Test
    fun `canonical payload matches desktop byte contract`() {
        val payload = SyncCrypto.canonicalPayload(
            method = "put",
            path = "/v1/captures/550e8400-e29b-41d4-a716-446655440000/chunks/0",
            deviceId = "7d9a2b92-c934-42f4-9d08-744563ebf8be",
            sequence = 42,
            body = ByteArray(0),
        ).toString(Charsets.UTF_8)

        assertEquals(
            "BRIEFLI-SYNC-V1\n" +
                "PUT\n" +
                "/v1/captures/550e8400-e29b-41d4-a716-446655440000/chunks/0\n" +
                "7d9a2b92-c934-42f4-9d08-744563ebf8be\n" +
                "42\n" +
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            payload,
        )
    }

    @Test
    fun `pairing QR parser normalizes endpoint and fingerprint`() {
        val payload = PairingPayload.parse(
            """{"protocolVersion":1,"endpoint":"https://192.168.1.2:43111/","certificateSha256":"${"AB".repeat(32)}","pairingToken":"abc_DEF-123","expiresAt":"2026-07-15T12:05:00Z"}""",
        )

        assertEquals("https://192.168.1.2:43111", payload.endpoint)
        assertEquals("ab".repeat(32), payload.certificateSha256)
    }

    @Test
    fun `pairing QR rejects non TLS endpoint`() {
        assertThrows(ProtocolException::class.java) {
            PairingPayload.parse(
                """{"protocolVersion":1,"endpoint":"http://192.168.1.2:43111","certificateSha256":"${"ab".repeat(32)}","pairingToken":"abc","expiresAt":"soon"}""",
            )
        }
    }

    @Test
    fun `capture manifest uses desktop field names`() {
        val manifest = CaptureManifest(
            captureId = "550e8400-e29b-41d4-a716-446655440000",
            title = "Architecture review",
            startedAt = "2026-07-15T10:00:00Z",
            durationMs = 600_000,
            byteLength = 8_000_000,
            mediaType = "audio/aac",
            fileExtension = "aac",
            sha256 = "AB".repeat(32),
        )
        val json = JSONObject(manifest.toJsonBytes().toString(Charsets.UTF_8))

        assertEquals(PROTOCOL_VERSION, json.getInt("protocolVersion"))
        assertEquals("550e8400-e29b-41d4-a716-446655440000", json.getString("captureId"))
        assertEquals("audio/aac", json.getString("mediaType"))
        assertEquals("ab".repeat(32), json.getString("sha256"))
    }

    @Test
    fun `sha256 test vector matches desktop`() {
        assertEquals(
            "313adda4407753d970adfbf3aec27b0475430b08e6e6477b054301ea3ba7199c",
            SyncCrypto.sha256Hex("briefli".toByteArray()),
        )
    }
}
