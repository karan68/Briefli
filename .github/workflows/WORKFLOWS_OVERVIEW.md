# GitHub Actions Workflows Overview

This document provides a quick overview of all available CI/CD workflows in this repository.

Most build and release workflows use manual triggers. `pr-quality-gate.yml` runs automatically on every pull request.

## Workflow Files

### 0. **pr-quality-gate.yml** - Required Pull Request Gate
**Purpose:** Automatic merge protection for every pull request

**Checks:**
- Append-only migration policy, secret/local-artifact scan, and workflow linting
- Frontend Bun tests and production Next.js build
- Rust library/test compilation on Windows and Linux, plus the full non-ignored Rust library suite on macOS
- Fresh SQLite migration chain plus realistic demo-seed integration checks
- Unsigned Windows desktop compile/build
- Stable aggregate check: `ci / required`

**Triggers:**
- Every pull request
- Manual dispatch for troubleshooting

**Branch protection:**
- Require `ci / required`
- Require the branch to be up to date before merge
- Require resolved review conversations

**Limitations:**
- CI cannot prove every microphone, speaker, GPU, permission, or real-world meeting condition.
- The physical audio-output test is skipped on headless PR runners and remains part of manual device testing.
- Signed/notarized installers remain covered by the manual build and release workflows because forked PRs do not receive signing secrets.

---

### 1. **build-devtest.yml** - DevTest Builds
**Purpose:** Fast builds for development and testing

**Key Features:**
- Signing OFF by default (faster builds)
- Optional signing via workflow dispatch input
- All platforms in parallel
- 14-day artifact retention

**Triggers:**
- Manual dispatch only

**Use When:**
- Regular development work
- Testing features
- Need fast feedback

---

### 2. **build-macos.yml** - macOS Standalone Builds
**Purpose:** Build and test specifically for Apple Silicon (M1/M2/M3)

**Key Features:**
- Apple Developer Certificate signing (optional)
- Notarization with Apple ID
- Signature verification
- macOS-focused optimizations

**Triggers:**
- Manual dispatch only

**Use When:**
- macOS-specific development
- Testing Metal GPU acceleration
- Verifying macOS-specific features

**Outputs:**
- `.dmg` installer
- `.app` bundle

---

### 3. **build-windows.yml** - Windows Standalone Builds
**Purpose:** Build and test specifically for Windows x64

**Key Features:**
- DigiCert KeyLocker signing (cloud HSM)
- Signs both MSI and NSIS installers
- Signature verification with PowerShell
- MSI installer validation

**Triggers:**
- Manual dispatch only

**Use When:**
- Windows-specific development
- Testing CUDA/Vulkan GPU acceleration
- Verifying Windows-specific features

**Outputs:**
- `.msi` installer
- `.exe` NSIS installer

---

### 4. **build-linux.yml** - Linux Standalone Builds
**Purpose:** Build and test for Linux distributions

**Key Features:**
- Support for Ubuntu 22.04 and 24.04
- Multiple bundle formats (DEB, AppImage, RPM)
- Tauri updater signing
- AppImage compatibility fixes
- Package verification

**Triggers:**
- Manual dispatch only

**Use When:**
- Linux-specific development
- Testing Vulkan GPU acceleration
- Verifying package formats

**Outputs:**
- `.deb` package (Ubuntu/Debian)
- `.AppImage` portable
- `.rpm` package (Fedora/RHEL)

---

### 5. **build-test.yml** - Multi-Platform Test Builds
**Purpose:** Test builds across all platforms with signing

**Key Features:**
- Signing ON by default
- All platforms in parallel
- Uses reusable `build.yml` workflow
- 30-day artifact retention
- Artifacts prefixed with `briefli-test-`

**Triggers:**
- Manual dispatch only

**Use When:**
- Pre-release testing
- Verifying signing infrastructure
- Testing across all platforms simultaneously

---

### 6. **build.yml** - Reusable Build Workflow
**Purpose:** Shared workflow used by other workflows

**Key Features:**
- Reusable workflow (called by others)
- Highly configurable inputs
- Used by test, beta, and stable release workflows

**Not directly triggered** - used as a building block

---

### 7. **release.yml** - Production Release
**Purpose:** Create official stable Windows releases

**Key Features:**
- Signing REQUIRED
- Creates GitHub Release (draft)
- Requires matching stable versions across package, Cargo, lock, and Tauri configuration
- Builds and smoke-tests Windows x64 MSI and NSIS installers
- Verifies Authenticode, updater signatures, provenance, and checksums
- Auto-generates `latest.json` for Tauri updater

**Triggers:**
- Manual dispatch only

**Use When:**
- Ready to publish a new version
- Creating official release artifacts

**Outputs:**
- GitHub Release (draft)
- Windows: MSI installer (signed), NSIS installer (signed), .sig files
- Updater manifest: latest.json
- SHA256SUMS.txt and GitHub build-provenance attestations

**Version Behavior:**
- Stable versions use `MAJOR.MINOR.PATCH`.
- Existing tags are never reused or auto-incremented; bump all version fields first.

**Note:** Linux builds are not included in releases. Use `build-linux.yml` for Linux testing.

---

### 8. **beta-release.yml** - Verified Beta Release
**Purpose:** Publish verified prereleases from `devtest`

**Key Features:**
- Requires matching `MAJOR.MINOR.PATCH-beta.NUMBER` versions
- Builds unsigned Windows x64 MSI and NSIS installers with updater signatures
- Builds the R8-minified Android companion APK with the durable release key
- Verifies installer smoke tests, Android signer identity, provenance, and checksums
- Publishes only after every verification job succeeds
- Updates the separate `beta-channel/latest.json` updater manifest

**Outputs:**
- Windows MSI and NSIS installers plus updater signatures
- Signed `Briefli-Companion_<version>.apk`
- `latest.json` and `SHA256SUMS.txt`
- GitHub build-provenance attestations

---

### 9. **pr-main-check.yml** - Validation Check
**Purpose:** Quick validation of version and configuration

**Key Features:**
- No builds triggered
- Validates version format
- Shows current branch info
- Provides next steps guidance

**Triggers:**
- Manual dispatch only

**Use When:**
- Quick configuration check
- Before running full builds

---

## How to Run Workflows

1. **Go to Actions tab** in GitHub repository
2. **Select workflow** from left sidebar
3. **Click "Run workflow"** button
4. **Select branch** to run against
5. **Configure options** (build type, signing, etc.)
6. **Click "Run workflow"** to start
7. **Monitor progress** in the Actions tab

---

## Quick Decision Guide

### "I'm developing a new feature..."
- **Use `build-devtest.yml`** (manual dispatch)
- Fast builds, no signing by default
- Enable signing checkbox if needed

### "I need to test macOS-specific code..."
- **Use `build-macos.yml`** (manual dispatch)
- Focus on macOS
- Optional signing

### "I need to test Windows-specific code..."
- **Use `build-windows.yml`** (manual dispatch)
- Focus on Windows
- Optional signing

### "I need to test Linux packages..."
- **Use `build-linux.yml`** (manual dispatch)
- Choose Ubuntu version
- Choose bundle types

### "I need signed builds for all platforms..."
- **Use `build-test.yml`** (manual dispatch)
- All platforms
- Signing enabled
- Full verification

### "I'm ready to release..."
- **Use `release.yml`** (manual dispatch)
- Creates GitHub Release
- Windows x64, fully signed
- Production-ready artifacts

---

## Workflow Dependencies

```
build.yml (reusable)
    |-- build-test.yml (calls build.yml)
    |-- release.yml (calls build.yml)

Standalone (don't use build.yml):
    |-- build-macos.yml
    |-- build-windows.yml
    |-- build-linux.yml
    |-- build-devtest.yml
    |-- pr-main-check.yml (validation only)
```

---

## Comparison Matrix

| Workflow | Platforms | Default Signing | Speed | Retention | Use Case |
|----------|-----------|----------------|-------|-----------|----------|
| `build-devtest.yml` | All | OFF | Fast | 14 days | Development |
| `build-macos.yml` | macOS | Optional | Medium | 30 days | macOS dev |
| `build-windows.yml` | Windows | Optional | Medium | 30 days | Windows dev |
| `build-linux.yml` | Linux | Optional | Medium | 30 days | Linux dev |
| `build-test.yml` | All | ON | Slow | 30 days | Pre-release |
| `release.yml` | Windows | REQUIRED | Slow | Permanent | Release |

---

## Artifact Naming Convention

```
briefli-{workflow}-{platform}-{target}-{version}
```

**Examples:**
- `briefli-devtest-macOS-aarch64-apple-darwin-0.5.0`
- `briefli-test-windows-x86_64-pc-windows-msvc-0.5.0`
- `briefli-macos-aarch64-release-0.5.0`

---

## Required Secrets

All workflows require these secrets to be configured:

### macOS Signing
- `APPLE_CERTIFICATE` - Developer ID certificate (base64)
- `APPLE_CERTIFICATE_PASSWORD` - Certificate password
- `APPLE_ID` - Apple ID email
- `APPLE_PASSWORD` - App-specific password
- `APPLE_TEAM_ID` - Team ID
- `KEYCHAIN_PASSWORD` - Temporary keychain password

### Windows Signing (DigiCert)
- `SM_HOST` - DigiCert host URL
- `SM_API_KEY` - API key
- `SM_CLIENT_CERT_FILE_B64` - Client cert (base64)
- `SM_CLIENT_CERT_PASSWORD` - Client cert password
- `SM_CODE_SIGNING_CERT_SHA1_HASH` - Certificate hash

### Tauri Updater (All Platforms)
- `TAURI_SIGNING_PRIVATE_KEY` - Ed25519 private key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` - Key password

---

## Performance Tips

1. **Use devtest workflow** for routine development (fastest)
2. **Enable signing** only when necessary (adds 10-15 minutes)
3. **Test specific platforms** when working on platform-specific code
4. **Run full builds** (`build-test.yml`) before releases
5. **Cache is enabled** - subsequent builds are faster

---

## Troubleshooting

### Build fails with version error (Windows MSI)
- Ensure version in `tauri.conf.json` doesn't contain non-numeric pre-release identifiers
- Use `0.1.3` not `0.1.2-pro-trial`

### Signing fails
- Verify all required secrets are configured
- Check secret expiration dates
- Review workflow logs for specific errors

### Artifacts not available
- Check build succeeded completely
- Artifacts expire based on retention period
- Ensure `upload-artifacts` is enabled

### Workflow not appearing in Actions
- Verify YAML syntax is valid
- Check file is in `.github/workflows/` directory
- Ensure file extension is `.yml` or `.yaml`

---

## Support

For issues with workflows:
1. Check workflow logs in Actions tab
2. Review this documentation
3. Check `README_DEVTEST.md` for devtest-specific help
4. Check `ACCELERATION_GUIDE.md` for GPU/performance info
