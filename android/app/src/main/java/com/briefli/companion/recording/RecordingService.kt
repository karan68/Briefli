package com.briefli.companion.recording

import android.Manifest
import android.annotation.SuppressLint
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaRecorder
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.IBinder
import android.os.PowerManager
import android.os.StatFs
import android.os.SystemClock
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import com.briefli.companion.BriefliApplication
import com.briefli.companion.R
import com.briefli.companion.capture.CaptureStatus
import com.briefli.companion.capture.SegmentFinalizer
import java.io.File

class RecordingService : Service() {
    private val repository by lazy { (application as BriefliApplication).container.captureRepository }
    private val audioManager by lazy { getSystemService(AudioManager::class.java) }
    private lateinit var workerThread: HandlerThread
    private lateinit var worker: Handler
    private var recorder: MediaRecorder? = null
    private var activeCaptureId: String? = null
    private var activeSegmentFile: File? = null
    private var activeSegmentStartedMs = 0L
    private var paused = false
    private var finishing = false
    private var wakeLock: PowerManager.WakeLock? = null
    private var audioFocusRequest: AudioFocusRequest? = null

    private val rotateSegment = object : Runnable {
        override fun run() {
            if (recorder == null || paused || finishing) return
            val captureId = activeCaptureId ?: return
            if (!hasRecordingSpace()) {
                finishCapture(captureId, "Recording stopped because storage is low")
                return
            }
            if (recordedDurationMs(captureId) >= MAX_CAPTURE_DURATION_MS) {
                finishCapture(captureId, "Recording reached the 24-hour limit")
                return
            }
            if (stopSegment(captureId)) {
                if (!finishing) startSegment(captureId)
            } else {
                finishCapture(captureId, "Recording stopped after a local catalog error")
            }
        }
    }

    private val audioFocusListener = AudioManager.OnAudioFocusChangeListener { change ->
        if (change == AudioManager.AUDIOFOCUS_LOSS || change == AudioManager.AUDIOFOCUS_LOSS_TRANSIENT) {
            worker.post {
                activeCaptureId?.let { finishCapture(it, "Recording stopped after an audio interruption") }
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        workerThread = HandlerThread("briefli-recorder").apply { start() }
        worker = Handler(workerThread.looper)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_START -> {
                val captureId = intent.getStringExtra(EXTRA_CAPTURE_ID) ?: return START_NOT_STICKY
                activeCaptureId = captureId
                startRecorderForeground(paused = false)
                worker.post { startCapture(captureId) }
            }
            ACTION_PAUSE -> worker.post { pauseCapture() }
            ACTION_RESUME -> worker.post { resumeCapture() }
            ACTION_STOP -> worker.post {
                activeCaptureId?.let { finishCapture(it, null) } ?: stopSelf()
            }
        }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        worker.removeCallbacksAndMessages(null)
        recorder?.release()
        recorder = null
        releaseAudioResources()
        workerThread.quitSafely()
        super.onDestroy()
    }

    private fun startCapture(captureId: String) {
        if (finishing || recorder != null) return
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            failCapture(captureId, "Microphone permission is required")
            return
        }
        if (!hasRecordingSpace()) {
            failCapture(captureId, "At least 64 MB of free storage is required")
            return
        }
        if (!requestAudioFocus()) {
            failCapture(captureId, "Another app is using the microphone")
            return
        }
        acquireWakeLock()
        startSegment(captureId)
    }

    private fun startSegment(captureId: String) {
        val output = repository.nextSegmentFile(captureId)
        output.parentFile?.mkdirs()
        val nextRecorder = createMediaRecorder()
        try {
            nextRecorder.apply {
                setAudioSource(MediaRecorder.AudioSource.MIC)
                setOutputFormat(MediaRecorder.OutputFormat.AAC_ADTS)
                setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
                setAudioChannels(1)
                setAudioSamplingRate(44_100)
                setAudioEncodingBitRate(64_000)
                setOutputFile(output.absolutePath)
                setOnErrorListener { _, _, _ ->
                    worker.post { finishCapture(captureId, "Android reported a microphone recording error") }
                }
                prepare()
                start()
            }
            recorder = nextRecorder
            activeSegmentFile = output
            activeSegmentStartedMs = SystemClock.elapsedRealtime()
            paused = false
            repository.setStatus(captureId, CaptureStatus.RECORDING)
            startRecorderForeground(paused = false)
            worker.postDelayed(rotateSegment, SEGMENT_DURATION_MS)
        } catch (error: Exception) {
            nextRecorder.release()
            output.delete()
            failCapture(captureId, "Could not start recording: ${error.message}")
        }
    }

    private fun stopSegment(captureId: String): Boolean {
        worker.removeCallbacks(rotateSegment)
        val current = recorder ?: return true
        val output = activeSegmentFile
        val duration = (SystemClock.elapsedRealtime() - activeSegmentStartedMs).coerceAtLeast(1)
        recorder = null
        activeSegmentFile = null
        try {
            current.stop()
            current.release()
            if (output != null && output.length() > MIN_SEGMENT_BYTES) {
                return runCatching {
                    repository.addSegment(captureId, output, duration)
                }.isSuccess
            } else {
                output?.delete()
            }
        } catch (_: RuntimeException) {
            current.release()
            output?.delete()
            return false
        }
        return true
    }

    private fun pauseCapture() {
        val captureId = activeCaptureId ?: return
        if (paused || finishing) return
        if (!stopSegment(captureId)) {
            finishCapture(captureId, "Recording stopped after a local catalog error")
            return
        }
        paused = true
        repository.setStatus(captureId, CaptureStatus.PAUSED)
        startRecorderForeground(paused = true)
    }

    private fun resumeCapture() {
        val captureId = activeCaptureId ?: return
        if (!paused || finishing) return
        if (!hasRecordingSpace()) {
            finishCapture(captureId, "Recording stopped because storage is low")
            return
        }
        startSegment(captureId)
    }

    private fun finishCapture(captureId: String, notice: String?) {
        if (finishing) return
        finishing = true
        stopSegment(captureId)
        repository.setStatus(captureId, CaptureStatus.FINALIZING, notice)
        runCatching {
            val cataloged = repository.listSegments(captureId).map { File(it.path) }
            val discovered = repository.captureDirectory(captureId)
                .resolve("segments")
                .listFiles { file -> file.isFile && file.extension.equals("aac", ignoreCase = true) }
                ?.toList()
                .orEmpty()
            val segments = (cataloged + discovered)
                .distinctBy(File::getAbsolutePath)
                .filter { it.length() > MIN_SEGMENT_BYTES }
            val finalized = SegmentFinalizer.finalize(repository.captureDirectory(captureId), segments)
            repository.completeCapture(
                captureId,
                finalized.durationMs.takeIf { it > 0 }
                    ?: recordedDurationMs(captureId).coerceIn(1, MAX_CAPTURE_DURATION_MS),
                finalized,
                notice,
            )
            repository.cleanupSegments(captureId)
        }.onFailure { error ->
            repository.setStatus(
                captureId,
                CaptureStatus.FAILED,
                "Could not finalize recording: ${error.message}",
            )
        }
        activeCaptureId = null
        releaseAudioResources()
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun failCapture(captureId: String, message: String) {
        repository.setStatus(captureId, CaptureStatus.FAILED, message)
        activeCaptureId = null
        releaseAudioResources()
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    private fun recordedDurationMs(captureId: String): Long =
        repository.listSegments(captureId).sumOf { it.durationMs }

    @Suppress("DEPRECATION")
    private fun createMediaRecorder(): MediaRecorder =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) MediaRecorder(this) else MediaRecorder()

    private fun hasRecordingSpace(): Boolean = StatFs(filesDir.absolutePath).availableBytes >= MIN_FREE_BYTES

    private fun acquireWakeLock() {
        if (wakeLock?.isHeld == true) return
        wakeLock = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "Briefli:MeetingCapture")
            .apply { acquire(MAX_WAKE_LOCK_MS) }
    }

    private fun requestAudioFocus(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val attributes = AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
                .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                .build()
            val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(attributes)
                .setOnAudioFocusChangeListener(audioFocusListener, worker)
                .build()
            audioFocusRequest = request
            audioManager.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        } else {
            @Suppress("DEPRECATION")
            audioManager.requestAudioFocus(
                audioFocusListener,
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN,
            ) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        }
    }

    private fun releaseAudioResources() {
        worker.removeCallbacks(rotateSegment)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            audioFocusRequest?.let(audioManager::abandonAudioFocusRequest)
        } else {
            @Suppress("DEPRECATION")
            audioManager.abandonAudioFocus(audioFocusListener)
        }
        audioFocusRequest = null
        wakeLock?.takeIf { it.isHeld }?.release()
        wakeLock = null
    }

    @SuppressLint("InlinedApi")
    private fun startRecorderForeground(paused: Boolean) {
        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            recordingNotification(paused),
            ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE,
        )
    }

    private fun recordingNotification(paused: Boolean): Notification {
        val toggleAction = if (paused) ACTION_RESUME else ACTION_PAUSE
        val toggleLabel = if (paused) "Resume" else "Pause"
        val actionIcon = android.graphics.drawable.Icon.createWithResource(this, R.drawable.ic_notification)
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        return builder
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle(if (paused) "Recording paused" else "Recording meeting")
            .setContentText("Audio stays on this phone until you sync it")
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .addAction(Notification.Action.Builder(actionIcon, toggleLabel, servicePendingIntent(toggleAction, 1)).build())
            .addAction(Notification.Action.Builder(actionIcon, "Stop", servicePendingIntent(ACTION_STOP, 2)).build())
            .build()
    }

    private fun servicePendingIntent(action: String, requestCode: Int): PendingIntent =
        PendingIntent.getService(
            this,
            requestCode,
            Intent(this, RecordingService::class.java).setAction(action),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            getString(R.string.recording_channel_name),
            NotificationManager.IMPORTANCE_LOW,
        ).apply { description = "Visible while Briefli records a meeting" }
        getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
    }

    companion object {
        private const val ACTION_START = "com.briefli.companion.recording.START"
        private const val ACTION_PAUSE = "com.briefli.companion.recording.PAUSE"
        private const val ACTION_RESUME = "com.briefli.companion.recording.RESUME"
        private const val ACTION_STOP = "com.briefli.companion.recording.STOP"
        private const val EXTRA_CAPTURE_ID = "capture_id"
        private const val CHANNEL_ID = "briefli-recording"
        private const val NOTIFICATION_ID = 1001
        private const val SEGMENT_DURATION_MS = 60_000L
        private const val MIN_SEGMENT_BYTES = 7L
        private const val MIN_FREE_BYTES = 64L * 1024L * 1024L
        private const val MAX_CAPTURE_DURATION_MS = 86_400_000L
        private const val MAX_WAKE_LOCK_MS = MAX_CAPTURE_DURATION_MS + 60_000L

        fun start(context: Context, captureId: String) {
            val intent = Intent(context, RecordingService::class.java)
                .setAction(ACTION_START)
                .putExtra(EXTRA_CAPTURE_ID, captureId)
            ContextCompat.startForegroundService(context, intent)
        }

        fun stop(context: Context) {
            context.startService(Intent(context, RecordingService::class.java).setAction(ACTION_STOP))
        }
    }
}
