// Deskdrop — macOS IPC client
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
    let last_error: String?
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
    /// Number of remote clipboard items waiting to be applied.
    let pending_clipboard_count: Int?
    /// This device's public-key fingerprint (hex) for display in the Security pane.
    let local_fingerprint: String?
    /// Active phone call state from a connected Android device (nil if no active call).
    let active_call: IpcActiveCallState?
    let peer_batteries: [IpcPeerBatteryState]?
    let active_transfers: [IpcFileTransferState]?
}

struct IpcFileTransferState: Codable {
    let transfer_id: String
    let from_device: String
    let file_name: String
    let bytes_total: Int64
    let bytes_received: Int64
    let percent: Int
    let status: String
}

/// Active call state from the daemon's status response.
struct IpcActiveCallState: Codable {
    let device_id: String
    let device_name: String
    let state: String
    let number: String
    let contact_name: String
}

/// Peer battery status.
struct IpcPeerBatteryState: Codable {
    let device_id: String
    let device_name: String
    let level: Int
    let charging: Bool
}

struct IpcCameraFrameResponse: Codable {
    let frame_base64: String?
}

struct IpcResponse<T: Codable>: Codable {
    let status: String
    let data: T?
    let message: String?
}

// ── IPC Client ────────────────────────────────────────────────────────────────

final class DeskdropIPCClient {
    static let shared = DeskdropIPCClient()

    private var socketPath: String {
        if let runtime = ProcessInfo.processInfo.environment["XDG_RUNTIME_DIR"] {
            return "\(runtime)/deskdrop.sock"
        }
        return "/tmp/deskdrop-\(getuid()).sock"
    }

    func status() async throws -> IpcStatusResponse {
        let raw = try await send(cmd: ["cmd": "status"])
        let resp = try JSONDecoder().decode(IpcResponse<IpcStatusResponse>.self, from: raw)
        guard let data = resp.data else { throw DeskdropIPCError.noData }
        return data
    }

    func ping() async throws {
        _ = try await send(cmd: ["cmd": "ping"])
    }

    func disconnectPeer(deviceId: String) async throws {
        _ = try await send(cmd: ["cmd": "disconnect_peer", "device_id": deviceId])
    }

    /// Send accept or decline call action to a ringing Android device.
    func callAction(action: String, targetDevice: String) async throws {
        _ = try await send(cmd: [
            "cmd": "call_action",
            "action": action,
            "target_device": targetDevice
        ])
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
        var cmd: [String: Any] = [
            "cmd":  "send_file_path",
            "path": url.path,
            "name": url.lastPathComponent,
            "mime": mimeType(for: url),
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

    func pauseFileTransfer(transferId: String) async throws {
        _ = try await send(cmd: ["cmd": "pause_file_transfer", "transfer_id": transferId])
    }

    func resumeFileTransfer(transferId: String) async throws {
        _ = try await send(cmd: ["cmd": "resume_file_transfer", "transfer_id": transferId])
    }

    func latestCameraFrame() async throws -> Data? {
        let raw = try await send(cmd: ["cmd": "latest_camera_frame"])
        let resp = try JSONDecoder().decode(IpcResponse<IpcCameraFrameResponse>.self, from: raw)
        guard let b64 = resp.data?.frame_base64 else { return nil }
        return Data(base64Encoded: b64)
    }

    // ── Internal Sender ───────────────────────────────────────────────────────────────

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

    // ── Transport (internal so store can issue ad-hoc commands) ──────────────

    func send(cmd: [String: Any]) async throws -> Data {
        var lastError: Error = DeskdropIPCError.connectionFailed
        for attempt in 0..<3 {
            do {
                return try await sendOnce(cmd: cmd)
            } catch DeskdropIPCError.connectionFailed {
                lastError = DeskdropIPCError.connectionFailed
                if attempt < 2 {
                    try? await Task.sleep(nanoseconds: 200_000_000) // 200 ms
                }
            } catch {
                throw error   // non-connection errors propagate immediately
            }
        }
        throw lastError
    }

    private func sendOnce(cmd: [String: Any]) async throws -> Data {
        let payload = try JSONSerialization.data(withJSONObject: cmd) + Data("\n".utf8)

        return try await withCheckedThrowingContinuation { continuation in
            do {
                let sock = socket(AF_UNIX, SOCK_STREAM, 0)
                guard sock >= 0 else { throw DeskdropIPCError.socketFailed }

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
                guard connectResult == 0 else {
                    Darwin.close(sock)
                    throw DeskdropIPCError.connectionFailed
                }

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

enum DeskdropIPCError: Error, Equatable {
    case socketFailed
    case connectionFailed
    case noData
}

// MARK: - Dashboard extensions

extension DeskdropIPCClient {

    /// Initiate an outbound TCP connection to a manually-entered address.
    /// Address format: "host:port" or bare "host" (uses daemon's configured port).
    /// Daemon resolves DNS — hostname or IP both work.
    func connectManual(address: String) async throws {
        let parts = address.split(separator: ":", maxSplits: 1)
        var cmd: [String: Any] = ["cmd": "connect_manual"]
        if parts.count == 2, let port = Int(parts[1]) {
            cmd["host"] = String(parts[0])
            cmd["port"] = port
        } else {
            cmd["host"] = address
        }
        _ = try await send(cmd: cmd)
    }

    /// Re-push a previously-received clipboard item (by hash) to connected peers.
    func sendClipboardByHash(hash: String, targetDeviceId: String?) async throws {
        var cmd: [String: Any] = ["cmd": "push_clipboard_hash", "hash": hash]
        if let id = targetDeviceId { cmd["target_device_id"] = id }
        _ = try await send(cmd: cmd)
    }

    /// Push the current local clipboard to connected peers (daemon reads OS clipboard).
    func sendClipboardCurrent(targetDeviceId: String?) async throws {
        var cmd: [String: Any] = ["cmd": "push_clipboard"]
        if let id = targetDeviceId { cmd["target_device_id"] = id }
        _ = try await send(cmd: cmd)
    }

    /// Persist settings changes to the daemon — partial patch, only set fields are applied.
    func saveSettings(_ snapshot: DeskdropSettingsSnapshot) async throws {
        let cmd: [String: Any] = [
            "cmd":                              "save_settings",
            "port":                             snapshot.port,
            "device_name":                      snapshot.deviceName,
            "sync_enabled":                     snapshot.syncEnabled,
            "sync_text":                        snapshot.syncText,
            "sync_images":                      snapshot.syncImages,
            "sync_files":                       snapshot.syncFiles,
            "history_limit":                    snapshot.historyLimit,
            "max_history_text_bytes":           snapshot.maxHistoryTextBytes,
            "max_payload_bytes":                snapshot.maxPayloadBytes,
            "clipboard_poll_ms":                snapshot.clipboardPollMs,
            "max_pushes_per_sec":               snapshot.maxPushesPerSec,
            "rate_limit_burst":                 snapshot.rateLimitBurst,
            "smart_sync_duplicate_window_ms":   snapshot.smartSyncDuplicateWindowMs,
            "smart_sync_debounce_ms":           snapshot.smartSyncDebounceMs,
            "block_sensitive_text":             snapshot.blockSensitiveText,
            "require_tofu_confirmation":        snapshot.requireTofuConfirmation,
            "show_receive_notification":        snapshot.showReceiveNotification,
            "ignore_patterns":                  snapshot.ignorePatterns,
        ]
        _ = try await send(cmd: cmd)
    }
}
