# Briefli Privacy and Data Handling

**Last updated:** 2026-07-14

Briefli is a local-first desktop application. It does not require a Briefli
account and does not operate a meeting-data cloud service.

## Data Kept on the Device

Briefli stores meeting recordings, transcripts, summaries, reviewed memories,
and recurring spaces in the application's local data folders.
Capture, transcription, full-text search, evidence matching, and the built-in
AI summary option run on the device.

Ollama uses `http://localhost:11434` by default. If you configure another
Ollama address, meeting text is sent to that address and may leave the device.

## Optional External AI Providers

Briefli supports OpenAI, Anthropic, Groq, OpenRouter, and custom
OpenAI-compatible endpoints. When one of these providers is selected for a
summary, Briefli sends the meeting transcript text, summary instructions, and
any custom summary prompt directly to that provider or endpoint. The recorded
audio file is not included in this summary request.

Those services process data under their own terms, privacy policies, retention
settings, and account configuration. Briefli cannot control what an external
provider retains. A custom endpoint may be local or remote; the person
configuring it is responsible for knowing where it runs.

Provider API keys and custom endpoint credentials are stored in Briefli's local
SQLite settings database. They are not described as encrypted at rest. Anyone
with access to the local application data may be able to access them.

## Other Network Requests

Briefli makes network requests when it downloads transcription or summary
models, checks GitHub Releases for application updates, or fetches a selected
provider's model list. These requests expose ordinary connection metadata such
as an IP address to the service being contacted, but they do not intentionally
include meeting content.

Briefli product telemetry is disabled. The optional brief/source usage counters
are disabled by default and, when enabled, remain in the local SQLite database.

## Recording Consent

Recording laws and workplace rules vary by location and context. Before
recording, tell every participant and obtain any consent required where the
conversation takes place. Briefli provides a reminder but cannot determine
whether a recording is lawful or permitted.

## Control and Portability

Briefli can copy transcripts and meeting records as Markdown, and Prepare briefs
as Markdown or JSON, without an account. The recordings folder is available
from Settings. The SQLite database and other application state remain in the
operating system's application-data directory and should be included in the
user's own backup and device-security practices.

Privacy questions and reproducible concerns can be filed in the
[Briefli issue tracker](https://github.com/karan68/Briefli/issues).
