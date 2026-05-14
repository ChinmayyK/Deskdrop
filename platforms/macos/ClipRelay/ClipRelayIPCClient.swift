// ClipRelay — macOS IPC client
// Communicates with the Rust daemon via Unix domain socket.
// All requests use the IpcRequest JSON protocol defined in ipc.rs.

import Foundation

// ── IPC response model ────────────────────────────────────────────────────────

struct IpcPeerRecord: Codable {
    let id: String
    let friendly_name: String
    let display_name: String?
    let platform: String?
    let status: String
    let trusted: Bool
    let remembered: Bool?
    let sync_enabled: Bool?
    let auto_connect: Bool?
    let last_seen: Int?
    let last_sync: Int?
}

struct IpcStatusResponse: Codable {
    let peers: [IpcPeerRecord]
    let last_sync_at: Int?
}

struct IpcResponse<T: Codable>: Codable {
    let status: String
    let data: T?
    let message: String?
}

// ── IPC Client ────────────────────────────────────────────────────────────────

final class ClipRelayIPCClient {
    static let shared = ClipRelayIPCClient()

    private var socketPath: String {
        if let runtime = ProcessInfo.processInfo.environment["XDG_RUNTIME_DIR"] {
            return "\(runtime)/cliprelay.sock"
        }
        return "/tmp/cliprelay-\(getuid()).sock"
    }

    func status() async throws -> IpcStatusResponse {
        let raw = try await send(cmd: ["cmd": "status"])
        let resp = try JSONDecoder().decode(IpcResponse<IpcStatusResponse>.self, from: raw)
        guard let data = resp.data else { throw ClipRelayIPCError.noData }
        return data
    }

    func disconnectPeer(deviceId: String) async throws {
        _ = try await send(cmd: ["cmd": "disconnect_peer", "device_id": deviceId])
    }

    func pauseSync(deviceId: String) async throws {
        _ = try await send(cmd: ["cmd": "pause_sync_peer", "device_id": deviceId])
    }

    func resumeSync(deviceId: String) async throws {
        _ = try await send(cmd: ["cmd": "resume_sync_peer", "device_id": deviceId])
    }

    func forgetDevice(deviceId: String) async throws {
        _ = try await send(cmd: ["cmd": "forget_device", "device_id": deviceId])
    }

    func revokeDevice(deviceId: String) async throws {
        _ = try await send(cmd: ["cmd": "revoke_trusted_device", "device_id": deviceId])
    }

    func setAutoConnect(deviceId: String, enabled: Bool) async throws {
        _ = try await send(cmd: ["cmd": "set_auto_connect", "device_id": deviceId, "enabled": enabled])
    }

    func renameDevice(deviceId: String, displayName: String) async throws {
        _ = try await send(cmd: ["cmd": "rename_trusted_device", "device_id": deviceId, "display_name": displayName])
    }

    func approveTrust(deviceId: String, deviceName: String, pubkeyBytes: Data) async throws {
        _ = try await send(cmd: [
            "cmd": "trust_peer",
            "device_id": deviceId,
            "device_name": deviceName,
            "pubkey_base64": pubkeyBytes.base64EncodedString()
        ])
    }

    func rejectTrust(deviceId: String) async throws {
        _ = try await send(cmd: ["cmd": "reject_peer", "device_id": deviceId])
    }

    // ── Activity Feed ─────────────────────────────────────────────────────────

    func activityRecent(limit: Int = 50) async throws -> [IpcActivityEntry] {
        let raw = try await send(cmd: ["cmd": "activity_recent", "limit": limit])
        let resp = try JSONDecoder().decode(IpcResponse<[IpcActivityEntry]>.self, from: raw)
        return resp.data ?? []
    }

    func activitySince(sinceId: Int64) async throws -> [IpcActivityEntry] {
        let raw = try await send(cmd: ["cmd": "activity_since", "since_id": sinceId])
        let resp = try JSONDecoder().decode(IpcResponse<[IpcActivityEntry]>.self, from: raw)
        return resp.data ?? []
    }

    func pendingRemoteClipboards() async throws -> [IpcActivityEntry] {
        let raw = try await send(cmd: ["cmd": "pending_remote_clipboards"])
        let resp = try JSONDecoder().decode(IpcResponse<[IpcActivityEntry]>.self, from: raw)
        return resp.data ?? []
    }

    // ── Timeline-first clipboard ──────────────────────────────────────────────

    /// Apply a remote clipboard item from the activity feed by its content hash.
    /// The engine writes the item to the local clipboard and marks it applied.
    func applyClipboard(contentHash: String) async throws {
        _ = try await send(cmd: ["cmd": "apply_clipboard", "content_hash": contentHash])
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    func setTimelineFirstMode(enabled: Bool) async throws {
        _ = try await send(cmd: ["cmd": "set_timeline_first_mode", "enabled": enabled])
    }

    func setAutoApplyClipboard(enabled: Bool) async throws {
        _ = try await send(cmd: ["cmd": "set_auto_apply_clipboard", "enabled": enabled])
    }

    // ── File Transfer ─────────────────────────────────────────────────────────

    /// Send a file to a specific peer, or all peers when targetDevice is nil.
    func sendFile(url: URL, targetDeviceId: String? = nil) async throws -> String {
        let data = try Data(contentsOf: url)
        var cmd: [String: Any] = [
            "cmd":        "send_file",
            "name":       url.lastPathComponent,
            "mime":       mimeType(for: url),
            "data_base64": data.base64EncodedString()
        ]
        if let t = targetDeviceId { cmd["target_device"] = t }
        let raw = try await send(cmd: cmd)
        let resp = try JSONDecoder().decode(IpcResponse<String>.self, from: raw)
        return resp.data ?? ""
    }

    func acceptFileTransfer(transferId: String) async throws {
        _ = try await send(cmd: ["cmd": "accept_file_transfer", "transfer_id": transferId])
    }

    func rejectFileTransfer(transferId: String, reason: String = "user rejected") async throws {
        _ = try await send(cmd: ["cmd": "reject_file_transfer",
                                 "transfer_id": transferId, "reason": reason])
    }

    func cancelFileTransfer(transferId: String) async throws {
        _ = try await send(cmd: ["cmd": "cancel_file_transfer", "transfer_id": transferId])
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private func mimeType(for url: URL) -> String {
        let ext = url.pathExtension.lowercased()
        let map: [String: String] = [
            "pdf": "application/pdf", "png": "image/png", "jpg": "image/jpeg",
            "jpeg": "image/jpeg", "gif": "image/gif", "txt": "text/plain",
            "zip": "application/zip", "tar": "application/x-tar",
            "gz": "application/gzip", "mp4": "video/mp4", "mov": "video/quicktime"
        ]
        return map[ext] ?? "application/octet-stream"
    }

    // ── Transport ─────────────────────────────────────────────────────────────

    private func send(cmd: [String: Any]) async throws -> Data {
        let payload = try JSONSerialization.data(withJSONObject: cmd) + Data("\n".utf8)

        return try await withCheckedThrowingContinuation { continuation in
            do {
                let sock = socket(AF_UNIX, SOCK_STREAM, 0)
                guard sock >= 0 else { throw ClipRelayIPCError.socketFailed }

                var addr = sockaddr_un()
                addr.sun_family = sa_family_t(AF_UNIX)
                withUnsafeMutablePointer(to: &addr.sun_path) {
                    $0.withMemoryRebound(to: Int8.self, capacity: 108) { ptr in
                        socketPath.withCString { src in _ = strncpy(ptr, src, 107) }
                    }
                }

                let connectResult = withUnsafePointer(to: &addr) {
                    $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                        Darwin.connect(sock, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
                    }
                }
                guard connectResult == 0 else { throw ClipRelayIPCError.connectionFailed }

                payload.withUnsafeBytes { _ = Darwin.send(sock, $0.baseAddress, payload.count, 0) }

                var response = Data()
                var buf = [UInt8](repeating: 0, count: 4096)
                while true {
                    let n = Darwin.recv(sock, &buf, buf.count, 0)
                    if n <= 0 { break }
                    response.append(contentsOf: buf[0..<n])
                    if response.last == UInt8(ascii: "\n") { break }
                }
                Darwin.close(sock)
                continuation.resume(returning: response)
            } catch {
                continuation.resume(throwing: error)
            }
        }
    }
}

enum ClipRelayIPCError: Error {
    case socketFailed
    case connectionFailed
    case noData
}

// MARK: - Dashboard extensions (stubs — wire to daemon when ready)

extension ClipRelayIPCClient {
    func connectManual(address: String) async throws {
        _ = try await send(cmd: ["op": "connect_manual", "address": address])
    }
    func sendClipboardByHash(hash: String, targetDeviceId: String?) async throws {
        var cmd: [String: Any] = ["op": "send_clipboard_hash", "hash": hash]
        if let id = targetDeviceId { cmd["target_device_id"] = id }
        _ = try await send(cmd: cmd)
    }
    func sendClipboardCurrent(targetDeviceId: String?) async throws {
        var cmd: [String: Any] = ["op": "send_clipboard_current"]
        if let id = targetDeviceId { cmd["target_device_id"] = id }
        _ = try await send(cmd: cmd)
    }
    func saveSettings(_ snapshot: ClipRelaySettingsSnapshot) async throws {
        let cmd: [String: Any] = [
            "op":           "save_settings",
            "port":         snapshot.port,
            "device_name":  snapshot.deviceName,
            "sync_enabled": snapshot.syncEnabled,
            "history_limit": snapshot.historyLimit,
        ]
        _ = try await send(cmd: cmd)
    }
}
