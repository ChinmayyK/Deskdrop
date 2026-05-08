// ClipRelay — macOS data models
// PeerViewModel uses human-readable names everywhere.
// Internal device UUIDs are intentionally absent from this layer.

import Foundation

/// View model for a connected/remembered peer.
/// All display fields are human-readable — no raw UUIDs.
struct PeerViewModel: Identifiable, Equatable {
    /// The stable device UUID is kept internally for IPC calls only.
    /// It is NEVER shown in the UI.
    let id: String

    /// Human-readable name shown in all UI (e.g. "Chinmay's Pixel 8").
    /// If the user has renamed the device locally, this reflects that rename.
    let displayName: String

    /// Platform string: "Android", "macOS", "Windows", "Linux"
    let platform: String?

    // ── Lifecycle state ───────────────────────────────────────────────────────
    let trusted: Bool
    let remembered: Bool
    let connected: Bool
    let syncEnabled: Bool
    let autoConnect: Bool

    // ── Timing ────────────────────────────────────────────────────────────────
    let lastSeen: Date?
    let lastSync: Date?

    /// Short status description shown under the device name.
    var statusLine: String {
        if !connected { return "Disconnected" }
        if !syncEnabled { return "Connected · Sync paused" }
        return "Connected · Syncing"
    }
}

/// Timeline / activity feed entry.
/// Uses friendly device names — raw UUIDs never appear here.
struct TimelineEntry: Identifiable {
    let id: UUID = UUID()
    let timestamp: Date
    /// e.g. "Chinmay's Pixel 8" — human-readable name
    let deviceName: String
    let kind: TimelineEntryKind
    let preview: String

    /// Formatted for the activity feed:
    /// "[Pixel 8] copied OTP" — NOT "[e72a91…] copied text"
    var summaryLine: String {
        switch kind {
        case .text:   return "[\(deviceName)] copied text"
        case .image:  return "[\(deviceName)] copied image"
        case .file:   return "[\(deviceName)] received file: \(preview)"
        }
    }

    /// Notification body: "Copied from Chinmay's Pixel 8"
    var notificationBody: String {
        "Copied from \(deviceName)"
    }
}

enum TimelineEntryKind: String {
    case text, image, file
}

/// Trust dialog model.
/// Shows the friendly name prominently; fingerprint in secondary position.
struct TrustPrompt: Identifiable {
    let id: UUID = UUID()
    let deviceId: String           // internal — for IPC approval call only
    let deviceName: String         // shown prominently in the dialog
    let fingerprintDisplay: String // shown in secondary "Fingerprint: A4:F2:91…" row
    let publicKeyBytes: Data
}

// ── Activity Feed — IPC model ─────────────────────────────────────────────────
// Mirrors crate::activity::ActivityEntry as a Codable Swift struct.

struct IpcActivityEntry: Codable, Identifiable {
    let id: Int64
    let timestamp_ms: Int64
    let device_id: String
    let device_name: String
    let kind: String
    let summary: String
    let content_hash: String?
    let text_preview: String?
    let file_name: String?
    let file_bytes: Int64?
    let transfer_id: String?
    var applied_locally: Bool
    let relay_path: [String]

    var isApplicable: Bool {
        kind == "remote_clipboard_available" && !applied_locally
    }

    var relayPathDisplay: String {
        relay_path.isEmpty ? "" : relay_path.joined(separator: " → ")
    }
}

// ── File transfer progress model ──────────────────────────────────────────────

struct FileTransferState: Identifiable {
    let id: String          // hex transfer ID
    let fromDeviceName: String
    let fileName: String
    let totalBytes: Int64
    var bytesReceived: Int64 = 0
    var percent: Int = 0
    var speedBps: Int64? = nil
    var etaSecs: Int64? = nil
    var status: FileTransferStatus = .incoming

    var formattedSize: String {
        let mb = Double(totalBytes) / 1_048_576.0
        if mb >= 1.0 { return String(format: "%.1f MB", mb) }
        let kb = Double(totalBytes) / 1_024.0
        if kb >= 1.0 { return String(format: "%.0f KB", kb) }
        return "\(totalBytes) B"
    }
}

enum FileTransferStatus {
    case incoming, transferring, verifying, complete(destPath: String), failed(reason: String), cancelled
}

// ── Clipboard apply policy preference ────────────────────────────────────────

struct ClipboardPolicy {
    var timelineFirstMode: Bool = true   // default: timeline-first
    var autoApply: Bool = false          // default: manual apply
    var autoApplyDebounceMs: Int = 500
}
