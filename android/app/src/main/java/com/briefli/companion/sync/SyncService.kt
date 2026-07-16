package com.briefli.companion.sync

import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.briefli.companion.BriefliApplication
import com.briefli.companion.R
import com.briefli.companion.capture.CaptureRecord
import com.briefli.companion.capture.CaptureStatus
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import java.io.File

class SyncService : Service() {
    private val container by lazy { (application as BriefliApplication).container }
    private val serviceScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var syncJob: Job? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action != ACTION_SYNC || syncJob?.isActive == true) return START_NOT_STICKY
        startSyncForeground("Preparing phone captures", 0)
        syncJob = serviceScope.launch { syncReadyCaptures() }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        serviceScope.cancel()
        super.onDestroy()
    }

    private fun syncReadyCaptures() {
        val repository = container.captureRepository
        if (!container.pairingStore.isPaired()) {
            finishService()
            return
        }

        val captures = repository.listSyncable()
        captures.forEachIndexed { index, capture ->
            runCatching {
                repository.setStatus(capture.id, CaptureStatus.SYNCING)
                val manifest = capture.toManifest()
                val file = File(requireNotNull(capture.finalPath) { "Finalized audio path is missing" })
                CaptureUploader(container.syncApi).upload(manifest, file) { uploaded ->
                    repository.updateSyncProgress(capture.id, uploaded)
                    val progress = if (capture.byteLength == 0L) 0 else {
                        ((uploaded * 100L) / capture.byteLength).toInt().coerceIn(0, 100)
                    }
                    startSyncForeground("Syncing ${index + 1} of ${captures.size}", progress)
                }
                repository.setStatus(capture.id, CaptureStatus.SYNCED)
            }.onFailure { error ->
                repository.markSyncFailed(
                    capture.id,
                    error.message ?: "Could not reach the Briefli desktop session",
                )
            }
        }
        finishService()
    }

    private fun CaptureRecord.toManifest(): CaptureManifest = CaptureManifest(
        captureId = id,
        title = title,
        startedAt = startedAt,
        durationMs = durationMs,
        byteLength = byteLength,
        mediaType = "audio/aac",
        fileExtension = "aac",
        sha256 = requireNotNull(sha256) { "Finalized audio hash is missing" },
    )

    private fun finishService() {
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    @SuppressLint("InlinedApi")
    private fun startSyncForeground(message: String, progress: Int) {
        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            syncNotification(message, progress),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
        )
    }

    private fun syncNotification(message: String, progress: Int): Notification {
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        return builder
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Syncing to Briefli")
            .setContentText(message)
            .setProgress(100, progress, progress == 0)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            getString(R.string.sync_channel_name),
            NotificationManager.IMPORTANCE_LOW,
        ).apply { description = "Visible while captures sync to Briefli on your local network" }
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

    companion object {
        private const val ACTION_SYNC = "com.briefli.companion.sync.START"
        private const val CHANNEL_ID = "briefli-sync"
        private const val NOTIFICATION_ID = 1002

        fun start(context: Context) {
            ContextCompat.startForegroundService(
                context,
                Intent(context, SyncService::class.java).setAction(ACTION_SYNC),
            )
        }
    }
}
