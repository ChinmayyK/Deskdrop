import Foundation
import Darwin

enum IPCClientError: LocalizedError {
    case daemonUnavailable
    case badResponse
    case daemonError(String)
    case invalidSocketPath

    var errorDescription: String? {
        switch self {
        case .daemonUnavailable:
            return "ClipRelay daemon is unavailable"
        case .badResponse:
            return "The daemon returned an invalid response"
        case .daemonError(let message):
            return message
        case .invalidSocketPath:
            return "The ClipRelay IPC socket path is invalid"
        }
    }
}

private struct IPCEnvelope<T: Decodable>: Decodable {
    let status: String
    let data: T?
    let message: String?
}

private struct EmptyPayload: Decodable {}

final class ClipRelayIPCClient {
    static let shared = ClipRelayIPCClient()

    private let decoder = JSONDecoder.cliprelay
    private let encoder = JSONEncoder()

    private init() {}

    func ping() async throws {
        _ = try await send(payload: ["cmd": "ping"], response: EmptyPayload.self, allowPong: true)
    }

    func status() async throws -> AppStatusSnapshot {
        try await send(payload: ["cmd": "status"], response: AppStatusSnapshot.self)
    }

    func history(limit: Int) async throws -> [TimelineItem] {
        try await send(payload: ["cmd": "history", "last": limit], response: [TimelineItem].self)
    }

    func searchHistory(query: String, limit: Int) async throws -> [TimelineItem] {
        try await send(
            payload: ["cmd": "history_search", "query": query, "limit": limit],
            response: [TimelineItem].self
        )
    }

    func historyPin(id: UInt64, pinned: Bool) async throws {
        _ = try await send(
            payload: ["cmd": "history_pin", "id": id, "pinned": pinned],
            response: TimelineItem.self
        )
    }

    func historyDelete(id: UInt64) async throws {
        _ = try await send(
            payload: ["cmd": "history_delete", "id": id],
            response: EmptyPayload.self
        )
    }

    func historyRepush(id: UInt64, target: String?) async throws -> DispatchReportSnapshot {
        var payload: [String: Any] = ["cmd": "history_repush", "id": id]
        payload["target"] = target
        return try await send(payload: payload, response: DispatchReportSnapshot.self)
    }

    func rememberText(_ text: String) async throws -> TimelineItem {
        try await send(payload: ["cmd": "remember_text", "text": text], response: TimelineItem.self)
    }

    func pushText(_ text: String, target: String?) async throws -> DispatchReportSnapshot {
        if let target {
            return try await send(
                payload: ["cmd": "push_text_to", "text": text, "target": target],
                response: DispatchReportSnapshot.self
            )
        }
        return try await send(
            payload: ["cmd": "push_text", "text": text],
            response: DispatchReportSnapshot.self
        )
    }

    func pushImage(mime: String, data: Data) async throws -> DispatchReportSnapshot {
        try await send(
            payload: [
                "cmd": "push_image",
                "mime": mime,
                "data_base64": data.base64EncodedString(),
            ],
            response: DispatchReportSnapshot.self
        )
    }

    func pushFile(name: String, data: Data) async throws -> DispatchReportSnapshot {
        try await send(
            payload: [
                "cmd": "push_file",
                "name": name,
                "data_base64": data.base64EncodedString(),
            ],
            response: DispatchReportSnapshot.self
        )
    }

    func peers() async throws -> [PeerSnapshot] {
        try await send(payload: ["cmd": "peers"], response: [PeerSnapshot].self)
    }

    func events(limit: Int) async throws -> [FeedbackEventSnapshot] {
        try await send(payload: ["cmd": "feedback", "last": limit], response: [FeedbackEventSnapshot].self)
    }

    func incomingClipboard(id: UInt64) async throws -> IncomingClipboardSnapshot {
        try await send(
            payload: ["cmd": "incoming_clipboard", "id": id],
            response: IncomingClipboardSnapshot.self
        )
    }

    func trustedDevices() async throws -> [TrustedDeviceSnapshot] {
        try await send(payload: ["cmd": "trusted_devices"], response: [TrustedDeviceSnapshot].self)
    }

    func deviceDetails(id: String) async throws -> DeviceDetailSnapshot {
        try await send(payload: ["cmd": "device_details", "device_id": id], response: DeviceDetailSnapshot.self)
    }

    func trust(deviceId: String) async throws {
        _ = try await send(payload: ["cmd": "trust_peer", "device_id": deviceId], response: EmptyPayload.self)
    }

    func reject(deviceId: String) async throws {
        _ = try await send(payload: ["cmd": "reject_peer", "device_id": deviceId], response: EmptyPayload.self)
    }

    func revoke(deviceId: String) async throws {
        _ = try await send(
            payload: ["cmd": "revoke_trusted_device", "device_id": deviceId],
            response: EmptyPayload.self
        )
    }

    func rename(deviceId: String, displayName: String) async throws {
        _ = try await send(
            payload: [
                "cmd": "rename_trusted_device",
                "device_id": deviceId,
                "display_name": displayName,
            ],
            response: EmptyPayload.self
        )
    }

    func connect(ip: String, port: Int) async throws {
        _ = try await send(
            payload: ["cmd": "connect_peer", "ip": ip, "port": port],
            response: EmptyPayload.self
        )
    }

    func disconnect(deviceId: String) async throws {
        _ = try await send(
            payload: ["cmd": "disconnect_peer", "device_id": deviceId],
            response: EmptyPayload.self
        )
    }

    func settings() async throws -> ClipRelaySettingsSnapshot {
        try await send(payload: ["cmd": "get_settings"], response: ClipRelaySettingsSnapshot.self)
    }

    func patchSettings(_ patch: [String: Any]) async throws {
        let patchData = try JSONSerialization.data(withJSONObject: patch, options: [])
        let patchString = String(decoding: patchData, as: UTF8.self)
        _ = try await send(
            payload: ["cmd": "patch_settings", "patch": patchString],
            response: EmptyPayload.self
        )
    }

    private func send<T: Decodable>(
        payload: [String: Any],
        response: T.Type,
        allowPong: Bool = false
    ) async throws -> T {
        let data = try await Task.detached(priority: .userInitiated) {
            try self.roundTrip(payload: payload)
        }.value

        if allowPong {
            if let pong = try? self.decoder.decode(IPCEnvelope<T>.self, from: data), pong.status == "pong" {
                return pong.data ?? (EmptyPayload() as! T)
            }
            if let pongDict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let status = pongDict["status"] as? String, status == "pong" {
                return EmptyPayload() as! T
            }
        }

        let envelope = try decoder.decode(IPCEnvelope<T>.self, from: data)
        switch envelope.status {
        case "ok":
            if let value = envelope.data {
                return value
            }
            if let empty = EmptyPayload() as? T {
                return empty
            }
            throw IPCClientError.badResponse
        case "error":
            throw IPCClientError.daemonError(envelope.message ?? "Unknown daemon error")
        default:
            throw IPCClientError.badResponse
        }
    }

    private func roundTrip(payload: [String: Any]) throws -> Data {
        guard JSONSerialization.isValidJSONObject(payload) else {
            throw IPCClientError.badResponse
        }

        let socketPath = Self.defaultSocketPath()
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else {
            throw IPCClientError.daemonUnavailable
        }
        defer { close(fd) }

        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)

        let utf8 = socketPath.utf8CString
        guard utf8.count < MemoryLayout.size(ofValue: address.sun_path) else {
            throw IPCClientError.invalidSocketPath
        }

        withUnsafeMutableBytes(of: &address.sun_path) { bytes in
            bytes.initializeMemory(as: CChar.self, repeating: 0)
            _ = utf8.withUnsafeBytes { source in
                memcpy(bytes.baseAddress, source.baseAddress, utf8.count)
            }
        }

        let pathLength = socklen_t(MemoryLayout<sa_family_t>.size + utf8.count)
        let connectResult = withUnsafePointer(to: &address) { pointer -> Int32 in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                Darwin.connect(fd, sockaddrPointer, pathLength)
            }
        }

        guard connectResult == 0 else {
            throw IPCClientError.daemonUnavailable
        }

        var jsonData = try JSONSerialization.data(withJSONObject: payload, options: [])
        jsonData.append(0x0A)
        try jsonData.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            var written = 0
            while written < buffer.count {
                let result = Darwin.write(fd, baseAddress.advanced(by: written), buffer.count - written)
                guard result >= 0 else {
                    throw IPCClientError.daemonUnavailable
                }
                written += result
            }
        }

        var received = Data()
        var temp = [UInt8](repeating: 0, count: 4096)
        while true {
            let count = Darwin.read(fd, &temp, temp.count)
            guard count >= 0 else {
                throw IPCClientError.daemonUnavailable
            }
            if count == 0 {
                break
            }
            received.append(temp, count: count)
            if received.contains(0x0A) {
                break
            }
        }

        guard let newlineIndex = received.firstIndex(of: 0x0A) else {
            throw IPCClientError.badResponse
        }
        return received.prefix(upTo: newlineIndex)
    }

    static func defaultSocketPath() -> String {
        if let runtime = ProcessInfo.processInfo.environment["XDG_RUNTIME_DIR"], !runtime.isEmpty {
            return runtime + "/cliprelay.sock"
        }
        return "/tmp/cliprelay-\(getuid()).sock"
    }
}
