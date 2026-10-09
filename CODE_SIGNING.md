# Code signing policy

Official Windows releases of Gitru are intended to be Authenticode-signed through
[SignPath.io](https://signpath.io/), with a certificate issued to SignPath
Foundation. Free code signing is provided by SignPath.io; the certificate is
provided by [SignPath Foundation](https://signpath.org/).

## Official source and releases

- Source repository: <https://github.com/ruru-m07/gitru>
- Official releases: <https://github.com/ruru-m07/gitru/releases>
- Security reports: [SECURITY.md](SECURITY.md)

Only artifacts built from the official repository by the GitHub Actions release
workflow are eligible for release signing. A valid Authenticode signature proves
the artifact passed the configured SignPath signing policy; it does not replace
Gitru's independent Tauri updater signature.

## Team roles

- Committer and reviewer: [Rutvik Movaliya (`ruru-m07`)](https://github.com/ruru-m07)
- Signing approver: [Rutvik Movaliya (`ruru-m07`)](https://github.com/ruru-m07)

Changes from other contributors are reviewed before they are merged. Changes to
release workflows, build scripts, dependencies, updater configuration, and this
policy receive the same review as application code.

## Signing controls

Production signing requests must:

1. originate from the official GitHub repository and a GitHub-hosted runner;
2. identify the source commit and build workflow through SignPath origin
   verification;
3. use the configured release-signing policy and artifact configuration;
4. receive the required manual approval in SignPath;
5. contain only Gitru-owned binaries selected by the artifact configuration;
6. pass Authenticode verification before publication; and
7. receive a fresh Tauri updater signature after Authenticode signing.

The SignPath API token is stored only as an encrypted GitHub Actions secret. The
release certificate's private key remains in SignPath's hardware-backed key
store and is never exported to GitHub Actions or a maintainer computer.

## Privacy

Gitru does not transfer repository paths, source code, diffs, remotes, branches,
commit data, credentials, or hosted-service tokens to Gitru or SignPath.

Anonymous usage analytics is disabled until the user explicitly enables it. If
enabled, Gitru sends fixed app-open and presence event names and basic runtime
metadata such as the operating system and screen size to the analytics endpoint
identified by the application. Autocapture, session recording, surveys, page
URLs, titles, referrers, and person profiles are disabled. The anonymous
identifier is memory-only and changes between launches. The in-app setting stops
collection immediately.

During release signing, GitHub and SignPath process the release artifact and its
build provenance. This is a maintainer-controlled release operation and does not
contain user repository data.

## Verification

On Windows, users can inspect an installer in **Properties > Digital
Signatures**, or run:

```powershell
Get-AuthenticodeSignature .\Gitru_*_x64-setup.exe | Format-List
```

The signature must be valid and chain to the publisher certificate displayed for
the official Gitru release. The Tauri updater separately validates the signature
published in Gitru's update manifest before installing an update.
