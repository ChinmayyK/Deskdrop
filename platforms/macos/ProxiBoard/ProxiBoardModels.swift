import Foundation
import SwiftUI

enum SyncModeModel: String, Codable, CaseIterable, Identifiable {
    case auto
    case manual

    var id: String { rawValue }
}

enum ConnectionStateModel: String, Codable {
    case connected
    case disconnected
    case connecting
    case failed
    case unknown

    var color: Color {
        switch self {
        case .connected: return .green
        case .connecting: return .orange
        case .failed: return .red
        case .disconnected, .unknown: return .secondary
        }
    }

    var label: String { rawValue.capitalized }
}

enum TrustStateModel: String, Codable {
    case trusted
    case untrusted
    case rejected
    case revoked

    var color: Color {
        switch self {
        case .trusted: return .green
        case .untrusted: return .orange
        case .rejected, .revoked: return .red
        }
    }
}

struct AppStatusSnapshot: Decodable {
    let deviceName: String
    let port: Int
    let syncEnabled: Bool
    let peerCount: Int
    let lastSyncAt: UInt64?
    let peers: [PeerSnapshot]
}

struct PeerSnapshot: Identifiable, Decodable {
    let id: String
    let friendlyName: String
    let ip: String?
    let port: Int
    let trusted: Bool
    let lastSeen: UInt64?
    let lastSync: UInt64?
    let status: ConnectionStateModel
    let lastError: String?
}

struct TrustedDeviceSnapshot: Identifiable, Decodable {
    let deviceId: String
    let deviceName: String
    let displayName: String?
    let state: TrustStateModel
    let firstSeen: UInt64
    let trustedSince: UInt64?
    let lastSeen: UInt64
    let keyFingerprint: [UInt8]

    var id: String { deviceId }
    var effectiveName: String { displayName?.isEmpty == false ? displayName! : deviceName }
    var shortFingerprint: String {
        let hex = keyFingerprint.prefix(16).map { String(format: "%02X", $0) }
        return stride(from: 0, to: hex.count, by: 2)
            .map { index in hex[index..<min(index + 2, hex.count)].joined() }
            .joined(separator: ":")
    }
}

struct DeviceDetailSnapshot: Decodable {
    let deviceId: String
    let deviceName: String
    let displayName: String?
    let effectiveName: String
    let state: TrustStateModel
    let fingerprint: String
    let firstSeen: UInt64
    let trustedSince: UInt64?
    let lastSeen: UInt64
}

struct FeedbackEventSnapshot: Identifiable, Decodable, Hashable {
    let timestamp: UInt64
    let kind: String
    let message: String
    let deviceId: String?
    let deviceName: String?
    let clipboardId: UInt64?

    var id: String {
        "\(timestamp)-\(kind)-\(message)-\(deviceId ?? "none")-\(clipboardId.map(String.init) ?? "none")"
    }
}

struct IncomingClipboardSnapshot: Decodable {
    let id: UInt64
    let payload: IncomingClipboardPayload

    private enum CodingKeys: String, CodingKey {
        case id
        case type
        case text
        case mime
        case name
        case dataBase64
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(UInt64.self, forKey: .id)
        switch try container.decode(String.self, forKey: .type) {
        case "text":
            payload = .text(try container.decode(String.self, forKey: .text))
        case "image":
            payload = .image(
                mime: try container.decode(String.self, forKey: .mime),
                dataBase64: try container.decode(String.self, forKey: .dataBase64)
            )
        case "file":
            payload = .file(
                name: try container.decode(String.self, forKey: .name),
                dataBase64: try container.decode(String.self, forKey: .dataBase64)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "Unknown incoming clipboard payload type"
            )
        }
    }
}

enum IncomingClipboardPayload: Hashable {
    case text(String)
    case image(mime: String, dataBase64: String)
    case file(name: String, dataBase64: String)
}

struct ClipRelaySettingsSnapshot: Decodable {
    var port: Int
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
    var maxPushesPerSec: Double
    var rateLimitBurst: Double
    var smartSyncDuplicateWindowMs: UInt64
    var smartSyncDebounceMs: UInt64
    var startOnLogin: Bool
}

struct TimelineItem: Identifiable, Decodable, Hashable {
    let id: UInt64
    let timestamp: UInt64
    let sourceDevice: String
    let payload: TimelinePayload
    let hash: String
    let pinned: Bool

    var title: String {
        switch payload {
        case .text(let value):
            return value.preview
        case .image(let value):
            return "[Image \(value.mime) \(Int(value.bytes / 1024)) KB]"
        case .file(let value):
            return "[File \(value.name)]"
        case .metadata(let value):
            return value.summary
        }
    }

    var typeLabel: String {
        switch payload {
        case .text: return "Text"
        case .image: return "Image"
        case .file: return "File"
        case .metadata(let value): return value.kind.capitalized
        }
    }

    var iconName: String {
        switch payload {
        case .text: return "doc.text"
        case .image: return "photo"
        case .file: return "doc"
        case .metadata: return "clock.arrow.circlepath"
        }
    }

    var fullText: String? {
        if case let .text(value) = payload {
            return value.fullText ?? value.preview
        }
        return nil
    }
}

enum TimelinePayload: Hashable, Decodable {
    case text(TimelineTextPayload)
    case image(TimelineImagePayload)
    case file(TimelineFilePayload)
    case metadata(TimelineMetadataPayload)

    private enum CodingKeys: String, CodingKey {
        case type
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        switch type {
        case "Text":
            self = .text(try TimelineTextPayload(from: decoder))
        case "Image":
            self = .image(try TimelineImagePayload(from: decoder))
        case "File":
            self = .file(try TimelineFilePayload(from: decoder))
        default:
            self = .metadata(try TimelineMetadataPayload(from: decoder))
        }
    }
}

struct TimelineTextPayload: Decodable, Hashable {
    let preview: String
    let fullLen: Int
    let isTruncated: Bool
    let fullText: String?
}

struct TimelineImagePayload: Decodable, Hashable {
    let mime: String
    let bytes: UInt64
}

struct TimelineFilePayload: Decodable, Hashable {
    let name: String
    let bytes: UInt64
}

struct TimelineMetadataPayload: Decodable, Hashable {
    let kind: String
    let bytes: UInt64
    let summary: String
    let contentAvailable: Bool
}

struct DispatchPeerSnapshot: Decodable {
    let deviceId: String
    let deviceName: String
    let delivered: Bool
    let metadataOnly: Bool
    let reason: String?
}

struct DispatchReportSnapshot: Decodable {
    let seq: UInt64
    let peers: [DispatchPeerSnapshot]
}

struct ToastItem: Identifiable, Equatable {
    let id = UUID()
    let title: String
    let body: String
    let tint: Color
}

struct QuickSendContext: Identifiable, Equatable {
    let id = UUID()
    let text: String
    let createdAt = Date()
}

struct ManagedDevice: Identifiable, Hashable {
    let id: String
    var name: String
    var rawName: String
    var endpoint: String?
    var connectionState: ConnectionStateModel
    var trustState: TrustStateModel
    var fingerprint: String?
    var lastSeen: UInt64?
    var lastSync: UInt64?
    var lastError: String?

    var isConnected: Bool { connectionState == .connected }
}

enum DashboardSection: String, CaseIterable, Identifiable {
    case timeline
    case devices
    case trust
    case settings

    var id: String { rawValue }
    var title: String {
        switch self {
        case .timeline: return "Timeline"
        case .devices: return "Devices"
        case .trust: return "Trust"
        case .settings: return "Settings"
        }
    }
}

extension JSONDecoder {
    static let cliprelay: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }()
}

extension UInt64 {
    func relativeTimeString(now: Date = .init()) -> String {
        let target = Date(timeIntervalSince1970: TimeInterval(self))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: target, relativeTo: now)
    }
}
