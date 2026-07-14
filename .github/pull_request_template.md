<!--
Complete every applicable section. Leave checks unmarked when they were not run
and explain why. CI results are not a substitute for author-provided evidence.
See CONTRIBUTING.md for the full contract.
-->

## Summary

Describe the change in 2-4 bullets.

- Change 1

## Why This Change Exists

Explain the user-visible or engineering reason for the change. Describe the
failure or limitation being addressed, not only the implementation.

## Related Issue

<!-- Use "Fixes #123" when this PR should close an issue. -->

## Change Type

- [ ] Bug fix
- [ ] New feature
- [ ] Documentation
- [ ] Refactor or maintenance
- [ ] Performance
- [ ] Database or migration
- [ ] Build, CI, installer, or release

## Validation

Mark only commands that you ran successfully. Add exact results, focused tests,
manual workflows, or an explanation for non-applicable checks below.

- [ ] `git diff --check`
- [ ] `node .github/scripts/pr-policy-check.mjs`
- [ ] `cargo fmt --all -- --check` (Rust changes)
- [ ] `cargo test -p briefli --lib --locked -- --skip audio::playback_monitor::tests::test_get_output_device` (Rust changes)
- [ ] `cd frontend && bun test tests/lib` (frontend changes)
- [ ] `cd frontend && pnpm build` (frontend changes)
- [ ] Fresh-database migration replay completed (schema changes)
- [ ] Manual workflow or hardware validation completed (when applicable)

### Results and Failure-Path Coverage

List command results and the loading, empty, cancellation, rollback, malformed
input, recovery, or other failure states exercised.

## Product and Trust Contract

- [ ] AI-generated records remain suggestions until confirmed or corrected
- [ ] Evidence excerpts come from stored transcript segments
- [ ] Confirmed/corrected records survive regeneration and rejected suggestions do not silently reappear
- [ ] Local and external-provider behavior remains accurately distinguishable
- [ ] This change does not overclaim platform, hardware, signing, privacy, or validation support
- [ ] Not applicable; explanation included below

### Contract Impact

Describe effects on meetings, transcripts, reviewed memory, evidence links,
recurring spaces, exports, recovery, or the local/cloud boundary.

## Public-Repository Privacy Check

- [ ] No secrets, API keys, certificates, private endpoints, private hostnames, or private URLs are included
- [ ] No user-home paths, key paths, local databases, recordings, model files, meeting-content logs, or generated build output are included
- [ ] Infrastructure blockers are summarized without raw SSH output, credentials, sensitive logs, or machine-specific details
- [ ] External AI behavior states exactly what meeting data is transmitted and where
- [ ] Privacy or recording-consent documentation was updated when behavior changed

## Database and Persistence Check

- [ ] Existing migration files were not modified; schema changes use a new timestamped migration
- [ ] Transactions, regeneration, malformed input, and user-reviewed-state preservation were tested where applicable
- [ ] Upgrade, uninstall, or recovery behavior preserves existing user data where applicable
- [ ] No database or persistence impact

## Evidence and Artifacts

Link focused logs, screenshots, recordings without private meeting content,
test output, or benchmark notes. UI changes require desktop and narrow-window
screenshots, including affected loading, empty, disabled, and error states.

For audio/device changes, include OS version, hardware, selected devices,
online vs. in-person mode, and the tested capture path. Never describe an
unsigned local artifact as a signed release.

## Documentation Impact

List updates to the README, product charter, privacy policy, build/release docs,
or user-facing copy. If no documentation changed, explain why.

## Final Checklist

- [ ] The PR targets `devtest` unless a maintainer requested another branch
- [ ] The change is focused and unrelated refactors or generated files are excluded
- [ ] Tests were added or updated for changed behavior, or the absence of tests is explained
- [ ] I self-reviewed the diff and documented known limitations or remaining risks