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
