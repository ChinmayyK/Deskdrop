# Deskdrop Backend High-Stakes Audit

Date: 2026-06-15

## Scope

This report audits Deskdrop's cross-platform backend, transport, persistence, release hardening, and operational safety across Android, macOS, and Windows.

It is based on:

- current repository inspection on 2026-06-15
- validation runs against the present codebase
- platform security guidance from Apple, Google, Microsoft, and OWASP

This report is intentionally strict. The user asked for high-stakes backend work, so the emphasis is on trust, secret handling, release integrity, data-loss prevention, and abuse resistance.

## Severity Model

- `P0`: ship-blocking trust, security, or release-integrity issue
- `P1`: major hardening or reliability gap with serious operational impact
- `P2`: important but lower-urgency improvement

## Validation Snapshot

- `cargo test --workspace --all-targets` currently fails due to a contract drift in file transfer tests: `deskdrop-core/src/file_transfer.rs:1040-1047`
- `./gradlew lintDebug testDebugUnitTest` currently fails at `:app:lintDebug`; first reported error is an API-level violation at `platforms/android/app/src/main/java/com/deskdrop/DeskdropService.kt:2200`
- `xcodebuild` was unavailable in this environment because the active developer directory was Command Line Tools instead of full Xcode
- `dotnet build` was unavailable in this environment because `dotnet` was not installed locally

That means this report is grounded in real code inspection plus partial execution, not in stale documentation alone.

## Executive Summary

Deskdrop's highest-stakes backend problem is not raw transport cryptography. The protocol and trust model are better than a casual first glance suggests.

The biggest risks are around when the system decides trust has been earned, how secrets are stored locally, how permissive the default remote-action settings are, and how much release hardening is still deferred to "later."

If I were prioritizing this as a ship-risk backlog, I would do these first:

1. block QR trust until proof is validated
2. move secrets into platform secret stores
3. flip remote clipboard and file intake defaults to least-privilege
4. harden release signing and bootstrap integrity on every platform
5. close the current CI and packaging regressions before further feature expansion

## 1. Gate trust on verified QR proof only

Severity: `P0`  
Platforms: `Android`, `macOS`, `Windows`, `core`

Current repo evidence:

- IPC trust-from-QR path trusts the peer before QR auth is validated: `deskdrop-core/src/ipc.rs:667-672`
- daemon path does the same: `deskdrop-core/src/bin/daemon.rs:696-700`
- token generation and send happen later: `deskdrop-core/src/engine.rs:2137-2154`
- actual token validation happens only when `AppMessage::QrAuth` is received: `deskdrop-core/src/engine.rs:4462-4488`

Why this is high stakes:

This is a trust-escalation flaw, not a polish issue. The system currently records trust before the remote side proves possession of the QR token. That means the UI and persistence layers can treat a peer as trusted earlier than the proof ceremony actually warrants.

Improvement:

- treat QR auth as a pending trust flow until token validation succeeds
- persist trust only after successful validation on both sides
- add regression tests for negative cases: stale token, wrong peer, replay, disconnected peer

## 2. Move identity and pairing secrets into OS-backed secret stores

Severity: `P0`  
Platforms: `Android`, `macOS`, `Windows`, `core`

Current repo evidence:

- the identity store comments explicitly note that keychain integration is not enabled in this build: `deskdrop-core/src/identity.rs:7-20`
- current load and save paths are plain file reads and writes: `deskdrop-core/src/identity.rs:139-199`

Why this is high stakes:

Long-lived device identity is the anchor for trust decisions. Keeping that key as a plain local file is not aligned with the security expectations of desktop platforms or with OWASP's storage guidance for apps handling sensitive trust material.

Improvement:

- macOS: store identity keys in Keychain, preferably with Secure Enclave-backed generation where feasible
- Android: store persistent device identity and pairing secrets in Android Keystore
- Windows: protect identity material with DPAPI
- add one-time migration from file storage into the platform store, with rollback only for explicit recovery mode

## 3. Flip dangerous remote-action defaults to least-privilege

Severity: `P0`  
Platforms: `Android`, `macOS`, `Windows`, `core`

Current repo evidence:

- defaults include unlimited payload size, automatic remote clipboard apply, automatic file acceptance, and unlimited auto-accept size: `deskdrop-core/src/settings.rs:184-207`

Why this is high stakes:

These defaults create a bad failure mode cluster:

- unexpected clipboard overwrite
- disk abuse through large remote transfers
- support burden from "it synced something I did not expect"
- privacy issues on shared or regulated machines

Improvement:

- default `auto_apply_remote_clipboard` to `false`
- default `auto_accept_file_transfers` to `false`
- ship conservative size ceilings
- require per-peer opt-in for privileged automation features
- expose clear UI and audit events when a user enables them

## 4. Encrypt sensitive local state at rest and classify what should persist

Severity: `P0`  
Platforms: `Android`, `macOS`, `Windows`, `core`

Current repo evidence:

- trust store persists as pretty JSON: `deskdrop-core/src/trust.rs:100-107`
- settings persist as pretty JSON: `deskdrop-core/src/settings.rs:351-359`
- history persists as pretty JSON after scrubbing: `deskdrop-core/src/history.rs:572-588`

Why this is high stakes:

The code does use atomic rename, which is good, but secrecy and durability are separate concerns. Trust graphs, device metadata, and clipboard-derived history can still be sensitive data-at-rest. On Windows and macOS laptops especially, local compromise and backup leakage are not hypothetical.

Improvement:

- classify persisted data into public, private, and secret tiers
- encrypt private and secret tiers with platform-backed keys
- add explicit policy for clipboard, filenames, and peer metadata retention
- verify backup and export behavior so sensitive state does not leak indirectly

## 5. Enforce sane transfer ceilings and disk budget controls

Severity: `P1`  
Platforms: `Android`, `macOS`, `Windows`, `core`

Current repo evidence:

- the comment says "Maximum transfer size (4 GB)"
- the actual constant is `1 TB`: `deskdrop-core/src/file_transfer.rs:42-45`
- inbound enforcement uses that constant: `deskdrop-core/src/file_transfer.rs:675-682`

Why this is high stakes:

This mismatch is large enough to change the threat model. A 1 TB announced transfer is not just a large file; it is a disk exhaustion and support incident waiting to happen, especially with auto-accept defaults still enabled.

Improvement:

- choose realistic platform-aware caps
- enforce per-peer quotas and total-disk budget checks
- surface caps in settings and onboarding
- add tests that fail if code comments, settings defaults, and enforcement diverge again

## 6. Make receive finalization crash-safe, durable, and resumable

Severity: `P1`  
Platforms: `Android`, `macOS`, `Windows`, `core`

Current repo evidence:

- finalize verifies checksum and renames the temp file, but there is no explicit durable flush before rename: `deskdrop-core/src/file_transfer.rs:452-490`

Why this is high stakes:

A passed checksum does not guarantee crash durability. Power loss or abrupt process termination between final writes and rename can still yield user-visible loss or confusing partial state.

Improvement:

- `sync_all` the file before finalization
- consider directory fsync where supported
- journal in-progress transfers so recovery can resume or cleanly roll back after crash
- add chaos tests for power-loss style interruption

## 7. Harden release signing and runtime policy on every platform

Severity: `P0`  
Platforms: `Android`, `macOS`, `Windows`

Current repo evidence:

- Android release build currently uses the debug signing config: `platforms/android/app/build.gradle:24-30`
- macOS entitlements disable library validation: `platforms/macos/Deskdrop/Deskdrop.entitlements:21-25`
- Windows installer versioning is also out of sync with the shipping app version, increasing release risk: `platforms/windows/installer/Deskdrop.wxs:13-18`

Why this is high stakes:

Release integrity is part of backend integrity. If the release artifact chain is weak, the strongest runtime protocol still ships inside a package users cannot safely trust.

Improvement:

- Android: move to a real release key and Play App Signing flow
- macOS: justify or remove `disable-library-validation`, and document the exact runtime dependency that requires it
- Windows: sign installer and binaries, and unify versioning so upgrade behavior matches shipped reality

## 8. Verify downloaded bootstrap artifacts before execution

Severity: `P0`  
Platforms: `Windows`

Current repo evidence:

- the Windows install script downloads `dotnet-install.ps1` and immediately executes it: `platforms/windows/Deskdrop.Windows/install_and_run.ps1:31-33`

Why this is high stakes:

Bootstrap scripts are part of the supply chain. Download-and-execute without integrity verification is a classic place to lose the whole trust story.

Improvement:

- pin a checksum or signature validation path for the bootstrap payload
- prefer official package channels when possible
- log the exact artifact version and verification result
- fail closed if integrity verification cannot be completed

## 9. Rebuild CI as a release gate, not a nice-to-have

Severity: `P0`  
Platforms: `Android`, `macOS`, `Windows`, `core`

Current repo evidence:

- the current Rust workspace test run fails because `FileTransferMetadata` is constructed with a nonexistent `sha256_checksum` field in tests: `deskdrop-core/src/file_transfer.rs:1040-1047`

Why this is high stakes:

This is a release-process smell, not merely a broken test. It shows that protocol-adjacent changes can land without a hard enough contract gate across the workspace. In a cross-platform sync product, that can quickly become silent divergence between UI assumptions and daemon reality.

Improvement:

- require green matrix CI before release tags
- add protocol schema contract tests and serialization round-trips
- run Android, desktop core, and packaging checks from one release workflow
- block version bumps if the matrix is red

## 10. Unify versioning and installer migration discipline

Severity: `P1`  
Platforms: `Windows`, `Android`, `macOS`, `core`

Current repo evidence:

- repo/app version is currently `1.2.1`
- Windows WiX package still declares `Version="0.1.0"`: `platforms/windows/installer/Deskdrop.wxs:13-18`

Why this is high stakes:

Version drift is how upgrade bugs become support incidents. On Windows in particular, installer version semantics affect whether users receive upgrades, downgrades, or duplicate installs.

Improvement:

- establish one canonical version source
- generate platform package versions from that source
- test upgrade, downgrade rejection, and side-by-side prevention in CI

## Recommended Delivery Order

1. QR trust gating fix
2. secret-store migration
3. least-privilege remote-action defaults
4. release signing and bootstrap integrity hardening
5. CI and protocol contract gates
6. transfer caps plus durable finalization
7. versioning and installer migration cleanup

## External Research Used

- Apple Developer: [Disable Library Validation Entitlement](https://developer.apple.com/documentation/BundleResources/Entitlements/com.apple.security.cs.disable-library-validation)
- Apple Developer: [Hardened Runtime](https://developer.apple.com/documentation/security/hardened-runtime)
- Apple Developer: [Keychain Services](https://developer.apple.com/documentation/security/keychain-services)
- Apple Developer: [Storing Keys in the Keychain](https://developer.apple.com/documentation/security/storing-keys-in-the-keychain)
- Apple Developer: [Protecting keys with the Secure Enclave](https://developer.apple.com/documentation/security/protecting-keys-with-the-secure-enclave)
- Android Developers: [Sign your app](https://developer.android.com/studio/publish/app-signing)
- Android Developers: [Build your app for release](https://developer.android.com/build/build-for-release)
- Android Developers: [Android Keystore system](https://developer.android.com/privacy-and-security/keystore)
- Android Developers: [Verify hardware-backed keys with key attestation](https://developer.android.com/privacy-and-security/security-key-attestation)
- Microsoft Learn: [CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
- Microsoft Learn: [CryptUnprotectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata)
- Microsoft Learn: [CNG DPAPI overview](https://learn.microsoft.com/en-us/windows/win32/seccng/cng-dpapi)
- OWASP MASVS: [MASVS-STORAGE](https://mas.owasp.org/MASVS/05-MASVS-STORAGE/)
- OWASP MASVS: [MASVS-STORAGE-1](https://mas.owasp.org/MASVS/controls/MASVS-STORAGE-1/)
- OWASP MASVS: [MASVS-STORAGE-2](https://mas.owasp.org/MASVS/controls/MASVS-STORAGE-2/)

## Closing Note

Deskdrop already has enough core product value that backend hardening is now a force multiplier. The next wins are not extra clever protocol features. They are about making trust decisions happen at the right time, making secrets live in the right places, and making the shipped artifact chain as defensible as the runtime design aims to be.
