package com.briefli.companion

import android.content.Context
import com.briefli.companion.capture.CaptureRepository
import com.briefli.companion.sync.DeviceIdentity
import com.briefli.companion.sync.PairingStore
import com.briefli.companion.sync.SyncApi

class AppContainer(context: Context) {
    val captureRepository = CaptureRepository(context)
    val pairingStore = PairingStore(context)
    val deviceIdentity = DeviceIdentity()
    val syncApi = SyncApi(pairingStore, deviceIdentity)
}
