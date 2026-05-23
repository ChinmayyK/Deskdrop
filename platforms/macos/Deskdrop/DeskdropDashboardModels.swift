// DeskdropDashboardModels.swift
// Models used exclusively by the macOS dashboard views.

import Foundation
import SwiftUI

// MARK: - Dashboard Navigation

enum DashboardSection: String, CaseIterable, Identifiable {
    case dashboard, history, devices, workflows, settings

    var id: String { rawValue }

    var title: String {
        switch self {
        case .dashboard: return "Dashboard"
        case .history:   return "Clipboard History"
        case .devices:  return "Synced Devices"
        case .workflows: return "Workflows"
        case .settings: return "Settings"
        }
    }

    var icon: String {
        switch self {
        case .dashboard: return "house"
        case .history:   return "doc.text"
        case .devices:  return "arrow.triangle.2.circlepath"
        case .workflows: return "square.grid.2x2"
        case .settings: return "gearshape"
        }
    }

    var eyebrow: String {
        switch self {
        case .dashboard: return "Overview"
        case .history:   return "Activity"
        case .devices:  return "Network"
        case .workflows: return "Automation"
        case .settings: return "Configuration"
        }
    }

    var subtitle: String {
        switch self {
        case .dashboard:
            return "Modern clipboard sync and continuity manager."
        case .history:
            return "All clipboard activity across your devices, in order."
        case .devices:
            return "Discover, connect, and manage nearby peers."
        case .workflows:
            return "Custom actions and automations for incoming clipboards."
        case .settings:
            return "Tune sync behaviour, filters, and network settings."
        }
    }
}

// MARK: - Timeline Item (display model for activity feed)

struct TimelineItem: Identifiable {
    let id: Int64
    let iconName: String
    let title: String
    let typeLabel: String
    let sourceDevice: String
    let timestamp: Date
    var pinned: Bool
    let fullText: String?
    let filePath: String?

    init(entry: IpcActivityEntry, pinned: Bool) {
        self.id           = entry.id
        self.sourceDevice = entry.device_name
        self.timestamp    = Date(timeIntervalSince1970: Double(entry.timestamp_ms) / 1000.0)
        self.pinned       = pinned
        self.fullText     = entry.text_preview
        // Use dest_path (full filesystem path) for Finder reveal; fall back to nil if unavailable.
        self.filePath     = entry.dest_path

        switch entry.kind {
        case "remote_clipboard_available", "clipboard_text":
            self.typeLabel = "Text"
            self.iconName  = "doc.on.clipboard"
            self.title     = entry.text_preview?.isEmpty == false
                ? entry.text_preview!
                : entry.summary
        case "clipboard_image":
            self.typeLabel = "Image"
            self.iconName  = "photo"
            self.title     = "Image from \(entry.device_name)"
        case "file_transfer_complete", "file_transfer_started":
            self.typeLabel = "File"
            self.iconName  = "doc.fill"
            self.title     = entry.file_name ?? entry.summary
        case "peer_connected":
            self.typeLabel = "Connection"
            self.iconName  = "wifi"
            self.title     = entry.summary
        case "peer_disconnected":
            self.typeLabel = "Connection"
            self.iconName  = "wifi.slash"
            self.title     = entry.summary
        default:
            self.typeLabel = "Event"
            self.iconName  = "bolt.circle"
            self.title     = entry.summary
        }
    }
}

// MARK: - Toast Notification

struct ToastItem: Identifiable {
    let id: UUID = UUID()
    let title: String
    let body: String
    let tint: Color
    let systemImage: String
    var detail: String?
    var ttl: TimeInterval?
    var progress: Double?
    var primaryAction: ToastAction?
    var secondaryAction: ToastAction?

    init(
        title: String,
        body: String,
        tint: Color,
        systemImage: String = "sparkles.rectangle.stack",
        detail: String? = nil,
        ttl: TimeInterval? = 4.0,
        progress: Double? = nil,
        primaryAction: ToastAction? = nil,
        secondaryAction: ToastAction? = nil
    ) {
        self.title = title
        self.body = body
        self.tint = tint
        self.systemImage = systemImage
        self.detail = detail
        self.ttl = ttl
        self.progress = progress
        self.primaryAction = primaryAction
        self.secondaryAction = secondaryAction
    }
}

struct ToastAction {
    enum Role {
        case primary
        case secondary
        case positive
        case destructive
    }

    let title: String
    let role: Role
    let handler: () -> Void
}

// MARK: - Quick Send Context

struct QuickSendContext {
    let text: String
    let timestamp: Date
}

// MARK: - Status Snapshot

struct StatusSnapshot {
    let peerCount: Int
    let trustedCount: Int
    let lastSyncAt: Date?
    let syncEnabled: Bool
    let daemonVersion: String?
}

// MARK: - Device Detail (for trust prompt)

struct DeviceDetailSnapshot {
    let deviceId: String
    let deviceName: String
    let fingerprint: String
    let lastSeen: Date?

    var effectiveName: String { deviceName }
}

// MARK: - Settings Snapshot

struct DeskdropSettingsSnapshot {
    var port: UInt16
    var deviceName: String
    var syncEnabled: Bool
    var syncText: Bool
    var syncImages: Bool
    var syncFiles: Bool
    var syncMode: SyncModeModel
    var maxPayloadBytes: UInt64
    var historyLimit: Int
    var maxHistoryTextBytes: Int
    var showReceiveNotification: Bool
    var requireTofuConfirmation: Bool
    var blockedDeviceIds: [String]
    var blockSensitiveText: Bool
    var ignorePatterns: [String]
    var clipboardPollMs: UInt64
    var maxPushesPerSec: Int
    var rateLimitBurst: Int
    var smartSyncDuplicateWindowMs: UInt64
    var smartSyncDebounceMs: UInt64
    var startOnLogin: Bool
}

// MARK: - Sync Mode

enum SyncModeModel: String, CaseIterable, Identifiable {
    case auto, manual, receive
    var id: String { rawValue }
}

// MARK: - Managed Device (UI wrapper over PeerViewModel)

struct ManagedDevice: Identifiable {
    let id: String
    var name: String
    let rawName: String
    let endpoint: String?
    let connectionState: DeviceConnectionState
    let trustState: DeviceTrustState
    let remembered: Bool
    let autoConnect: Bool
    let fingerprint: String?
    let lastSeen: Date?
    let lastSync: Date?
    let lastError: String?

    var isConnected: Bool { connectionState == .connected }
    var canReconnect: Bool { trustState == .trusted && remembered && autoConnect && connectionState != .connected }

    init(peer: PeerViewModel) {
        self.id              = peer.id
        self.name            = peer.displayName
        self.rawName         = peer.displayName
        self.endpoint        = nil
        self.connectionState = if peer.connectionStatus == "connecting" {
            .connecting
        } else if peer.connectionStatus == "failed" {
            .attention
        } else if peer.connected {
            .connected
        } else if peer.trusted && peer.remembered && peer.autoConnect {
            .reconnectable
        } else {
            .disconnected
        }
        self.trustState      = if peer.trusted {
            .trusted
        } else if peer.lastError?.localizedCaseInsensitiveContains("rejected") == true {
            .rejected
        } else {
            .untrusted
        }
        self.remembered      = peer.remembered
        self.autoConnect     = peer.autoConnect
        self.fingerprint     = nil
        self.lastSeen        = peer.lastSeen
        self.lastSync        = peer.lastSync
        self.lastError       = peer.lastError
    }
}

// MARK: - Device Connection State

enum DeviceConnectionState {
    case connected, connecting, reconnectable, attention, disconnected

    var label: String {
        switch self {
        case .connected:    return "Connected"
        case .connecting:   return "Connecting"
        case .reconnectable:return "Ready"
        case .attention:    return "Needs Attention"
        case .disconnected: return "Offline"
        }
    }

    var color: Color {
        switch self {
        case .connected:    return PBTheme.accentGreen
        case .connecting:   return PBTheme.accentBlue
        case .reconnectable:return PBTheme.brandElectric
        case .attention:    return PBTheme.accentGold
        case .disconnected: return PBTheme.inkSoft
        }
    }
}

// MARK: - Device Trust State

enum DeviceTrustState: String {
    case trusted, untrusted, rejected

    var color: Color {
        switch self {
        case .trusted:   return PBTheme.accentGreen
        case .untrusted: return PBTheme.accentGold
        case .rejected:  return PBTheme.accentRed
        }
    }
}

// MARK: - Date Extension

extension Date {
    func relativeTimeString() -> String {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: self, relativeTo: Date())
    }
}
