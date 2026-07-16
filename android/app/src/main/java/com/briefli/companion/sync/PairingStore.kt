package com.briefli.companion.sync

import android.annotation.SuppressLint
import android.content.Context
import java.util.UUID

data class SyncSession(
    val endpoint: String,
    val certificateSha256: String,
)

@SuppressLint("UseKtx")
class PairingStore(context: Context) {
    private val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
    private val lock = Any()

    val deviceId: String
        get() = synchronized(lock) {
            preferences.getString(DEVICE_ID, null) ?: UUID.randomUUID().toString().also { generated ->
                check(preferences.edit().putString(DEVICE_ID, generated).commit()) {
                    "Could not persist device identity"
                }
            }
        }

    fun currentSession(): SyncSession? {
        val endpoint = preferences.getString(ENDPOINT, null) ?: return null
        val fingerprint = preferences.getString(CERTIFICATE_SHA256, null) ?: return null
        return SyncSession(endpoint, fingerprint)
    }

    fun savePairedSession(payload: PairingPayload) {
        val validated = payload.validate()
        check(
            preferences.edit()
                .putString(ENDPOINT, validated.endpoint)
                .putString(CERTIFICATE_SHA256, validated.certificateSha256)
                .putBoolean(PAIRED, true)
                .commit(),
        ) { "Could not persist pairing session" }
    }

    fun isPaired(): Boolean = preferences.getBoolean(PAIRED, false) && currentSession() != null

    fun clearSession() {
        preferences.edit()
            .remove(ENDPOINT)
            .remove(CERTIFICATE_SHA256)
            .putBoolean(PAIRED, false)
            .apply()
    }

    fun reserveSequence(): Long = synchronized(lock) {
        val current = preferences.getLong(SEQUENCE, 0)
        check(current < Long.MAX_VALUE) { "Request sequence is exhausted" }
        val next = current + 1
        check(preferences.edit().putLong(SEQUENCE, next).commit()) {
            "Could not persist request sequence"
        }
        next
    }

    private companion object {
        const val PREFERENCES = "briefli_sync"
        const val DEVICE_ID = "device_id"
        const val ENDPOINT = "endpoint"
        const val CERTIFICATE_SHA256 = "certificate_sha256"
        const val PAIRED = "paired"
        const val SEQUENCE = "request_sequence"
    }
}
