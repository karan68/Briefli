package com.briefli.companion

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Sync
import androidx.compose.material.icons.rounded.SyncProblem
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.briefli.companion.capture.CaptureRecord
import com.briefli.companion.capture.CaptureStatus
import com.briefli.companion.ui.BriefliTheme
import com.briefli.companion.ui.QrScanner
import kotlinx.coroutines.delay
import java.text.DateFormat
import java.util.Date

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            BriefliTheme {
                BriefliCompanionApp()
            }
        }
    }
}

@Composable
private fun BriefliCompanionApp(viewModel: MainViewModel = viewModel()) {
    val context = LocalContext.current
    val captures by viewModel.captures.collectAsStateWithLifecycle()
    val paired by viewModel.paired.collectAsStateWithLifecycle()
    val busy by viewModel.busy.collectAsStateWithLifecycle()
    val error by viewModel.error.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    var scannerVisible by remember { mutableStateOf(false) }
    var pendingRecordingTitle by remember { mutableStateOf<String?>(null) }

    val recordPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { grants ->
        val granted = grants[Manifest.permission.RECORD_AUDIO] == true
        if (granted) pendingRecordingTitle?.let(viewModel::startRecording)
        pendingRecordingTitle = null
    }
    val cameraPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { grants ->
        val cameraGranted = grants[Manifest.permission.CAMERA] == true ||
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
        val networkGranted = Build.VERSION.SDK_INT < 37 ||
            grants[LOCAL_NETWORK_PERMISSION] == true ||
            ContextCompat.checkSelfPermission(context, LOCAL_NETWORK_PERMISSION) == PackageManager.PERMISSION_GRANTED
        scannerVisible = cameraGranted && networkGranted
        if (!scannerVisible) viewModel.reportError("Camera and local network access are required to pair")
    }
    val networkPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { granted ->
        if (granted) viewModel.syncReadyCaptures()
        else viewModel.reportError("Local network access is required to sync")
    }

    LaunchedEffect(error) {
        error?.let {
            snackbarHostState.showSnackbar(it)
            viewModel.clearError()
        }
    }

    if (scannerVisible) {
        QrScanner(
            onCode = { raw ->
                scannerVisible = false
                viewModel.pair(raw)
            },
            onClose = { scannerVisible = false },
        )
        return
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbarHostState) },
        containerColor = MaterialTheme.colorScheme.background,
    ) { contentPadding ->
        Box(modifier = Modifier.fillMaxSize().padding(contentPadding)) {
            CompanionHome(
                captures = captures,
                paired = paired,
                onStartRecording = { title ->
                    if (ContextCompat.checkSelfPermission(
                            context,
                            Manifest.permission.RECORD_AUDIO,
                        ) == PackageManager.PERMISSION_GRANTED
                    ) {
                        viewModel.startRecording(title)
                    } else {
                        pendingRecordingTitle = title
                        val permissions = buildList {
                            add(Manifest.permission.RECORD_AUDIO)
                            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                                add(Manifest.permission.POST_NOTIFICATIONS)
                            }
                        }
                        recordPermissionLauncher.launch(permissions.toTypedArray())
                    }
                },
                onStopRecording = viewModel::stopRecording,
                onScan = {
                    val hasCamera = ContextCompat.checkSelfPermission(
                        context,
                        Manifest.permission.CAMERA,
                    ) == PackageManager.PERMISSION_GRANTED
                    val hasNetwork = Build.VERSION.SDK_INT < 37 || ContextCompat.checkSelfPermission(
                        context,
                        LOCAL_NETWORK_PERMISSION,
                    ) == PackageManager.PERMISSION_GRANTED
                    if (hasCamera && hasNetwork) {
                        scannerVisible = true
                    } else {
                        cameraPermissionLauncher.launch(
                            buildList {
                                if (!hasCamera) add(Manifest.permission.CAMERA)
                                if (!hasNetwork) add(LOCAL_NETWORK_PERMISSION)
                            }.toTypedArray(),
                        )
                    }
                },
                onSync = {
                    if (Build.VERSION.SDK_INT < 37 || ContextCompat.checkSelfPermission(
                            context,
                            LOCAL_NETWORK_PERMISSION,
                        ) == PackageManager.PERMISSION_GRANTED
                    ) {
                        viewModel.syncReadyCaptures()
                    } else {
                        networkPermissionLauncher.launch(LOCAL_NETWORK_PERMISSION)
                    }
                },
            )
            if (busy) {
                Surface(
                    color = MaterialTheme.colorScheme.scrim.copy(alpha = 0.28f),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    Box(contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                }
            }
        }
    }
}

@Composable
private fun CompanionHome(
    captures: List<CaptureRecord>,
    paired: Boolean,
    onStartRecording: (String) -> Unit,
    onStopRecording: () -> Unit,
    onScan: () -> Unit,
    onSync: () -> Unit,
) {
    val active = captures.firstOrNull {
        it.status == CaptureStatus.RECORDING ||
            it.status == CaptureStatus.PAUSED ||
            it.status == CaptureStatus.FINALIZING
    }
    var title by remember { mutableStateOf("") }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column {
                    Text("Briefli", style = MaterialTheme.typography.headlineLarge)
                    Text("Capture", style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.secondary)
                }
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        if (paired) Icons.Rounded.Lock else Icons.Rounded.PhoneAndroid,
                        contentDescription = null,
                        tint = if (paired) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Spacer(Modifier.size(8.dp))
                    Text(
                        if (paired) "Session saved" else "No desktop session",
                        style = MaterialTheme.typography.labelMedium,
                    )
                }
            }
        }

        item {
            Surface(
                color = MaterialTheme.colorScheme.surfaceContainer,
                shape = RoundedCornerShape(6.dp),
            ) {
                Column(
                    modifier = Modifier.fillMaxWidth().padding(20.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    if (active == null) {
                        OutlinedTextField(
                            value = title,
                            onValueChange = { title = it.take(200) },
                            modifier = Modifier.fillMaxWidth(),
                            singleLine = true,
                            label = { Text("Meeting title") },
                        )
                        Spacer(Modifier.height(18.dp))
                        FloatingActionButton(
                            onClick = { onStartRecording(title) },
                            modifier = Modifier.size(84.dp),
                            shape = CircleShape,
                            containerColor = MaterialTheme.colorScheme.primary,
                        ) {
                            Icon(Icons.Rounded.Mic, contentDescription = "Start recording", modifier = Modifier.size(34.dp))
                        }
                        Spacer(Modifier.height(10.dp))
                        Text("Record", style = MaterialTheme.typography.labelLarge)
                    } else {
                        ActiveRecording(capture = active, onStop = onStopRecording)
                    }
                }
            }
        }

        item {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                OutlinedButton(onClick = onScan, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Rounded.QrCodeScanner, contentDescription = null)
                    Spacer(Modifier.size(8.dp))
                    Text("Scan QR")
                }
                Button(onClick = onSync, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Rounded.Sync, contentDescription = null)
                    Spacer(Modifier.size(8.dp))
                    Text("Sync ready")
                }
            }
        }

        item { HorizontalDivider() }
        item {
            Text("Captures", style = MaterialTheme.typography.titleLarge)
        }
        if (captures.isEmpty()) {
            item {
                Text("No captures", color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            items(captures, key = CaptureRecord::id) { capture ->
                CaptureRow(capture)
            }
        }
    }
}

@Composable
private fun ActiveRecording(capture: CaptureRecord, onStop: () -> Unit) {
    val now by produceState(initialValue = System.currentTimeMillis(), capture.id) {
        while (true) {
            value = System.currentTimeMillis()
            delay(1_000)
        }
    }
    Text(capture.title, style = MaterialTheme.typography.titleLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
    Spacer(Modifier.height(8.dp))
    Text(
        formatDuration((now - capture.startedAtEpochMs).coerceAtLeast(0)),
        style = MaterialTheme.typography.displaySmall,
        fontWeight = FontWeight.SemiBold,
    )
    Spacer(Modifier.height(18.dp))
    FloatingActionButton(
        onClick = onStop,
        modifier = Modifier.size(76.dp),
        shape = CircleShape,
        containerColor = MaterialTheme.colorScheme.error,
    ) {
        Icon(Icons.Rounded.Stop, contentDescription = "Stop recording", modifier = Modifier.size(32.dp))
    }
    Spacer(Modifier.height(10.dp))
    Text(capture.status.displayName(), style = MaterialTheme.typography.labelLarge)
}

@Composable
private fun CaptureRow(capture: CaptureRecord) {
    val statusColor = when (capture.status) {
        CaptureStatus.SYNCED -> MaterialTheme.colorScheme.primary
        CaptureStatus.FAILED -> MaterialTheme.colorScheme.error
        CaptureStatus.READY -> MaterialTheme.colorScheme.secondary
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }
    val statusIcon = when (capture.status) {
        CaptureStatus.SYNCED -> Icons.Rounded.CheckCircle
        CaptureStatus.FAILED -> Icons.Rounded.ErrorOutline
        CaptureStatus.READY -> Icons.Rounded.Sync
        else -> Icons.Rounded.SyncProblem
    }
    Surface(
        color = MaterialTheme.colorScheme.surface,
        shape = RoundedCornerShape(6.dp),
        tonalElevation = 1.dp,
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(statusIcon, contentDescription = null, tint = statusColor)
            Spacer(Modifier.size(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(capture.title, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    "${DateFormat.getDateTimeInstance(DateFormat.MEDIUM, DateFormat.SHORT).format(Date(capture.startedAtEpochMs))}  ·  ${formatDuration(capture.durationMs)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                capture.error?.let {
                    Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error, maxLines = 2)
                }
            }
            Text(capture.status.displayName(), style = MaterialTheme.typography.labelMedium, color = statusColor)
        }
    }
}

private fun CaptureStatus.displayName(): String = when (this) {
    CaptureStatus.RECORDING -> "Recording"
    CaptureStatus.PAUSED -> "Paused"
    CaptureStatus.FINALIZING -> "Finishing"
    CaptureStatus.READY -> "Ready"
    CaptureStatus.SYNCING -> "Syncing"
    CaptureStatus.SYNCED -> "Synced"
    CaptureStatus.FAILED -> "Needs attention"
}

private fun formatDuration(durationMs: Long): String {
    val totalSeconds = durationMs.coerceAtLeast(0) / 1_000
    val hours = totalSeconds / 3_600
    val minutes = (totalSeconds % 3_600) / 60
    val seconds = totalSeconds % 60
    return if (hours > 0) "%d:%02d:%02d".format(hours, minutes, seconds) else "%02d:%02d".format(minutes, seconds)
}

private const val LOCAL_NETWORK_PERMISSION = "android.permission.ACCESS_LOCAL_NETWORK"
