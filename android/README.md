# Briefli Capture for Android

Native Android companion for recording in-person meetings locally and transferring them to Briefli Desktop over the local network.

## Toolchain

- Android Studio Quail 2 or newer
- Android Gradle Plugin 9.2.1
- Gradle 9.4.1
- JDK 17 or newer (Android Studio JBR is supported)
- Android SDK Platform 37.0
- Android SDK Build Tools 36.0.0
- Minimum Android version: Android 6.0 (API 23)
- Target Android version: Android 17 (API 37)

Set `ANDROID_HOME` to the Android SDK and run commands from this directory.

```powershell
.\gradlew.bat :app:lintDebug :app:testDebugUnitTest :app:assembleDebug
```

The debug APK is written to `app/build/outputs/apk/debug/app-debug.apk`.

## Architecture

- `capture/`: app-private SQLite catalog, segmented AAC recording metadata, atomic finalization, and startup recovery.
- `recording/`: microphone foreground service with 60-second AAC/ADTS segments, pause/stop notification actions, audio-focus handling, low-storage checks, and a bounded wake lock.
- `sync/`: QR protocol models, Android Keystore P-256 identity, exact certificate fingerprint pinning, durable request sequences, and resumable 1 MiB uploads.
- `ui/`: Compose workflow and CameraX/ML Kit QR scanning. Barcode recognition is bundled and does not require a cloud service.

Audio and capture metadata stay in app-private storage. Android backup and device transfer are disabled. The app does not declare cleartext network access.

## Protocol Boundary

The Android client implements desktop protocol version 1:

- TLS certificate SHA-256 pinned from the desktop QR code.
- One-time pairing token sent only to the pinned endpoint.
- Android Keystore P-256 public key registered during pairing.
- Every capture request signed as DER-encoded ECDSA over the canonical desktop payload.
- Strictly increasing request sequence persisted before each signed request.
- Capture registration and finalization are idempotent.
- Chunk response loss is reconciled with `GET /v1/captures/{id}` before retry.

Android 17 targets require the `ACCESS_LOCAL_NETWORK` runtime permission for direct LAN TCP connections. The app requests it before pairing or syncing on API 37 and newer.

## Physical Acceptance Checklist

These checks require a connected, authorized phone and a running Briefli Desktop build:

1. Install the APK with `adb install -r app/build/outputs/apk/debug/app-debug.apk`.
2. Launch and grant microphone, camera, notification, and local-network permissions when requested.
3. Record for more than 60 seconds and confirm segment rotation does not interrupt audible capture.
4. Lock the screen for at least 10 minutes while recording, then stop from the notification.
5. Interrupt recording with an incoming call or another audio-focus owner and confirm a recoverable capture remains.
6. Force-stop or kill the process during a later segment, reopen the app, and confirm startup recovery creates a ready capture.
7. Scan the current desktop Phone Sync QR code and verify certificate-pinned pairing.
8. Drop Wi-Fi during a multi-chunk upload, reconnect, and confirm resume starts at the desktop offset.
9. Retry the same finalized capture and confirm the desktop creates only one meeting.
10. Verify the imported AAC capture transcribes and appears as a normal Briefli meeting.
11. Exercise denied/revoked local-network permission on Android 17.
12. Exercise low-storage behavior near the 64 MB reserve threshold.

Do not treat an APK build alone as completion of these device-level checks.
