# Deskdrop Cross-Platform Feature Parity Audit

Date: May 27, 2026

## Overview

Deskdrop is designed to be a unified cross-platform clipboard and file-sharing tool. However, due to the realities of maintaining four separate frontend codebases (Swift, Kotlin, C#/WPF, and Rust/CLI), feature drift has occurred. 

This audit maps out exactly which features are supported on which platforms, highlighting where platforms are lagging behind the reference implementations (usually macOS and Android).

## Parity Matrix

| Feature | macOS | Android | Windows | Linux |
| :--- | :---: | :---: | :---: | :---: |
| **Connection & Discovery** | | | | |
| Local Network Discovery (mDNS) | ✅ | ✅ | ✅ | ✅ |
| Manual IP Connect | ✅ | ✅ | ✅ | ❌ |
| Show QR Code for Pairing | ✅ | ❌ | ✅ | ❌ |
| Scan QR Code | ❌ | ✅ | ❌ | ❌ |
| Magic Link Pairing (`deskdrop://`) | ✅ | ✅ | ⚠️ (Partial) | ❌ |
| | | | | |
| **Trust & Security** | | | | |
| TOFU / SAS PIN Validation | ✅ | ✅ | ✅ | ✅ (CLI) |
| Device Fingerprint Verification | ✅ | ✅ | ✅ | ✅ (CLI) |
| | | | | |
| **Clipboard Sync** | | | | |
| Text Sync | ✅ | ✅ | ✅ | ✅ |
| Image Sync | ✅ | ✅ | ✅ | ✅ |
| File Sync / Send File | ✅ | ✅ | ✅ | ❌ |
| Background Auto-Apply | ✅ | ✅ (Fg Svc) | ✅ | ✅ |
| Local Clipboard Dedup (Hash) | ✅ | ✅ | ✅ | ✅ |
| | | | | |
| **Advanced Features** | | | | |
| Timeline / History UI | ✅ | ✅ | ✅ | ❌ |
| Pin/Delete Timeline Items | ✅ | ✅ | ❌ | ❌ |
| Send Quick Context | ✅ | ❌ | ❌ | ❌ |
| Broadcast Camera | ❌ | ❌ | ✅ | ❌ |
| Diagnostics / Repair UI | ✅ | ✅ | ❌ | ❌ |

---

## Key Findings & Drift Analysis

### 1. The Windows "Divergence"
The Windows client has implemented an experimental **Broadcast Camera** feature (`BorderBroadcastCamera_Click`) that does not exist on any other platform. At the same time, Windows is missing core utility features like the **Diagnostics UI** and **Timeline Pinning**, which are heavily relied upon in the macOS and Android apps. 

### 2. Linux is strictly Headless
The Linux client (`platforms/linux/src/main.rs`) is completely headless. It relies entirely on `notify-send` for user alerts and `deskdrop-cli` for handling pairing approvals and trust management. While this is great for power users, there is zero UI for viewing the clipboard timeline, managing settings, or manually sending files.

### 3. Asymmetric Pairing UX
The pairing story is highly asymmetric:
- **macOS and Windows** can *show* QR codes but cannot scan them.
- **Android** can *scan* QR codes but does not generate them to be scanned.
- **Linux** uses CLI commands exclusively (`deskdrop-cli trust <id>`).

While this makes sense given device capabilities (desktops lack rear cameras; phones have them), it means the onboarding documentation must account for entirely different user flows depending on the combination of devices being paired.

### 4. macOS "Quick Context" is Exclusive
macOS has a `sendQuickContext` feature (clearing the clipboard and adding a contextual string before sending) that hasn't made its way to the other platforms.

## Strategic Recommendations

> [!WARNING]
> We should establish a strict feature freeze on experimental capabilities (like Windows Camera Broadcasting) until the baseline utility features are equalized across all GUI platforms.

1. **Build Windows Diagnostics:** Bring the Diagnostics/Repair UI to Windows. Without it, debugging connection issues on PC relies heavily on log parsing.
2. **Implement Timeline Management on Windows:** Add the ability to Pin and Delete items in the Windows timeline to match macOS and Android.
3. **Decide on Linux GUI:** Determine if Linux will remain strictly a daemon + CLI tool, or if we will build the planned GTK4 tray app mentioned in the source comments to bring it to feature parity.
