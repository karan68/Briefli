# Briefli Code Signing Policy

**Status:** SignPath Foundation application planned after the first verifiable
beta release. Current beta installers are not Authenticode signed.

## Signing Provider

For trusted stable Windows releases, Briefli intends to use:

> Free code signing provided by [SignPath.io](https://about.signpath.io/),
> certificate by [SignPath Foundation](https://signpath.org/).

This statement describes the intended provider and does not claim approval before
Briefli appears in the [SignPath Foundation project list](https://signpath.org/projects).
Until approval is public, GitHub release notes and the README must describe
Windows installers as unsigned.

## Source and Release Origin

- Source repository: [karan68/Briefli](https://github.com/karan68/Briefli)
- Release page: [GitHub Releases](https://github.com/karan68/Briefli/releases)
- License: MIT
- Supported public release target: Windows 10/11 x64
- Android companion package: `com.briefli.companion`

Release binaries must be produced by GitHub-hosted runners from the exact commit
identified by the release workflow. Locally produced binaries are never
published as official releases.

## Team Roles

Briefli is currently maintained by one project owner:

- **Author and committer:** [@karan68](https://github.com/karan68)
- **Reviewer for external contributions:** [@karan68](https://github.com/karan68)
- **Release/signing approver:** [@karan68](https://github.com/karan68)

Changes from external contributors require maintainer review before merge. The
release approver reviews each signing request and release evidence manually.
These roles will be updated if the maintainer team changes.

## Release Requirements

Every public Windows release must:

1. use one version across `package.json`, `Cargo.toml`, `Cargo.lock`, and
   `tauri.conf.json`;
2. build from a pinned Git commit on GitHub-hosted runners;
3. pass the repository PR quality gate and release-specific validation;
4. produce Tauri updater signatures using the public key committed in
   `tauri.conf.json`;
5. produce SHA-256 checksums and GitHub build-provenance attestations;
6. pass clean install, launch, uninstall, and local-database preservation smoke
   tests; and
7. remain a draft or prerelease until its signing status and limitations are
   accurately documented.

After SignPath Foundation approval, stable MSI and NSIS installers must also
carry a valid Authenticode signature issued through the approved Briefli SignPath
project and signing policy. The release workflow must verify the resulting
signature before publication.

## Beta Releases

An unsigned beta may be published only when it is clearly labeled **Beta** and
**not Authenticode signed**. It must still satisfy the provenance, checksum,
updater-signature, and clean-runner smoke gates above. Windows may show an
unknown-publisher or SmartScreen warning for such a beta.

Users can verify beta provenance with GitHub CLI:

```powershell
$sourceSha = gh release view v<version> --repo karan68/Briefli --json targetCommitish --jq .targetCommitish
gh attestation verify .\Briefli_<version>_x64-setup.exe --repo karan68/Briefli --signer-workflow karan68/Briefli/.github/workflows/build.yml --source-digest $sourceSha --deny-self-hosted-runners
gh attestation verify .\Briefli_<version>_x64_en-US.msi --repo karan68/Briefli --signer-workflow karan68/Briefli/.github/workflows/build.yml --source-digest $sourceSha --deny-self-hosted-runners
```

Users should also compare `Get-FileHash -Algorithm SHA256` output with the
release's `SHA256SUMS.txt`.

## Android Companion Releases

Every published Android APK must:

1. use the same human-readable version as the desktop beta;
2. increment `versionCode` for every published APK;
3. be signed with the durable Briefli Android release key, never a debug key;
4. have its signer SHA-256 digest checked by the release workflow;
5. carry GitHub build-provenance attestation from the beta release workflow; and
6. be included in the release `SHA256SUMS.txt`.

The Android keystore and passwords are stored as encrypted GitHub Actions
secrets. A protected maintainer backup is required because Android updates must
continue using the same signing identity.

## Privacy

Briefli's data and network behavior is documented in
[`PRIVACY_POLICY.md`](PRIVACY_POLICY.md). Briefli does not transfer meeting
content unless the user selects or configures an external AI endpoint. The
summary workflow sends transcript text and instructions, not recorded audio.

Signing does not change the application's privacy behavior. Any new network
operation or data transfer requires corresponding product disclosure and privacy
policy updates before release.

## Key Handling

- Authenticode private keys remain in the signing provider's HSM and are never
  committed to or exported from this repository.
- The Tauri updater private key is stored only as an encrypted GitHub Actions
  secret and in an access-restricted maintainer backup.
- The Android release key is stored only as encrypted GitHub Actions secrets
  and in an access-restricted maintainer backup; it is never committed.
- Private keys, passwords, API tokens, certificates, and signing credentials
  must never appear in source, workflow output, issues, pull requests, or
  release assets.
- Losing the updater private key or passphrase requires a new application trust
  root and breaks automatic updates from existing versions; backups must be
  protected accordingly.

Report suspected signing-policy violations through the
[Briefli issue tracker](https://github.com/karan68/Briefli/issues). Suspected
credential exposure should be reported privately to the maintainer rather than
posted with the credential value.
