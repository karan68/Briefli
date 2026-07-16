package com.briefli.companion.sync

import android.annotation.SuppressLint
import java.security.SecureRandom
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager

internal data class PinnedSocketFactory(
    val sslContext: SSLContext,
    val trustManager: X509TrustManager,
)

@SuppressLint("CustomX509TrustManager")
internal fun pinnedSocketFactory(expectedFingerprint: String): PinnedSocketFactory {
    val normalized = expectedFingerprint.lowercase()
    require(Regex("^[0-9a-f]{64}$").matches(normalized)) { "Invalid certificate fingerprint" }

    val trustManager = object : X509TrustManager {
        override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
            throw CertificateException("Client certificates are not accepted")
        }

        override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
            val certificate = chain?.firstOrNull()
                ?: throw CertificateException("Server did not provide a certificate")
            val actual = SyncCrypto.sha256Hex(certificate.encoded)
            if (!actual.equals(normalized, ignoreCase = true)) {
                throw CertificateException("Briefli certificate fingerprint does not match the QR code")
            }
        }

        override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
    }
    val sslContext = SSLContext.getInstance("TLS")
    sslContext.init(null, arrayOf<TrustManager>(trustManager), SecureRandom())
    return PinnedSocketFactory(sslContext, trustManager)
}
