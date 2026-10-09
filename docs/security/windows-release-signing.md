# Windows release-signing architecture

This document is the implementation source of truth for integrating SignPath
with Gitru's Tauri release pipeline.

## Current state

- SignPath organization: `Gitru [OSS]`
- SignPath project slug: `gitru`
- Trusted build system: GitHub.com, linked to the project
- Test policy: `test-signing`, valid, no approval or origin requirement
- Release policy: `release-signing`, one approval, trusted-build and origin
  verification enabled
- Release certificate: pending certificate-signing request; the production
  policy remains invalid until issuance completes
- Default artifact configuration: a single PE file with Authenticode signing
- GitHub default branch: `dev`
- Current production workflow: `.github/workflows/ship.yml`, triggered by a
  published GitHub release
- Windows release bundle: Tauri NSIS `Gitru_<version>_x64-setup.exe`

The current GitHub ruleset prevents deletion and non-fast-forward updates of the
default branch. It does not currently require pull requests or approvals. The
SignPath release policy currently accepts branch pattern `**` and does not
restrict build definitions. These controls must be tightened before production
signing is enabled.

## Trust boundaries

Three signatures serve different purposes:

1. **Authenticode** is applied by SignPath to the Windows installer and identifies
   the publisher while protecting installer integrity.
2. **Tauri updater signing** uses Gitru's existing minisign-compatible updater
   key. It authorizes an artifact for installation by existing Gitru clients.
3. **Git tags and GitHub release provenance** identify the source revision and
   release record, but do not replace either binary signature.

Authenticode changes the installer bytes. Therefore, the Tauri updater signature
must be generated from the final SignPath output. Reusing the pre-SignPath `.sig`
would make updater verification fail and must be treated as a release-blocking
error.

## Required production flow

1. Validate the release tag and resolve one immutable source commit.
2. Build and test on GitHub-hosted runners from that commit.
3. Build the Windows NSIS installer without publishing it.
4. Upload the unsigned installer as a GitHub workflow artifact without wrapping
   it in another ZIP archive.
5. Submit that workflow artifact to SignPath with verified GitHub origin.
6. Wait for the required manual production approval and signing completion.
7. Download the signed installer and verify a valid Authenticode signature,
   expected filename, PE metadata, and source version.
8. Generate a new Tauri updater signature over the signed installer.
9. Publish the signed installer and its matching updater signature together.
10. Generate `latest.json` only from the final published assets, validate every
    URL and signature, then publish the manifest.

No unsigned Windows installer may be uploaded to a public release as a fallback.
A signing failure must fail the Windows release and block manifest publication.

## Qualification stages

### Stage 1: test certificate

`.github/workflows/signpath-qualification.yml` builds an unsigned NSIS installer,
submits it using `test-signing`, verifies the returned Authenticode signature,
and stores the result as a private workflow artifact. It cannot publish GitHub
release assets or update manifests. The build job has no signing secret. A
separate signing job uses the protected `signpath-test` GitHub environment and
runs no repository build scripts.

This stage establishes:

- the actual Tauri output path and filename;
- compatibility with SignPath's direct PE artifact configuration;
- GitHub trusted-build provenance;
- the returned artifact layout; and
- PowerShell Authenticode verification behavior.

### Stage 2: release configuration

After Stage 1 succeeds, create a dedicated SignPath artifact configuration with
metadata restrictions based on the observed installer metadata. Do not guess
metadata values. Restrict the release policy to the release source branch and
the exact workflow definition, and enforce GitHub-hosted runners through a
pipeline policy.

### Stage 3: production pipeline

Refactor `ship.yml` so Windows build, SignPath signing, updater signing, asset
publication, and manifest publication are explicit ordered jobs. macOS and Linux
publication may continue independently, but manifest publication must wait for
all required platform assets.

## Secret handling

- `SIGNPATH_API_TOKEN` is an encrypted GitHub Actions environment secret. The
  test token is scoped to `signpath-test`; production must use a separate
  protected environment. Build jobs do not receive either token.
- SignPath's certificate private key is non-exportable and must remain in its
  configured HSM-backed key store.
- `TAURI_SIGNING_PRIVATE_KEY` remains a separate GitHub Actions secret. It is
  consumed only after SignPath returns the final Windows installer.
- Workflows must not print tokens, signing keys, complete environment dumps, or
  signed-artifact download URLs with embedded authorization.
- New third-party actions used in the signing path are pinned to immutable commit
  SHAs and reviewed before updates.

## Release-blocking invariants

- The resolved release commit is the same commit built by every platform job.
- Production signing uses `release-signing`, never `test-signing`.
- Production signing is unavailable while the release certificate or policy is
  invalid.
- The SignPath request reports verified origin for the official repository.
- Authenticode verification reports `Valid` and a timestamped signature.
- The Tauri updater signature is newer than and matches the signed installer.
- `latest.json` references the signed Windows asset and contains its matching
  Tauri signature.
- Failure at any step prevents Windows publication and manifest promotion.
