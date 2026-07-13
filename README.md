<div align="center">

# Briefli

**Private meeting memory — works even for in-person meetings.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows-white)](https://github.com/karan68/Briefli/releases)
[![CI](https://github.com/karan68/Briefli/actions/workflows/pr-quality-gate.yml/badge.svg)](https://github.com/karan68/Briefli/actions/workflows/pr-quality-gate.yml)

</div>

Briefli is a desktop app that records, transcribes, and summarizes your meetings **entirely on your own machine**. No cloud. No bots. No subscription. Audio never leaves your device.

Built on [Tauri](https://tauri.app/) (Rust) + Next.js. Forked from [Meetily](https://github.com/Zackriya-Solutions/meetily) and extended with cross-meeting memory, commitments tracking, and in-person capture.

---

## What makes it different

| Feature | Briefli |
|---|---|
| 100% local — audio never leaves device | ✅ |
| Works for in-person meetings (mic-only) | ✅ |
| Cross-meeting memory search | ✅ |
| Commitments / action-item tracker | ✅ |
| Zero telemetry | ✅ |
| Free, MIT licensed | ✅ |

---

## Key features

- **Local transcription** — ships with tiny Whisper (31 MB, instant) and upgrades to Parakeet (670 MB, higher accuracy) in the background.
- **AI summaries** — runs on Ollama, OpenAI, Anthropic, Groq, or OpenRouter. You choose the model.
- **Cross-meeting search** — full-text search over all your transcripts via SQLite FTS5. Click a result to jump to that moment.
- **Commitments tracker** — extracts and reconciles action items across meetings, linking each back to the source transcript.
- **In-person mode** — mic-only capture, no system audio required.
- **Interactive timeline** — click any segment to seek to that point in the recording.

---

## Getting started

### Prerequisites

- Windows 10/11 (x64)
- [Node.js 20+](https://nodejs.org/)
- [Rust stable](https://rustup.rs/)
- [Ollama](https://ollama.com/) (optional, for local AI summaries)

### Development

```powershell
# Install dependencies
cd frontend
npm install

# Run dev server (first build is slow — Rust compiles ~5 min)
.\node_modules\.bin\tauri.cmd dev
```

### Type-check only

```powershell
cd frontend
.\node_modules\.bin\tsc.cmd --noEmit -p tsconfig.json
```

---

## Testing

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
| `tests/lib/onboarding-summary-mode.test.ts` | Onboarding model selection flow |
| `tests/lib/summary-language-prefer.test.ts` | Language preference for summaries |
| `tests/lib/blocknote-markdown.test.ts` | BlockNote ↔ Markdown serialisation |
| `.github/scripts/pr-policy-check.test.mjs` | PR policy checker self-tests |

---

## CI / CD pipeline

Every pull request runs the full quality gate ([`.github/workflows/pr-quality-gate.yml`](.github/workflows/pr-quality-gate.yml)):

| Job | What runs |
|---|---|
| `ci / policy` | PR policy checker, migration append-only validation, workflow YAML lint |
| `ci / frontend` | `bun test tests/lib`, seed-script compile check, `pnpm build` |
| `ci / rust` | `cargo check`, `cargo clippy`, `cargo test` (Linux + Windows) |

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
- **Backend (Rust/Tauri)**: audio capture (WASAPI/CoreAudio), Whisper.cpp, Parakeet, SQLite (via Diesel), summary engine (sidecar + HTTP), system tray
- **Database**: `meeting_minutes.sqlite` — meetings, transcripts, summaries, FTS5 index
- **AI**: pluggable — Ollama (local), OpenAI, Anthropic, Groq, OpenRouter

---

## Roadmap

- [ ] People and commitments tracking across contacts
- [ ] Installer bundling tiny Whisper model (no download step)
- [ ] Custom logo / icon
- [ ] Semantic (embedding-based) search alongside FTS5
- [ ] macOS support

---

## Contributing

PRs are welcome. Please keep changes focused — one feature or fix per PR. The CI quality gate runs automatically and must pass before merge.

---

## License

MIT — see [LICENSE](./LICENSE).

Originally forked from [Meetily](https://github.com/Zackriya-Solutions/meetily) by Zackriya Solutions (MIT).
