package com.briefli.companion.sync

import android.os.Build
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.io.IOException
import java.util.concurrent.TimeUnit

class SyncHttpException(
    val status: Int,
    val code: String,
    override val message: String,
) : IOException(message)

class SyncApi(
    private val pairingStore: PairingStore,
    private val identity: DeviceIdentity,
) : CaptureSyncGateway {
    fun pair(payload: PairingPayload): Result<Unit> = runCatching {
        val validated = payload.validate()
        val body = JSONObject()
            .put("protocolVersion", PROTOCOL_VERSION)
            .put("deviceId", pairingStore.deviceId)
            .put("deviceName", "${Build.MANUFACTURER} ${Build.MODEL}".trim())
            .put("publicKeySpkiBase64", identity.publicKeySpkiBase64())
            .toString()
            .toByteArray(Charsets.UTF_8)
        val request = Request.Builder()
            .url("${validated.endpoint}/v1/pair")
            .header("Authorization", "Bearer ${validated.pairingToken}")
            .post(body.toRequestBody(JSON_MEDIA_TYPE))
            .build()
        execute(client(validated.certificateSha256), request)
        pairingStore.savePairedSession(validated)
    }

    override fun registerCapture(manifest: CaptureManifest): RemoteCapture {
        val body = manifest.toJsonBytes()
        return executeRemote(signedRequest("POST", "/v1/captures", body, JSON_MEDIA_TYPE))
    }

    override fun getCapture(captureId: String): RemoteCapture =
        executeRemote(signedRequest("GET", "/v1/captures/$captureId", EMPTY_BODY, null))

    override fun uploadChunk(captureId: String, offset: Long, bytes: ByteArray): RemoteCapture {
        require(bytes.isNotEmpty() && bytes.size <= MAX_CHUNK_BYTES) { "Invalid chunk size" }
        return executeRemote(
            signedRequest(
                method = "PUT",
                path = "/v1/captures/$captureId/chunks/$offset",
                body = bytes,
                mediaType = BINARY_MEDIA_TYPE,
            ),
        )
    }

    override fun finalizeCapture(captureId: String): RemoteCapture = executeRemote(
        signedRequest("POST", "/v1/captures/$captureId/finalize", EMPTY_BODY, null),
    )

    private fun signedRequest(
        method: String,
        path: String,
        body: ByteArray,
        mediaType: okhttp3.MediaType?,
    ): PreparedRequest {
        val session = pairingStore.currentSession() ?: throw IOException("Scan the desktop QR code first")
        val sequence = pairingStore.reserveSequence()
        val canonical = SyncCrypto.canonicalPayload(
            method = method,
            path = path,
            deviceId = pairingStore.deviceId,
            sequence = sequence,
            body = body,
        )
        val builder = Request.Builder()
            .url("${session.endpoint}$path")
            .header("x-briefli-device-id", pairingStore.deviceId)
            .header("x-briefli-sequence", sequence.toString())
            .header("x-briefli-signature", identity.sign(canonical))

        when (method) {
            "GET" -> builder.get()
            "POST" -> builder.post(body.toRequestBody(mediaType))
            "PUT" -> builder.put(body.toRequestBody(mediaType))
            else -> error("Unsupported sync method: $method")
        }
        return PreparedRequest(client(session.certificateSha256), builder.build())
    }

    private fun executeRemote(prepared: PreparedRequest): RemoteCapture =
        RemoteCapture.parse(execute(prepared.client, prepared.request))

    private fun execute(client: OkHttpClient, request: Request): String {
        client.newCall(request).execute().use { response ->
            val body = response.body.string()
            if (!response.isSuccessful) {
                val error = runCatching { JSONObject(body) }.getOrNull()
                throw SyncHttpException(
                    status = response.code,
                    code = error?.optString("code").orEmpty().ifBlank { "http_${response.code}" },
                    message = error?.optString("message").orEmpty().ifBlank {
                        "Briefli sync request failed with HTTP ${response.code}"
                    },
                )
            }
            return body
        }
    }

    private fun client(fingerprint: String): OkHttpClient {
        val pinned = pinnedSocketFactory(fingerprint)
        return OkHttpClient.Builder()
            .sslSocketFactory(pinned.sslContext.socketFactory, pinned.trustManager)
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(45, TimeUnit.SECONDS)
            .writeTimeout(45, TimeUnit.SECONDS)
            .retryOnConnectionFailure(false)
            .build()
    }

    private data class PreparedRequest(val client: OkHttpClient, val request: Request)

    private companion object {
        val EMPTY_BODY = ByteArray(0)
        val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()
        val BINARY_MEDIA_TYPE = "application/octet-stream".toMediaType()
    }
}
