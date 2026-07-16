# Windows Release Checklist

Use this checklist for every public Briefli release. Keep the GitHub release in
draft until all required rows have evidence attached to the release or issue.

## GitHub Release Prerequisites

### Verifiable beta

The `Beta Release` workflow requires:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

It publishes an explicitly unsigned prerelease only after GitHub provenance
attestation, updater-manifest validation, checksum generation, and clean-runner
install/launch/uninstall/data-preservation smoke tests pass. It does not require
GitHub Packages. Verified prereleases update a dedicated mutable
`beta-channel/latest.json` manifest; this channel must remain separate from the
stable `/releases/latest/` endpoint.

### Trusted stable release

Briefli's intended free Authenticode provider is SignPath Foundation. The public
requirements and team roles are documented in
[`../CODE_SIGNING_POLICY.md`](../CODE_SIGNING_POLICY.md). Until SignPath approves
the project and its signing configuration is integrated, do not dispatch or
publish the trusted stable release workflow.

The DigiCert variables below describe the repository's existing stable workflow
implementation, not the selected long-term free-signing route. They can be
removed when the approved SignPath project IDs, signing policy, artifact
configuration, and API token are integrated.

The repository's **Actions > Release > Run workflow** path requires these
repository Actions secrets before it can produce a trusted public installer:

### DigiCert KeyLocker / Authenticode

- `SM_HOST`
- `SM_API_KEY`
- `SM_CLIENT_CERT_FILE_B64`
- `SM_CLIENT_CERT_PASSWORD`
- `SM_CODE_SIGNING_CERT_SHA1_HASH`

The certificate must be a trusted Windows code-signing certificate available to
the configured KeyLocker account. The workflow and installer smoke test both
verify that the resulting signer thumbprint matches
`SM_CODE_SIGNING_CERT_SHA1_HASH`.

### Tauri updater artifacts

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

The private key must match the updater public key in
`frontend/src-tauri/tauri.conf.json`. Do not generate or replace one side of the
key pair independently. Never commit either the private key or certificate
credentials.

Configure secrets under **Repository Settings > Secrets and variables > Actions**
or with `gh secret set`. Enter secret values directly into the GitHub or terminal
prompt; do not paste them into an issue, pull request, chat, log, or command that
will be committed.

Before dispatching the release workflow:

1. Merge the release-ready changes into the branch/commit that will be released.
2. Confirm `package.json`, `Cargo.toml`, `Cargo.lock`, and `tauri.conf.json` use
   the same version and that its `v<version>` tag does not already exist.
3. Confirm the PR quality gate is green on that exact commit.
4. Run **Actions > Release > Run workflow** on that commit's branch.
5. Review the generated draft and complete the clean-machine matrix below.
6. Publish the draft only after the signed installer smoke and manual rows pass.

GitHub Packages is not required for Briefli's desktop installers. MSI, NSIS,
updater artifacts, and release notes belong in GitHub Releases.

## Automated Draft-Release Gate

The `Windows signed installer smoke` job runs against the NSIS installer
uploaded to the draft release. It must pass before the release summary job:

- installer Authenticode signature is valid;
- silent installation succeeds;
- installed `Briefli.exe` Authenticode signature is valid;
- the application reaches an interactive window;
- the SQLite meeting database is created;
- silent uninstall succeeds; and
- uninstall leaves the database unchanged in `%APPDATA%\com.briefli.app`.

## Clean-Machine Matrix

Run the following on clean, fully updated x64 virtual machines. Use a real or
USB-passthrough microphone. Revert each VM to its clean snapshot first.

| Check | Windows 10 22H2 | Windows 11 current |
|---|---|---|
| Installer signature reports `Valid` and the expected publisher | [ ] | [ ] |
| Install and first launch complete without development tools | [ ] | [ ] |
| Tiny Whisper (~31 MB) is shown as the ready-to-record model | [ ] | [ ] |
| Continue unlocks before optional Parakeet finishes | [ ] | [ ] |
| Microphone-only recording produces a searchable transcript | [ ] | [ ] |
| Microphone plus system-audio recording captures both sources | [ ] | [ ] |
| Consent reminder is visible before/when recording starts | [ ] | [ ] |
| Force-quit recovery restores an interrupted transcript | [ ] | [ ] |
| Reviewed memories and source links survive restart | [ ] | [ ] |
| Uninstall keeps `%APPDATA%\com.briefli.app\meeting_minutes.sqlite` | [ ] | [ ] |

For the force-quit check, speak for at least 30 seconds, end the process from
Task Manager without stopping the meeting, relaunch Briefli, recover the
interrupted meeting, and verify both transcript text and source navigation.

## Upgrade and Updater

Updater validation requires two independently signed versions. It cannot be
marked passed using a single first release. GitHub's `/releases/latest/`
endpoint does not expose draft or prerelease assets, so a candidate draft cannot
be tested through the production endpoint.

1. Install the previous public Briefli version.
2. Record a meeting and confirm at least one memory in a recurring space.
3. Back up `%APPDATA%\com.briefli.app\meeting_minutes.sqlite`.
4. Build a previous-version test copy with its updater endpoint overridden to a
   private staging host that serves the candidate's signed updater artifact and
   `latest.json`. Do not point the production build at staging.
5. Use **Check for Updates** in that test copy and install the staged update.
6. Confirm the displayed version, meeting, transcript, reviewed memory, space,
   and source link all survive.
7. Confirm `latest.json` reports the same version as `package.json`,
   `Cargo.toml`, and `tauri.conf.json`, with a non-empty Windows signature.

For Briefli's first public release, record updater validation as **not
applicable: no previous Briefli release**, not as passed. The next release must
complete the two-version staging test before publication. After publication,
verify the production endpoint once more from the previous public version; if it
fails, withdraw the release rather than publishing replacement assets under the
same version.