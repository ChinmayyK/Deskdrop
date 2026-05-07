// ClipRelay — macOS app state store
// Bridges IPC responses to SwiftUI view models.
// Enforces: no raw UUIDs in public-facing state.

import Foundation
import Combine

@MainActor
final class ClipRelayStore: ObservableObject {
    @Published var peers: [PeerViewModel] = []
    @Published var timeline: [TimelineEntry] = []
    @Published var activeTrustPrompt: TrustPrompt? = nil
    @Published var isRunning: Bool = false
    @Published var statusLine: String = "Starting…"
    @Published var connectedCount: Int = 0

    private let ipc: ClipRelayIPCClient
    private var pollTimer: Timer?
    private var pendingRename: PeerViewModel? = nil

    init(ipc: ClipRelayIPCClient = .shared) {
        self.ipc = ipc
        startPolling()
    }

    // ── Polling ───────────────────────────────────────────────────────────────

    private func startPolling() {
        pollTimer = Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { [weak self] _ in
            Task { await self?.refresh() }
        }
    }

    func refresh() async {
        do {
            let status = try await ipc.status()
            isRunning = true
            connectedCount = status.peers.filter { $0.status == "connected" }.count
            statusLine = connectedCount == 0
                ? "Ready — no devices nearby"
                : "\(connectedCount) device\(connectedCount == 1 ? "" : "s") connected"
            peers = status.peers.map { makePeerViewModel($0) }
        } catch {
            isRunning = false
            statusLine = "Daemon not running"
        }
    }

    // ── Device lifecycle controls ─────────────────────────────────────────────

    func disconnect(_ peer: PeerViewModel) {
        Task {
            try? await ipc.disconnectPeer(deviceId: peer.id)
            await refresh()
        }
    }

    func pauseSync(_ peer: PeerViewModel) {
        Task {
            try? await ipc.pauseSync(deviceId: peer.id)
            await refresh()
        }
    }

    func resumeSync(_ peer: PeerViewModel) {
        Task {
            try? await ipc.resumeSync(deviceId: peer.id)
            await refresh()
        }
    }

    func forgetDevice(_ peer: PeerViewModel) {
        Task {
            try? await ipc.forgetDevice(deviceId: peer.id)
            await refresh()
        }
    }

    func revokeTrust(_ peer: PeerViewModel) {
        Task {
            try? await ipc.revokeDevice(deviceId: peer.id)
            await refresh()
        }
    }

    func toggleAutoConnect(_ peer: PeerViewModel) {
        Task {
            try? await ipc.setAutoConnect(deviceId: peer.id, enabled: !peer.autoConnect)
            await refresh()
        }
    }

    func beginRename(_ peer: PeerViewModel) {
        pendingRename = peer
        // Trigger a rename sheet — handled by the view
        NotificationCenter.default.post(name: .beginRename, object: peer)
    }

    func applyRename(deviceId: String, newName: String) {
        Task {
            try? await ipc.renameDevice(deviceId: deviceId, displayName: newName)
            await refresh()
        }
    }

    // ── Trust prompts ─────────────────────────────────────────────────────────

    func approveTrust(_ prompt: TrustPrompt) {
        Task {
            try? await ipc.approveTrust(
                deviceId: prompt.deviceId,
                deviceName: prompt.deviceName,
                pubkeyBytes: prompt.publicKeyBytes
            )
            activeTrustPrompt = nil
            await refresh()
        }
    }

    func rejectTrust(_ prompt: TrustPrompt) {
        Task {
            try? await ipc.rejectTrust(deviceId: prompt.deviceId)
            activeTrustPrompt = nil
        }
    }

    // ── Timeline ──────────────────────────────────────────────────────────────

    func addTimelineEntry(_ entry: TimelineEntry) {
        timeline.insert(entry, at: 0)
        if timeline.count > 200 { timeline.removeLast() }
    }

    // ── View model mapping ────────────────────────────────────────────────────

    private func makePeerViewModel(_ raw: IpcPeerRecord) -> PeerViewModel {
        PeerViewModel(
            id: raw.id,
            // displayName uses the trust store's display_name override if set,
            // otherwise the peer's friendly_name. Never the raw UUID.
            displayName: raw.display_name?.isEmpty == false ? raw.display_name! : raw.friendly_name,
            platform: raw.platform,
            trusted: raw.trusted,
            remembered: raw.remembered ?? true,
            connected: raw.status == "connected",
            syncEnabled: raw.sync_enabled ?? true,
            autoConnect: raw.auto_connect ?? true,
            lastSeen: raw.last_seen.map { Date(timeIntervalSince1970: TimeInterval($0)) },
            lastSync: raw.last_sync.map { Date(timeIntervalSince1970: TimeInterval($0)) }
        )
    }
}

extension Notification.Name {
    static let beginRename = Notification.Name("ClipRelayBeginRename")
}
