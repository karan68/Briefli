package com.briefli.companion.capture

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper

internal class CaptureDatabase(context: Context) :
    SQLiteOpenHelper(context, DATABASE_NAME, null, DATABASE_VERSION) {

    init {
        setWriteAheadLoggingEnabled(true)
    }

    override fun onConfigure(database: SQLiteDatabase) {
        database.setForeignKeyConstraintsEnabled(true)
    }

    override fun onCreate(database: SQLiteDatabase) {
        database.execSQL(
            """
            CREATE TABLE captures (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                started_at TEXT NOT NULL,
                started_at_epoch_ms INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                final_path TEXT,
                byte_length INTEGER NOT NULL DEFAULT 0,
                sha256 TEXT,
                status TEXT NOT NULL,
                synced_bytes INTEGER NOT NULL DEFAULT 0,
                error TEXT,
                updated_at_epoch_ms INTEGER NOT NULL
            )
            """.trimIndent(),
        )
        database.execSQL(
            """
            CREATE TABLE capture_segments (
                capture_id TEXT NOT NULL,
                segment_index INTEGER NOT NULL,
                path TEXT NOT NULL,
                byte_length INTEGER NOT NULL,
                duration_ms INTEGER NOT NULL,
                PRIMARY KEY (capture_id, segment_index),
                FOREIGN KEY (capture_id) REFERENCES captures(id) ON DELETE CASCADE
            )
            """.trimIndent(),
        )
        database.execSQL("CREATE INDEX idx_captures_status_updated ON captures(status, updated_at_epoch_ms)")
    }

    override fun onUpgrade(database: SQLiteDatabase, oldVersion: Int, newVersion: Int) = Unit

    private companion object {
        const val DATABASE_NAME = "briefli_captures.db"
        const val DATABASE_VERSION = 1
    }
}
