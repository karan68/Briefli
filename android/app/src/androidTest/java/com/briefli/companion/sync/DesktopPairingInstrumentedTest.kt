package com.briefli.companion.sync

import android.graphics.BitmapFactory
import android.util.Base64
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.common.InputImage
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

@RunWith(AndroidJUnit4::class)
class DesktopPairingInstrumentedTest {
    @Test
    fun decodesQrAndPairsWithLiveDesktopSession() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val qrFilename = InstrumentationRegistry.getArguments().getString("qrFilename")
        assumeTrue("qrFilename instrumentation argument is required", !qrFilename.isNullOrBlank())

        val context = instrumentation.targetContext
        val bitmap = BitmapFactory.decodeFile(File(context.filesDir, qrFilename!!).absolutePath)
        requireNotNull(bitmap) { "Could not decode pairing QR bitmap" }

        val result = AtomicReference<String?>()
        val error = AtomicReference<Exception?>()
        val latch = CountDownLatch(1)
        val scanner = BarcodeScanning.getClient()
        scanner.process(InputImage.fromBitmap(bitmap, 0))
            .addOnSuccessListener { barcodes -> result.set(barcodes.firstNotNullOfOrNull { it.rawValue }) }
            .addOnFailureListener(error::set)
            .addOnCompleteListener { latch.countDown() }

        assertTrue("ML Kit QR decoding timed out", latch.await(10, TimeUnit.SECONDS))
        scanner.close()
        error.get()?.let { throw it }

        val rawPayload = requireNotNull(result.get()) { "ML Kit found no QR payload" }
        val store = PairingStore(context)
        SyncApi(store, DeviceIdentity()).pair(PairingPayload.parse(rawPayload)).getOrThrow()
        assertTrue(store.isPaired())
    }

    @Test
    fun pairsWithLiveDesktopSession() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val encodedPayload = InstrumentationRegistry.getArguments().getString("pairingPayload")
        assumeTrue("pairingPayload instrumentation argument is required", !encodedPayload.isNullOrBlank())

        val rawPayload = String(Base64.decode(encodedPayload, Base64.DEFAULT), Charsets.UTF_8)
        val context = instrumentation.targetContext
        val store = PairingStore(context)
        val result = SyncApi(store, DeviceIdentity()).pair(PairingPayload.parse(rawPayload))

        result.getOrThrow()
        assertTrue(store.isPaired())
    }
}
