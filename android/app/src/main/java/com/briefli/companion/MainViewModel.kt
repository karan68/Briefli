package com.briefli.companion

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.briefli.companion.capture.CaptureRecord
import com.briefli.companion.capture.CaptureStatus
import com.briefli.companion.recording.RecordingService
import com.briefli.companion.sync.PairingPayload
import com.briefli.companion.sync.SyncService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainViewModel(application: Application) : AndroidViewModel(application) {
    private val container = (application as BriefliApplication).container
    val captures: StateFlow<List<CaptureRecord>> = container.captureRepository.captures

    private val mutablePaired = MutableStateFlow(container.pairingStore.isPaired())
    val paired = mutablePaired.asStateFlow()

    private val mutableBusy = MutableStateFlow(false)
    val busy = mutableBusy.asStateFlow()

    private val mutableError = MutableStateFlow<String?>(null)
    val error = mutableError.asStateFlow()

    fun pair(rawQrValue: String) {
        if (mutableBusy.value) return
        viewModelScope.launch {
            mutableBusy.value = true
            mutableError.value = null
            runCatching {
                withContext(Dispatchers.IO) {
                    val payload = PairingPayload.parse(rawQrValue)
                    container.syncApi.pair(payload).getOrThrow()
                }
            }.onSuccess {
                mutablePaired.value = true
            }.onFailure { failure ->
                mutableError.value = failure.message ?: "Could not pair with the Briefli desktop session"
            }
            mutableBusy.value = false
        }
    }

    fun startRecording(title: String) {
        if (captures.value.any { it.status.isActiveRecording() }) return
        runCatching {
            val capture = container.captureRepository.createCapture(title)
            RecordingService.start(getApplication(), capture.id)
        }.onFailure { failure ->
            mutableError.value = failure.message ?: "Could not start recording"
        }
    }

    fun stopRecording() {
        RecordingService.stop(getApplication())
    }

    fun syncReadyCaptures() {
        if (!container.pairingStore.isPaired()) {
            mutableError.value = "Scan the current Briefli desktop QR code first"
            return
        }
        if (captures.value.none { it.status == CaptureStatus.READY }) {
            mutableError.value = "There are no ready captures to sync"
            return
        }
        SyncService.start(getApplication())
    }

    fun clearError() {
        mutableError.value = null
    }

    fun reportError(message: String) {
        mutableError.value = message
    }

    private fun CaptureStatus.isActiveRecording(): Boolean =
        this == CaptureStatus.RECORDING || this == CaptureStatus.PAUSED || this == CaptureStatus.FINALIZING
}
