package com.briefli.companion.capture

import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import java.util.UUID

class CaptureRepository(private val context: Context) {
    private val database = CaptureDatabase(context)
    private val mutableCaptures = MutableStateFlow<List<CaptureRecord>>(emptyList())
    val captures: StateFlow<List<CaptureRecord>> = mutableCaptures.asStateFlow()

    init {
        val values = ContentValues().apply {
            put("status", CaptureStatus.READY.name)
            put("error", "Sync interrupted; ready to retry")
            put("updated_at_epoch_ms", System.currentTimeMillis())
        }
        database.writableDatabase.update(
            "captures",
            values,
            "status = ?",
            arrayOf(CaptureStatus.SYNCING.name),
        )
        refresh()
    }

    @Synchronized
    fun createCapture(title: String, nowEpochMs: Long = System.currentTimeMillis()): CaptureRecord {
        val normalizedTitle = title.trim().ifBlank { "Meeting" }.take(200)
        val id = UUID.randomUUID().toString()
        val values = ContentValues().apply {
            put("id", id)
            put("title", normalizedTitle)
            put("started_at", rfc3339(nowEpochMs))
            put("started_at_epoch_ms", nowEpochMs)
            put("status", CaptureStatus.RECORDING.name)
            put("updated_at_epoch_ms", nowEpochMs)
        }
        check(database.writableDatabase.insertOrThrow("captures", null, values) != -1L)
        captureDirectory(id).resolve("segments").mkdirs()
        refresh()
        return requireNotNull(getCapture(id))
    }

    @Synchronized
    fun addSegment(captureId: String, file: File, durationMs: Long) {
        require(file.isFile && file.length() > 0) { "Recorded segment is empty" }
        val values = ContentValues().apply {
            put("capture_id", captureId)
            put("segment_index", nextSegmentIndex(captureId))
            put("path", file.absolutePath)
            put("byte_length", file.length())
            put("duration_ms", durationMs.coerceAtLeast(1))
        }
        database.writableDatabase.insertOrThrow("capture_segments", null, values)
        touch(captureId)
    }

    @Synchronized
    fun getCapture(captureId: String): CaptureRecord? = database.readableDatabase.query(
        "captures",
        CAPTURE_COLUMNS,
        "id = ?",
        arrayOf(captureId),
        null,
        null,
        null,
        "1",
    ).use { cursor -> if (cursor.moveToFirst()) cursor.toCapture() else null }

    @Synchronized
    fun listSegments(captureId: String): List<CaptureSegment> = database.readableDatabase.query(
        "capture_segments",
        SEGMENT_COLUMNS,
        "capture_id = ?",
        arrayOf(captureId),
        null,
        null,
        "segment_index ASC",
    ).use { cursor -> buildList { while (cursor.moveToNext()) add(cursor.toSegment()) } }

    @Synchronized
    fun listInterrupted(): List<CaptureRecord> = listByStatuses(
        CaptureStatus.RECORDING,
        CaptureStatus.PAUSED,
        CaptureStatus.FINALIZING,
    )

    @Synchronized
    fun listSyncable(): List<CaptureRecord> = listByStatuses(CaptureStatus.READY)

    @Synchronized
    fun setStatus(captureId: String, status: CaptureStatus, error: String? = null) {
        val values = ContentValues().apply {
            put("status", status.name)
            if (error == null) putNull("error") else put("error", error.take(500))
            put("updated_at_epoch_ms", System.currentTimeMillis())
        }
        check(database.writableDatabase.update("captures", values, "id = ?", arrayOf(captureId)) == 1)
        refresh()
    }

    @Synchronized
    fun completeCapture(
        captureId: String,
        durationMs: Long,
        finalized: FinalizedAudio,
        warning: String? = null,
    ) {
        val values = ContentValues().apply {
            put("duration_ms", durationMs.coerceAtLeast(1))
            put("final_path", finalized.file.absolutePath)
            put("byte_length", finalized.byteLength)
            put("sha256", finalized.sha256)
            put("status", CaptureStatus.READY.name)
            put("synced_bytes", 0)
            if (warning == null) putNull("error") else put("error", warning.take(500))
            put("updated_at_epoch_ms", System.currentTimeMillis())
        }
        check(database.writableDatabase.update("captures", values, "id = ?", arrayOf(captureId)) == 1)
        refresh()
    }

    @Synchronized
    fun updateSyncProgress(captureId: String, bytes: Long) {
        val values = ContentValues().apply {
            put("synced_bytes", bytes.coerceAtLeast(0))
            put("updated_at_epoch_ms", System.currentTimeMillis())
        }
        check(database.writableDatabase.update("captures", values, "id = ?", arrayOf(captureId)) == 1)
        refresh()
    }

    @Synchronized
    fun markSyncFailed(captureId: String, message: String) {
        setStatus(captureId, CaptureStatus.READY, message)
    }

    fun captureDirectory(captureId: String): File = File(context.filesDir, "captures/$captureId")

    fun segmentFile(captureId: String, segmentIndex: Int): File =
        captureDirectory(captureId).resolve("segments/segment-${segmentIndex.toString().padStart(5, '0')}.aac")

    @Synchronized
    fun nextSegmentFile(captureId: String): File =
        segmentFile(captureId, nextSegmentIndex(captureId))

    @Synchronized
    fun cleanupSegments(captureId: String) {
        database.writableDatabase.delete(
            "capture_segments",
            "capture_id = ?",
            arrayOf(captureId),
        )
        captureDirectory(captureId).resolve("segments").let { directory ->
            directory.listFiles()?.forEach(File::delete)
            directory.delete()
        }
    }

    @Synchronized
    private fun nextSegmentIndex(captureId: String): Int = database.readableDatabase.rawQuery(
        "SELECT COALESCE(MAX(segment_index), -1) + 1 FROM capture_segments WHERE capture_id = ?",
        arrayOf(captureId),
    ).use { cursor -> cursor.moveToFirst(); cursor.getInt(0) }

    @Synchronized
    private fun listByStatuses(vararg statuses: CaptureStatus): List<CaptureRecord> {
        val placeholders = statuses.joinToString(",") { "?" }
        return database.readableDatabase.query(
            "captures",
            CAPTURE_COLUMNS,
            "status IN ($placeholders)",
            statuses.map { it.name }.toTypedArray(),
            null,
            null,
            "started_at_epoch_ms DESC",
        ).use { cursor -> buildList { while (cursor.moveToNext()) add(cursor.toCapture()) } }
    }

    @Synchronized
    private fun touch(captureId: String) {
        val values = ContentValues().apply { put("updated_at_epoch_ms", System.currentTimeMillis()) }
        database.writableDatabase.update("captures", values, "id = ?", arrayOf(captureId))
        refresh()
    }

    @Synchronized
    private fun refresh() {
        mutableCaptures.value = database.readableDatabase.query(
            "captures",
            CAPTURE_COLUMNS,
            null,
            null,
            null,
            null,
            "started_at_epoch_ms DESC",
        ).use { cursor -> buildList { while (cursor.moveToNext()) add(cursor.toCapture()) } }
    }

    private fun Cursor.toCapture() = CaptureRecord(
        id = getString(getColumnIndexOrThrow("id")),
        title = getString(getColumnIndexOrThrow("title")),
        startedAt = getString(getColumnIndexOrThrow("started_at")),
        startedAtEpochMs = getLong(getColumnIndexOrThrow("started_at_epoch_ms")),
        durationMs = getLong(getColumnIndexOrThrow("duration_ms")),
        finalPath = stringOrNull("final_path"),
        byteLength = getLong(getColumnIndexOrThrow("byte_length")),
        sha256 = stringOrNull("sha256"),
        status = CaptureStatus.valueOf(getString(getColumnIndexOrThrow("status"))),
        syncedBytes = getLong(getColumnIndexOrThrow("synced_bytes")),
        error = stringOrNull("error"),
        updatedAtEpochMs = getLong(getColumnIndexOrThrow("updated_at_epoch_ms")),
    )

    private fun Cursor.toSegment() = CaptureSegment(
        captureId = getString(getColumnIndexOrThrow("capture_id")),
        segmentIndex = getInt(getColumnIndexOrThrow("segment_index")),
        path = getString(getColumnIndexOrThrow("path")),
        byteLength = getLong(getColumnIndexOrThrow("byte_length")),
        durationMs = getLong(getColumnIndexOrThrow("duration_ms")),
    )

    private fun Cursor.stringOrNull(column: String): String? {
        val index = getColumnIndexOrThrow(column)
        return if (isNull(index)) null else getString(index)
    }

    private fun rfc3339(epochMs: Long): String = SimpleDateFormat(
        "yyyy-MM-dd'T'HH:mm:ss.SSS'Z'",
        Locale.US,
    ).apply { timeZone = TimeZone.getTimeZone("UTC") }.format(Date(epochMs))

    private companion object {
        val CAPTURE_COLUMNS = arrayOf(
            "id", "title", "started_at", "started_at_epoch_ms", "duration_ms",
            "final_path", "byte_length", "sha256", "status", "synced_bytes",
            "error", "updated_at_epoch_ms",
        )
        val SEGMENT_COLUMNS = arrayOf(
            "capture_id", "segment_index", "path", "byte_length", "duration_ms",
        )
    }
}
