<div align="center">

# Briefli

**Private meeting memory — works even for in-person meetings.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows-white)](https://github.com/karan68/Briefli/releases)
[![CI](https://github.com/karan68/Briefli/actions/workflows/pr-quality-gate.yml/badge.svg)](https://github.com/karan68/Briefli/actions/workflows/pr-quality-gate.yml)

</div>

Briefli is a Windows desktop app that records and transcribes meetings on your device, without a meeting bot or account. It turns reviewed decisions, commitments, and open questions into source-backed memory that can be brought back before the next conversation.

Built on [Tauri](https://tauri.app/) (Rust) + Next.js.

---

## Download

Download Windows x64 and experimental Windows ARM64 builds from [GitHub Releases](https://github.com/karan68/Briefli/releases).

- **Stable releases:** reserved for installers with trusted Authenticode signing.
- **Beta releases:** may be unsigned and trigger an unknown-publisher or SmartScreen warning. The release notes state the signing status for each version.

Do not treat temporary workflow artifacts or locally built installers as public releases.

### User requirements

- Windows 10/11 x64, or Windows 11 ARM64 (experimental)
- Microphone permission
- Internet access for first-run model downloads and application updates
- Several gigabytes of free space if you install a local summary model and the optional Parakeet transcription upgrade

Briefli needs no account. Tiny Whisper, the initial transcription model, is approximately 31 MB. Parakeet is an optional approximately 670 MB accuracy upgrade. The recommended local summary model depends on available memory and is downloaded during setup.

---

## Why Briefli

| Feature | Briefli |
|---|---|
| Local capture, transcription, storage, and cross-meeting search | Yes |
| Online-call and in-person microphone-only capture | Yes |
| Decisions, commitments, and open questions require user review | Yes |
| Memory items link back to transcript evidence when a strong match exists | Yes |
| Repeat-conversation briefs use only confirmed or corrected memory | Yes |
| Briefli product telemetry | None |
| Account, meeting bot, or subscription | None |
| License | Free and MIT licensed |

---

## Quick Start

1. **Install and launch Briefli.** Complete first-run setup. Recording unlocks as soon as Tiny Whisper or another local transcription engine is ready.
2. **Choose your capture mode.** Expand the sidebar, open **Settings > Recordings**, and select a microphone and system-audio device. Enable **In-person meeting** for microphone-only capture.
3. **Record.** Return to **Home** and select **Start recording**. Briefli reminds you to inform participants and obtain any required consent. Pause, resume, or stop from the recording bar or system tray.
4. **Build the meeting record.** Open the saved meeting and select **Build record**. Choose a template, output language, and AI provider if needed.
5. **Review Memory.** Expand the sidebar and open **Memory**. Confirm, correct, or reject suggested decisions, commitments, and open questions.
6. **Group repeat conversations.** Open **Prepare**, create a recurring space for a client or project, and assign meetings to it.
7. **Prepare next time.** Briefli builds a source-backed brief from confirmed decisions and still-open commitments and questions. Open a source to return to the meeting evidence, or copy the brief as Markdown or JSON.

---

## Feature Guide

### Capture and transcription

| Feature | How to use it | Important behavior |
|---|---|---|
| Online-call capture | Open **Settings > Recordings**, choose microphone and system-audio devices, then select **Start recording** on Home. | Captures microphone and Windows system audio locally. Device access and microphone permission are required. |
| In-person capture | Open **Settings > Recordings** and enable **In-person meeting** before recording. | Captures the microphone only. System audio is disabled even if a system device is selected. |
| Fast first transcript | Complete onboarding and wait for the **Ready-to-record model** to finish. | Tiny Whisper runs locally after download. Recording is not blocked by the larger optional Parakeet model. |
| Accuracy upgrade | During onboarding, select **Download accuracy upgrade**, or manage transcription models later in **Settings > Transcription**. | Parakeet runs locally and is approximately 670 MB. It is optional. |
| Live controls | Use **Pause**, **Resume**, and **Stop** in the recording bar, or use the Briefli system-tray menu. | Stopping waits for queued transcript work before saving the meeting. Closing the main window hides Briefli to the tray rather than quitting. |
| Interrupted-meeting recovery | Relaunch Briefli after an interrupted recording. Choose **Recover** or **Delete** in the recovery dialog. | Recoverable transcript state is kept locally for up to seven days. Transcript recovery can work without audio if audio checkpoints are unavailable. |
| Saved audio | Open **Settings > Recordings**, enable **Save Audio Recordings**, and use **Open Folder** to inspect recordings. | Disabling audio saving still keeps transcripts and meeting records, but later playback will not be available. |

### Meetings, search, and evidence

| Feature | How to use it | Important behavior |
|---|---|---|
| Meeting library | Expand the sidebar and select a meeting. Hover a meeting to rename or delete it. | Meetings and transcripts are stored in local SQLite. Deleting a meeting removes its database records but does not delete its recording folder from disk. |
| Search all meetings | Expand the sidebar and type terms into **Search all meetings**. Select a result to open that meeting. | Search is local and case-insensitive. Every typed term must occur somewhere in the meeting. Results are ranked by occurrence count. |
| Evidence navigation | Select a result from search, **Memory**, or **Prepare**. | Briefli opens and highlights the matching transcript segment. If timed audio exists, it also seeks to that point. |
| Playback | Open a completed meeting and use the controls above its transcript. | Playback is local and appears only when a saved audio file is available. |
| Interactive timeline | Open a meeting, expand **Timeline**, and select a marker. | Timeline markers are generated deterministically for chapters, likely actions, questions, and resumptions after long gaps. They are navigation hints, not AI-confirmed commitments. |

### AI meeting records

1. Open a completed meeting.
2. Optionally add names or meeting context in the transcript panel.
3. Choose **AI Model**, **Template**, and summary language.
4. Select **Build record**.
5. Edit the generated record and select **Save**. Transcript and record Markdown can also be copied to the clipboard.

Bundled templates include Standard Meeting, Daily Standup, Project Sync, Retrospective, Client/Sales, and Psychiatric Session. Summary language can be detected automatically or selected explicitly.

#### AI provider behavior

| Provider | Where processing happens |
|---|---|
| Built-in AI | On this device. This is the fresh-install default. |
| Ollama at `http://localhost:11434` | On this device. |
| Ollama at another address | At the configured server; transcript text may leave this device. |
| OpenAI, Anthropic, Groq, or OpenRouter | The selected provider receives transcript text, summary instructions, and any custom summary prompt. |
| Custom OpenAI-compatible endpoint | At the configured endpoint, which may be local or remote. |

The summary workflow does not send the recorded audio file to an external AI provider. Provider retention and account policies still apply to text sent to that provider. See [Privacy and data handling](PRIVACY_POLICY.md).

### Trusted Memory

Memory suggestions are created from recognized sections in a generated meeting record. A custom summary that omits decisions, action items/commitments/next steps, and open questions may produce no Memory suggestions.

1. Generate the meeting record.
2. Expand the sidebar and open **Memory**.
3. Use **Review** to inspect new suggestions.
4. Select **Confirm**, **Correct**, or **Reject**. Corrections can include an owner and due date for commitments.
5. Use **Open loops** to mark commitments and questions done or reopen them.
6. Use **Weekly** to revisit confirmed open loops that have not been reviewed for seven days.
7. Select the meeting/source link to verify the transcript evidence.

Machine-generated items never become confirmed facts automatically. Evidence is shown only when local transcript matching clears the required threshold; otherwise Briefli asks you to verify the meeting directly. Correcting an owner can teach Briefli an exact local name alias, which can be inspected and forgotten from the Memory page.

### Prepare for a repeat conversation

1. Expand the sidebar and open **Prepare**.
2. Create a recurring space, such as a client or project.
3. Assign prior meetings to that space.
4. Review the brief containing confirmed/corrected decisions, open commitments, and unresolved questions.
5. Select a source link to return to its transcript evidence.
6. Copy the brief as Markdown or JSON when you need it elsewhere.

Spaces are managed manually; Briefli does not infer a person or client graph. Prepare excludes rejected suggestions and completed commitments/questions. Optional Prepare usage counters are disabled by default, stored only in local SQLite, and can be cleared from the Prepare page.

### Import and improve existing audio (Beta)

1. Open **Settings > Beta** and enable **Import Audio & Retranscribe**.
2. Use **Import Audio** in the sidebar or drag an audio/video file into Briefli.
3. Choose an installed local transcription model and import the file.
4. For an existing meeting with saved audio, use **Enhance** to retranscribe it.

Supported inputs include MP4, M4A, WAV, MP3, FLAC, OGG, AAC, MKV, WebM, and WMA. This workflow is local and uses bundled FFmpeg plus an installed transcription model. It remains a beta feature and is hidden by default.

### Settings and updates

- **General:** recording notifications and the local recordings folder.
- **Recordings:** audio saving, microphone/system devices, and in-person mode.
- **Transcription:** install and select local Whisper or Parakeet models.
- **Summary:** choose local or remote AI, control automatic summaries, and set preferred languages.
- **Beta:** enable audio import and retranscription.
- **About > Check for Updates:** check GitHub Releases and install a signed update.

The tray menu can start, pause, resume, and stop a recording; open the main window or Settings; check for updates; or quit Briefli completely.

---

## Data and Privacy

- Meeting records and transcripts are stored in `%APPDATA%\com.briefli.app\meeting_minutes.sqlite` on Windows.
- Recordings stay in the local recordings folder shown in Settings.
- Briefli product telemetry is disabled.
- Optional Prepare usage counters stay in the local database and are disabled by default.
- External summary providers receive text only when you select/configure them.
- You are responsible for informing participants and obtaining any consent required where you record.

Read the complete [Briefli Privacy and Data Handling policy](PRIVACY_POLICY.md).

---

## Current Limitations

- Windows 10/11 x64 is the supported beta target. Windows 11 ARM64 is experimental and has not been exercised with physical audio devices.
- Search is lexical substring matching, not semantic search.
- Memory suggestions require a generated record with recognized sections.
- Briefli does not currently provide speaker diarization, contacts, calendar integration, or automatic reminders.
- Audio import/retranscription is beta.
- The installer downloads Tiny Whisper during onboarding; the model is not yet bundled in the installer.

---

## Development Setup

### Prerequisites

- [Node.js 20+](https://nodejs.org/)
- [Rust stable](https://rustup.rs/)
- pnpm 9.15.9
- Bun 1.3.14 for frontend tests
- Platform build dependencies from [docs/BUILDING.md](docs/BUILDING.md)

### Development

```powershell
# Install locked dependencies
cd frontend
pnpm install --frozen-lockfile

# Run the desktop app (the first Rust build is slow)
.\node_modules\.bin\tauri.cmd dev
```

### Type-check only

```powershell
cd frontend
.\node_modules\.bin\tsc.cmd --noEmit -p tsconfig.json
```

---

## Development Testing

Tests are written with [Bun](https://bun.sh/) and run automatically in CI on every pull request.

```powershell
# Run all unit tests
cd frontend
bun test tests/lib
```

### Test suite

| File | What it covers |
|---|---|
| `tests/lib/meeting-timeline.test.ts` | Timeline segment parsing and audio seek |
| `tests/lib/meeting-memory.test.ts` | Cross-meeting memory and search ranking |
| `tests/lib/conversation-brief.test.ts` | Meeting brief / preparation summaries |
| `tests/lib/onboarding-summary-model.test.mjs` | Onboarding summary-model selection flow |
| `tests/lib/summary-language-preferences.test.js` | Language preference persistence and fallback behavior |
| `tests/lib/blocknote-markdown.test.ts` | BlockNote ↔ Markdown serialisation |
| `.github/scripts/pr-policy-check.test.mjs` | PR policy checker self-tests |

---

## CI / CD pipeline

Every pull request runs the full quality gate ([`.github/workflows/pr-quality-gate.yml`](.github/workflows/pr-quality-gate.yml)):

| Job | What runs |
|---|---|
| `ci / policy` | PR policy checker, migration append-only validation, workflow YAML lint |
| `ci / frontend` | `bun test tests/lib`, seed-script compile check, `pnpm build` |
| `ci / rust` | `cargo check` and `cargo test` (Linux, Windows, and macOS) |

Release builds are handled by [`.github/workflows/release.yml`](.github/workflows/release.yml).

---

## Project structure

```
frontend/                   Next.js + Tauri desktop app
  src/                      React components and pages
  src-tauri/                Rust backend (audio capture, transcription, DB)
  tests/lib/                Unit tests (Bun)
.github/
  workflows/                CI/CD (GitHub Actions)
  scripts/                  PR policy checker
docs/                       Product charter and design notes
```

---

## Architecture

- **Frontend**: Next.js 14 (App Router), React, Tailwind CSS, shadcn/ui, BlockNote
- **Backend (Rust/Tauri)**: audio capture (WASAPI/CoreAudio), Whisper.cpp, Parakeet, SQLite via SQLx, summary engine (sidecar + HTTP), system tray
- **Database**: `meeting_minutes.sqlite` — meetings, transcripts, summaries, reviewed memory, and recurring spaces
- **AI**: pluggable - Built-in AI, Ollama, OpenAI, Anthropic, Groq, OpenRouter, and custom OpenAI-compatible endpoints

---

## Roadmap

- [ ] Contact-aware people views without an automatic entity graph
- [ ] Installer bundling tiny Whisper model (no download step)
- [ ] Semantic retrieval where real lexical-search failures justify it
- [ ] Completion, contradiction, and supersession suggestions with user confirmation
- [ ] Validated macOS public release support

---

## Contributing

PRs are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before starting, keep each change focused, and complete the default pull request template with the exact validation and evidence you collected. The CI quality gate runs automatically and must pass before merge.

---

## License

MIT — see [LICENSE](./LICENSE).

Originally forked from [Meetily](https://github.com/Zackriya-Solutions/meetily) by Zackriya Solutions (MIT).
