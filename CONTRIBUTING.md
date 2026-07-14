# Contributing to Briefli

Thank you for improving Briefli. Briefli is a privacy-first meeting memory
application, so correctness includes persistence, evidence provenance, honest
local/cloud boundaries, and recovery behavior, not only whether a screen works.

## Before You Start

- Search existing issues and pull requests before starting duplicate work.
- Open an issue before a large feature, schema change, dependency change, or
   product-direction change.
- Keep each pull request focused on one feature, fix, or maintenance concern.
- Target `devtest` unless a maintainer asks for another branch.
- Do not commit generated installers, model files, databases, recordings,
   credentials, or machine-specific configuration.

The active product contract and non-goals are documented in
[`docs/PRODUCT_CHARTER.md`](docs/PRODUCT_CHARTER.md). Privacy behavior is
specified in [`PRIVACY_POLICY.md`](PRIVACY_POLICY.md).

## Development Setup

Briefli uses Node.js 20, pnpm 9.15.9, Bun 1.3.14, Rust stable, and Tauri 2.
Platform build dependencies are described in [`docs/BUILDING.md`](docs/BUILDING.md)
and [`docs/GPU_ACCELERATION.md`](docs/GPU_ACCELERATION.md).

From the repository root:

```bash
cd frontend
pnpm install --frozen-lockfile
pnpm tauri:dev
```

On Windows, the CPU-only development command can also be run directly:

```powershell
cd frontend
.\node_modules\.bin\tauri.cmd dev
```

## Engineering Boundaries

Keep changes within the existing ownership model:

- SQLite is the source of truth for meetings, transcripts, summaries, and
   durable memory.
- Rust repositories own transactions, reconciliation, evidence matching, and
   state validation.
- Tauri commands expose typed operations to the frontend.
- React owns presentation and interaction, with rollback for failed mutations.
- AI output remains a suggestion until the user confirms or corrects it.
- Evidence excerpts must come from stored transcript segments.
- Confirmed or corrected memories must survive summary regeneration.
- Local and external-provider behavior must remain distinguishable to users.

Do not add semantic retrieval, automatic entity graphs, cloud processing, or
other roadmap work merely to complete a checkbox. New behavior should solve an
observed product problem and preserve source traceability.

## Database Changes

Migrations are append-only after they are merged. Never edit an existing file in
`frontend/src-tauri/migrations/`; add a new timestamped migration instead.

Schema changes must include focused repository tests and must be tested by
applying every migration in filename order to a fresh SQLite database. Changes
that reconcile generated data must also test regeneration, malformed input,
and preservation of user-reviewed state.

## Privacy and Public-Repository Safety

Before opening a pull request:

- Remove API keys, tokens, certificates, private hostnames, and private URLs.
- Remove local databases, recordings, model files, logs with meeting content,
   and generated build output.
- Remove user-home paths, key paths, and machine-specific commands or output.
- Summarize infrastructure failures without pasting credentials, private
   endpoints, raw SSH output, or sensitive logs.
- State exactly when transcript text is sent to an external provider; do not
   describe an external endpoint as local unless that is guaranteed.
- Do not claim encryption, platform support, hardware support, signing, or
   successful validation without current evidence.

The PR policy check rejects common private or generated files and modified
historical migrations:

```bash
node .github/scripts/pr-policy-check.mjs
node --test .github/scripts/pr-policy-check.test.mjs
```

## Validation

Run the narrowest relevant checks while developing, then report every check you
actually ran in the pull request. Do not mark a check complete because CI is
expected to run it later.

### Every Change

```bash
git diff --check
node .github/scripts/pr-policy-check.mjs
```

### Frontend Changes

```bash
cd frontend
bun test tests/lib
pnpm build
```

Add or update focused tests under `frontend/tests/lib` for changed helpers and
user-visible state behavior.

### Rust Changes

```bash
cargo fmt --all -- --check
cargo test -p briefli --lib --locked -- \
   --skip audio::playback_monitor::tests::test_get_output_device
```

Run a narrower test first when possible. Audio, device, or acceleration changes
must include the operating system, hardware, selected devices, and capture mode
used for manual validation.

### Workflow Changes

Run [actionlint](https://github.com/rhysd/actionlint) against all files in
`.github/workflows`. Keep reusable workflow inputs and release permissions as
narrow as possible.

### UI Changes

Attach screenshots for desktop and narrow-window layouts. Include any loading,
empty, disabled, error, confirmation, and rollback states affected by the
change. Verify that text does not overlap or overflow.

### Release and Installer Changes

Document the exact Windows version, installer format, signing status, and manual
install/upgrade/uninstall checks performed. Never describe a local unsigned
build as a signed release artifact.

## Pull Requests

GitHub automatically pre-fills the repository pull request template. Complete
all applicable sections and leave non-applicable checks unchecked with a short
explanation. A useful pull request includes:

- a concise summary and the reason the change exists;
- a linked issue when one exists;
- exact commands run and their results;
- screenshots or hardware evidence where relevant;
- privacy, persistence, migration, and product-contract impact; and
- documentation updates or a reason none are needed.

CI is required but is not a substitute for accurate author-provided evidence.
Reviewers may ask for a smaller change, additional failure-path coverage, or
manual validation on supported Windows environments.

## License

By contributing, you agree that your contributions will be licensed under the
repository's MIT License.